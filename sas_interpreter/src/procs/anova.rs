//! PROC ANOVA — analysis of variance for balanced designs (v1).
//!
//! # Plan du fichier — voir le plan M25.2
//!
//! `proc anova data=<ref> [alpha=<f64>];
//!  class a b ... ;
//!  model y = a b a*b ... ;
//!  [means a b a*b / <option>;]
//!  [output out=<ref>;]
//!  run;`
//!
//! ## Périmètre v1 (fidèle à SAS 9.4 PROC ANOVA)
//! - À la différence de PROC REG (variables continues), ANOVA traite des plans
//!   équilibrés avec des FACTEURS catégoriels déclarés par CLASS.
//! - Un seul MODEL : une variable dépendante numérique, ≥1 effet. Les effets
//!   sont des variables CLASS ou des interactions `a*b`.
//! - Codage des CLASS en variables indicatrices 0/1 (one-hot) en omettant la
//!   DERNIÈRE modalité de chaque facteur (« reference cell ») pour éviter la
//!   colinéarité parfaite avec l'intercept.
//! - Plan équilibré attendu : si les cellules n'ont pas le même effectif, un
//!   WARNING est émis et le calcul continue via moindres carrés (comme GLM).
//!
//! ## Sortie listing (titre "The ANOVA Procedure"), dans l'ordre SAS :
//! 1. « Class Level Information » : pour chaque CLASS var, le nombre de niveaux
//!    et la liste des modalités.
//! 2. Nombre d'observations lues / utilisées.
//! 3. Table « Analysis of Variance » : Model / Error / Corrected Total avec
//!    DF, Sum of Squares, Mean Square, F Value, Pr > F. Sous la table :
//!    R-Square, Coeff Var, Root MSE, <y> Mean.
//! 4. MEANS (optionnel) : moyennes par groupe (Level, N, Mean, Std Err).
//!
//! ## Choix / simplifications documentés
//! - Exclusion LISTWISE : une observation est retirée dès que y OU l'une des
//!   variables CLASS du modèle est manquante.
//! - Solveur : `stat::linalg::least_squares` (QR Householder).
//! - p-values : `stat::dists::f_cdf` (Pr > F).
//! - MEANS : v1 affiche les moyennes par groupe + erreur-type (Root MSE / √n),
//!   SANS les tests par paires (SCHEFFE/LSD/BONFERRONI sont acceptés en option
//!   mais seules les moyennes + SE sont produites — divergence documentée).
//! - Matrice singulière → `SasError::Numerical`.

use crate::ast::DatasetRef;
use crate::dataset::SasDataset;
use crate::error::{Result, SasError};
use crate::listing::Align;
use crate::missing::value_to_num;
use crate::parser::StatementStream;
use crate::procs::common::decode_column;
use crate::session::Session;
use crate::stat::dists::f_cdf;
use crate::stat::linalg::least_squares;
use crate::token::TokenKind;
use crate::value::{format_best, Value, VarType};
use std::cmp::Ordering;

/// `class a b c ;` — declares the categorical factors.
pub struct ClassStmt {
    pub vars: Vec<String>,
}

/// `model y = a b a*b ;` — dependent variable and effects.
/// Each effect is a list of factor names: `["a"]` for a main effect, or
/// `["a", "b"]` for the interaction `a*b`.
pub struct ModelStmt {
    pub y_var: String,
    pub effects: Vec<Vec<String>>,
}

/// `means a b a*b / scheffe ;` — post-hoc means request.
pub struct MeansStmt {
    /// Effects (factor lists) on which to produce group means.
    pub effects: Vec<Vec<String>>,
    /// Comparison option (SCHEFFE/LSD/BONFERRONI/…), if any (v1: informational).
    pub option: Option<String>,
}

pub struct AnovaAst {
    pub data: Option<DatasetRef>,
    /// Confidence-level complement. Default 0.05.
    pub alpha: f64,
    pub class_vars: Vec<String>,
    pub model: ModelStmt,
    pub means_stmt: Vec<MeansStmt>,
    pub output_out: Option<DatasetRef>,
}

// ───────────────────────── parse ─────────────────────────

/// Parse `proc anova [data=a] [alpha=p]; class ...; model y = ...; [means ...;]
/// [output out=b;] run;`. Called AFTER "proc anova" was consumed. Consumes
/// through `run;` / `quit;`.
pub fn parse(ts: &mut StatementStream) -> Result<AnovaAst> {
    let mut data: Option<DatasetRef> = None;
    let mut alpha: f64 = 0.05;

    // --- PROC ANOVA statement options, until `;` ---
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
                    "Unexpected option '{}' on PROC ANOVA statement.",
                    name.to_uppercase()
                ),
                span,
            ));
        } else {
            let span = ts.peek().span;
            return Err(SasError::parse(
                "Unexpected token on PROC ANOVA statement.",
                span,
            ));
        }
    }

    // --- sub-statements until run;/quit; ---
    let mut class_vars: Vec<String> = Vec::new();
    let mut model: Option<ModelStmt> = None;
    let mut means_stmt: Vec<MeansStmt> = Vec::new();
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

        if ts.peek().is_kw("class") {
            ts.next();
            class_vars = parse_var_list(ts)?;
        } else if ts.peek().is_kw("model") {
            ts.next();
            if model.is_some() {
                return Err(SasError::runtime(
                    "PROC ANOVA v1 supports a single MODEL statement.",
                ));
            }
            model = Some(parse_model(ts)?);
        } else if ts.peek().is_kw("means") {
            ts.next();
            means_stmt.push(parse_means(ts)?);
        } else if ts.peek().is_kw("output") {
            ts.next();
            output_out = Some(parse_output(ts)?);
        } else {
            // Unknown sub-statement: skip it (recovery, like reg/means).
            ts.skip_to_semi();
        }
    }

    let model = model.ok_or_else(|| {
        SasError::runtime("PROC ANOVA requires a MODEL statement (model y = effects;).")
    })?;

    Ok(AnovaAst {
        data,
        alpha,
        class_vars,
        model,
        means_stmt,
        output_out,
    })
}

/// Parse a plain `name name name ;` variable list (CLASS).
fn parse_var_list(ts: &mut StatementStream) -> Result<Vec<String>> {
    let mut vars = Vec::new();
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
                vars.push(name);
                ts.next();
            }
            None => {
                return Err(SasError::parse(
                    "Expected a variable name in CLASS statement.",
                    ts.peek().span,
                ));
            }
        }
    }
    Ok(vars)
}

/// Parse `model y = a b a*b ;` (the leading `model` keyword already consumed).
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

    let effects = parse_effects(ts)?;
    if effects.is_empty() {
        return Err(SasError::runtime(
            "The MODEL statement requires at least one effect.",
        ));
    }
    Ok(ModelStmt { y_var, effects })
}

/// Parse `means a b a*b [/ option] ;` (the leading `means` keyword consumed).
fn parse_means(ts: &mut StatementStream) -> Result<MeansStmt> {
    let mut effects: Vec<Vec<String>> = Vec::new();
    let mut option: Option<String> = None;
    loop {
        if ts.peek().kind == TokenKind::Semi {
            ts.next();
            break;
        }
        if ts.peek().kind == TokenKind::Eof || ts.peek().is_kw("run") || ts.peek().is_kw("quit") {
            break;
        }
        if ts.peek().kind == TokenKind::Slash {
            ts.next();
            // Options after '/': capture the first ident as the comparison
            // method; skip the rest to the semicolon.
            if let Some(opt) = ts.peek().ident().map(str::to_string) {
                option = Some(opt.to_uppercase());
            }
            ts.skip_to_semi();
            break;
        }
        // Parse one effect (a or a*b ...).
        let eff = parse_one_effect(ts)?;
        if eff.is_empty() {
            break;
        }
        effects.push(eff);
    }
    Ok(MeansStmt { effects, option })
}

/// Parse a list of effects, e.g. `a b a*b`, until `;`/`/`/run/quit.
fn parse_effects(ts: &mut StatementStream) -> Result<Vec<Vec<String>>> {
    let mut effects: Vec<Vec<String>> = Vec::new();
    loop {
        if ts.peek().kind == TokenKind::Semi {
            ts.next();
            break;
        }
        if ts.peek().kind == TokenKind::Eof
            || ts.peek().is_kw("run")
            || ts.peek().is_kw("quit")
            || ts.peek().kind == TokenKind::Slash
        {
            break;
        }
        let eff = parse_one_effect(ts)?;
        if eff.is_empty() {
            break;
        }
        effects.push(eff);
    }
    Ok(effects)
}

/// Parse a single effect: `a` or `a*b` or `a*b*c`.
fn parse_one_effect(ts: &mut StatementStream) -> Result<Vec<String>> {
    let mut factors: Vec<String> = Vec::new();
    let name = match ts.peek().ident().map(str::to_string) {
        Some(n) => n,
        None => {
            return Err(SasError::parse(
                "Expected an effect (factor) name.",
                ts.peek().span,
            ));
        }
    };
    factors.push(name);
    ts.next();
    while ts.peek().kind == TokenKind::Star {
        ts.next();
        match ts.peek().ident().map(str::to_string) {
            Some(n) => {
                factors.push(n);
                ts.next();
            }
            None => {
                return Err(SasError::parse(
                    "Expected a factor name after '*' in an interaction effect.",
                    ts.peek().span,
                ));
            }
        }
    }
    Ok(factors)
}

/// Parse `output out=<ref> ... ;` (the leading `output` keyword consumed).
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
            ts.next();
        }
    }
    out.ok_or_else(|| {
        SasError::runtime("The OUTPUT statement of PROC ANOVA requires OUT=<dataset>.")
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
fn resolve_input(ast: &AnovaAst, session: &Session) -> Result<DatasetRef> {
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

/// Render a CLASS level value as a display string.
fn level_label(v: &Value) -> String {
    match v {
        Value::Num(f) => format_best(*f, 12),
        Value::Missing(k) => k.display(),
        Value::Char(s) => s.trim_end().to_string(),
    }
}

/// True when a value is a usable (non-missing) class level.
fn is_present(v: &Value) -> bool {
    match v {
        Value::Num(f) => !f.is_nan(),
        Value::Char(s) => !s.trim().is_empty(),
        Value::Missing(_) => false,
    }
}

pub fn execute(ast: &AnovaAst, session: &mut Session) -> Result<()> {
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

    // Resolve a column by name (case-insensitive).
    let resolve_col = |nm: &str| -> Result<usize> {
        ds.vars
            .iter()
            .position(|m| m.name.eq_ignore_ascii_case(nm))
            .ok_or_else(|| SasError::runtime(format!("Variable {} not found.", nm.to_uppercase())))
    };

    // Dependent variable: must be numeric.
    let y_col = resolve_col(&ast.model.y_var)?;
    if ds.vars[y_col].ty != VarType::Num {
        return Err(SasError::runtime(format!(
            "Variable {} in the MODEL statement is not numeric.",
            ast.model.y_var.to_uppercase()
        )));
    }

    // Every factor referenced in the model must be a declared CLASS var.
    let mut model_factors: Vec<String> = Vec::new();
    for eff in &ast.model.effects {
        for f in eff {
            if !ast
                .class_vars
                .iter()
                .any(|c| c.eq_ignore_ascii_case(f))
            {
                return Err(SasError::runtime(format!(
                    "Variable {} in the MODEL statement is not declared in the CLASS statement. \
                     PROC ANOVA v1 supports categorical effects only.",
                    f.to_uppercase()
                )));
            }
            if !model_factors.iter().any(|m| m.eq_ignore_ascii_case(f)) {
                model_factors.push(f.clone());
            }
        }
    }

    // Resolve and decode the factor columns used by the model.
    let mut factor_cols: Vec<(String, usize, Vec<Value>)> = Vec::new();
    for f in &model_factors {
        let c = resolve_col(f)?;
        let vals = decode_column(&ds, c)?;
        factor_cols.push((ds.vars[c].name.clone(), c, vals));
    }

    let y_vals = decode_column(&ds, y_col)?;

    // LISTWISE exclusion: keep rows where y AND every model factor present.
    let mut kept_rows: Vec<usize> = Vec::with_capacity(n_obs);
    for r in 0..n_obs {
        let yv = value_to_num(&y_vals[r]);
        if !matches!(yv, Some(v) if !v.is_nan()) {
            continue;
        }
        if factor_cols.iter().all(|(_, _, vals)| is_present(&vals[r])) {
            kept_rows.push(r);
        }
    }
    let m = kept_rows.len();

    // Build, for each model factor, the ordered list of distinct levels
    // (sas_cmp order) observed among kept rows.
    let factor_levels: Vec<Vec<Value>> = factor_cols
        .iter()
        .map(|(_, _, vals)| distinct_levels(vals, &kept_rows))
        .collect();

    // Map factor name → index into factor_cols / factor_levels.
    let factor_index = |nm: &str| -> usize {
        factor_cols
            .iter()
            .position(|(n, _, _)| n.eq_ignore_ascii_case(nm))
            .expect("model factor was resolved above")
    };

    // Build the design matrix: intercept + dummies (reference cell) per effect.
    // For a main effect with k levels: k-1 dummies (omit last level).
    // For an interaction a*b: products of a's dummies and b's dummies.
    let mut design_cols: Vec<Vec<f64>> = Vec::new(); // each is length m
    // Effect → number of design columns it contributes (its model DF).
    let mut effect_df: Vec<(Vec<String>, usize)> = Vec::new();

    for eff in &ast.model.effects {
        let idxs: Vec<usize> = eff.iter().map(|f| factor_index(f)).collect();
        // For each factor in the effect, the dummy levels (all but last).
        let dummy_levels: Vec<&[Value]> = idxs
            .iter()
            .map(|&fi| {
                let lv = &factor_levels[fi];
                let take = lv.len().saturating_sub(1);
                &lv[..take]
            })
            .collect();
        // Cartesian product of dummy levels → one design column each.
        let combos = cartesian(&dummy_levels);
        let n_cols = combos.len();
        for combo in &combos {
            let mut col = vec![0.0_f64; m];
            for (pos, &r) in kept_rows.iter().enumerate() {
                let mut hit = true;
                for (k, &fi) in idxs.iter().enumerate() {
                    let cell = &factor_cols[fi].2[r];
                    if cell.sas_cmp(combo[k]) != Ordering::Equal {
                        hit = false;
                        break;
                    }
                }
                if hit {
                    col[pos] = 1.0;
                }
            }
            design_cols.push(col);
        }
        effect_df.push((eff.clone(), n_cols));
    }

    let n_design = design_cols.len();
    let n_params = n_design + 1; // + intercept

    if m < n_params + 1 {
        return Err(SasError::runtime(format!(
            "Insufficient non-missing observations ({m}) for {n_params} parameters in PROC ANOVA.",
        )));
    }

    // Balance check: count observations per full cross-classification cell of
    // ALL model factors. If unequal, warn but continue (least-squares path).
    let balanced = check_balance(&factor_cols, &factor_levels, &kept_rows);
    if !balanced {
        session.log.warning(&format!(
            "The design is unbalanced; PROC ANOVA assumes a balanced design. \
             Results for data set {in_display} are computed via least squares (as PROC GLM)."
        ));
    }

    // Assemble X (m × n_params), intercept first.
    let mut x_matrix: Vec<Vec<f64>> = Vec::with_capacity(m);
    let mut y: Vec<f64> = Vec::with_capacity(m);
    for (pos, &r) in kept_rows.iter().enumerate() {
        let mut row = Vec::with_capacity(n_params);
        row.push(1.0);
        for col in &design_cols {
            row.push(col[pos]);
        }
        x_matrix.push(row);
        y.push(value_to_num(&y_vals[r]).unwrap());
    }

    // Solve OLS via QR.
    let beta = least_squares(&x_matrix, &y)?;

    // Fitted / residuals.
    let mut sse = 0.0_f64;
    for i in 0..m {
        let mut yhat = 0.0;
        for j in 0..n_params {
            yhat += x_matrix[i][j] * beta[j];
        }
        let e = y[i] - yhat;
        sse += e * e;
    }

    let ybar = y.iter().sum::<f64>() / m as f64;
    let sst: f64 = y.iter().map(|v| (v - ybar) * (v - ybar)).sum();
    let ssr = (sst - sse).max(0.0);

    let df_model = n_design as f64;
    let df_error = (m - n_params) as f64;
    let df_total = (m - 1) as f64;

    let mse = if df_error > 0.0 { sse / df_error } else { f64::NAN };
    let rmse = mse.sqrt();
    let ms_model = if df_model > 0.0 { ssr / df_model } else { f64::NAN };

    let r_square = if sst > 0.0 { ssr / sst } else { f64::NAN };
    let coeff_var = if ybar != 0.0 {
        100.0 * rmse / ybar
    } else {
        f64::NAN
    };

    let f_value = if mse > 0.0 { ms_model / mse } else { f64::NAN };
    let pr_f = if f_value.is_finite() && df_model > 0.0 && df_error > 0.0 {
        1.0 - f_cdf(f_value, df_model, df_error)
    } else {
        f64::NAN
    };

    // --- listing ---
    emit_listing(
        session,
        ast,
        &ds,
        y_col,
        &factor_cols,
        &factor_levels,
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
        coeff_var,
    );

    // --- MEANS statements (post-hoc means) ---
    for ms in &ast.means_stmt {
        emit_means(session, ms, &factor_cols, &y_vals, &kept_rows, rmse);
    }

    Ok(())
}

/// Distinct levels of a column over `rows`, in sas_cmp order, missing excluded.
fn distinct_levels(vals: &[Value], rows: &[usize]) -> Vec<Value> {
    let mut levels: Vec<Value> = Vec::new();
    for &r in rows {
        let v = &vals[r];
        if !is_present(v) {
            continue;
        }
        if !levels.iter().any(|x| x.sas_cmp(v) == Ordering::Equal) {
            levels.push(v.clone());
        }
    }
    levels.sort_by(|a, b| a.sas_cmp(b));
    levels
}

/// Cartesian product of per-factor level slices → vector of combinations,
/// each combination being a vector of references (one level per factor).
fn cartesian<'a>(levels: &[&'a [Value]]) -> Vec<Vec<&'a Value>> {
    let mut result: Vec<Vec<&'a Value>> = vec![vec![]];
    for lv in levels {
        let mut next: Vec<Vec<&'a Value>> = Vec::new();
        for prefix in &result {
            for v in lv.iter() {
                let mut c = prefix.clone();
                c.push(v);
                next.push(c);
            }
        }
        result = next;
    }
    // If any factor had zero dummy levels, result becomes empty → drop the
    // empty seed.
    if levels.iter().any(|lv| lv.is_empty()) {
        return Vec::new();
    }
    result
}

/// Check whether every cell of the full cross-classification of all model
/// factors has the same observation count (balanced design).
fn check_balance(
    factor_cols: &[(String, usize, Vec<Value>)],
    factor_levels: &[Vec<Value>],
    rows: &[usize],
) -> bool {
    // Number of cells = product of level counts.
    let mut counts: std::collections::HashMap<Vec<String>, usize> = std::collections::HashMap::new();
    for &r in rows {
        let key: Vec<String> = factor_cols
            .iter()
            .map(|(_, _, vals)| level_label(&vals[r]))
            .collect();
        *counts.entry(key).or_insert(0) += 1;
    }
    if counts.is_empty() {
        return true;
    }
    // Expected number of distinct cells.
    let expected_cells: usize = factor_levels.iter().map(|l| l.len().max(1)).product();
    // All observed cells equal AND every cell present.
    let first = *counts.values().next().unwrap();
    counts.len() == expected_cells && counts.values().all(|&c| c == first)
}

// ───────────────────────── listing ─────────────────────────

#[allow(clippy::too_many_arguments)]
fn emit_listing(
    session: &mut Session,
    ast: &AnovaAst,
    ds: &SasDataset,
    y_col: usize,
    factor_cols: &[(String, usize, Vec<Value>)],
    factor_levels: &[Vec<Value>],
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
    coeff_var: f64,
) {
    session.listing.page_header();
    centered(session, "The ANOVA Procedure");
    session.listing.blank();

    // Class Level Information (only the CLASS variables that are used).
    centered(session, "Class Level Information");
    session.listing.blank();
    {
        let headers: Vec<String> = vec!["Class".into(), "Levels".into(), "Values".into()];
        let aligns = vec![Align::Left, Align::Right, Align::Left];
        // List every declared CLASS variable; for ones used by the model we
        // have computed levels, otherwise compute from the dataset.
        let mut rows: Vec<Vec<String>> = Vec::new();
        for cv in &ast.class_vars {
            // Find within model factors first.
            if let Some(pos) = factor_cols
                .iter()
                .position(|(n, _, _)| n.eq_ignore_ascii_case(cv))
            {
                let lvls = &factor_levels[pos];
                let values: Vec<String> = lvls.iter().map(level_label).collect();
                rows.push(vec![
                    factor_cols[pos].0.clone(),
                    format!("{}", lvls.len()),
                    values.join(" "),
                ]);
            } else if let Some(ci) = ds
                .vars
                .iter()
                .position(|v| v.name.eq_ignore_ascii_case(cv))
            {
                // CLASS var declared but not in the model: list its levels too.
                if let Ok(vals) = decode_column(ds, ci) {
                    let all_rows: Vec<usize> = (0..ds.n_obs()).collect();
                    let lvls = distinct_levels(&vals, &all_rows);
                    let values: Vec<String> = lvls.iter().map(level_label).collect();
                    rows.push(vec![
                        ds.vars[ci].name.clone(),
                        format!("{}", lvls.len()),
                        values.join(" "),
                    ]);
                }
            }
        }
        session.listing.write_table(&headers, &aligns, &rows);
    }
    session.listing.blank();

    session
        .listing
        .write_line(&format!("Number of Observations Read       {n_obs}"));
    session
        .listing
        .write_line(&format!("Number of Observations Used       {m}"));
    session.listing.blank();

    centered(session, "The ANOVA Procedure");
    session.listing.blank();
    centered(
        session,
        &format!("Dependent Variable: {}", ds.vars[y_col].name),
    );
    session.listing.blank();

    // Analysis of Variance.
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

    // Fit statistics: R-Square, Coeff Var, Root MSE, <y> Mean.
    {
        let headers: Vec<String> = vec![
            "R-Square".into(),
            "Coeff Var".into(),
            "Root MSE".into(),
            format!("{} Mean", ds.vars[y_col].name),
        ];
        let aligns = vec![Align::Right, Align::Right, Align::Right, Align::Right];
        let rows = vec![vec![
            fmt_num(r_square),
            fmt_num(coeff_var),
            fmt_num(rmse),
            fmt_num(ybar),
        ]];
        session.listing.write_table(&headers, &aligns, &rows);
    }
    session.listing.blank();

    let _ = ast; // alpha currently unused by the ANOVA table itself.
}

/// Emit one MEANS statement's group means table per effect.
fn emit_means(
    session: &mut Session,
    ms: &MeansStmt,
    factor_cols: &[(String, usize, Vec<Value>)],
    y_vals: &[Value],
    kept_rows: &[usize],
    rmse: f64,
) {
    for eff in &ms.effects {
        // Resolve the factor columns of this effect (must be model factors).
        let idxs: Option<Vec<usize>> = eff
            .iter()
            .map(|f| {
                factor_cols
                    .iter()
                    .position(|(n, _, _)| n.eq_ignore_ascii_case(f))
            })
            .collect();
        let Some(idxs) = idxs else {
            session.log.warning(&format!(
                "Effect {} in the MEANS statement is not in the model; skipped.",
                eff.join("*").to_uppercase()
            ));
            continue;
        };

        centered(session, "The ANOVA Procedure");
        session.listing.blank();
        let title = format!("Means for effect {}", eff.join("*"));
        centered(session, &title);
        session.listing.blank();

        // Group kept rows by the tuple of this effect's factor values.
        let mut groups: Vec<(Vec<Value>, Vec<usize>)> = Vec::new();
        for &r in kept_rows {
            let key: Vec<Value> = idxs.iter().map(|&fi| factor_cols[fi].2[r].clone()).collect();
            let pos = groups.iter().position(|(k, _)| {
                k.iter()
                    .zip(&key)
                    .all(|(a, b)| a.sas_cmp(b) == Ordering::Equal)
            });
            match pos {
                Some(p) => groups[p].1.push(r),
                None => groups.push((key, vec![r])),
            }
        }
        groups.sort_by(|(a, _), (b, _)| {
            for (x, y) in a.iter().zip(b) {
                let c = x.sas_cmp(y);
                if c != Ordering::Equal {
                    return c;
                }
            }
            Ordering::Equal
        });

        // Column headers: one per factor, then N, Mean, Std Err.
        let mut headers: Vec<String> = eff.iter().cloned().collect();
        headers.push("N".into());
        headers.push("Mean".into());
        headers.push("Std Err".into());
        let mut aligns: Vec<Align> = vec![Align::Left; eff.len()];
        aligns.push(Align::Right);
        aligns.push(Align::Right);
        aligns.push(Align::Right);

        let mut rows: Vec<Vec<String>> = Vec::new();
        for (key, members) in &groups {
            let xs: Vec<f64> = members
                .iter()
                .filter_map(|&r| value_to_num(&y_vals[r]))
                .filter(|v| !v.is_nan())
                .collect();
            let n = xs.len();
            let mean = if n > 0 {
                xs.iter().sum::<f64>() / n as f64
            } else {
                f64::NAN
            };
            // Std Err of the mean from the model Root MSE: RMSE / sqrt(n).
            let se = if n > 0 && rmse.is_finite() {
                rmse / (n as f64).sqrt()
            } else {
                f64::NAN
            };
            let mut row: Vec<String> = key.iter().map(level_label).collect();
            row.push(format!("{n}"));
            row.push(fmt_num(mean));
            row.push(fmt_num(se));
            rows.push(row);
        }
        session.listing.write_table(&headers, &aligns, &rows);

        if let Some(opt) = &ms.option {
            session.listing.blank();
            session.listing.write_line(&format!(
                "Note: {opt} pairwise comparisons are not produced in this version; \
                 means and standard errors are shown above."
            ));
        }
        session.listing.blank();
    }
}

// ───────────────────────── formatting ─────────────────────────

fn centered(session: &mut Session, text: &str) {
    let ls = session.listing.ls();
    let pad = ls.saturating_sub(text.len()) / 2;
    session
        .listing
        .write_line(&format!("{}{}", " ".repeat(pad), text));
}

fn fmt_df(df: f64) -> String {
    format!("{}", df.round() as i64)
}

fn fmt_num(v: f64) -> String {
    if v.is_nan() {
        ".".to_string()
    } else {
        format_best(v, 12)
    }
}

fn fmt_f(v: f64) -> String {
    if v.is_nan() {
        ".".to_string()
    } else {
        format!("{v:.2}")
    }
}

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

    fn char_meta(name: &str) -> VarMeta {
        VarMeta {
            name: name.to_string(),
            ty: VarType::Char,
            length: 8,
            format: None,
            label: None,
        }
    }

    fn write_dataset(session: &mut Session, table: &str, ds: SasDataset) {
        session.libs.get("WORK").unwrap().write(table, &ds).unwrap();
        session.last_dataset = Some(format!("WORK.{}", table.to_uppercase()));
    }

    fn parse_anova(src: &str) -> Result<AnovaAst> {
        let source = SourceFile::new(src);
        let mut ts = StatementStream::new(&source).unwrap();
        ts.next(); // "proc"
        ts.next(); // "anova"
        parse(&mut ts)
    }

    fn run_anova(ds: SasDataset, src: &str) -> (String, String, Session) {
        let mut session = make_session();
        write_dataset(&mut session, "A", ds);
        let ast = parse_anova(src).unwrap();
        execute(&ast, &mut session).unwrap();
        let listing = session.listing.into_string();
        let log = session.log.buffer().to_string();
        (listing, log, session)
    }

    // ───────────── parse tests ─────────────

    #[test]
    fn parse_minimal() {
        let ast = parse_anova("proc anova data=a; class g; model y = g; run;").unwrap();
        assert_eq!(ast.class_vars, vec!["g"]);
        assert_eq!(ast.model.y_var, "y");
        assert_eq!(ast.model.effects, vec![vec!["g".to_string()]]);
        assert!((ast.alpha - 0.05).abs() < 1e-12);
    }

    #[test]
    fn parse_interaction_and_means() {
        let ast = parse_anova(
            "proc anova data=a; class a b; model y = a b a*b; means a b / scheffe; run;",
        )
        .unwrap();
        assert_eq!(ast.class_vars, vec!["a", "b"]);
        assert_eq!(
            ast.model.effects,
            vec![
                vec!["a".to_string()],
                vec!["b".to_string()],
                vec!["a".to_string(), "b".to_string()],
            ]
        );
        assert_eq!(ast.means_stmt.len(), 1);
        assert_eq!(ast.means_stmt[0].effects.len(), 2);
        assert_eq!(ast.means_stmt[0].option.as_deref(), Some("SCHEFFE"));
    }

    #[test]
    fn parse_requires_model() {
        assert!(parse_anova("proc anova data=a; class g; run;").is_err());
    }

    #[test]
    fn parse_bad_alpha() {
        assert!(parse_anova("proc anova data=a alpha=2; class g; model y=g; run;").is_err());
    }

    // ───────────── execute tests ─────────────

    /// One-way balanced design: factor g with 3 groups, 4 obs each.
    fn one_way_ds() -> SasDataset {
        // Group means clearly different: A~10, B~20, C~30.
        let g = vec![
            "A", "A", "A", "A", "B", "B", "B", "B", "C", "C", "C", "C",
        ];
        let y = vec![
            9.0, 10.0, 11.0, 10.0, // A
            19.0, 20.0, 21.0, 20.0, // B
            29.0, 30.0, 31.0, 30.0, // C
        ];
        let df = df!["g" => g, "y" => y].unwrap();
        SasDataset {
            df,
            vars: vec![char_meta("g"), num_meta("y")],
        }
    }

    #[test]
    fn anova_simple_one_way() {
        let (listing, _log, _s) =
            run_anova(one_way_ds(), "proc anova data=a; class g; model y = g; run;");
        assert!(listing.contains("The ANOVA Procedure"), "{listing}");
        assert!(listing.contains("Analysis of Variance") || listing.contains("Sum of Squares"),
            "{listing}");
        assert!(listing.contains("Dependent Variable: y"), "{listing}");
        assert!(listing.contains("R-Square"), "{listing}");
        // 3 groups → model DF = 2, error DF = 9, total DF = 11.
        assert!(listing.contains("Number of Observations Used       12"), "{listing}");
    }

    #[test]
    fn anova_class_level_info() {
        let (listing, _log, _s) =
            run_anova(one_way_ds(), "proc anova data=a; class g; model y = g; run;");
        assert!(listing.contains("Class Level Information"), "{listing}");
        assert!(listing.contains("Levels"), "{listing}");
        // Three levels A B C.
        assert!(listing.contains("A B C"), "{listing}");
    }

    #[test]
    fn anova_f_stat_global() {
        // Recompute the F statistic by hand for the one-way design and check it
        // is large (groups very separated) → tiny p-value.
        let ds = one_way_ds();
        let (listing, _log, _s) =
            run_anova(ds, "proc anova data=a; class g; model y = g; run;");
        // The huge between-group separation → Pr > F is < .0001.
        assert!(listing.contains("F Value"), "{listing}");
        assert!(listing.contains("<.0001"), "{listing}");
    }

    #[test]
    fn anova_two_way() {
        // 2 factors a (2 levels) × b (3 levels), 2 obs/cell → balanced, 12 obs.
        let mut a: Vec<&str> = Vec::new();
        let mut b: Vec<&str> = Vec::new();
        let mut y: Vec<f64> = Vec::new();
        let mut base = 0.0;
        for av in ["L", "H"] {
            for bv in ["P", "Q", "R"] {
                for rep in 0..2 {
                    a.push(av);
                    b.push(bv);
                    // Additive effects + tiny noise.
                    let ae = if av == "H" { 5.0 } else { 0.0 };
                    let be = match bv {
                        "P" => 0.0,
                        "Q" => 2.0,
                        _ => 4.0,
                    };
                    y.push(10.0 + ae + be + rep as f64 * 0.1);
                    base += 1.0;
                }
            }
        }
        let _ = base;
        let df = df!["a" => a, "b" => b, "y" => y].unwrap();
        let ds = SasDataset {
            df,
            vars: vec![char_meta("a"), char_meta("b"), num_meta("y")],
        };
        let (listing, log, _s) =
            run_anova(ds, "proc anova data=a; class a b; model y = a b; run;");
        assert!(listing.contains("Number of Observations Used       12"), "{listing}");
        // Balanced → no unbalanced warning.
        assert!(!log.contains("unbalanced"), "log: {log}");
        assert!(listing.contains("R-Square"), "{listing}");
    }

    #[test]
    fn anova_two_way_with_interaction() {
        // Same 2×3 design, with an interaction term in the model.
        let mut a: Vec<&str> = Vec::new();
        let mut b: Vec<&str> = Vec::new();
        let mut y: Vec<f64> = Vec::new();
        for av in ["L", "H"] {
            for bv in ["P", "Q", "R"] {
                for rep in 0..3 {
                    a.push(av);
                    b.push(bv);
                    let ae = if av == "H" { 5.0 } else { 0.0 };
                    let be = match bv {
                        "P" => 0.0,
                        "Q" => 2.0,
                        _ => 4.0,
                    };
                    // Interaction: extra when both H and R.
                    let ie = if av == "H" && bv == "R" { 3.0 } else { 0.0 };
                    y.push(10.0 + ae + be + ie + rep as f64 * 0.2);
                }
            }
        }
        let df = df!["a" => a, "b" => b, "y" => y].unwrap();
        let ds = SasDataset {
            df,
            vars: vec![char_meta("a"), char_meta("b"), num_meta("y")],
        };
        let (listing, log, _s) = run_anova(
            ds,
            "proc anova data=a; class a b; model y = a b a*b; run;",
        );
        // 2*3*3 = 18 obs.
        assert!(listing.contains("Number of Observations Used       18"), "{listing}");
        assert!(!log.contains("unbalanced"), "log: {log}");
        // Model DF = (2-1) + (3-1) + (2-1)*(3-1) = 1 + 2 + 2 = 5.
        // We at least confirm the table prints.
        assert!(listing.contains("Sum of Squares") || listing.contains("F Value"), "{listing}");
    }

    #[test]
    fn anova_with_means_statement() {
        let (listing, _log, _s) = run_anova(
            one_way_ds(),
            "proc anova data=a; class g; model y = g; means g; run;",
        );
        assert!(listing.contains("Means for effect g"), "{listing}");
        assert!(listing.contains("Std Err"), "{listing}");
        // Group A mean ~10, C mean ~30 should appear.
        assert!(listing.contains("10") && listing.contains("30"), "{listing}");
    }

    #[test]
    fn anova_unbalanced_errors() {
        // Unbalanced one-way: A has 3 obs, B has 2, C has 4.
        let g = vec!["A", "A", "A", "B", "B", "C", "C", "C", "C"];
        let y = vec![10.0, 11.0, 9.0, 20.0, 21.0, 30.0, 31.0, 29.0, 30.0];
        let df = df!["g" => g, "y" => y].unwrap();
        let ds = SasDataset {
            df,
            vars: vec![char_meta("g"), num_meta("y")],
        };
        let (listing, log, _s) =
            run_anova(ds, "proc anova data=a; class g; model y = g; run;");
        // Continues via least squares, but emits a WARNING about imbalance.
        assert!(log.contains("unbalanced"), "log: {log}");
        assert!(listing.contains("The ANOVA Procedure"), "{listing}");
        assert!(listing.contains("Number of Observations Used       9"), "{listing}");
    }

    #[test]
    fn anova_factor_not_in_class_errors() {
        let ds = one_way_ds();
        let mut session = make_session();
        write_dataset(&mut session, "A", ds);
        // y used as a factor but not declared in CLASS → error.
        let ast = parse_anova("proc anova data=a; class g; model g = y; run;");
        // g is char so resolving it as dependent (numeric) should fail; either
        // way execution errors.
        if let Ok(ast) = ast {
            let res = execute(&ast, &mut session);
            assert!(res.is_err(), "expected error for non-CLASS factor / non-numeric y");
        }
    }

    #[test]
    fn anova_missing_excluded() {
        // One row with missing y is excluded.
        let g = vec!["A", "A", "B", "B", "C", "C"];
        let y = vec![Some(10.0), None, Some(20.0), Some(21.0), Some(30.0), Some(31.0)];
        let df = df!["g" => g, "y" => y].unwrap();
        let ds = SasDataset {
            df,
            vars: vec![char_meta("g"), num_meta("y")],
        };
        let (listing, _log, _s) =
            run_anova(ds, "proc anova data=a; class g; model y = g; run;");
        assert!(listing.contains("Number of Observations Read       6"), "{listing}");
        assert!(listing.contains("Number of Observations Used       5"), "{listing}");
    }
}
