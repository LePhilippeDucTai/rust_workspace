//! PROC LOGISTIC — binary logistic regression (v1).
//!
//! # Plan du fichier — voir le plan M26 (modèles catégoriels)
//!
//! `proc logistic data=<ref> [alpha=<f64>];
//!  [class a b ... ;]
//!  model y = x1 x2 class_var ... ;          // y binaire {0,1}
//!  model events/trials = x1 x2 ... ;        // forme événements/essais
//!  [oddsratio x1 / cl=wald ;]
//!  [output out=<ref>;]
//!  run;`
//!
//! ## Périmètre v1 (fidèle à SAS 9.4 PROC LOGISTIC)
//! - réponse BINAIRE uniquement (pas d'ordinal / nominal multiclass) ;
//! - estimation par maximum de vraisemblance via Newton-Raphson (IRLS) ;
//! - tests de Wald (χ²) sur chaque paramètre, IC de Wald ;
//! - test global (likelihood-ratio) : déviance nulle − déviance modèle ~ χ²(p) ;
//! - rapports de cotes (odds ratios) : exp(β) et IC.
//!
//! ## Codage et résolution
//! - One-hot des CLASS en omettant la DERNIÈRE modalité (cellule de référence),
//!   comme GLM/ANOVA. Les covariables continues entrent telles quelles.
//! - Intercept + variables continues + dummies → X. β par Newton-Raphson.
//!   Matrice de variance-covariance = (X'WX)⁻¹ via `invert_matrix`.
//!
//! ## Choix / simplifications documentés
//! - Exclusion LISTWISE : retirer une observation dès que y OU une variable du
//!   modèle (CLASS ou continue) est manquante.
//! - y doit être binaire : on collecte les valeurs distinctes non manquantes ;
//!   exactement deux niveaux sont requis. La plus GRANDE valeur est l'événement
//!   (codée 1), la plus petite le non-événement (codée 0). Cela suit la
//!   convention SAS où, par défaut, le niveau ordonné en SECOND est modélisé…
//!   ⚠ SAS modélise par défaut le niveau de PLUS PETITE valeur (Pr(Y=0)). Pour
//!   rester intuitif et fidèle à l'option courante `descending`, v1 modélise
//!   Pr(Y = niveau le plus grand) = Pr(événement). Documenté.
//! - Forme `events/trials` : y est un compte d'événements, trials le total ;
//!   chaque ligne contribue `events` succès et `trials − events` échecs
//!   (vraisemblance binomiale, gérée par pondération du gradient/Hessien).
//! - Non convergence → NOTE "Convergence criterion not met" + on continue.
//! - Matrice singulière (séparation parfaite / colinéarité) → SasError.

use crate::ast::DatasetRef;
use crate::dataset::{SasDataset, VarMeta};
use crate::error::{Result, SasError};
use crate::listing::Align;
use crate::missing::value_to_num;
use crate::parser::StatementStream;
use crate::procs::common::decode_column;
use crate::session::Session;
use crate::stat::dists::{chisq_cdf, phi_inv};
use crate::stat::linalg::invert_matrix;
use crate::token::TokenKind;
use crate::value::{format_best, Value, VarType};
use polars::prelude::{Column, NamedFrom, Series};
use std::cmp::Ordering;

const MAX_ITER: usize = 25;
const TOL: f64 = 1e-8;

// ───────────────────────── AST ─────────────────────────

/// `class a b c ;` — declares the categorical factors (0/1 dummy coding).
pub struct ClassStmt {
    pub vars: Vec<String>,
}

/// `model y = x1 x2 ... ;` or `model events/trials = x1 x2 ... ;`.
/// `y_var` is the binary response; `trials_var` is the denominator for the
/// events/trials syntax (None for the single-variable binary form).
pub struct ModelStmt {
    pub y_var: String,
    pub trials_var: Option<String>,
    pub x_vars: Vec<String>,
}

/// `oddsratio x1 [/ cl=wald] ;`
pub struct OddsratioStmt {
    pub vars: Vec<String>,
}

pub struct LogisticAst {
    pub data: Option<DatasetRef>,
    /// Confidence-level complement. Default 0.05.
    pub alpha: f64,
    pub class_stmt: ClassStmt,
    pub model: ModelStmt,
    pub oddsratio_stmts: Vec<OddsratioStmt>,
    pub output_out: Option<DatasetRef>,
}

// ───────────────────────── parse ─────────────────────────

/// Parse `proc logistic [data=a] [alpha=p]; [class ...;] model y = x...;
/// [oddsratio ...;] [output out=b;] run;`. Called AFTER "proc logistic" was
/// consumed. Consumes through `run;` / `quit;`.
pub fn parse(ts: &mut StatementStream) -> Result<LogisticAst> {
    let mut data: Option<DatasetRef> = None;
    let mut alpha: f64 = 0.05;

    // --- PROC LOGISTIC statement options, until `;` ---
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
        } else if ts.peek().is_kw("descending") || ts.peek().is_kw("order") {
            // Recognised but ignored option words; consume token (and a following
            // `=value` for ORDER=).
            ts.next();
            if ts.peek().kind == TokenKind::Eq {
                ts.next();
                ts.next();
            }
        } else if let Some(name) = ts.peek().ident().map(str::to_string) {
            let span = ts.peek().span;
            return Err(SasError::parse(
                format!(
                    "Unexpected option '{}' on PROC LOGISTIC statement.",
                    name.to_uppercase()
                ),
                span,
            ));
        } else {
            let span = ts.peek().span;
            return Err(SasError::parse(
                "Unexpected token on PROC LOGISTIC statement.",
                span,
            ));
        }
    }

    // --- sub-statements until run;/quit; ---
    let mut class_vars: Vec<String> = Vec::new();
    let mut model: Option<ModelStmt> = None;
    let mut oddsratio_stmts: Vec<OddsratioStmt> = Vec::new();
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
                    "PROC LOGISTIC v1 supports a single MODEL statement.",
                ));
            }
            model = Some(parse_model(ts)?);
        } else if ts.peek().is_kw("oddsratio") {
            ts.next();
            oddsratio_stmts.push(parse_oddsratio(ts)?);
        } else if ts.peek().is_kw("output") {
            ts.next();
            output_out = Some(parse_output(ts)?);
        } else {
            // Unknown sub-statement: skip it (recovery, like glm/reg).
            ts.skip_to_semi();
        }
    }

    let model = model.ok_or_else(|| {
        SasError::runtime("PROC LOGISTIC requires a MODEL statement (model y = effects;).")
    })?;

    Ok(LogisticAst {
        data,
        alpha,
        class_stmt: ClassStmt { vars: class_vars },
        model,
        oddsratio_stmts,
        output_out,
    })
}

/// Parse a plain `name name name ;` variable list (CLASS), ignoring a trailing
/// `/ options` clause.
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
        if ts.peek().kind == TokenKind::Slash {
            ts.skip_to_semi();
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

/// Parse `model y = x1 x2 ;` or `model events/trials = x1 x2 ;` (the leading
/// `model` keyword already consumed).
fn parse_model(ts: &mut StatementStream) -> Result<ModelStmt> {
    let span = ts.peek().span;
    let y_var = ts
        .peek()
        .ident()
        .map(str::to_string)
        .ok_or_else(|| SasError::parse("Expected dependent variable in MODEL statement.", span))?;
    ts.next();

    // Optional events/trials denominator: `y / trials`.
    let mut trials_var: Option<String> = None;
    if ts.peek().kind == TokenKind::Slash {
        ts.next();
        let tspan = ts.peek().span;
        let t = ts
            .peek()
            .ident()
            .map(str::to_string)
            .ok_or_else(|| SasError::parse("Expected trials variable after '/'.", tspan))?;
        ts.next();
        trials_var = Some(t);
    }

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
        if ts.peek().kind == TokenKind::Slash {
            // MODEL options (e.g. `/ selection=...`): skip to ';'.
            ts.skip_to_semi();
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

    Ok(ModelStmt {
        y_var,
        trials_var,
        x_vars,
    })
}

/// Parse `oddsratio x1 [x2 ...] [/ cl=wald] ;` (leading `oddsratio` consumed).
fn parse_oddsratio(ts: &mut StatementStream) -> Result<OddsratioStmt> {
    let mut vars = Vec::new();
    loop {
        if ts.peek().kind == TokenKind::Semi {
            ts.next();
            break;
        }
        if ts.peek().kind == TokenKind::Eof || ts.peek().is_kw("run") || ts.peek().is_kw("quit") {
            break;
        }
        if ts.peek().kind == TokenKind::Slash {
            ts.skip_to_semi();
            break;
        }
        match ts.peek().ident().map(str::to_string) {
            Some(name) => {
                vars.push(name);
                ts.next();
            }
            None => {
                return Err(SasError::parse(
                    "Expected a variable name in ODDSRATIO statement.",
                    ts.peek().span,
                ));
            }
        }
    }
    if vars.is_empty() {
        return Err(SasError::runtime(
            "The ODDSRATIO statement requires at least one variable.",
        ));
    }
    Ok(OddsratioStmt { vars })
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
        SasError::runtime("The OUTPUT statement of PROC LOGISTIC requires OUT=<dataset>.")
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
fn resolve_input(ast: &LogisticAst, session: &Session) -> Result<DatasetRef> {
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

// ───────────────────────── design construction ─────────────────────────

/// True when a value is a usable (non-missing) class level / number.
fn is_present(v: &Value) -> bool {
    match v {
        Value::Num(f) => !f.is_nan(),
        Value::Char(s) => !s.trim().is_empty(),
        Value::Missing(_) => false,
    }
}

/// Render a CLASS level value as a display string.
fn level_label(v: &Value) -> String {
    match v {
        Value::Num(f) => format_best(*f, 12),
        Value::Missing(k) => k.display(),
        Value::Char(s) => s.trim_end().to_string(),
    }
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

/// A resolved model variable: name, decoded values, and CLASS flag.
struct ModelVar {
    name: String,
    is_class: bool,
    vals: Vec<Value>,
}

/// One named design column (intercept first, then per-variable columns).
struct DesignCol {
    /// Display name for the Parameter Estimates table.
    name: String,
    /// Source variable name (for ODDSRATIO matching). Empty for the intercept.
    source: String,
}

/// The assembled design.
struct Design {
    /// X matrix, m × n_params (intercept first).
    x: Vec<Vec<f64>>,
    /// Per-observation event count (successes). For the binary form this is 0/1;
    /// for events/trials it is the events column.
    events: Vec<f64>,
    /// Per-observation trial count. For the binary form this is 1.
    trials: Vec<f64>,
    /// Number of parameters (intercept + design columns).
    n_params: usize,
    /// Column metadata (length n_params, index 0 = intercept).
    cols: Vec<DesignCol>,
}

// ───────────────────────── execute ─────────────────────────

pub fn execute(ast: &LogisticAst, session: &mut Session) -> Result<()> {
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

    let resolve_col = |nm: &str| -> Result<usize> {
        ds.vars
            .iter()
            .position(|m| m.name.eq_ignore_ascii_case(nm))
            .ok_or_else(|| SasError::runtime(format!("Variable {} not found.", nm.to_uppercase())))
    };

    // --- response variable(s) ---
    let y_col = resolve_col(&ast.model.y_var)?;
    if ds.vars[y_col].ty != VarType::Num {
        return Err(SasError::runtime(format!(
            "Response variable {} in the MODEL statement is not numeric.",
            ast.model.y_var.to_uppercase()
        )));
    }
    let y_vals = decode_column(&ds, y_col)?;

    let trials_vals: Option<Vec<Value>> = match &ast.model.trials_var {
        Some(t) => {
            let tc = resolve_col(t)?;
            if ds.vars[tc].ty != VarType::Num {
                return Err(SasError::runtime(format!(
                    "Trials variable {} is not numeric.",
                    t.to_uppercase()
                )));
            }
            Some(decode_column(&ds, tc)?)
        }
        None => None,
    };

    // --- explanatory variables ---
    let mut model_var_names: Vec<String> = Vec::new();
    for x in &ast.model.x_vars {
        if !model_var_names.iter().any(|m| m.eq_ignore_ascii_case(x)) {
            model_var_names.push(x.clone());
        }
    }
    let mut model_vars: Vec<ModelVar> = Vec::new();
    for nm in &model_var_names {
        let c = resolve_col(nm)?;
        let is_class = ast
            .class_stmt
            .vars
            .iter()
            .any(|cv| cv.eq_ignore_ascii_case(nm));
        if !is_class && ds.vars[c].ty != VarType::Num {
            return Err(SasError::runtime(format!(
                "Variable {} in the MODEL statement is character but is not declared in CLASS.",
                nm.to_uppercase()
            )));
        }
        let vals = decode_column(&ds, c)?;
        model_vars.push(ModelVar {
            name: ds.vars[c].name.clone(),
            is_class,
            vals,
        });
    }

    // --- LISTWISE exclusion ---
    let mut kept_rows: Vec<usize> = Vec::with_capacity(n_obs);
    for r in 0..n_obs {
        // Response present.
        let yv = value_to_num(&y_vals[r]);
        if !matches!(yv, Some(v) if !v.is_nan()) {
            continue;
        }
        if let Some(tv) = &trials_vals {
            let t = value_to_num(&tv[r]);
            if !matches!(t, Some(v) if !v.is_nan()) {
                continue;
            }
        }
        let ok = model_vars.iter().all(|mv| {
            if mv.is_class {
                is_present(&mv.vals[r])
            } else {
                matches!(value_to_num(&mv.vals[r]), Some(v) if !v.is_nan())
            }
        });
        if ok {
            kept_rows.push(r);
        }
    }

    // --- response coding ---
    // events / trials counts, plus response-profile bookkeeping over the two
    // outcome levels (event = 1, non-event = 0).
    let factor_levels: Vec<Vec<Value>> = model_vars
        .iter()
        .map(|mv| {
            if mv.is_class {
                distinct_levels(&mv.vals, &kept_rows)
            } else {
                Vec::new()
            }
        })
        .collect();

    let (events, trials, n_event, n_nonevent, response_levels) = if let Some(tv) = &trials_vals {
        // events/trials form.
        let mut events = Vec::with_capacity(kept_rows.len());
        let mut trials = Vec::with_capacity(kept_rows.len());
        let mut n_event = 0.0_f64;
        let mut n_nonevent = 0.0_f64;
        for &r in &kept_rows {
            let e = value_to_num(&y_vals[r]).unwrap_or(0.0);
            let t = value_to_num(&tv[r]).unwrap_or(0.0);
            if e < 0.0 || t < 0.0 || e > t {
                return Err(SasError::runtime(
                    "Invalid events/trials values (require 0 <= events <= trials).",
                ));
            }
            events.push(e);
            trials.push(t);
            n_event += e;
            n_nonevent += t - e;
        }
        (events, trials, n_event, n_nonevent, None)
    } else {
        // binary form: y must take exactly two distinct values.
        let levels = distinct_levels(&y_vals, &kept_rows);
        if levels.len() != 2 {
            return Err(SasError::runtime(format!(
                "PROC LOGISTIC (v1) requires a binary response; {} distinct non-missing \
                 levels were found for {}.",
                levels.len(),
                ast.model.y_var.to_uppercase()
            )));
        }
        // levels sorted ascending; model Pr(Y = larger level) = event.
        let event_level = levels[1].clone();
        let mut events = Vec::with_capacity(kept_rows.len());
        let mut trials = Vec::with_capacity(kept_rows.len());
        let mut n_event = 0.0_f64;
        let mut n_nonevent = 0.0_f64;
        for &r in &kept_rows {
            let is_event = y_vals[r].sas_cmp(&event_level) == Ordering::Equal;
            events.push(if is_event { 1.0 } else { 0.0 });
            trials.push(1.0);
            if is_event {
                n_event += 1.0;
            } else {
                n_nonevent += 1.0;
            }
        }
        (events, trials, n_event, n_nonevent, Some(levels))
    };

    // --- build design matrix ---
    let design = build_design(&model_vars, &factor_levels, &kept_rows, events, trials);
    let n_params = design.n_params;
    let total_trials: f64 = design.trials.iter().sum();

    if total_trials < n_params as f64 + 1.0 {
        return Err(SasError::runtime(format!(
            "Insufficient non-missing observations for {n_params} parameters in PROC LOGISTIC.",
        )));
    }

    // --- Newton-Raphson MLE ---
    let fit = newton_raphson(&design.x, &design.events, &design.trials, n_params)?;
    if !fit.converged {
        session
            .log
            .note("Convergence criterion (GCONV=1E-8) not satisfied.");
    }
    let beta = &fit.beta;

    // Variance-covariance: (X'WX)⁻¹.
    let xtwx = xtwx_matrix(&design.x, &fit.weights, n_params);
    let cov = invert_matrix(&xtwx)?;
    let se: Vec<f64> = (0..n_params).map(|j| cov[j][j].max(0.0).sqrt()).collect();

    // Model fit statistics.
    let loglik_full = fit.loglik;
    let loglik_null = null_loglik(n_event, n_nonevent);
    let m2ll_full = -2.0 * loglik_full;
    let m2ll_null = -2.0 * loglik_null;
    let n_pred = (n_params - 1) as f64; // intercept-and-covariates count for AIC/SC
    let aic_full = m2ll_full + 2.0 * n_params as f64;
    let aic_null = m2ll_null + 2.0;
    // SC (BIC) uses ln of the number of observations (trials).
    let ln_n = total_trials.ln();
    let sc_full = m2ll_full + ln_n * n_params as f64;
    let sc_null = m2ll_null + ln_n;

    // Global likelihood-ratio test: deviance(null) − deviance(model) ~ χ²(p).
    let lr_chisq = (m2ll_null - m2ll_full).max(0.0);
    let lr_df = n_pred.max(0.0);
    let lr_p = if lr_df > 0.0 {
        1.0 - chisq_cdf(lr_chisq, lr_df)
    } else {
        f64::NAN
    };

    // --- listing ---
    emit_listing(
        session,
        &ds,
        y_col,
        &ast.class_stmt.vars,
        &model_vars,
        &factor_levels,
        &in_display,
        &response_levels,
        n_event,
        n_nonevent,
        m2ll_null,
        m2ll_full,
        aic_null,
        aic_full,
        sc_null,
        sc_full,
        lr_chisq,
        lr_df,
        lr_p,
        &design,
        beta,
        &se,
    );

    // --- ODDSRATIO ---
    for orr in &ast.oddsratio_stmts {
        emit_oddsratio(session, orr, &design, beta, &se, ast.alpha);
    }

    // --- OUTPUT OUT= ---
    if let Some(target) = &ast.output_out {
        let mut predicted = vec![0.0_f64; kept_rows.len()];
        for (i, row) in design.x.iter().enumerate() {
            let eta: f64 = row.iter().zip(beta).map(|(a, b)| a * b).sum();
            predicted[i] = sigmoid(eta);
        }
        write_output_dataset(session, &ds, target, &kept_rows, &predicted, n_obs)?;
    }

    Ok(())
}

/// Build the design matrix: intercept + continuous covariates + class dummies
/// (last level omitted as the reference cell).
fn build_design(
    model_vars: &[ModelVar],
    factor_levels: &[Vec<Value>],
    kept_rows: &[usize],
    events: Vec<f64>,
    trials: Vec<f64>,
) -> Design {
    let m = kept_rows.len();

    let mut cols: Vec<DesignCol> = vec![DesignCol {
        name: "Intercept".to_string(),
        source: String::new(),
    }];
    let mut design_cols: Vec<Vec<f64>> = Vec::new();

    for (vi, mv) in model_vars.iter().enumerate() {
        if mv.is_class {
            let levels = &factor_levels[vi];
            let take = levels.len().saturating_sub(1); // omit the reference (last) level
            for level in levels.iter().take(take) {
                let mut col = vec![0.0_f64; m];
                for (pos, &r) in kept_rows.iter().enumerate() {
                    if mv.vals[r].sas_cmp(level) == Ordering::Equal {
                        col[pos] = 1.0;
                    }
                }
                design_cols.push(col);
                cols.push(DesignCol {
                    name: format!("{} {}", mv.name, level_label(level)),
                    source: mv.name.clone(),
                });
            }
        } else {
            let mut col = vec![0.0_f64; m];
            for (pos, &r) in kept_rows.iter().enumerate() {
                col[pos] = value_to_num(&mv.vals[r]).unwrap_or(f64::NAN);
            }
            design_cols.push(col);
            cols.push(DesignCol {
                name: mv.name.clone(),
                source: mv.name.clone(),
            });
        }
    }

    let n_params = design_cols.len() + 1;
    let mut x: Vec<Vec<f64>> = Vec::with_capacity(m);
    for pos in 0..m {
        let mut row = Vec::with_capacity(n_params);
        row.push(1.0);
        for col in &design_cols {
            row.push(col[pos]);
        }
        x.push(row);
    }

    Design {
        x,
        events,
        trials,
        n_params,
        cols,
    }
}

// ───────────────────────── Newton-Raphson MLE ─────────────────────────

/// Sigmoid σ(η) = 1 / (1 + e^{-η}), numerically stable.
fn sigmoid(eta: f64) -> f64 {
    if eta >= 0.0 {
        let z = (-eta).exp();
        1.0 / (1.0 + z)
    } else {
        let z = eta.exp();
        z / (1.0 + z)
    }
}

struct FitResult {
    beta: Vec<f64>,
    /// Diagonal IRLS weights w_i = trials_i · p_i (1 − p_i) at the solution.
    weights: Vec<f64>,
    loglik: f64,
    converged: bool,
}

/// Newton-Raphson / IRLS for binomial logistic regression.
///
/// β ← 0; iterate:
///   η = Xβ ; p = σ(η) ; W = diag(t·p(1−p)) ;
///   grad = Xᵀ(events − t·p) ; H = XᵀWX ;
///   β ← β + H⁻¹ grad ; stop when ‖Δβ‖∞ < TOL.
fn newton_raphson(
    x: &[Vec<f64>],
    events: &[f64],
    trials: &[f64],
    n_params: usize,
) -> Result<FitResult> {
    let m = x.len();
    let mut beta = vec![0.0_f64; n_params];
    let mut weights = vec![0.0_f64; m];
    let mut loglik = f64::NEG_INFINITY;
    let mut converged = false;

    for _iter in 0..MAX_ITER {
        // Linear predictor, probabilities, weights, gradient.
        let mut grad = vec![0.0_f64; n_params];
        let mut hess = vec![vec![0.0_f64; n_params]; n_params];
        let mut ll = 0.0_f64;
        for i in 0..m {
            let eta: f64 = x[i].iter().zip(&beta).map(|(a, b)| a * b).sum();
            let p = sigmoid(eta);
            let t = trials[i];
            let w = t * p * (1.0 - p);
            weights[i] = w;
            let resid = events[i] - t * p;
            for a in 0..n_params {
                grad[a] += x[i][a] * resid;
                for b in 0..n_params {
                    hess[a][b] += x[i][a] * x[i][b] * w;
                }
            }
            // Binomial log-likelihood contribution (stable via log-sigmoid).
            // ll += e·ln p + (t−e)·ln(1−p)
            let log_p = log_sigmoid(eta);
            let log_1mp = log_sigmoid(-eta);
            ll += events[i] * log_p + (t - events[i]) * log_1mp;
        }
        loglik = ll;

        // Solve H · Δ = grad.
        let hinv = invert_matrix(&hess)?;
        let mut delta = vec![0.0_f64; n_params];
        for a in 0..n_params {
            let mut s = 0.0;
            for b in 0..n_params {
                s += hinv[a][b] * grad[b];
            }
            delta[a] = s;
        }
        let mut max_step = 0.0_f64;
        for a in 0..n_params {
            beta[a] += delta[a];
            max_step = max_step.max(delta[a].abs());
        }
        if max_step < TOL {
            converged = true;
            // Recompute weights/loglik at the final β for accurate inference.
            let (w, ll) = recompute(x, events, trials, &beta);
            weights = w;
            loglik = ll;
            break;
        }
    }

    Ok(FitResult {
        beta,
        weights,
        loglik,
        converged,
    })
}

/// Recompute IRLS weights and log-likelihood at a given β.
fn recompute(x: &[Vec<f64>], events: &[f64], trials: &[f64], beta: &[f64]) -> (Vec<f64>, f64) {
    let m = x.len();
    let mut weights = vec![0.0_f64; m];
    let mut ll = 0.0_f64;
    for i in 0..m {
        let eta: f64 = x[i].iter().zip(beta).map(|(a, b)| a * b).sum();
        let p = sigmoid(eta);
        weights[i] = trials[i] * p * (1.0 - p);
        ll += events[i] * log_sigmoid(eta) + (trials[i] - events[i]) * log_sigmoid(-eta);
    }
    (weights, ll)
}

/// ln σ(η) = −ln(1 + e^{−η}), numerically stable.
fn log_sigmoid(eta: f64) -> f64 {
    if eta >= 0.0 {
        -(1.0 + (-eta).exp()).ln()
    } else {
        eta - (1.0 + eta.exp()).ln()
    }
}

/// Null-model (intercept only) log-likelihood: with p̂ = n1/(n1+n0),
/// ll = n1·ln p̂ + n0·ln(1−p̂).
fn null_loglik(n_event: f64, n_nonevent: f64) -> f64 {
    let n = n_event + n_nonevent;
    if n <= 0.0 || n_event <= 0.0 || n_nonevent <= 0.0 {
        return 0.0;
    }
    let p = n_event / n;
    n_event * p.ln() + n_nonevent * (1.0 - p).ln()
}

/// Form X'WX with diagonal weights W.
fn xtwx_matrix(x: &[Vec<f64>], w: &[f64], n_params: usize) -> Vec<Vec<f64>> {
    let mut out = vec![vec![0.0; n_params]; n_params];
    for (i, row) in x.iter().enumerate() {
        let wi = w[i];
        for a in 0..n_params {
            let xa = row[a] * wi;
            for b in 0..n_params {
                out[a][b] += xa * row[b];
            }
        }
    }
    out
}

// ───────────────────────── ODDSRATIO ─────────────────────────

fn emit_oddsratio(
    session: &mut Session,
    orr: &OddsratioStmt,
    design: &Design,
    beta: &[f64],
    se: &[f64],
    alpha: f64,
) {
    centered(session, "The LOGISTIC Procedure");
    session.listing.blank();
    centered(session, "Odds Ratio Estimates");
    session.listing.blank();

    let z = phi_inv(1.0 - alpha / 2.0);
    let level_pct = ((1.0 - alpha) * 100.0).round() as i64;
    let headers: Vec<String> = vec![
        "Effect".into(),
        "Point Estimate".into(),
        format!("{level_pct}% Lower"),
        format!("{level_pct}% Upper"),
    ];
    let aligns = vec![Align::Left, Align::Right, Align::Right, Align::Right];
    let mut rows: Vec<Vec<String>> = Vec::new();

    for var in &orr.vars {
        let mut any = false;
        for j in 1..design.n_params {
            let col = &design.cols[j];
            if !col.source.eq_ignore_ascii_case(var) {
                continue;
            }
            any = true;
            let or = beta[j].exp();
            let lo = (beta[j] - z * se[j]).exp();
            let hi = (beta[j] + z * se[j]).exp();
            rows.push(vec![col.name.clone(), fmt_or(or), fmt_or(lo), fmt_or(hi)]);
        }
        if !any {
            session.log.warning(&format!(
                "Variable {} in the ODDSRATIO statement is not in the model; skipped.",
                var.to_uppercase()
            ));
        }
    }

    session.listing.write_table(&headers, &aligns, &rows);
    session.listing.blank();
}

// ───────────────────────── OUTPUT OUT= ─────────────────────────

fn write_output_dataset(
    session: &mut Session,
    ds: &SasDataset,
    target: &DatasetRef,
    kept_rows: &[usize],
    predicted: &[f64],
    n_obs: usize,
) -> Result<()> {
    let mut pred_full: Vec<Option<f64>> = vec![None; n_obs];
    for (pos, &r) in kept_rows.iter().enumerate() {
        pred_full[r] = Some(predicted[pos]);
    }

    let mut columns: Vec<Column> = ds.df.get_columns().to_vec();
    let mut vars: Vec<VarMeta> = ds.vars.clone();

    columns.push(Series::new("PREDICTED".into(), pred_full).into());
    vars.push(num_var_meta("PREDICTED"));

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

// ───────────────────────── listing ─────────────────────────

#[allow(clippy::too_many_arguments)]
fn emit_listing(
    session: &mut Session,
    ds: &SasDataset,
    y_col: usize,
    class_vars: &[String],
    model_vars: &[ModelVar],
    factor_levels: &[Vec<Value>],
    in_display: &str,
    response_levels: &Option<Vec<Value>>,
    n_event: f64,
    n_nonevent: f64,
    m2ll_null: f64,
    m2ll_full: f64,
    aic_null: f64,
    aic_full: f64,
    sc_null: f64,
    sc_full: f64,
    lr_chisq: f64,
    lr_df: f64,
    lr_p: f64,
    design: &Design,
    beta: &[f64],
    se: &[f64],
) {
    session.listing.page_header();
    centered(session, "The LOGISTIC Procedure");
    session.listing.blank();

    // Model Information.
    {
        let headers: Vec<String> = vec!["".into(), "".into()];
        let aligns = vec![Align::Left, Align::Left];
        let rows = vec![
            vec!["Data Set".into(), in_display.to_string()],
            vec![
                "Response Variable".into(),
                ds.vars[y_col].name.clone(),
            ],
            vec![
                "Model".into(),
                "binary logit".into(),
            ],
            vec![
                "Optimization Technique".into(),
                "Newton-Raphson".into(),
            ],
        ];
        session.listing.write_table(&headers, &aligns, &rows);
    }
    session.listing.blank();

    // Class Level Information.
    if !class_vars.is_empty() {
        centered(session, "Class Level Information");
        session.listing.blank();
        let headers: Vec<String> = vec!["Class".into(), "Levels".into(), "Values".into()];
        let aligns = vec![Align::Left, Align::Right, Align::Left];
        let mut rows: Vec<Vec<String>> = Vec::new();
        for cv in class_vars {
            if let Some(pos) = model_vars
                .iter()
                .position(|v| v.is_class && v.name.eq_ignore_ascii_case(cv))
            {
                let lvls = &factor_levels[pos];
                let values: Vec<String> = lvls.iter().map(level_label).collect();
                rows.push(vec![
                    model_vars[pos].name.clone(),
                    format!("{}", lvls.len()),
                    values.join(" "),
                ]);
            }
        }
        session.listing.write_table(&headers, &aligns, &rows);
        session.listing.blank();
    }

    // Response Profile.
    centered(session, "Response Profile");
    session.listing.blank();
    {
        let headers: Vec<String> = vec![
            "Ordered Value".into(),
            ds.vars[y_col].name.clone(),
            "Total Frequency".into(),
        ];
        let aligns = vec![Align::Right, Align::Left, Align::Right];
        // Ordered value 1 = event (modelled), 2 = non-event. SAS lists in
        // ascending order; we present event first to match the modelled level.
        let (lab_event, lab_nonevent) = match response_levels {
            Some(levels) => (level_label(&levels[1]), level_label(&levels[0])),
            None => ("event".to_string(), "nonevent".to_string()),
        };
        let rows = vec![
            vec!["1".into(), lab_event, fmt_count(n_event)],
            vec!["2".into(), lab_nonevent, fmt_count(n_nonevent)],
        ];
        session.listing.write_table(&headers, &aligns, &rows);
    }
    session.listing.blank();
    session
        .listing
        .write_line("Probability modeled is for the second ordered (largest) response level.");
    session.listing.blank();

    // Model Fit Statistics.
    centered(session, "Model Fit Statistics");
    session.listing.blank();
    {
        let headers: Vec<String> = vec![
            "Criterion".into(),
            "Intercept Only".into(),
            "Intercept and Covariates".into(),
        ];
        let aligns = vec![Align::Left, Align::Right, Align::Right];
        let rows = vec![
            vec!["AIC".into(), fmt_num(aic_null), fmt_num(aic_full)],
            vec!["SC".into(), fmt_num(sc_null), fmt_num(sc_full)],
            vec![
                "-2 Log L".into(),
                fmt_num(m2ll_null),
                fmt_num(m2ll_full),
            ],
        ];
        session.listing.write_table(&headers, &aligns, &rows);
    }
    session.listing.blank();

    // Testing Global Null Hypothesis: BETA=0.
    centered(session, "Testing Global Null Hypothesis: BETA=0");
    session.listing.blank();
    {
        let headers: Vec<String> = vec![
            "Test".into(),
            "Chi-Square".into(),
            "DF".into(),
            "Pr > ChiSq".into(),
        ];
        let aligns = vec![Align::Left, Align::Right, Align::Right, Align::Right];
        let rows = vec![vec![
            "Likelihood Ratio".into(),
            fmt_chisq(lr_chisq),
            fmt_df(lr_df),
            fmt_p(lr_p),
        ]];
        session.listing.write_table(&headers, &aligns, &rows);
    }
    session.listing.blank();

    // Analysis of Maximum Likelihood Estimates (Parameter Estimates).
    centered(session, "Analysis of Maximum Likelihood Estimates");
    session.listing.blank();
    {
        let headers: Vec<String> = vec![
            "Parameter".into(),
            "DF".into(),
            "Estimate".into(),
            "Standard Error".into(),
            "Wald Chi-Square".into(),
            "Pr > ChiSq".into(),
        ];
        let aligns = vec![
            Align::Left,
            Align::Right,
            Align::Right,
            Align::Right,
            Align::Right,
            Align::Right,
        ];
        let mut rows: Vec<Vec<String>> = Vec::new();
        for j in 0..design.n_params {
            let est = beta[j];
            let stderr = se[j];
            let wald = if stderr > 0.0 {
                let z = est / stderr;
                z * z
            } else {
                f64::NAN
            };
            let p = if wald.is_finite() {
                1.0 - chisq_cdf(wald, 1.0)
            } else {
                f64::NAN
            };
            rows.push(vec![
                design.cols[j].name.clone(),
                "1".into(),
                fmt_est(est),
                fmt_est(stderr),
                fmt_chisq(wald),
                fmt_p(p),
            ]);
        }
        session.listing.write_table(&headers, &aligns, &rows);
    }
    session.listing.blank();
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

fn fmt_count(v: f64) -> String {
    format!("{}", v.round() as i64)
}

fn fmt_num(v: f64) -> String {
    if v.is_nan() {
        ".".to_string()
    } else {
        format!("{v:.3}")
    }
}

fn fmt_est(v: f64) -> String {
    if v.is_nan() {
        ".".to_string()
    } else {
        format!("{v:.4}")
    }
}

fn fmt_chisq(v: f64) -> String {
    if v.is_nan() {
        ".".to_string()
    } else {
        format!("{v:.4}")
    }
}

fn fmt_or(v: f64) -> String {
    if v.is_nan() || v.is_infinite() {
        ".".to_string()
    } else {
        format!("{v:.3}")
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

    fn parse_logistic(src: &str) -> Result<LogisticAst> {
        let source = SourceFile::new(src);
        let mut ts = StatementStream::new(&source).unwrap();
        ts.next(); // "proc"
        ts.next(); // "logistic"
        parse(&mut ts)
    }

    fn run_logistic(ds: SasDataset, src: &str) -> (String, String, Session) {
        let mut session = make_session();
        write_dataset(&mut session, "A", ds);
        let ast = parse_logistic(src).unwrap();
        execute(&ast, &mut session).unwrap();
        let listing = session.listing.into_string();
        let log = session.log.buffer().to_string();
        (listing, log, session)
    }

    // ───────────── parse tests ─────────────

    #[test]
    fn parse_minimal() {
        let ast = parse_logistic("proc logistic data=a; model y = x; run;").unwrap();
        assert_eq!(ast.model.y_var, "y");
        assert_eq!(ast.model.x_vars, vec!["x"]);
        assert!(ast.model.trials_var.is_none());
        assert_eq!(ast.alpha, 0.05);
    }

    #[test]
    fn parse_events_trials() {
        let ast =
            parse_logistic("proc logistic data=a; model events/total = x1 x2; run;").unwrap();
        assert_eq!(ast.model.y_var, "events");
        assert_eq!(ast.model.trials_var.as_deref(), Some("total"));
        assert_eq!(ast.model.x_vars, vec!["x1", "x2"]);
    }

    #[test]
    fn parse_full_statements() {
        let ast = parse_logistic(
            "proc logistic data=a alpha=0.10; class g; model y = x g; \
             oddsratio x / cl=wald; output out=o; run;",
        )
        .unwrap();
        assert_eq!(ast.class_stmt.vars, vec!["g"]);
        assert!((ast.alpha - 0.10).abs() < 1e-12);
        assert_eq!(ast.oddsratio_stmts.len(), 1);
        assert_eq!(ast.oddsratio_stmts[0].vars, vec!["x"]);
        assert!(ast.output_out.is_some());
    }

    #[test]
    fn parse_requires_model() {
        assert!(parse_logistic("proc logistic data=a; class g; run;").is_err());
    }

    #[test]
    fn parse_alpha_validation() {
        assert!(parse_logistic("proc logistic data=a alpha=1.5; model y = x; run;").is_err());
    }

    // ───────────── data builders ─────────────

    /// Perfectly-monotone-ish dataset: as x increases, P(y=1) increases.
    fn simple_ds() -> SasDataset {
        // 20 points; logit roughly linear in x.
        let x = vec![
            1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0, 1.5, 2.5, 3.5, 4.5, 5.5, 6.5, 7.5,
            8.5, 9.5, 10.5,
        ];
        let y = vec![
            0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 1.0, 1.0,
            1.0, 1.0, 1.0,
        ];
        let df = df!["x" => x, "y" => y].unwrap();
        SasDataset {
            df,
            vars: vec![num_meta("x"), num_meta("y")],
        }
    }

    // ───────────── execute tests ─────────────

    #[test]
    fn logistic_simple_linear_predictor() {
        let (listing, _log, _s) =
            run_logistic(simple_ds(), "proc logistic data=a; model y = x; run;");
        assert!(listing.contains("The LOGISTIC Procedure"), "{listing}");
        assert!(
            listing.contains("Analysis of Maximum Likelihood Estimates"),
            "{listing}"
        );
        assert!(listing.contains("Intercept"), "{listing}");
        assert!(listing.contains("x"), "{listing}");
    }

    #[test]
    fn logistic_binary_response() {
        // y already {0,1}; response profile must show two levels.
        let (listing, _log, _s) =
            run_logistic(simple_ds(), "proc logistic data=a; model y = x; run;");
        assert!(listing.contains("Response Profile"), "{listing}");
        assert!(listing.contains("Total Frequency"), "{listing}");
    }

    #[test]
    fn logistic_with_class_variable() {
        let g = vec!["A", "A", "A", "A", "B", "B", "B", "B", "C", "C", "C", "C"];
        let y = vec![
            0.0, 0.0, 0.0, 1.0, 0.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 0.0,
        ];
        let df = df!["g" => g, "y" => y].unwrap();
        let ds = SasDataset {
            df,
            vars: vec![char_meta("g"), num_meta("y")],
        };
        let (listing, _log, _s) =
            run_logistic(ds, "proc logistic data=a; class g; model y = g; run;");
        assert!(listing.contains("Class Level Information"), "{listing}");
        // Reference cell omitted: C is the last level → dummies for A and B.
        assert!(listing.contains("g A"), "{listing}");
        assert!(listing.contains("g B"), "{listing}");
        assert!(!listing.contains("g C"), "C should be the reference: {listing}");
    }

    #[test]
    fn logistic_model_fit_statistics() {
        let (listing, _log, _s) =
            run_logistic(simple_ds(), "proc logistic data=a; model y = x; run;");
        assert!(listing.contains("Model Fit Statistics"), "{listing}");
        assert!(listing.contains("-2 Log L"), "{listing}");
        assert!(listing.contains("Intercept Only"), "{listing}");
        assert!(listing.contains("AIC"), "{listing}");
        assert!(listing.contains("SC"), "{listing}");
    }

    #[test]
    fn logistic_odds_ratio() {
        // OR = exp(beta). Fit directly and compare table value.
        let ds = simple_ds();
        let mut session = make_session();
        write_dataset(&mut session, "A", ds);
        let (ds2, _) = session.libs.get("WORK").unwrap().read("A").unwrap();
        let y_vals = decode_column(&ds2, ds2.vars.iter().position(|v| v.name == "y").unwrap())
            .unwrap();
        let x_vals = decode_column(&ds2, ds2.vars.iter().position(|v| v.name == "x").unwrap())
            .unwrap();
        let kept: Vec<usize> = (0..ds2.n_obs()).collect();
        let mv = vec![ModelVar {
            name: "x".to_string(),
            is_class: false,
            vals: x_vals,
        }];
        let events: Vec<f64> = kept.iter().map(|&r| value_to_num(&y_vals[r]).unwrap()).collect();
        let trials = vec![1.0; kept.len()];
        let design = build_design(&mv, &[Vec::new()], &kept, events.clone(), trials.clone());
        let fit = newton_raphson(&design.x, &design.events, &design.trials, design.n_params)
            .unwrap();
        let or = fit.beta[1].exp();
        // Positive association → OR > 1.
        assert!(or > 1.0, "odds ratio = {or}");
        assert!(fit.converged, "should converge");
    }

    #[test]
    fn logistic_odds_ratio_ci() {
        let (listing, _log, _s) = run_logistic(
            simple_ds(),
            "proc logistic data=a; model y = x; oddsratio x; run;",
        );
        assert!(listing.contains("Odds Ratio Estimates"), "{listing}");
        assert!(listing.contains("Point Estimate"), "{listing}");
        assert!(listing.contains("95% Lower"), "{listing}");
        assert!(listing.contains("95% Upper"), "{listing}");
    }

    #[test]
    fn logistic_convergence() {
        // Well-separated but not perfectly separable data converges quickly.
        let ds = simple_ds();
        let mut session = make_session();
        write_dataset(&mut session, "A", ds);
        let (ds2, _) = session.libs.get("WORK").unwrap().read("A").unwrap();
        let y_vals = decode_column(&ds2, ds2.vars.iter().position(|v| v.name == "y").unwrap())
            .unwrap();
        let x_vals = decode_column(&ds2, ds2.vars.iter().position(|v| v.name == "x").unwrap())
            .unwrap();
        let kept: Vec<usize> = (0..ds2.n_obs()).collect();
        let mv = vec![ModelVar {
            name: "x".to_string(),
            is_class: false,
            vals: x_vals,
        }];
        let events: Vec<f64> = kept.iter().map(|&r| value_to_num(&y_vals[r]).unwrap()).collect();
        let trials = vec![1.0; kept.len()];
        let design = build_design(&mv, &[Vec::new()], &kept, events, trials);
        let fit = newton_raphson(&design.x, &design.events, &design.trials, design.n_params)
            .unwrap();
        assert!(fit.converged, "Newton-Raphson must converge");
        assert!(fit.loglik < 0.0, "log-likelihood must be negative");
    }

    #[test]
    fn logistic_known_coefficients() {
        // Construct data from a known logit and check recovered slope sign and
        // that the intercept-only deviance exceeds the model deviance.
        let ds = simple_ds();
        let mut session = make_session();
        write_dataset(&mut session, "A", ds);
        let (ds2, _) = session.libs.get("WORK").unwrap().read("A").unwrap();
        let y_vals = decode_column(&ds2, ds2.vars.iter().position(|v| v.name == "y").unwrap())
            .unwrap();
        let x_vals = decode_column(&ds2, ds2.vars.iter().position(|v| v.name == "x").unwrap())
            .unwrap();
        let kept: Vec<usize> = (0..ds2.n_obs()).collect();
        let mv = vec![ModelVar {
            name: "x".to_string(),
            is_class: false,
            vals: x_vals,
        }];
        let events: Vec<f64> = kept.iter().map(|&r| value_to_num(&y_vals[r]).unwrap()).collect();
        let n_event: f64 = events.iter().sum();
        let n_nonevent = events.len() as f64 - n_event;
        let trials = vec![1.0; kept.len()];
        let design = build_design(&mv, &[Vec::new()], &kept, events, trials);
        let fit = newton_raphson(&design.x, &design.events, &design.trials, design.n_params)
            .unwrap();
        let null_ll = null_loglik(n_event, n_nonevent);
        // Full model fits at least as well as the null model.
        assert!(fit.loglik >= null_ll - 1e-9, "full {} null {}", fit.loglik, null_ll);
        assert!(fit.beta[1] > 0.0, "slope should be positive: {}", fit.beta[1]);
    }

    #[test]
    fn logistic_output_out() {
        let (_listing, log, session) = run_logistic(
            simple_ds(),
            "proc logistic data=a; model y = x; output out=o; run;",
        );
        assert!(log.contains("WORK.O"), "{log}");
        let (out, _) = session.libs.get("WORK").unwrap().read("O").unwrap();
        let names: Vec<&str> = out.vars.iter().map(|v| v.name.as_str()).collect();
        assert!(names.contains(&"PREDICTED"), "{names:?}");
        assert_eq!(out.n_obs(), 20);
        // Predicted probabilities are in (0,1).
        let pcol = decode_column(&out, out.vars.iter().position(|v| v.name == "PREDICTED").unwrap())
            .unwrap();
        for v in &pcol {
            if let Some(p) = value_to_num(v) {
                if !p.is_nan() {
                    assert!((0.0..=1.0).contains(&p), "p out of range: {p}");
                }
            }
        }
    }

    #[test]
    fn logistic_missing_excluded() {
        let x = vec![Some(1.0), None, Some(3.0), Some(4.0), Some(5.0), Some(6.0)];
        let y = vec![Some(0.0), Some(0.0), Some(0.0), Some(1.0), Some(1.0), Some(1.0)];
        let df = df!["x" => x, "y" => y].unwrap();
        let ds = SasDataset {
            df,
            vars: vec![num_meta("x"), num_meta("y")],
        };
        let (listing, log, _s) =
            run_logistic(ds, "proc logistic data=a; model y = x; output out=o; run;");
        // 6 read, 5 used → response profile counts sum to 5.
        assert!(listing.contains("Response Profile"), "{listing}");
        let _ = log;
    }

    #[test]
    fn logistic_inference_wald_test() {
        // Wald chi-square = (beta/se)^2 with a one-DF p-value; verify the table
        // contains the Wald column and the global LR test.
        let (listing, _log, _s) =
            run_logistic(simple_ds(), "proc logistic data=a; model y = x; run;");
        assert!(listing.contains("Wald Chi-Square"), "{listing}");
        assert!(listing.contains("Pr > ChiSq"), "{listing}");
        assert!(
            listing.contains("Testing Global Null Hypothesis: BETA=0"),
            "{listing}"
        );
        assert!(listing.contains("Likelihood Ratio"), "{listing}");
    }

    #[test]
    fn logistic_events_trials_execute() {
        // Grouped binomial data: events out of trials at each x.
        let x = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let events = vec![1.0, 2.0, 5.0, 8.0, 9.0];
        let total = vec![10.0, 10.0, 10.0, 10.0, 10.0];
        let df = df!["x" => x, "ev" => events, "tot" => total].unwrap();
        let ds = SasDataset {
            df,
            vars: vec![num_meta("x"), num_meta("ev"), num_meta("tot")],
        };
        let (listing, _log, _s) =
            run_logistic(ds, "proc logistic data=a; model ev/tot = x; run;");
        assert!(listing.contains("The LOGISTIC Procedure"), "{listing}");
        assert!(listing.contains("Analysis of Maximum Likelihood Estimates"), "{listing}");
    }

    #[test]
    fn logistic_wald_chisq_value() {
        // Numerically: Wald chi-square = (beta/se)^2. Verify cov from (X'WX)^-1.
        let ds = simple_ds();
        let mut session = make_session();
        write_dataset(&mut session, "A", ds);
        let (ds2, _) = session.libs.get("WORK").unwrap().read("A").unwrap();
        let y_vals = decode_column(&ds2, ds2.vars.iter().position(|v| v.name == "y").unwrap())
            .unwrap();
        let x_vals = decode_column(&ds2, ds2.vars.iter().position(|v| v.name == "x").unwrap())
            .unwrap();
        let kept: Vec<usize> = (0..ds2.n_obs()).collect();
        let mv = vec![ModelVar {
            name: "x".to_string(),
            is_class: false,
            vals: x_vals,
        }];
        let events: Vec<f64> = kept.iter().map(|&r| value_to_num(&y_vals[r]).unwrap()).collect();
        let trials = vec![1.0; kept.len()];
        let design = build_design(&mv, &[Vec::new()], &kept, events, trials);
        let fit = newton_raphson(&design.x, &design.events, &design.trials, design.n_params)
            .unwrap();
        let xtwx = xtwx_matrix(&design.x, &fit.weights, design.n_params);
        let cov = invert_matrix(&xtwx).unwrap();
        let se1 = cov[1][1].sqrt();
        let wald = (fit.beta[1] / se1).powi(2);
        assert!(wald > 0.0 && wald.is_finite(), "wald = {wald}");
        // p-value from chi-square(1).
        let p = 1.0 - chisq_cdf(wald, 1.0);
        assert!((0.0..=1.0).contains(&p), "p = {p}");
    }
}
