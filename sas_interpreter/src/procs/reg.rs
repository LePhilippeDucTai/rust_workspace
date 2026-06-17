//! PROC REG — ordinary least-squares linear regression (v1).
//!
//! # Plan du fichier — voir le plan M25.1
//!
//! `proc reg data=<ref> [alpha=<f64>];
//!  model y = x1 x2 ... ;
//!  [output out=<ref>;]
//!  run;`
//!
//! ## Périmètre v1 (fidèle à SAS 9.4 PROC REG, OLS uniquement)
//! - Statement PROC : `data=` (défaut `_LAST_`), `alpha=` (défaut 0.05).
//! - Un seul MODEL : une variable dépendante, ≥1 variable explicative.
//! - OUTPUT OUT= optionnel : recopie le dataset d'entrée et ajoute les
//!   colonnes PREDICTED et RESIDUAL (missing sur les observations exclues).
//!
//! ## Sortie listing (titre "The REG Procedure"), dans l'ordre SAS :
//! 1. Bloc « Model Information » (data set, nb obs lues/utilisées, variable
//!    dépendante).
//! 2. Table « Analysis of Variance » : Model / Error / Corrected Total avec
//!    DF, Sum of Squares, Mean Square, F Value, Pr > F. Sous la table :
//!    Root MSE, Dependent Mean, R-Square, Adj R-Sq.
//! 3. Table « Parameter Estimates » : pour l'intercept puis chaque variable,
//!    DF, Parameter Estimate, Standard Error, t Value, Pr > |t| et les bornes
//!    de l'intervalle de confiance (1 − alpha).
//!
//! ## Choix / simplifications documentés
//! - Exclusion LISTWISE : une observation est retirée dès que y OU l'une des
//!   x_i est manquante (cohérent avec SAS, complete-case analysis).
//! - Le solveur est `stat::linalg::least_squares` (QR Householder). La matrice
//!   (X'X)⁻¹ pour les erreurs standard provient de `stat::linalg::invert_matrix`.
//! - p-values : `stat::dists::student_t_cdf` (Pr > |t|), `stat::dists::f_cdf`
//!   (Pr > F). Quantile t pour l'IC : `procs::common::t_quantile`.
//! - Matrice singulière (colinéarité parfaite) → `SasError::Numerical`.

use crate::ast::DatasetRef;
use crate::dataset::{SasDataset, VarMeta};
use crate::error::{Result, SasError};
use crate::listing::Align;
use crate::missing::value_to_num;
use crate::parser::StatementStream;
use crate::procs::common::{decode_column, t_quantile};
use crate::session::Session;
use crate::stat::dists::{f_cdf, student_t_cdf};
use crate::stat::linalg::{invert_matrix, least_squares};
use crate::token::TokenKind;
use crate::value::{format_best, VarType};
use polars::prelude::{Column, NamedFrom, Series};

/// A single `model y = x1 x2 ... ;` statement.
pub struct ModelStmt {
    pub y_var: String,
    pub x_vars: Vec<String>,
}

pub struct RegAst {
    pub data: Option<DatasetRef>,
    /// Confidence-level complement for parameter intervals. Default 0.05.
    pub alpha: f64,
    pub model: ModelStmt,
    /// OUTPUT OUT= target dataset (PREDICTED / RESIDUAL columns), if any.
    pub output_out: Option<DatasetRef>,
}

/// Parse `proc reg [data=a] [alpha=p]; model y = x...; [output out=b;] run;`.
/// Called AFTER "proc reg" was consumed. Consumes through `run;` / `quit;`.
pub fn parse(ts: &mut StatementStream) -> Result<RegAst> {
    let mut data: Option<DatasetRef> = None;
    let mut alpha: f64 = 0.05;

    // --- PROC REG statement options, until `;` ---
    loop {
        if ts.peek().kind == TokenKind::Semi {
            ts.next();
            break;
        }
        if ts.peek().kind == TokenKind::Eof {
            break;
        }
        if ts.peek().is_kw("data") {
            ts.next();
            expect_eq(ts, "DATA")?;
            data = Some(ts.parse_dataset_ref()?);
        } else if ts.peek().is_kw("alpha") {
            ts.next();
            expect_eq(ts, "ALPHA")?;
            let span = ts.peek().span;
            alpha = parse_number(ts)?;
            if !(alpha > 0.0 && alpha < 1.0) {
                return Err(SasError::parse(
                    "The ALPHA= value must be between 0 and 1.",
                    span,
                ));
            }
        } else if let Some(name) = ts.peek().ident().map(str::to_string) {
            let span = ts.peek().span;
            return Err(SasError::parse(
                format!(
                    "Unexpected option '{}' on PROC REG statement.",
                    name.to_uppercase()
                ),
                span,
            ));
        } else {
            let span = ts.peek().span;
            return Err(SasError::parse("Unexpected token on PROC REG statement.", span));
        }
    }

    // --- sub-statements until run;/quit; ---
    let mut model: Option<ModelStmt> = None;
    let mut output_out: Option<DatasetRef> = None;

    loop {
        while ts.peek().kind == TokenKind::Semi {
            ts.next();
        }
        if ts.peek().kind == TokenKind::Eof {
            break;
        }
        if ts.peek().is_kw("run") || ts.peek().is_kw("quit") {
            ts.next();
            if ts.peek().kind == TokenKind::Semi {
                ts.next();
            }
            break;
        }

        if ts.peek().is_kw("model") {
            ts.next();
            if model.is_some() {
                return Err(SasError::runtime(
                    "PROC REG v1 supports a single MODEL statement.",
                ));
            }
            model = Some(parse_model(ts)?);
        } else if ts.peek().is_kw("output") {
            ts.next();
            output_out = Some(parse_output(ts)?);
        } else {
            // Unknown sub-statement: skip it (recovery, like means/print).
            ts.skip_to_semi();
        }
    }

    let model = model.ok_or_else(|| {
        SasError::runtime("PROC REG requires a MODEL statement (model y = x...;).")
    })?;

    Ok(RegAst {
        data,
        alpha,
        model,
        output_out,
    })
}

/// Parse `model y = x1 x2 ... ;` (the leading `model` keyword already consumed).
fn parse_model(ts: &mut StatementStream) -> Result<ModelStmt> {
    let span = ts.peek().span;
    let y_var = ts
        .peek()
        .ident()
        .map(str::to_string)
        .ok_or_else(|| SasError::parse("Expected dependent variable in MODEL statement.", span))?;
    ts.next();

    if ts.peek().kind != TokenKind::Eq {
        return Err(SasError::parse(
            "Expected '=' after the dependent variable in MODEL statement.",
            ts.peek().span,
        ));
    }
    ts.next();

    let mut x_vars: Vec<String> = Vec::new();
    loop {
        if ts.peek().kind == TokenKind::Semi {
            ts.next();
            break;
        }
        if ts.peek().kind == TokenKind::Eof || ts.peek().is_kw("run") || ts.peek().is_kw("quit") {
            break;
        }
        match ts.peek().ident().map(str::to_string) {
            Some(name) => {
                x_vars.push(name);
                ts.next();
            }
            None => {
                return Err(SasError::parse(
                    "Expected an explanatory variable name in MODEL statement.",
                    ts.peek().span,
                ));
            }
        }
    }

    if x_vars.is_empty() {
        return Err(SasError::runtime(
            "The MODEL statement requires at least one explanatory variable.",
        ));
    }

    Ok(ModelStmt { y_var, x_vars })
}

/// Parse `output out=<ref> ... ;` (the leading `output` keyword consumed).
/// v1 recognises only OUT=; other keywords are skipped to the semicolon.
fn parse_output(ts: &mut StatementStream) -> Result<DatasetRef> {
    let mut out: Option<DatasetRef> = None;
    loop {
        if ts.peek().kind == TokenKind::Semi {
            ts.next();
            break;
        }
        if ts.peek().kind == TokenKind::Eof || ts.peek().is_kw("run") || ts.peek().is_kw("quit") {
            break;
        }
        if ts.peek().is_kw("out") {
            ts.next();
            expect_eq(ts, "OUT")?;
            out = Some(ts.parse_dataset_ref()?);
        } else {
            // Skip unrecognised OUTPUT keyword (e.g. p=, r=) in v1.
            ts.next();
        }
    }
    out.ok_or_else(|| {
        SasError::runtime("The OUTPUT statement of PROC REG requires OUT=<dataset>.")
    })
}

fn expect_eq(ts: &mut StatementStream, opt: &str) -> Result<()> {
    if ts.peek().kind != TokenKind::Eq {
        return Err(SasError::parse(
            format!("expected '=' after {opt}"),
            ts.peek().span,
        ));
    }
    ts.next();
    Ok(())
}

/// Parse a numeric literal (used for ALPHA=).
fn parse_number(ts: &mut StatementStream) -> Result<f64> {
    match ts.peek().kind.clone() {
        TokenKind::Num(v) => {
            ts.next();
            Ok(v)
        }
        _ => Err(SasError::parse("expected a number", ts.peek().span)),
    }
}

/// Resolve `data=` or `_LAST_` into a concrete DatasetRef.
fn resolve_input(ast: &RegAst, session: &Session) -> Result<DatasetRef> {
    match &ast.data {
        Some(r) => Ok(r.clone()),
        None => {
            let last = session.last_dataset.clone().ok_or_else(|| {
                SasError::runtime("There is no default input data set (_LAST_ is undefined).")
            })?;
            let parts: Vec<&str> = last.splitn(2, '.').collect();
            if parts.len() == 2 {
                Ok(DatasetRef {
                    libref: Some(parts[0].to_string()),
                    name: parts[1].to_string(),
                })
            } else {
                Ok(DatasetRef {
                    libref: None,
                    name: last,
                })
            }
        }
    }
}

// ───────────────────────── execute ─────────────────────────

pub fn execute(ast: &RegAst, session: &mut Session) -> Result<()> {
    let in_ref = resolve_input(ast, session)?;
    let in_libref = in_ref.libref_or_work();
    let in_table = in_ref.name.to_uppercase();
    let in_display = format!("{in_libref}.{in_table}");

    let provider = session.libs.get(&in_libref)?;
    let (ds, notes) = provider.read(&in_table)?;
    for note in notes {
        session.log.forward(&note);
    }

    let n_obs = ds.n_obs();

    // Resolve a numeric variable by name to its column index.
    let resolve_num = |nm: &str| -> Result<usize> {
        match ds.vars.iter().position(|m| m.name.eq_ignore_ascii_case(nm)) {
            Some(i) => {
                if ds.vars[i].ty != VarType::Num {
                    Err(SasError::runtime(format!(
                        "Variable {} in the MODEL statement is not numeric.",
                        nm.to_uppercase()
                    )))
                } else {
                    Ok(i)
                }
            }
            None => Err(SasError::runtime(format!(
                "Variable {} not found.",
                nm.to_uppercase()
            ))),
        }
    };

    let y_col = resolve_num(&ast.model.y_var)?;
    let x_cols: Vec<usize> = ast
        .model
        .x_vars
        .iter()
        .map(|nm| resolve_num(nm))
        .collect::<Result<_>>()?;
    let n_params = x_cols.len() + 1; // + intercept

    // Decode needed columns once.
    let y_vals = decode_column(&ds, y_col)?;
    let mut x_vals: Vec<Vec<crate::value::Value>> = Vec::with_capacity(x_cols.len());
    for &c in &x_cols {
        x_vals.push(decode_column(&ds, c)?);
    }

    // LISTWISE exclusion: keep rows where y AND every x_i are non-missing.
    let mut kept_rows: Vec<usize> = Vec::with_capacity(n_obs);
    for r in 0..n_obs {
        let yv = value_to_num(&y_vals[r]);
        let ok_y = matches!(yv, Some(v) if !v.is_nan());
        if !ok_y {
            continue;
        }
        let ok_x = x_vals.iter().all(|col| {
            matches!(value_to_num(&col[r]), Some(v) if !v.is_nan())
        });
        if ok_x {
            kept_rows.push(r);
        }
    }

    let m = kept_rows.len();
    if m < n_params {
        return Err(SasError::runtime(format!(
            "Insufficient non-missing observations ({m}) for {n_params} parameters in PROC REG.",
        )));
    }

    // Build the design matrix X (m × n_params), intercept first, and y.
    let mut x_matrix: Vec<Vec<f64>> = Vec::with_capacity(m);
    let mut y: Vec<f64> = Vec::with_capacity(m);
    for &r in &kept_rows {
        let mut row = Vec::with_capacity(n_params);
        row.push(1.0);
        for col in &x_vals {
            row.push(value_to_num(&col[r]).unwrap());
        }
        x_matrix.push(row);
        y.push(value_to_num(&y_vals[r]).unwrap());
    }

    // Solve OLS via QR.
    let beta = least_squares(&x_matrix, &y)?;

    // Fitted values and residuals.
    let mut fitted = vec![0.0_f64; m];
    let mut resid = vec![0.0_f64; m];
    for i in 0..m {
        let mut yhat = 0.0;
        for j in 0..n_params {
            yhat += x_matrix[i][j] * beta[j];
        }
        fitted[i] = yhat;
        resid[i] = y[i] - yhat;
    }

    // Sums of squares.
    let ybar = y.iter().sum::<f64>() / m as f64;
    let sst: f64 = y.iter().map(|v| (v - ybar) * (v - ybar)).sum();
    let sse: f64 = resid.iter().map(|e| e * e).sum();
    let ssr = (sst - sse).max(0.0);

    let df_model = (n_params - 1) as f64; // number of regressors
    let df_error = (m - n_params) as f64;
    let df_total = (m - 1) as f64;

    let mse = if df_error > 0.0 { sse / df_error } else { f64::NAN };
    let rmse = mse.sqrt();
    let ms_model = if df_model > 0.0 { ssr / df_model } else { f64::NAN };

    let r_square = if sst > 0.0 { ssr / sst } else { f64::NAN };
    let adj_r_square = if df_total > 0.0 && df_error > 0.0 {
        1.0 - (1.0 - r_square) * (df_total / df_error)
    } else {
        f64::NAN
    };

    let f_value = if mse > 0.0 { ms_model / mse } else { f64::NAN };
    let pr_f = if f_value.is_finite() && df_model > 0.0 && df_error > 0.0 {
        1.0 - f_cdf(f_value, df_model, df_error)
    } else {
        f64::NAN
    };

    // (X'X)⁻¹ for standard errors: form X'X then invert.
    let xtx = xtx_matrix(&x_matrix, n_params);
    let xtx_inv = invert_matrix(&xtx)?;

    // Per-parameter inference.
    let t_crit = if df_error > 0.0 {
        t_quantile(1.0 - ast.alpha / 2.0, df_error)
    } else {
        f64::NAN
    };
    let mut estimates: Vec<ParamEst> = Vec::with_capacity(n_params);
    for j in 0..n_params {
        let var_bj = xtx_inv[j][j] * mse;
        let se = if var_bj > 0.0 { var_bj.sqrt() } else { 0.0 };
        let tval = if se > 0.0 { beta[j] / se } else { f64::NAN };
        let pr_t = if tval.is_finite() && df_error > 0.0 {
            2.0 * (1.0 - student_t_cdf(tval.abs(), df_error))
        } else {
            f64::NAN
        };
        let (lo, hi) = if t_crit.is_finite() {
            (beta[j] - t_crit * se, beta[j] + t_crit * se)
        } else {
            (f64::NAN, f64::NAN)
        };
        estimates.push(ParamEst {
            estimate: beta[j],
            std_err: se,
            t_value: tval,
            pr_t,
            ci_lo: lo,
            ci_hi: hi,
        });
    }

    // --- listing ---
    emit_listing(
        session,
        ast,
        &ds,
        &in_display,
        y_col,
        &x_cols,
        n_obs,
        m,
        df_model,
        df_error,
        df_total,
        ssr,
        sse,
        sst,
        ms_model,
        mse,
        f_value,
        pr_f,
        rmse,
        ybar,
        r_square,
        adj_r_square,
        &estimates,
    );

    // --- OUTPUT OUT= (optional) ---
    if let Some(target) = &ast.output_out {
        write_output_dataset(session, &ds, target, &kept_rows, &fitted, &resid, n_obs)?;
    }

    Ok(())
}

/// One row of the Parameter Estimates table.
struct ParamEst {
    estimate: f64,
    std_err: f64,
    t_value: f64,
    pr_t: f64,
    ci_lo: f64,
    ci_hi: f64,
}

/// Form X'X (n_params × n_params) from the design matrix.
fn xtx_matrix(x: &[Vec<f64>], n_params: usize) -> Vec<Vec<f64>> {
    let mut out = vec![vec![0.0; n_params]; n_params];
    for row in x {
        for a in 0..n_params {
            for b in 0..n_params {
                out[a][b] += row[a] * row[b];
            }
        }
    }
    out
}

#[allow(clippy::too_many_arguments)]
fn emit_listing(
    session: &mut Session,
    ast: &RegAst,
    ds: &SasDataset,
    in_display: &str,
    y_col: usize,
    x_cols: &[usize],
    n_obs: usize,
    m: usize,
    df_model: f64,
    df_error: f64,
    df_total: f64,
    ssr: f64,
    sse: f64,
    sst: f64,
    ms_model: f64,
    mse: f64,
    f_value: f64,
    pr_f: f64,
    rmse: f64,
    ybar: f64,
    r_square: f64,
    adj_r_square: f64,
    estimates: &[ParamEst],
) {
    session.listing.page_header();
    centered(session, "The REG Procedure");
    session.listing.blank();
    centered(session, "Model: MODEL1");
    centered(
        session,
        &format!("Dependent Variable: {}", ds.vars[y_col].name),
    );
    session.listing.blank();

    // Model Information.
    session
        .listing
        .write_line(&format!("Data Set                {in_display}"));
    session
        .listing
        .write_line(&format!("Number of Observations Read       {n_obs}"));
    session
        .listing
        .write_line(&format!("Number of Observations Used       {m}"));
    if m < n_obs {
        let dropped = n_obs - m;
        session.listing.write_line(&format!(
            "Number of Observations with Missing Values       {dropped}"
        ));
    }
    session.listing.blank();

    // Analysis of Variance.
    centered(session, "Analysis of Variance");
    session.listing.blank();
    {
        let headers: Vec<String> = vec![
            "Source".into(),
            "DF".into(),
            "Sum of Squares".into(),
            "Mean Square".into(),
            "F Value".into(),
            "Pr > F".into(),
        ];
        let aligns = vec![
            Align::Left,
            Align::Right,
            Align::Right,
            Align::Right,
            Align::Right,
            Align::Right,
        ];
        let rows = vec![
            vec![
                "Model".into(),
                fmt_df(df_model),
                format_best(ssr, 12),
                format_best(ms_model, 12),
                fmt_f(f_value),
                fmt_p(pr_f),
            ],
            vec![
                "Error".into(),
                fmt_df(df_error),
                format_best(sse, 12),
                fmt_num(mse),
                String::new(),
                String::new(),
            ],
            vec![
                "Corrected Total".into(),
                fmt_df(df_total),
                format_best(sst, 12),
                String::new(),
                String::new(),
                String::new(),
            ],
        ];
        session.listing.write_table(&headers, &aligns, &rows);
    }
    session.listing.blank();

    // Fit statistics (Root MSE, Dependent Mean, R-Square, Adj R-Sq).
    session
        .listing
        .write_line(&format!("Root MSE           {}", fmt_num(rmse)));
    session
        .listing
        .write_line(&format!("Dependent Mean     {}", fmt_num(ybar)));
    session
        .listing
        .write_line(&format!("R-Square           {}", fmt_num(r_square)));
    session
        .listing
        .write_line(&format!("Adj R-Sq           {}", fmt_num(adj_r_square)));
    session.listing.blank();

    // Parameter Estimates.
    centered(session, "Parameter Estimates");
    session.listing.blank();
    {
        let level = ((1.0 - ast.alpha) * 100.0).round() as i64;
        let lo_hdr = format!("{level}% Conf");
        let hi_hdr = "Limits".to_string();
        let headers: Vec<String> = vec![
            "Variable".into(),
            "DF".into(),
            "Parameter Estimate".into(),
            "Standard Error".into(),
            "t Value".into(),
            "Pr > |t|".into(),
            lo_hdr,
            hi_hdr,
        ];
        let aligns = vec![
            Align::Left,
            Align::Right,
            Align::Right,
            Align::Right,
            Align::Right,
            Align::Right,
            Align::Right,
            Align::Right,
        ];
        // Row labels: Intercept, then each x variable.
        let mut labels: Vec<String> = vec!["Intercept".into()];
        for &c in x_cols {
            labels.push(ds.vars[c].name.clone());
        }
        let mut rows: Vec<Vec<String>> = Vec::with_capacity(estimates.len());
        for (i, est) in estimates.iter().enumerate() {
            rows.push(vec![
                labels[i].clone(),
                "1".into(),
                fmt_num(est.estimate),
                fmt_num(est.std_err),
                fmt_f(est.t_value),
                fmt_p(est.pr_t),
                fmt_num(est.ci_lo),
                fmt_num(est.ci_hi),
            ]);
        }
        session.listing.write_table(&headers, &aligns, &rows);
    }
    session.listing.blank();
}

/// Build the OUTPUT OUT= dataset: a copy of the input with PREDICTED and
/// RESIDUAL appended. Rows excluded from the fit get missing predicted/residual.
fn write_output_dataset(
    session: &mut Session,
    ds: &SasDataset,
    target: &DatasetRef,
    kept_rows: &[usize],
    fitted: &[f64],
    resid: &[f64],
    n_obs: usize,
) -> Result<()> {
    // Map original row index → fit position.
    let mut pred_full: Vec<Option<f64>> = vec![None; n_obs];
    let mut res_full: Vec<Option<f64>> = vec![None; n_obs];
    for (pos, &r) in kept_rows.iter().enumerate() {
        pred_full[r] = Some(fitted[pos]);
        res_full[r] = Some(resid[pos]);
    }

    let mut columns: Vec<Column> = ds.df.get_columns().to_vec();
    let mut vars: Vec<VarMeta> = ds.vars.clone();

    columns.push(Series::new("PREDICTED".into(), pred_full).into());
    vars.push(num_var_meta("PREDICTED"));
    columns.push(Series::new("RESIDUAL".into(), res_full).into());
    vars.push(num_var_meta("RESIDUAL"));

    let df = polars::prelude::DataFrame::new(columns)?;
    let out_ds = SasDataset { df, vars };

    let out_libref = target.libref_or_work();
    let out_table = target.name.to_uppercase();
    let display = format!("{out_libref}.{out_table}");
    let n_rows = out_ds.n_obs();
    let n_vars = out_ds.vars.len();

    session.libs.get(&out_libref)?.write(&out_table, &out_ds)?;
    session.last_dataset = Some(display.clone());
    session.log.note(&format!(
        "The data set {} has {} observations and {} variables.",
        display, n_rows, n_vars
    ));
    Ok(())
}

fn num_var_meta(name: &str) -> VarMeta {
    VarMeta {
        name: name.to_string(),
        ty: VarType::Num,
        length: 8,
        format: None,
        label: None,
    }
}

// ───────────────────────── formatting ─────────────────────────

/// Write a centered line within LINESIZE.
fn centered(session: &mut Session, text: &str) {
    let ls = session.listing.ls();
    let pad = ls.saturating_sub(text.len()) / 2;
    session
        .listing
        .write_line(&format!("{}{}", " ".repeat(pad), text));
}

/// Degrees of freedom: integer display.
fn fmt_df(df: f64) -> String {
    format!("{}", df.round() as i64)
}

/// A generic numeric cell (BESTw. style), missing/NaN → ".".
fn fmt_num(v: f64) -> String {
    if v.is_nan() {
        ".".to_string()
    } else {
        format_best(v, 12)
    }
}

/// F or t value: two decimals SAS-style, NaN → ".".
fn fmt_f(v: f64) -> String {
    if v.is_nan() {
        ".".to_string()
    } else {
        format!("{v:.2}")
    }
}

/// p-value SAS-style: `<.0001`, else 4 decimals; NaN → ".".
fn fmt_p(p: f64) -> String {
    if p.is_nan() {
        ".".to_string()
    } else if p < 0.0001 {
        "<.0001".to_string()
    } else {
        format!("{p:.4}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dataset::{SasDataset, VarMeta};
    use crate::session::Session;
    use crate::source::SourceFile;
    use crate::value::VarType;
    use polars::prelude::*;
    use std::path::PathBuf;

    fn make_session() -> Session {
        Session::new(None, PathBuf::from("."), true).unwrap()
    }

    fn num_meta(name: &str) -> VarMeta {
        VarMeta {
            name: name.to_string(),
            ty: VarType::Num,
            length: 8,
            format: None,
            label: None,
        }
    }

    fn write_dataset(session: &mut Session, table: &str, ds: SasDataset) {
        session.libs.get("WORK").unwrap().write(table, &ds).unwrap();
        session.last_dataset = Some(format!("WORK.{}", table.to_uppercase()));
    }

    fn parse_reg(src: &str) -> Result<RegAst> {
        let source = SourceFile::new(src);
        let mut ts = StatementStream::new(&source).unwrap();
        ts.next(); // "proc"
        ts.next(); // "reg"
        parse(&mut ts)
    }

    /// Run PROC REG end to end on a freshly-written WORK dataset; return the
    /// (listing, log) strings and the resulting session for dataset checks.
    /// `listing.into_string()` drains (it takes `&mut self`); `log` is read
    /// last because its `into_string` consumes the field.
    fn run_reg(ds: SasDataset, src: &str) -> (String, String, Session) {
        let mut session = make_session();
        write_dataset(&mut session, "A", ds);
        let ast = parse_reg(src).unwrap();
        execute(&ast, &mut session).unwrap();
        let listing = session.listing.into_string();
        let log = session.log.buffer().to_string();
        (listing, log, session)
    }

    // ───────────── parse tests ─────────────

    #[test]
    fn parse_minimal_model() {
        let ast = parse_reg("proc reg data=a; model y = x; run;").unwrap();
        assert_eq!(ast.data.as_ref().unwrap().name, "a");
        assert_eq!(ast.model.y_var, "y");
        assert_eq!(ast.model.x_vars, vec!["x"]);
        assert!((ast.alpha - 0.05).abs() < 1e-12);
        assert!(ast.output_out.is_none());
    }

    #[test]
    fn parse_alpha_and_output() {
        let ast = parse_reg("proc reg data=a alpha=0.10; model y = x1 x2; output out=b; run;")
            .unwrap();
        assert!((ast.alpha - 0.10).abs() < 1e-12);
        assert_eq!(ast.model.x_vars, vec!["x1", "x2"]);
        assert_eq!(ast.output_out.as_ref().unwrap().name, "b");
    }

    #[test]
    fn parse_bad_alpha_errors() {
        assert!(parse_reg("proc reg data=a alpha=1.5; model y=x; run;").is_err());
        assert!(parse_reg("proc reg data=a alpha=0; model y=x; run;").is_err());
    }

    #[test]
    fn parse_requires_model() {
        assert!(parse_reg("proc reg data=a; run;").is_err());
    }

    // ───────────── numeric / execute tests ─────────────

    fn simple_ds(x: &[f64], y: &[f64]) -> SasDataset {
        let df = df!["x" => x.to_vec(), "y" => y.to_vec()].unwrap();
        SasDataset {
            df,
            vars: vec![num_meta("x"), num_meta("y")],
        }
    }

    #[test]
    fn reg_simple() {
        // y = 2 + 3x exactly.
        let x = [0.0, 1.0, 2.0, 3.0, 4.0];
        let y: Vec<f64> = x.iter().map(|v| 2.0 + 3.0 * v).collect();
        let ds = simple_ds(&x, &y);
        let (listing, _log, _s) = run_reg(ds, "proc reg data=a; model y = x; run;");
        assert!(listing.contains("The REG Procedure"), "{listing}");
        assert!(listing.contains("Parameter Estimates"), "{listing}");
        // Intercept 2, slope 3 should appear with BEST formatting.
        assert!(listing.contains("Intercept"), "{listing}");
        // R-Square ~ 1 for a perfect fit.
        assert!(listing.contains("R-Square"), "{listing}");
    }

    #[test]
    fn reg_simple_coefficients_exact() {
        // Verify the solved beta directly through the public solver path.
        let x = [0.0, 1.0, 2.0, 3.0, 4.0];
        let y: Vec<f64> = x.iter().map(|v| 2.0 + 3.0 * v).collect();
        let design: Vec<Vec<f64>> = x.iter().map(|&v| vec![1.0, v]).collect();
        let beta = least_squares(&design, &y).unwrap();
        assert!((beta[0] - 2.0).abs() < 1e-9, "intercept={}", beta[0]);
        assert!((beta[1] - 3.0).abs() < 1e-9, "slope={}", beta[1]);
    }

    #[test]
    fn reg_multiple() {
        // y = 1 + 2*x1 + 3*x2 exactly.
        let x1 = [1.0, 2.0, 3.0, 4.0, 5.0, 6.0];
        let x2 = [2.0, 1.0, 4.0, 3.0, 6.0, 5.0];
        let y: Vec<f64> = x1
            .iter()
            .zip(x2.iter())
            .map(|(a, b)| 1.0 + 2.0 * a + 3.0 * b)
            .collect();
        let df = df!["x1" => x1.to_vec(), "x2" => x2.to_vec(), "y" => y].unwrap();
        let ds = SasDataset {
            df,
            vars: vec![num_meta("x1"), num_meta("x2"), num_meta("y")],
        };
        let (listing, _log, _s) = run_reg(ds, "proc reg data=a; model y = x1 x2; run;");
        assert!(listing.contains("x1"), "{listing}");
        assert!(listing.contains("x2"), "{listing}");
        // Solve directly to assert exact coefficients.
        let design: Vec<Vec<f64>> = x1
            .iter()
            .zip(x2.iter())
            .map(|(&a, &b)| vec![1.0, a, b])
            .collect();
        let yv: Vec<f64> = x1
            .iter()
            .zip(x2.iter())
            .map(|(a, b)| 1.0 + 2.0 * a + 3.0 * b)
            .collect();
        let beta = least_squares(&design, &yv).unwrap();
        assert!((beta[0] - 1.0).abs() < 1e-7, "b0={}", beta[0]);
        assert!((beta[1] - 2.0).abs() < 1e-7, "b1={}", beta[1]);
        assert!((beta[2] - 3.0).abs() < 1e-7, "b2={}", beta[2]);
    }

    #[test]
    fn reg_with_missing() {
        // One row has a missing x; it must be excluded (Used = 4 of 5).
        let x = vec![Some(1.0), Some(2.0), None, Some(4.0), Some(5.0)];
        let y = vec![Some(3.0), Some(5.0), Some(7.0), Some(9.0), Some(11.0)];
        let df = df![
            "x" => x,
            "y" => y,
        ]
        .unwrap();
        let ds = SasDataset {
            df,
            vars: vec![num_meta("x"), num_meta("y")],
        };
        let (listing, _log, _s) = run_reg(ds, "proc reg data=a; model y = x; run;");
        assert!(
            listing.contains("Number of Observations Read       5"),
            "{listing}"
        );
        assert!(
            listing.contains("Number of Observations Used       4"),
            "{listing}"
        );
        assert!(
            listing.contains("Number of Observations with Missing Values"),
            "{listing}"
        );
    }

    #[test]
    fn reg_output() {
        // OUTPUT OUT= adds PREDICTED and RESIDUAL; excluded rows are missing.
        let x = vec![Some(1.0), Some(2.0), None, Some(4.0)];
        let y = vec![Some(3.0), Some(5.0), Some(7.0), Some(9.0)];
        let df = df!["x" => x, "y" => y].unwrap();
        let ds = SasDataset {
            df,
            vars: vec![num_meta("x"), num_meta("y")],
        };
        let (_listing, log, session) =
            run_reg(ds, "proc reg data=a; model y = x; output out=o; run;");
        assert!(log.contains("WORK.O"), "{log}");
        let (out, _) = session.libs.get("WORK").unwrap().read("O").unwrap();
        let names: Vec<&str> = out.vars.iter().map(|v| v.name.as_str()).collect();
        assert!(names.contains(&"PREDICTED"), "{names:?}");
        assert!(names.contains(&"RESIDUAL"), "{names:?}");
        assert_eq!(out.n_obs(), 4);
        // The excluded row (index 2) must have a missing PREDICTED.
        let pred = out
            .df
            .column("PREDICTED")
            .unwrap()
            .as_materialized_series()
            .f64()
            .unwrap();
        assert!(pred.get(2).is_none(), "excluded row should be missing");
        assert!(pred.get(0).is_some());
    }

    #[test]
    fn reg_alpha_option() {
        // ALPHA=0.10 changes the confidence-interval header to 90%.
        let x = [1.0, 2.0, 3.0, 4.0, 5.0];
        let y = [2.1, 3.9, 6.2, 7.8, 10.1];
        let ds = simple_ds(&x, &y);
        let (listing, _log, _s) =
            run_reg(ds, "proc reg data=a alpha=0.10; model y = x; run;");
        assert!(listing.contains("90% Conf"), "{listing}");
        assert!(!listing.contains("95% Conf"), "{listing}");
    }

    #[test]
    fn reg_r_squared() {
        // Known data: x=[1,2,3,4,5], y=[2,4,5,4,5].
        // Computed R^2 = 0.6 (SSR=6, SST=10).
        let x = [1.0, 2.0, 3.0, 4.0, 5.0];
        let y = [2.0, 4.0, 5.0, 4.0, 5.0];
        let design: Vec<Vec<f64>> = x.iter().map(|&v| vec![1.0, v]).collect();
        let beta = least_squares(&design, &y.to_vec()).unwrap();
        let m = x.len();
        let ybar = y.iter().sum::<f64>() / m as f64;
        let mut sse = 0.0;
        for i in 0..m {
            let yhat = beta[0] + beta[1] * x[i];
            sse += (y[i] - yhat) * (y[i] - yhat);
        }
        let sst: f64 = y.iter().map(|v| (v - ybar) * (v - ybar)).sum();
        let r2 = (sst - sse) / sst;
        assert!((r2 - 0.6).abs() < 1e-9, "r2={r2}");

        // And the listing prints an R-Square line.
        let ds = simple_ds(&x, &y);
        let (listing, _log, _s) = run_reg(ds, "proc reg data=a; model y = x; run;");
        assert!(listing.contains("R-Square"), "{listing}");
    }

    #[test]
    fn reg_f_test() {
        // For x=[1,2,3,4,5], y=[2,4,5,4,5]: SSR=6, SSE=4, df=(1,3).
        // F = (6/1)/(4/3) = 4.5; Pr>F = 1 - F_cdf(4.5, 1, 3).
        let f = 4.5;
        let p = 1.0 - f_cdf(f, 1.0, 3.0);
        // Sanity: p between 0 and 1, and not tiny (small sample).
        assert!(p > 0.0 && p < 1.0, "p={p}");
        assert!((p - 0.1241).abs() < 1e-2, "p={p}");

        let x = [1.0, 2.0, 3.0, 4.0, 5.0];
        let y = [2.0, 4.0, 5.0, 4.0, 5.0];
        let ds = simple_ds(&x, &y);
        let (listing, _log, _s) = run_reg(ds, "proc reg data=a; model y = x; run;");
        assert!(listing.contains("F Value"), "{listing}");
        assert!(listing.contains("Analysis of Variance"), "{listing}");
    }

    #[test]
    fn reg_singular_errors() {
        // x2 = 2*x1 → perfectly collinear → singular X'X.
        let x1 = [1.0, 2.0, 3.0, 4.0];
        let x2: Vec<f64> = x1.iter().map(|v| 2.0 * v).collect();
        let y = [1.0, 2.0, 3.0, 5.0];
        let df = df!["x1" => x1.to_vec(), "x2" => x2, "y" => y.to_vec()].unwrap();
        let ds = SasDataset {
            df,
            vars: vec![num_meta("x1"), num_meta("x2"), num_meta("y")],
        };
        let mut session = make_session();
        write_dataset(&mut session, "A", ds);
        let ast = parse_reg("proc reg data=a; model y = x1 x2; run;").unwrap();
        let res = execute(&ast, &mut session);
        assert!(res.is_err(), "collinear model should error");
    }
}
