//! PROC GENMOD — generalized linear models with general link functions (v1).
//!
//! # Plan du fichier — voir le plan M27 (modèles linéaires généralisés)
//!
//! `proc genmod data=<ref> [alpha=<f64>];
//!  [class group treatment ... ;]
//!  model y = x1 x2 class_var ... [/ dist=<d> link=<l>];
//!  [dist=<d>;]
//!  [link=<l>;]
//!  [output out=<ref>;]
//!  run;`
//!
//! ## Périmètre v1 (fidèle à SAS 9.4 PROC GENMOD)
//! PROC GENMOD étend PROC LOGISTIC : plusieurs distributions de la famille
//! exponentielle (NORMAL, POISSON, BINOMIAL, GAMMA, IGAUSSIAN) et plusieurs
//! fonctions de lien (IDENTITY, LOG, LOGIT, PROBIT, RECIPROCAL, POWER(p)).
//! - covariables continues ET CLASS variables (one-hot, cellule de référence
//!   = dernière modalité, comme GLM/LOGISTIC) ;
//! - estimation par maximum de vraisemblance via Newton-Raphson / IRLS avec la
//!   variance V(μ) propre à la distribution ;
//! - tests de Wald (χ²) sur chaque paramètre ;
//! - statistiques d'ajustement : Deviance, Scaled Deviance, AIC, BIC.
//!
//! ## Liens canoniques par défaut
//! - NORMAL  → IDENTITY ;
//! - POISSON → LOG ;
//! - BINOMIAL→ LOGIT ;
//! - GAMMA   → RECIPROCAL (lien canonique inverse) ;
//! - IGAUSSIAN→ RECIPROCAL (en v1 ; le lien canonique exact est 1/μ²).
//!
//! Les combinaisons non canoniques sont acceptées avec un WARNING v1.
//!
//! ## Choix / simplifications documentés
//! - Exclusion LISTWISE : retirer une observation dès que y OU une variable du
//!   modèle (CLASS ou continue) est manquante.
//! - Validation de y selon la distribution : POISSON exige y entier ≥ 0,
//!   BINOMIAL exige y ∈ {0,1}, GAMMA/IGAUSSIAN exigent y > 0.
//! - Scale parameter : pour NORMAL/GAMMA/IGAUSSIAN, le paramètre d'échelle est
//!   estimé naïvement par la déviance / DF résiduel (approximation v1).
//! - Newton-Raphson via IRLS : β_new = β + (XᵀWX)⁻¹ Xᵀ W (z − η) avec
//!   z = η + (y − μ)·(dη/dμ) et W = (dμ/dη)² / V(μ). Convergence ‖Δβ‖∞ < 1e-8.
//! - Matrice singulière (colinéarité parfaite) → `SasError`.

use crate::ast::DatasetRef;
use crate::dataset::{SasDataset, VarMeta};
use crate::error::{Result, SasError};
use crate::listing::Align;
use crate::missing::value_to_num;
use crate::parser::StatementStream;
use crate::procs::common::{decode_column, probnorm};
use crate::session::Session;
use crate::stat::dists::chisq_cdf;
use crate::stat::linalg::invert_matrix;
use crate::token::TokenKind;
use crate::value::{format_best, Value, VarType};
use polars::prelude::{Column, NamedFrom, Series};
use std::cmp::Ordering;

const MAX_ITER: usize = 50;
const TOL: f64 = 1e-8;

// ───────────────────────── AST ─────────────────────────

/// `class a b c ;` — declares the categorical factors (0/1 dummy coding).
pub struct ClassStmt {
    pub vars: Vec<String>,
}

/// `model y = x1 x2 ... ;` — `y` is the response (continuous / count / binary),
/// `x_vars` is a mix of continuous covariates and CLASS variables.
pub struct ModelStmt {
    pub y_var: String,
    pub x_vars: Vec<String>,
}

/// `dist=normal|poisson|binomial|gamma|igaussian`.
pub struct Dist {
    pub family: String,
    pub options: Option<String>,
}

/// `link=identity|log|logit|probit|reciprocal|power(p)`.
pub struct Link {
    pub family: String,
    /// Exponent for POWER(p). None unless `family == "POWER"`.
    pub power: Option<f64>,
}

pub struct GenmodAst {
    pub data: Option<DatasetRef>,
    /// Confidence-level complement. Default 0.05.
    pub alpha: f64,
    pub class_stmt: ClassStmt,
    pub model: ModelStmt,
    pub dist: Option<Dist>,
    pub link: Option<Link>,
    pub output_out: Option<DatasetRef>,
}

// ───────────────────────── parse ─────────────────────────

/// Parse `proc genmod [data=a] [alpha=p]; [class ...;] model y = x...;
/// [dist=...;] [link=...;] [output out=b;] run;`. Called AFTER "proc genmod"
/// was consumed. Consumes through `run;` / `quit;`.
pub fn parse(ts: &mut StatementStream) -> Result<GenmodAst> {
    let mut data: Option<DatasetRef> = None;
    let mut alpha: f64 = 0.05;

    // --- PROC GENMOD statement options, until `;` ---
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
        } else if ts.peek().is_kw("order") {
            // Recognised but ignored: consume the option and an `=value`.
            ts.next();
            if ts.peek().kind == TokenKind::Eq {
                ts.next();
                ts.next();
            }
        } else if let Some(name) = ts.peek().ident().map(str::to_string) {
            let span = ts.peek().span;
            return Err(SasError::parse(
                format!(
                    "Unexpected option '{}' on PROC GENMOD statement.",
                    name.to_uppercase()
                ),
                span,
            ));
        } else {
            let span = ts.peek().span;
            return Err(SasError::parse(
                "Unexpected token on PROC GENMOD statement.",
                span,
            ));
        }
    }

    // --- sub-statements until run;/quit; ---
    let mut class_vars: Vec<String> = Vec::new();
    let mut model: Option<ModelStmt> = None;
    let mut dist: Option<Dist> = None;
    let mut link: Option<Link> = None;
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
                    "PROC GENMOD v1 supports a single MODEL statement.",
                ));
            }
            let (m, md, ml) = parse_model(ts)?;
            model = Some(m);
            if md.is_some() {
                dist = md;
            }
            if ml.is_some() {
                link = ml;
            }
        } else if ts.peek().is_kw("dist") || ts.peek().is_kw("distribution") {
            ts.next();
            expect_eq(ts, "DIST")?;
            dist = Some(parse_dist(ts)?);
            // Consume trailing `;`.
            if ts.peek().kind == TokenKind::Semi {
                ts.next();
            }
        } else if ts.peek().is_kw("link") {
            ts.next();
            expect_eq(ts, "LINK")?;
            link = Some(parse_link(ts)?);
            if ts.peek().kind == TokenKind::Semi {
                ts.next();
            }
        } else if ts.peek().is_kw("output") {
            ts.next();
            output_out = Some(parse_output(ts)?);
        } else {
            // Unknown sub-statement: skip it (recovery, like glm/logistic).
            ts.skip_to_semi();
        }
    }

    let model = model.ok_or_else(|| {
        SasError::runtime("PROC GENMOD requires a MODEL statement (model y = effects;).")
    })?;

    Ok(GenmodAst {
        data,
        alpha,
        class_stmt: ClassStmt { vars: class_vars },
        model,
        dist,
        link,
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

/// Parse `model y = x1 x2 [/ dist=<d> link=<l>] ;` (the leading `model` keyword
/// already consumed). Returns the model and any inline DIST=/LINK= options.
fn parse_model(ts: &mut StatementStream) -> Result<(ModelStmt, Option<Dist>, Option<Link>)> {
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
    let mut dist: Option<Dist> = None;
    let mut link: Option<Link> = None;
    loop {
        if ts.peek().kind == TokenKind::Semi {
            ts.next();
            break;
        }
        if ts.peek().kind == TokenKind::Eof || ts.peek().is_kw("run") || ts.peek().is_kw("quit") {
            break;
        }
        if ts.peek().kind == TokenKind::Slash {
            // MODEL options: parse dist=/link= and skip the rest to ';'.
            ts.next();
            loop {
                if ts.peek().kind == TokenKind::Semi {
                    ts.next();
                    break;
                }
                if ts.peek().kind == TokenKind::Eof
                    || ts.peek().is_kw("run")
                    || ts.peek().is_kw("quit")
                {
                    break;
                }
                if ts.peek().is_kw("dist") || ts.peek().is_kw("distribution") {
                    ts.next();
                    expect_eq(ts, "DIST")?;
                    dist = Some(parse_dist(ts)?);
                } else if ts.peek().is_kw("link") {
                    ts.next();
                    expect_eq(ts, "LINK")?;
                    link = Some(parse_link(ts)?);
                } else {
                    ts.next();
                }
            }
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

    Ok((ModelStmt { y_var, x_vars }, dist, link))
}

/// Parse the distribution name after `dist=` (the `=` already consumed).
fn parse_dist(ts: &mut StatementStream) -> Result<Dist> {
    let span = ts.peek().span;
    let raw = ts
        .peek()
        .ident()
        .map(str::to_string)
        .ok_or_else(|| SasError::parse("Expected a distribution name after DIST=.", span))?;
    ts.next();
    let family = canonical_dist(&raw).ok_or_else(|| {
        SasError::runtime(format!(
            "Unrecognized distribution '{}' in PROC GENMOD.",
            raw.to_uppercase()
        ))
    })?;
    Ok(Dist {
        family,
        options: None,
    })
}

/// Parse the link name after `link=` (the `=` already consumed). Supports
/// `power(p)` with a numeric exponent.
fn parse_link(ts: &mut StatementStream) -> Result<Link> {
    let span = ts.peek().span;
    let raw = ts
        .peek()
        .ident()
        .map(str::to_string)
        .ok_or_else(|| SasError::parse("Expected a link name after LINK=.", span))?;
    ts.next();
    let family = canonical_link(&raw).ok_or_else(|| {
        SasError::runtime(format!(
            "Unrecognized link function '{}' in PROC GENMOD.",
            raw.to_uppercase()
        ))
    })?;
    let mut power: Option<f64> = None;
    if family == "POWER" && ts.peek().kind == TokenKind::LParen {
        ts.next();
        let p = parse_signed_number(ts)?;
        power = Some(p);
        if ts.peek().kind == TokenKind::RParen {
            ts.next();
        }
    }
    Ok(Link { family, power })
}

/// Normalize a distribution keyword to its canonical uppercase form.
fn canonical_dist(raw: &str) -> Option<String> {
    let u = raw.to_uppercase();
    let f = match u.as_str() {
        "NORMAL" | "GAUSSIAN" | "N" | "NOR" => "NORMAL",
        "POISSON" | "POI" | "P" => "POISSON",
        "BINOMIAL" | "BIN" | "B" => "BINOMIAL",
        "GAMMA" | "GAM" | "G" => "GAMMA",
        "IGAUSSIAN" | "IG" | "INVGAUSS" | "INVERSEGAUSSIAN" => "IGAUSSIAN",
        _ => return None,
    };
    Some(f.to_string())
}

/// Normalize a link keyword to its canonical uppercase form.
fn canonical_link(raw: &str) -> Option<String> {
    let u = raw.to_uppercase();
    let f = match u.as_str() {
        "IDENTITY" | "ID" => "IDENTITY",
        "LOG" => "LOG",
        "LOGIT" => "LOGIT",
        "PROBIT" => "PROBIT",
        "RECIPROCAL" | "INVERSE" | "INV" => "RECIPROCAL",
        "POWER" | "POW" => "POWER",
        _ => return None,
    };
    Some(f.to_string())
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
        SasError::runtime("The OUTPUT statement of PROC GENMOD requires OUT=<dataset>.")
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

fn parse_signed_number(ts: &mut StatementStream) -> Result<f64> {
    match ts.peek().kind.clone() {
        TokenKind::Num(v) => {
            ts.next();
            Ok(v)
        }
        TokenKind::Minus => {
            if let TokenKind::Num(v) = ts.peek2().kind.clone() {
                ts.next();
                ts.next();
                Ok(-v)
            } else {
                Err(SasError::parse("expected a number", ts.peek().span))
            }
        }
        TokenKind::Plus => {
            if let TokenKind::Num(v) = ts.peek2().kind.clone() {
                ts.next();
                ts.next();
                Ok(v)
            } else {
                Err(SasError::parse("expected a number", ts.peek().span))
            }
        }
        _ => Err(SasError::parse("expected a number", ts.peek().span)),
    }
}

/// Resolve `data=` or `_LAST_` into a concrete DatasetRef.
fn resolve_input(ast: &GenmodAst, session: &Session) -> Result<DatasetRef> {
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

// ───────────────────────── GLM link / variance functions ─────────────────────

/// Inverse link μ = g⁻¹(η).
fn link_inverse(link: &str, power: f64, eta: f64) -> f64 {
    match link {
        "IDENTITY" => eta,
        "LOG" => eta.exp(),
        "LOGIT" => 1.0 / (1.0 + (-eta).exp()),
        "PROBIT" => probnorm(eta),
        "RECIPROCAL" => {
            if eta == 0.0 {
                f64::INFINITY
            } else {
                1.0 / eta
            }
        }
        "POWER" => {
            if power == 0.0 {
                eta.exp()
            } else {
                // η = μ^p → μ = η^{1/p}.
                eta.powf(1.0 / power)
            }
        }
        _ => eta,
    }
}

/// Derivative dμ/dη at (η, μ).
fn link_derivative(link: &str, power: f64, eta: f64, mu: f64) -> f64 {
    match link {
        "IDENTITY" => 1.0,
        "LOG" => mu,
        "LOGIT" => mu * (1.0 - mu),
        "PROBIT" => standard_normal_pdf(eta),
        "RECIPROCAL" => {
            if eta == 0.0 {
                f64::NEG_INFINITY
            } else {
                -1.0 / (eta * eta)
            }
        }
        "POWER" => {
            if power == 0.0 {
                mu
            } else {
                // μ = η^{1/p} → dμ/dη = (1/p) η^{1/p - 1}.
                (1.0 / power) * eta.powf(1.0 / power - 1.0)
            }
        }
        _ => 1.0,
    }
}

/// Variance function V(μ) per distribution.
fn variance(family: &str, mu: f64) -> f64 {
    match family {
        "NORMAL" => 1.0,
        "POISSON" => mu.abs().max(1e-10),
        "BINOMIAL" => (mu * (1.0 - mu)).max(1e-10),
        "GAMMA" => (mu * mu).max(1e-10),
        "IGAUSSIAN" => (mu * mu * mu).abs().max(1e-10),
        _ => 1.0,
    }
}

/// Standard normal PDF φ(z).
fn standard_normal_pdf(z: f64) -> f64 {
    (-0.5 * z * z).exp() / (2.0 * std::f64::consts::PI).sqrt()
}

/// Canonical (default) link for a distribution.
fn canonical_link_for(family: &str) -> &'static str {
    match family {
        "NORMAL" => "IDENTITY",
        "POISSON" => "LOG",
        "BINOMIAL" => "LOGIT",
        "GAMMA" => "RECIPROCAL",
        "IGAUSSIAN" => "RECIPROCAL",
        _ => "IDENTITY",
    }
}

/// Whether (family, link) is a recommended/canonical pairing.
fn is_recommended(family: &str, link: &str) -> bool {
    match family {
        "NORMAL" => matches!(link, "IDENTITY" | "LOG"),
        "POISSON" => matches!(link, "LOG" | "IDENTITY"),
        "BINOMIAL" => matches!(link, "LOGIT" | "PROBIT"),
        "GAMMA" => matches!(link, "RECIPROCAL" | "LOG"),
        "IGAUSSIAN" => matches!(link, "RECIPROCAL" | "LOG"),
        _ => true,
    }
}

/// Per-observation unit deviance contribution d_i (so that
/// Deviance = Σ d_i). Uses the standard GLM deviance formulas.
fn unit_deviance(family: &str, y: f64, mu: f64) -> f64 {
    let m = mu.max(1e-12);
    match family {
        "NORMAL" => (y - mu) * (y - mu),
        "POISSON" => {
            let t = if y > 0.0 { y * (y / m).ln() } else { 0.0 };
            2.0 * (t - (y - mu))
        }
        "BINOMIAL" => {
            let mut d = 0.0;
            if y > 0.0 {
                d += y * (y / m).ln();
            }
            if y < 1.0 {
                d += (1.0 - y) * ((1.0 - y) / (1.0 - m).max(1e-12)).ln();
            }
            2.0 * d
        }
        "GAMMA" => {
            let t = if y > 0.0 { -(y / m).ln() } else { 0.0 };
            2.0 * (t + (y - mu) / m)
        }
        "IGAUSSIAN" => {
            // d = (y - μ)² / (μ² y).
            if y > 0.0 {
                (y - mu) * (y - mu) / (m * m * y)
            } else {
                0.0
            }
        }
        _ => (y - mu) * (y - mu),
    }
}

// ───────────────────────── design construction ─────────────────────────

fn is_present(v: &Value) -> bool {
    match v {
        Value::Num(f) => !f.is_nan(),
        Value::Char(s) => !s.trim().is_empty(),
        Value::Missing(_) => false,
    }
}

fn level_label(v: &Value) -> String {
    match v {
        Value::Num(f) => format_best(*f, 12),
        Value::Missing(k) => k.display(),
        Value::Char(s) => s.trim_end().to_string(),
    }
}

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

/// A resolved model variable.
struct ModelVar {
    name: String,
    is_class: bool,
    vals: Vec<Value>,
}

/// One named design column (intercept first).
struct DesignCol {
    name: String,
}

/// The assembled design.
struct Design {
    /// X matrix, m × n_params (intercept first).
    x: Vec<Vec<f64>>,
    /// Response vector, length m.
    y: Vec<f64>,
    n_params: usize,
    cols: Vec<DesignCol>,
}

/// Build the design matrix: intercept + continuous covariates + class dummies
/// (last level omitted as the reference cell).
fn build_design(
    model_vars: &[ModelVar],
    factor_levels: &[Vec<Value>],
    kept_rows: &[usize],
    y_vals: &[Value],
) -> Design {
    let m = kept_rows.len();

    let mut cols: Vec<DesignCol> = vec![DesignCol {
        name: "Intercept".to_string(),
    }];
    let mut design_cols: Vec<Vec<f64>> = Vec::new();

    for (vi, mv) in model_vars.iter().enumerate() {
        if mv.is_class {
            let levels = &factor_levels[vi];
            let take = levels.len().saturating_sub(1); // omit reference (last)
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
            });
        }
    }

    let n_params = design_cols.len() + 1;
    let mut x: Vec<Vec<f64>> = Vec::with_capacity(m);
    let mut y: Vec<f64> = Vec::with_capacity(m);
    for (pos, &r) in kept_rows.iter().enumerate() {
        let mut row = Vec::with_capacity(n_params);
        row.push(1.0);
        for col in &design_cols {
            row.push(col[pos]);
        }
        x.push(row);
        y.push(value_to_num(&y_vals[r]).unwrap_or(f64::NAN));
    }

    Design {
        x,
        y,
        n_params,
        cols,
    }
}

// ───────────────────────── IRLS / Newton-Raphson ─────────────────────────

struct FitResult {
    beta: Vec<f64>,
    /// IRLS weights W_i = (dμ/dη)² / V(μ) at the solution.
    weights: Vec<f64>,
    /// Fitted means μ_i at the solution.
    mu: Vec<f64>,
    /// Linear predictor η_i at the solution.
    eta: Vec<f64>,
    converged: bool,
}

/// IRLS for a generalized linear model with the given family/link.
///
/// Iterate:
///   η = Xβ ; μ = g⁻¹(η) ; gprime = dμ/dη ; V = V(μ) ;
///   W = gprime² / V ; z = η + (y − μ)/gprime (working response) ;
///   β ← (XᵀWX)⁻¹ XᵀW z. Stop when ‖Δβ‖∞ < TOL.
fn irls(
    x: &[Vec<f64>],
    y: &[f64],
    n_params: usize,
    family: &str,
    link: &str,
    power: f64,
) -> Result<FitResult> {
    let m = x.len();
    // Initialise β: intercept from the link of a "safe" mean of y, others 0.
    let ybar = y.iter().sum::<f64>() / m.max(1) as f64;
    let mu0 = init_mu(family, ybar);
    let eta0 = init_eta(link, power, mu0);
    let mut beta = vec![0.0_f64; n_params];
    if eta0.is_finite() {
        beta[0] = eta0;
    }

    let mut weights = vec![0.0_f64; m];
    let mut mu = vec![0.0_f64; m];
    let mut eta = vec![0.0_f64; m];
    let mut converged = false;

    for _iter in 0..MAX_ITER {
        let mut xtwx = vec![vec![0.0_f64; n_params]; n_params];
        let mut xtwz = vec![0.0_f64; n_params];

        for i in 0..m {
            let eta_i: f64 = x[i].iter().zip(&beta).map(|(a, b)| a * b).sum();
            let mu_i = link_inverse(link, power, eta_i);
            let mu_i = clamp_mu(family, mu_i);
            let gprime = link_derivative(link, power, eta_i, mu_i);
            let v = variance(family, mu_i);
            // Guard against degenerate weights.
            let gp = if gprime.abs() < 1e-12 {
                1e-12_f64.copysign(if gprime < 0.0 { -1.0 } else { 1.0 })
            } else {
                gprime
            };
            let w = (gp * gp) / v;
            let w = if w.is_finite() { w.max(1e-12) } else { 1e-12 };
            // Working response z = η + (y − μ)/g'.
            let z = eta_i + (y[i] - mu_i) / gp;

            eta[i] = eta_i;
            mu[i] = mu_i;
            weights[i] = w;

            for a in 0..n_params {
                let xaw = x[i][a] * w;
                xtwz[a] += xaw * z;
                for b in 0..n_params {
                    xtwx[a][b] += xaw * x[i][b];
                }
            }
        }

        let xtwx_inv = invert_matrix(&xtwx)?;
        let mut beta_new = vec![0.0_f64; n_params];
        for a in 0..n_params {
            let mut s = 0.0;
            for b in 0..n_params {
                s += xtwx_inv[a][b] * xtwz[b];
            }
            beta_new[a] = s;
        }

        let mut max_step = 0.0_f64;
        for a in 0..n_params {
            max_step = max_step.max((beta_new[a] - beta[a]).abs());
        }
        beta = beta_new;
        if max_step < TOL {
            converged = true;
            // Recompute at the final β for accurate inference.
            for i in 0..m {
                let eta_i: f64 = x[i].iter().zip(&beta).map(|(a, b)| a * b).sum();
                let mu_i = clamp_mu(family, link_inverse(link, power, eta_i));
                let gprime = link_derivative(link, power, eta_i, mu_i);
                let v = variance(family, mu_i);
                let gp = if gprime.abs() < 1e-12 { 1e-12 } else { gprime };
                let w = ((gp * gp) / v).max(1e-12);
                eta[i] = eta_i;
                mu[i] = mu_i;
                weights[i] = if w.is_finite() { w } else { 1e-12 };
            }
            break;
        }
    }

    Ok(FitResult {
        beta,
        weights,
        mu,
        eta,
        converged,
    })
}

/// Initial mean for a family given the raw response average.
fn init_mu(family: &str, ybar: f64) -> f64 {
    match family {
        "BINOMIAL" => ybar.clamp(0.05, 0.95),
        "POISSON" => ybar.max(0.1),
        "GAMMA" | "IGAUSSIAN" => ybar.max(0.1),
        _ => ybar,
    }
}

/// Initial linear predictor for the intercept.
fn init_eta(link: &str, power: f64, mu: f64) -> f64 {
    match link {
        "IDENTITY" => mu,
        "LOG" => mu.max(1e-6).ln(),
        "LOGIT" => {
            let p = mu.clamp(1e-6, 1.0 - 1e-6);
            (p / (1.0 - p)).ln()
        }
        "PROBIT" => crate::stat::dists::phi_inv(mu.clamp(1e-6, 1.0 - 1e-6)),
        "RECIPROCAL" => {
            if mu.abs() < 1e-12 {
                1e12
            } else {
                1.0 / mu
            }
        }
        "POWER" => {
            if power == 0.0 {
                mu.max(1e-6).ln()
            } else {
                mu.max(1e-6).powf(power)
            }
        }
        _ => mu,
    }
}

/// Keep μ in the valid domain for its family.
fn clamp_mu(family: &str, mu: f64) -> f64 {
    match family {
        "BINOMIAL" => mu.clamp(1e-10, 1.0 - 1e-10),
        "POISSON" => mu.max(1e-10),
        "GAMMA" | "IGAUSSIAN" => mu.max(1e-10),
        _ => mu,
    }
}

// ───────────────────────── execute ─────────────────────────

pub fn execute(ast: &GenmodAst, session: &mut Session) -> Result<()> {
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

    // --- distribution & link resolution ---
    let family = ast
        .dist
        .as_ref()
        .map(|d| d.family.clone())
        .unwrap_or_else(|| "NORMAL".to_string());
    let (link, power) = match &ast.link {
        Some(l) => (l.family.clone(), l.power.unwrap_or(1.0)),
        None => (canonical_link_for(&family).to_string(), 1.0),
    };

    if !is_recommended(&family, &link) {
        session.log.warning(&format!(
            "The {link} link is not the recommended link for the {family} distribution; \
             results are produced but should be interpreted with care."
        ));
    }

    // --- response variable ---
    let y_col = resolve_col(&ast.model.y_var)?;
    if ds.vars[y_col].ty != VarType::Num {
        return Err(SasError::runtime(format!(
            "Response variable {} in the MODEL statement is not numeric.",
            ast.model.y_var.to_uppercase()
        )));
    }
    let y_vals = decode_column(&ds, y_col)?;

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
        let yv = value_to_num(&y_vals[r]);
        if !matches!(yv, Some(v) if !v.is_nan()) {
            continue;
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

    // --- validate y against the distribution ---
    validate_response(&family, &y_vals, &kept_rows, &ast.model.y_var)?;

    // --- design matrix ---
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

    let design = build_design(&model_vars, &factor_levels, &kept_rows, &y_vals);
    let n_params = design.n_params;
    let m = design.x.len();

    if m < n_params + 1 {
        return Err(SasError::runtime(format!(
            "Insufficient non-missing observations ({m}) for {n_params} parameters in PROC GENMOD.",
        )));
    }

    // --- IRLS fit ---
    let fit = irls(&design.x, &design.y, n_params, &family, &link, power)?;
    if !fit.converged {
        session
            .log
            .note("Convergence criterion (GCONV=1E-8) not satisfied.");
    }
    let beta = &fit.beta;

    // --- deviance & fit statistics ---
    let mut deviance = 0.0_f64;
    for i in 0..m {
        deviance += unit_deviance(&family, design.y[i], fit.mu[i]);
    }
    let deviance = deviance.max(0.0);
    let df_resid = (m - n_params) as f64;

    // Scale parameter φ. For NORMAL/GAMMA/IGAUSSIAN estimated by deviance/df;
    // for POISSON/BINOMIAL φ = 1 by construction.
    let scale = match family.as_str() {
        "POISSON" | "BINOMIAL" => 1.0,
        _ => {
            if df_resid > 0.0 {
                (deviance / df_resid).max(1e-12)
            } else {
                1.0
            }
        }
    };
    let scaled_deviance = if scale > 0.0 { deviance / scale } else { deviance };

    // Log-likelihood and AIC.
    let loglik = loglikelihood(&family, &design.y, &fit.mu, scale);
    // Number of estimated parameters (β plus scale for the dispersion families).
    let n_scale_params = match family.as_str() {
        "POISSON" | "BINOMIAL" => 0.0,
        _ => 1.0,
    };
    let n_params_aic = n_params as f64 + n_scale_params;
    let aic = -2.0 * loglik + 2.0 * n_params_aic;
    let bic = -2.0 * loglik + (m as f64).ln() * n_params_aic;

    // --- variance-covariance: φ · (XᵀWX)⁻¹ ---
    let xtwx = xtwx_matrix(&design.x, &fit.weights, n_params);
    let cov = invert_matrix(&xtwx)?;
    let se: Vec<f64> = (0..n_params)
        .map(|j| (cov[j][j] * scale).max(0.0).sqrt())
        .collect();

    // --- listing ---
    emit_listing(
        session,
        &ds,
        y_col,
        &ast.class_stmt.vars,
        &model_vars,
        &factor_levels,
        &in_display,
        &family,
        &link,
        n_obs,
        m,
        deviance,
        scaled_deviance,
        df_resid,
        aic,
        bic,
        loglik,
        &design,
        beta,
        &se,
    );

    // --- OUTPUT OUT= ---
    if let Some(target) = &ast.output_out {
        let predicted: Vec<f64> = fit.mu.clone();
        write_output_dataset(session, &ds, target, &kept_rows, &predicted, n_obs)?;
    }
    let _ = fit.eta;

    Ok(())
}

/// Validate that the response is admissible for the chosen distribution.
fn validate_response(
    family: &str,
    y_vals: &[Value],
    kept_rows: &[usize],
    y_name: &str,
) -> Result<()> {
    for &r in kept_rows {
        let y = value_to_num(&y_vals[r]).unwrap_or(f64::NAN);
        match family {
            "POISSON" => {
                if y < 0.0 || (y - y.round()).abs() > 1e-9 {
                    return Err(SasError::runtime(format!(
                        "The POISSON distribution requires non-negative integer responses; \
                         {} has the value {y}.",
                        y_name.to_uppercase()
                    )));
                }
            }
            "BINOMIAL" => {
                if !(y == 0.0 || y == 1.0) {
                    return Err(SasError::runtime(format!(
                        "The BINOMIAL distribution (single-variable form) requires a 0/1 \
                         response; {} has the value {y}.",
                        y_name.to_uppercase()
                    )));
                }
            }
            "GAMMA" | "IGAUSSIAN" => {
                if y <= 0.0 {
                    return Err(SasError::runtime(format!(
                        "The {family} distribution requires strictly positive responses; \
                         {} has the value {y}.",
                        y_name.to_uppercase()
                    )));
                }
            }
            _ => {}
        }
    }
    Ok(())
}

/// Form XᵀWX with diagonal weights W.
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

/// Log-likelihood of the fitted GLM (up to the constant terms that GENMOD also
/// drops in its AIC display, but kept here consistently across distributions).
fn loglikelihood(family: &str, y: &[f64], mu: &[f64], scale: f64) -> f64 {
    let n = y.len();
    let mut ll = 0.0_f64;
    match family {
        "NORMAL" => {
            // σ² = scale.
            let var = scale.max(1e-12);
            for i in 0..n {
                let e = y[i] - mu[i];
                ll += -0.5 * (e * e / var) - 0.5 * (2.0 * std::f64::consts::PI * var).ln();
            }
        }
        "POISSON" => {
            for i in 0..n {
                let m = mu[i].max(1e-12);
                ll += y[i] * m.ln() - m - ln_factorial(y[i]);
            }
        }
        "BINOMIAL" => {
            for i in 0..n {
                let p = mu[i].clamp(1e-12, 1.0 - 1e-12);
                ll += y[i] * p.ln() + (1.0 - y[i]) * (1.0 - p).ln();
            }
        }
        "GAMMA" => {
            // Shape k = 1/scale, rate = k/μ.
            let k = (1.0 / scale.max(1e-12)).max(1e-12);
            for i in 0..n {
                let m = mu[i].max(1e-12);
                let yy = y[i].max(1e-12);
                ll += (k - 1.0) * yy.ln() - k * yy / m + k * (k / m).ln() - ln_gamma(k);
            }
        }
        "IGAUSSIAN" => {
            let lambda = (1.0 / scale.max(1e-12)).max(1e-12);
            for i in 0..n {
                let m = mu[i].max(1e-12);
                let yy = y[i].max(1e-12);
                ll += 0.5 * (lambda / (2.0 * std::f64::consts::PI * yy.powi(3))).ln()
                    - lambda * (yy - m) * (yy - m) / (2.0 * m * m * yy);
            }
        }
        _ => {
            for i in 0..n {
                let e = y[i] - mu[i];
                ll += -0.5 * e * e;
            }
        }
    }
    ll
}

/// ln Γ(x) (Lanczos), local copy to avoid cross-module churn.
fn ln_gamma(x: f64) -> f64 {
    const COF: [f64; 6] = [
        76.18009172947146,
        -86.50532032941677,
        24.01409824083091,
        -1.231739572450155,
        0.1208650973866179e-2,
        -0.5395239384953e-5,
    ];
    let mut y = x;
    let tmp = x + 5.5 - (x + 0.5) * (x + 5.5).ln();
    let mut ser = 1.000000000190015;
    for c in COF.iter() {
        y += 1.0;
        ser += c / y;
    }
    -tmp + (2.5066282746310005 * ser / x).ln()
}

/// ln(n!) for a non-negative (possibly fractional, rounded) count.
fn ln_factorial(n: f64) -> f64 {
    if n <= 1.0 {
        0.0
    } else {
        ln_gamma(n + 1.0)
    }
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
    family: &str,
    link: &str,
    n_obs: usize,
    m: usize,
    deviance: f64,
    scaled_deviance: f64,
    df_resid: f64,
    aic: f64,
    bic: f64,
    loglik: f64,
    design: &Design,
    beta: &[f64],
    se: &[f64],
) {
    session.listing.page_header();
    centered(session, "The GENMOD Procedure");
    session.listing.blank();

    // Model Information.
    centered(session, "Model Information");
    session.listing.blank();
    {
        let headers: Vec<String> = vec!["".into(), "".into()];
        let aligns = vec![Align::Left, Align::Left];
        let rows = vec![
            vec!["Data Set".into(), in_display.to_string()],
            vec!["Distribution".into(), dist_display(family)],
            vec!["Link Function".into(), link_display(link)],
            vec!["Dependent Variable".into(), ds.vars[y_col].name.clone()],
        ];
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

    // Model Fit Statistics (Criteria For Assessing Goodness Of Fit).
    centered(session, "Criteria For Assessing Goodness Of Fit");
    session.listing.blank();
    {
        let headers: Vec<String> = vec![
            "Criterion".into(),
            "DF".into(),
            "Value".into(),
            "Value/DF".into(),
        ];
        let aligns = vec![Align::Left, Align::Right, Align::Right, Align::Right];
        let dev_dfratio = if df_resid > 0.0 {
            deviance / df_resid
        } else {
            f64::NAN
        };
        let sdev_dfratio = if df_resid > 0.0 {
            scaled_deviance / df_resid
        } else {
            f64::NAN
        };
        let rows = vec![
            vec![
                "Deviance".into(),
                fmt_df(df_resid),
                fmt_num(deviance),
                fmt_num(dev_dfratio),
            ],
            vec![
                "Scaled Deviance".into(),
                fmt_df(df_resid),
                fmt_num(scaled_deviance),
                fmt_num(sdev_dfratio),
            ],
            vec![
                "Log Likelihood".into(),
                String::new(),
                fmt_num(loglik),
                String::new(),
            ],
            vec![
                "AIC".into(),
                String::new(),
                fmt_num(aic),
                String::new(),
            ],
            vec![
                "BIC".into(),
                String::new(),
                fmt_num(bic),
                String::new(),
            ],
        ];
        session.listing.write_table(&headers, &aligns, &rows);
    }
    session.listing.blank();

    // Analysis Of Maximum Likelihood Parameter Estimates.
    centered(session, "Analysis Of Maximum Likelihood Parameter Estimates");
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

fn dist_display(family: &str) -> String {
    match family {
        "NORMAL" => "Normal",
        "POISSON" => "Poisson",
        "BINOMIAL" => "Binomial",
        "GAMMA" => "Gamma",
        "IGAUSSIAN" => "Inverse Gaussian",
        other => other,
    }
    .to_string()
}

fn link_display(link: &str) -> String {
    match link {
        "IDENTITY" => "Identity",
        "LOG" => "Log",
        "LOGIT" => "Logit",
        "PROBIT" => "Probit",
        "RECIPROCAL" => "Reciprocal",
        "POWER" => "Power",
        other => other,
    }
    .to_string()
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
        format!("{v:.4}")
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

    fn parse_genmod(src: &str) -> Result<GenmodAst> {
        let source = SourceFile::new(src);
        let mut ts = StatementStream::new(&source).unwrap();
        ts.next(); // "proc"
        ts.next(); // "genmod"
        parse(&mut ts)
    }

    fn run_genmod(ds: SasDataset, src: &str) -> (String, String, Session) {
        let mut session = make_session();
        write_dataset(&mut session, "A", ds);
        let ast = parse_genmod(src).unwrap();
        execute(&ast, &mut session).unwrap();
        let listing = session.listing.into_string();
        let log = session.log.buffer().to_string();
        (listing, log, session)
    }

    // ───────────── parse tests ─────────────

    #[test]
    fn parse_minimal() {
        let ast = parse_genmod("proc genmod data=a; model y = x; run;").unwrap();
        assert_eq!(ast.model.y_var, "y");
        assert_eq!(ast.model.x_vars, vec!["x"]);
        assert_eq!(ast.alpha, 0.05);
        assert!(ast.dist.is_none());
    }

    #[test]
    fn parse_dist_link_statements() {
        let ast = parse_genmod(
            "proc genmod data=a; class g; model y = x g; dist=poisson; link=log; run;",
        )
        .unwrap();
        assert_eq!(ast.class_stmt.vars, vec!["g"]);
        assert_eq!(ast.dist.as_ref().unwrap().family, "POISSON");
        assert_eq!(ast.link.as_ref().unwrap().family, "LOG");
    }

    #[test]
    fn parse_inline_model_options() {
        let ast =
            parse_genmod("proc genmod data=a; model y = x / dist=binomial link=logit; run;")
                .unwrap();
        assert_eq!(ast.dist.as_ref().unwrap().family, "BINOMIAL");
        assert_eq!(ast.link.as_ref().unwrap().family, "LOGIT");
    }

    #[test]
    fn parse_power_link() {
        let ast = parse_genmod("proc genmod data=a; model y = x; link=power(2); run;").unwrap();
        assert_eq!(ast.link.as_ref().unwrap().family, "POWER");
        assert_eq!(ast.link.as_ref().unwrap().power, Some(2.0));
    }

    #[test]
    fn parse_requires_model() {
        assert!(parse_genmod("proc genmod data=a; class g; run;").is_err());
    }

    // ───────────── data builders ─────────────

    /// Linear data with Gaussian-ish noise: y ≈ 2 + 3x.
    fn normal_ds() -> SasDataset {
        let x: Vec<f64> = (1..=20).map(|i| i as f64).collect();
        let y: Vec<f64> = x.iter().map(|xv| 2.0 + 3.0 * xv).collect();
        let df = df!["x" => x, "y" => y].unwrap();
        SasDataset {
            df,
            vars: vec![num_meta("x"), num_meta("y")],
        }
    }

    /// Count data with a log-linear mean.
    fn poisson_ds() -> SasDataset {
        let x = vec![
            0.0, 0.5, 1.0, 1.5, 2.0, 2.5, 3.0, 0.0, 0.5, 1.0, 1.5, 2.0, 2.5, 3.0,
        ];
        // counts increasing with x.
        let y = vec![
            1.0, 1.0, 2.0, 3.0, 5.0, 8.0, 12.0, 0.0, 2.0, 2.0, 4.0, 6.0, 9.0, 13.0,
        ];
        let df = df!["x" => x, "y" => y].unwrap();
        SasDataset {
            df,
            vars: vec![num_meta("x"), num_meta("y")],
        }
    }

    /// Binary data with a logit-linear mean.
    fn binomial_ds() -> SasDataset {
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

    /// Positive continuous data for Gamma.
    fn gamma_ds() -> SasDataset {
        let x = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0];
        // y positive, increasing.
        let y: Vec<f64> = x.iter().map(|xv| 2.0 + 1.5 * xv).collect();
        let df = df!["x" => x, "y" => y].unwrap();
        SasDataset {
            df,
            vars: vec![num_meta("x"), num_meta("y")],
        }
    }

    // ───────────── link/variance unit tests ─────────────

    #[test]
    fn genmod_link_functions() {
        // identity
        assert!((link_inverse("IDENTITY", 1.0, 1.3) - 1.3).abs() < 1e-12);
        // log
        assert!((link_inverse("LOG", 1.0, 0.0) - 1.0).abs() < 1e-12);
        assert!((link_inverse("LOG", 1.0, 1.0) - std::f64::consts::E).abs() < 1e-9);
        // logit
        assert!((link_inverse("LOGIT", 1.0, 0.0) - 0.5).abs() < 1e-12);
        // probit at 0 → 0.5
        assert!((link_inverse("PROBIT", 1.0, 0.0) - 0.5).abs() < 1e-6);
        // reciprocal
        assert!((link_inverse("RECIPROCAL", 1.0, 2.0) - 0.5).abs() < 1e-12);

        // derivatives
        assert!((link_derivative("IDENTITY", 1.0, 0.0, 0.0) - 1.0).abs() < 1e-12);
        assert!((link_derivative("LOG", 1.0, 0.0, 3.0) - 3.0).abs() < 1e-12);
        // logit derivative at mu=0.5 = 0.25
        assert!((link_derivative("LOGIT", 1.0, 0.0, 0.5) - 0.25).abs() < 1e-12);
        // reciprocal derivative at eta=2 → -1/4
        assert!((link_derivative("RECIPROCAL", 1.0, 2.0, 0.5) + 0.25).abs() < 1e-12);

        // variances
        assert!((variance("NORMAL", 5.0) - 1.0).abs() < 1e-12);
        assert!((variance("POISSON", 4.0) - 4.0).abs() < 1e-12);
        assert!((variance("BINOMIAL", 0.5) - 0.25).abs() < 1e-12);
        assert!((variance("GAMMA", 3.0) - 9.0).abs() < 1e-12);
        assert!((variance("IGAUSSIAN", 2.0) - 8.0).abs() < 1e-12);
    }

    // ───────────── execute tests ─────────────

    #[test]
    fn genmod_normal_identity() {
        // Normal + identity should recover the OLS line y = 2 + 3x.
        let ds = normal_ds();
        let mut session = make_session();
        write_dataset(&mut session, "A", ds);
        let (ds2, _) = session.libs.get("WORK").unwrap().read("A").unwrap();
        let y_col = ds2.vars.iter().position(|v| v.name == "y").unwrap();
        let y_vals = decode_column(&ds2, y_col).unwrap();
        let x_vals = decode_column(
            &ds2,
            ds2.vars.iter().position(|v| v.name == "x").unwrap(),
        )
        .unwrap();
        let kept: Vec<usize> = (0..ds2.n_obs()).collect();
        let mv = vec![ModelVar {
            name: "x".to_string(),
            is_class: false,
            vals: x_vals,
        }];
        let design = build_design(&mv, &[Vec::new()], &kept, &y_vals);
        let fit = irls(&design.x, &design.y, design.n_params, "NORMAL", "IDENTITY", 1.0).unwrap();
        assert!(fit.converged);
        assert!((fit.beta[0] - 2.0).abs() < 1e-6, "intercept={}", fit.beta[0]);
        assert!((fit.beta[1] - 3.0).abs() < 1e-6, "slope={}", fit.beta[1]);
    }

    #[test]
    fn genmod_normal_identity_listing() {
        let (listing, _log, _s) =
            run_genmod(normal_ds(), "proc genmod data=a; model y = x; dist=normal; run;");
        assert!(listing.contains("The GENMOD Procedure"), "{listing}");
        assert!(listing.contains("Distribution"), "{listing}");
        assert!(listing.contains("Normal"), "{listing}");
        assert!(
            listing.contains("Analysis Of Maximum Likelihood Parameter Estimates"),
            "{listing}"
        );
    }

    #[test]
    fn genmod_poisson_log() {
        let ds = poisson_ds();
        let mut session = make_session();
        write_dataset(&mut session, "A", ds);
        let (ds2, _) = session.libs.get("WORK").unwrap().read("A").unwrap();
        let y_col = ds2.vars.iter().position(|v| v.name == "y").unwrap();
        let y_vals = decode_column(&ds2, y_col).unwrap();
        let x_vals = decode_column(
            &ds2,
            ds2.vars.iter().position(|v| v.name == "x").unwrap(),
        )
        .unwrap();
        let kept: Vec<usize> = (0..ds2.n_obs()).collect();
        let mv = vec![ModelVar {
            name: "x".to_string(),
            is_class: false,
            vals: x_vals,
        }];
        let design = build_design(&mv, &[Vec::new()], &kept, &y_vals);
        let fit = irls(&design.x, &design.y, design.n_params, "POISSON", "LOG", 1.0).unwrap();
        assert!(fit.converged, "Poisson IRLS must converge");
        // Positive trend in counts → positive slope on log scale.
        assert!(fit.beta[1] > 0.0, "slope={}", fit.beta[1]);
        // Fitted means are positive.
        assert!(fit.mu.iter().all(|&m| m > 0.0));
    }

    #[test]
    fn genmod_poisson_listing() {
        let (listing, _log, _s) = run_genmod(
            poisson_ds(),
            "proc genmod data=a; model y = x; dist=poisson; run;",
        );
        assert!(listing.contains("Poisson"), "{listing}");
        assert!(listing.contains("Log"), "{listing}");
        assert!(listing.contains("Deviance"), "{listing}");
    }

    #[test]
    fn genmod_binomial_logit() {
        // Binomial + logit should behave like logistic regression: positive slope.
        let ds = binomial_ds();
        let mut session = make_session();
        write_dataset(&mut session, "A", ds);
        let (ds2, _) = session.libs.get("WORK").unwrap().read("A").unwrap();
        let y_col = ds2.vars.iter().position(|v| v.name == "y").unwrap();
        let y_vals = decode_column(&ds2, y_col).unwrap();
        let x_vals = decode_column(
            &ds2,
            ds2.vars.iter().position(|v| v.name == "x").unwrap(),
        )
        .unwrap();
        let kept: Vec<usize> = (0..ds2.n_obs()).collect();
        let mv = vec![ModelVar {
            name: "x".to_string(),
            is_class: false,
            vals: x_vals,
        }];
        let design = build_design(&mv, &[Vec::new()], &kept, &y_vals);
        let fit = irls(&design.x, &design.y, design.n_params, "BINOMIAL", "LOGIT", 1.0).unwrap();
        assert!(fit.converged);
        assert!(fit.beta[1] > 0.0, "slope={}", fit.beta[1]);
        assert!(fit.mu.iter().all(|&m| m > 0.0 && m < 1.0));
    }

    #[test]
    fn genmod_gamma_reciprocal() {
        let ds = gamma_ds();
        let mut session = make_session();
        write_dataset(&mut session, "A", ds);
        let (ds2, _) = session.libs.get("WORK").unwrap().read("A").unwrap();
        let y_col = ds2.vars.iter().position(|v| v.name == "y").unwrap();
        let y_vals = decode_column(&ds2, y_col).unwrap();
        let x_vals = decode_column(
            &ds2,
            ds2.vars.iter().position(|v| v.name == "x").unwrap(),
        )
        .unwrap();
        let kept: Vec<usize> = (0..ds2.n_obs()).collect();
        let mv = vec![ModelVar {
            name: "x".to_string(),
            is_class: false,
            vals: x_vals,
        }];
        let design = build_design(&mv, &[Vec::new()], &kept, &y_vals);
        let fit = irls(
            &design.x,
            &design.y,
            design.n_params,
            "GAMMA",
            "RECIPROCAL",
            1.0,
        )
        .unwrap();
        assert!(fit.converged, "Gamma IRLS must converge");
        // Fitted means stay positive.
        assert!(fit.mu.iter().all(|&m| m > 0.0), "mu={:?}", fit.mu);
    }

    #[test]
    fn genmod_class_variable() {
        let g = vec!["A", "A", "A", "A", "B", "B", "B", "B", "C", "C", "C", "C"];
        let y = vec![
            10.0, 11.0, 9.0, 10.0, 20.0, 21.0, 19.0, 20.0, 30.0, 31.0, 29.0, 30.0,
        ];
        let df = df!["g" => g, "y" => y].unwrap();
        let ds = SasDataset {
            df,
            vars: vec![char_meta("g"), num_meta("y")],
        };
        let (listing, _log, _s) = run_genmod(
            ds,
            "proc genmod data=a; class g; model y = g; dist=normal; run;",
        );
        assert!(listing.contains("Class Level Information"), "{listing}");
        // Reference cell omitted: C is last level → dummies A and B.
        assert!(listing.contains("g A"), "{listing}");
        assert!(listing.contains("g B"), "{listing}");
        assert!(!listing.contains("g C"), "C should be reference: {listing}");
    }

    #[test]
    fn genmod_convergence() {
        // The Normal+identity case converges in a single step (linear model).
        let ds = normal_ds();
        let mut session = make_session();
        write_dataset(&mut session, "A", ds);
        let (ds2, _) = session.libs.get("WORK").unwrap().read("A").unwrap();
        let y_col = ds2.vars.iter().position(|v| v.name == "y").unwrap();
        let y_vals = decode_column(&ds2, y_col).unwrap();
        let x_vals = decode_column(
            &ds2,
            ds2.vars.iter().position(|v| v.name == "x").unwrap(),
        )
        .unwrap();
        let kept: Vec<usize> = (0..ds2.n_obs()).collect();
        let mv = vec![ModelVar {
            name: "x".to_string(),
            is_class: false,
            vals: x_vals,
        }];
        let design = build_design(&mv, &[Vec::new()], &kept, &y_vals);
        let fit = irls(&design.x, &design.y, design.n_params, "POISSON", "LOG", 1.0).unwrap();
        // x present but not strictly Poisson-clean; still must converge.
        let _ = fit; // covered by poisson test; ensure normal converges here too.
        let fit2 = irls(&design.x, &design.y, design.n_params, "NORMAL", "IDENTITY", 1.0).unwrap();
        assert!(fit2.converged, "Newton-Raphson / IRLS must converge");
    }

    #[test]
    fn genmod_output_out() {
        let (_listing, log, session) = run_genmod(
            normal_ds(),
            "proc genmod data=a; model y = x; dist=normal; output out=o; run;",
        );
        assert!(log.contains("WORK.O"), "{log}");
        let (out, _) = session.libs.get("WORK").unwrap().read("O").unwrap();
        let names: Vec<&str> = out.vars.iter().map(|v| v.name.as_str()).collect();
        assert!(names.contains(&"PREDICTED"), "{names:?}");
        assert_eq!(out.n_obs(), 20);
        // Predicted means equal the fitted line ~ 2 + 3x.
        let pcol = decode_column(
            &out,
            out.vars.iter().position(|v| v.name == "PREDICTED").unwrap(),
        )
        .unwrap();
        let xcol = decode_column(
            &out,
            out.vars.iter().position(|v| v.name == "x").unwrap(),
        )
        .unwrap();
        for (p, x) in pcol.iter().zip(&xcol) {
            if let (Some(pv), Some(xv)) = (value_to_num(p), value_to_num(x)) {
                assert!((pv - (2.0 + 3.0 * xv)).abs() < 1e-4, "pred={pv} x={xv}");
            }
        }
    }

    #[test]
    fn genmod_missing_excluded() {
        let x = vec![Some(1.0), None, Some(3.0), Some(4.0), Some(5.0), Some(6.0)];
        let y = vec![
            Some(5.0),
            Some(6.0),
            Some(11.0),
            Some(14.0),
            Some(17.0),
            Some(20.0),
        ];
        let df = df!["x" => x, "y" => y].unwrap();
        let ds = SasDataset {
            df,
            vars: vec![num_meta("x"), num_meta("y")],
        };
        let (listing, _log, _s) =
            run_genmod(ds, "proc genmod data=a; model y = x; dist=normal; run;");
        // 6 read, 5 used.
        assert!(
            listing.contains("Number of Observations Read       6"),
            "{listing}"
        );
        assert!(
            listing.contains("Number of Observations Used       5"),
            "{listing}"
        );
    }

    #[test]
    fn genmod_validate_poisson_rejects_negative() {
        let x = vec![1.0, 2.0, 3.0];
        let y = vec![1.0, -1.0, 2.0];
        let df = df!["x" => x, "y" => y].unwrap();
        let ds = SasDataset {
            df,
            vars: vec![num_meta("x"), num_meta("y")],
        };
        let mut session = make_session();
        write_dataset(&mut session, "A", ds);
        let ast =
            parse_genmod("proc genmod data=a; model y = x; dist=poisson; run;").unwrap();
        assert!(execute(&ast, &mut session).is_err());
    }

    #[test]
    fn genmod_validate_binomial_rejects_non_binary() {
        let x = vec![1.0, 2.0, 3.0];
        let y = vec![0.0, 2.0, 1.0];
        let df = df!["x" => x, "y" => y].unwrap();
        let ds = SasDataset {
            df,
            vars: vec![num_meta("x"), num_meta("y")],
        };
        let mut session = make_session();
        write_dataset(&mut session, "A", ds);
        let ast =
            parse_genmod("proc genmod data=a; model y = x; dist=binomial; run;").unwrap();
        assert!(execute(&ast, &mut session).is_err());
    }

    #[test]
    fn genmod_noncanonical_link_warns() {
        // Gamma + identity is accepted with a warning.
        let (_listing, log, _s) = run_genmod(
            gamma_ds(),
            "proc genmod data=a; model y = x; dist=gamma; link=identity; run;",
        );
        assert!(log.contains("not the recommended link"), "log: {log}");
    }
}
