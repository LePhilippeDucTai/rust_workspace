//! PROC REG — ordinary least squares regression (M25.1).
//!
//! ## Plan d'implémentation (M25.1 — Opus, élevé)
//!
//! Ordinary least squares (OLS) linear regression: one or more MODEL
//! statements fit `y = b0 + b1·x1 + ... + bk·xk` (or without intercept under
//! NOINT). The procedure produces, per MODEL, an Analysis of Variance table,
//! summary fit statistics (Root MSE, R², Adjusted R², Dependent Mean, Coeff
//! Var), and a Parameter Estimates table (estimate, standard error, t value,
//! two-sided p-value, optional confidence limits). An optional OUTPUT
//! statement augments the input data set with predicted and residual columns.
//!
//! ### Grammar (faithful to SAS 9.4, v1 subset)
//! - `PROC REG DATA=ds [OUTEST=ds] [ALPHA=p];`
//!   - DATA= : input data set (default = _LAST_).
//!   - OUTEST= : parameter-estimates output data set. Parsed and recorded but
//!     the dataset emission is **deferred** in v1 (documented limitation).
//!   - ALPHA= : confidence level for CLB (default 0.05).
//! - `MODEL y = x1 x2 ... </ options>;` (REQUIRED, ≥1). Options after `/`:
//!   - NOINT : suppress the intercept.
//!   - CLB : print confidence limits for the parameter estimates.
//!   Multiple MODEL statements are allowed; each is processed in sequence as
//!   its own analysis block (labelled MODEL1, MODEL2, ...).
//! - `OUTPUT OUT=ds P=name R=name;` (optional). `PREDICTED=`/`RESIDUAL=` are
//!   accepted as aliases of `P=`/`R=`. Applies to the LAST model fitted, as in
//!   SAS. One row per ORIGINAL observation; rows dropped from the fit
//!   (listwise-incomplete) receive missing predicted/residual values.
//! - `VAR`, `BY`, `TEST`, `PLOT`, ... : recognized and skipped/deferred via
//!   `skip_to_semi()` recovery. BY-group processing and the TEST statement are
//!   **deferred** in v1 (documented).
//! - Unknown PROC option → parse error (mirrors corr/ttest). Unknown
//!   sub-statement → skip_to_semi recovery.
//!
//! ### Algorithm (OLS via QR)
//! For a model with `p` parameters (intercept + k regressors, or k regressors
//! under NOINT) over `n` listwise-complete observations:
//! 1. Build the design matrix X (n×p): leading column of 1s (unless NOINT)
//!    followed by the regressor columns. Listwise-complete = drop any row with
//!    a missing y OR any missing regressor (NaN ≠ null; specials decode to NaN
//!    via `value_to_num` and are excluded).
//! 2. Solve β minimizing ‖Xβ − y‖² via QR decomposition (`stat::least_squares`).
//! 3. Fitted ŷ = Xβ, residuals e = y − ŷ.
//! 4. SSE = Σe². With intercept: SST = Σ(y − ȳ)² (corrected total), df_total =
//!    n−1. Under NOINT SAS uses the *uncorrected* total SST = Σy² with
//!    df_total = n; v1 implements the corrected case faithfully and documents
//!    the NOINT total as a simplification.
//!    SSR = SST − SSE. df_model = k (regressors, excluding intercept),
//!    df_error = n − p. MSE = SSE/df_error, Root MSE = √MSE.
//! 5. R² = SSR/SST = 1 − SSE/SST; Adj R² = 1 − (1−R²)(n−1)/(n−p).
//! 6. F = (SSR/df_model)/MSE; Pr > F = 1 − F_cdf(F, df_model, df_error).
//! 7. Var(β) = MSE·(X'X)⁻¹ (X'X built manually, inverted via
//!    `stat::invert_matrix`); SE_j = √diag_j; t_j = β_j/SE_j; two-sided
//!    p_j = 2·(1 − student_t_cdf(|t_j|, df_error)); CLB_j = β_j ± t_{1−α/2}·SE_j
//!    using a private bisection t-quantile (mirrors ttest.rs).
//!
//! ### Degenerate cases (clean errors / NOTEs, never a panic)
//! - n ≤ p (not enough observations) → runtime error.
//! - Singular / collinear X'X (rank deficient) → the QR back-substitution or
//!   the inversion surfaces a `Numerical` error which is returned cleanly.
//! - Non-numeric or missing dependent/regressor variable → runtime error.
//!
//! ### Listing structure (per MODEL, faithful, SAS-plausible)
//! 1. "The REG Procedure"
//! 2. "Model: MODELk" and "Dependent Variable: y"
//! 3. "Analysis of Variance" table: Source (Model/Error/Corrected Total), DF,
//!    Sum of Squares, Mean Square, F Value, Pr > F.
//! 4. Fit-summary block: Root MSE / Dependent Mean / Coeff Var on the left,
//!    R-Square / Adj R-Sq on the right (two small two-column tables stacked).
//! 5. "Parameter Estimates" table: Variable, DF, Parameter Estimate, Standard
//!    Error, t Value, Pr > |t|, and (if CLB) the lower/upper confidence limits.
//!    The intercept row is labelled "Intercept".
//!
//! ### v1 deferrals (documented for the orchestrator)
//! - TEST statement (linear hypothesis tests) — recognized, skipped.
//! - BY-group processing — recognized, skipped.
//! - OUTEST= dataset emission — parsed/recorded only.
//! - Collinearity diagnostics (VIF, COLLIN, tolerance) — not implemented.
//! - NOINT uses the corrected SST convention (SAS uses uncorrected); R²/F
//!   under NOINT therefore differ from SAS and are documented as approximate.
//! - Multiple MODEL statements: each fitted independently; OUTPUT binds to the
//!   last model (SAS associates OUTPUT with the preceding MODEL — simplified).
//!
//! ### Tests
//! - `fit_ols` factored so numeric oracles assert directly:
//!   - perfect line y=2x+1 → intercept 1, slope 2, R²=1, SSE=0 (1e-9).
//!   - sashelp.class weight on height → documented SAS estimates (~1e-3).
//!   - multiple regression weight = height age → runs, R² in (0.77, 0.80).
//! - parse tests (MODEL, OUTPUT P=/R=, NOINT, unknown option error).
//! - execute-level test through a Session: listing contains "Parameter
//!   Estimates", "Root MSE", the slope value; OUTPUT OUT= creates the P=/R=
//!   columns.

use crate::ast::DatasetRef;
use crate::dataset::{SasDataset, VarMeta};
use crate::error::{Result, SasError};
use crate::listing::Align;
use crate::missing::value_to_num;
use crate::parser::StatementStream;
use crate::procs::common::decode_column;
use crate::session::Session;
use crate::stat::{f_cdf, invert_matrix, least_squares, student_t_cdf};
use crate::token::TokenKind;
use crate::value::{format_best, Value, VarType};
use polars::prelude::{Column, DataFrame, NamedFrom, Series};

// ───────────────────────── AST ─────────────────────────

#[derive(Debug, Clone)]
pub struct RegAst {
    pub data: Option<DatasetRef>,
    /// OUTEST= dataset (parsed/recorded; emission deferred in v1).
    pub outest: Option<DatasetRef>,
    /// Confidence level for CLB (default 0.05).
    pub alpha: f64,
    /// One or more MODEL statements (REQUIRED, ≥1).
    pub models: Vec<ModelSpec>,
    /// Optional OUTPUT statement.
    pub output: Option<OutputSpec>,
}

#[derive(Debug, Clone)]
pub struct ModelSpec {
    /// Dependent variable name.
    pub depvar: String,
    /// Regressor variable names (in MODEL order).
    pub regressors: Vec<String>,
    /// NOINT: suppress the intercept.
    pub noint: bool,
    /// CLB: print confidence limits for the estimates.
    pub clb: bool,
}

#[derive(Debug, Clone)]
pub struct OutputSpec {
    pub out: DatasetRef,
    /// P= / PREDICTED= column name.
    pub predicted: Option<String>,
    /// R= / RESIDUAL= column name.
    pub residual: Option<String>,
}

// ───────────────────────── parse ─────────────────────────

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

/// Read a numeric option value (e.g. `ALPHA=`).
fn parse_number(ts: &mut StatementStream, opt: &str) -> Result<f64> {
    let tok = ts.peek().clone();
    match tok.kind {
        TokenKind::Num(f) => {
            ts.next();
            Ok(f)
        }
        _ => Err(SasError::parse(
            format!("expected a number after {opt}="),
            tok.span,
        )),
    }
}

/// Parse PROC REG and its statements. Called AFTER "proc reg" was consumed;
/// consumes through `run;`/`quit;`. Mirrors corr/ttest structure.
pub fn parse(ts: &mut StatementStream) -> Result<RegAst> {
    let mut data: Option<DatasetRef> = None;
    let mut outest: Option<DatasetRef> = None;
    let mut alpha = 0.05_f64;

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
        } else if ts.peek().is_kw("outest") {
            ts.next();
            expect_eq(ts, "OUTEST")?;
            outest = Some(ts.parse_dataset_ref()?);
        } else if ts.peek().is_kw("alpha") {
            ts.next();
            expect_eq(ts, "ALPHA")?;
            let val = parse_number(ts, "ALPHA")?;
            if !(val > 0.0 && val < 1.0) {
                return Err(SasError::runtime(format!(
                    "The ALPHA= value {val} must be between 0 and 1."
                )));
            }
            alpha = val;
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
            return Err(SasError::parse(
                "Unexpected token on PROC REG statement.",
                span,
            ));
        }
    }

    // --- sub-statements until run;/quit; ---
    let mut models: Vec<ModelSpec> = Vec::new();
    let mut output: Option<OutputSpec> = None;

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
            let model = parse_model(ts)?;
            ts.expect_semi()?;
            models.push(model);
        } else if ts.peek().is_kw("output") {
            ts.next();
            output = Some(parse_output(ts)?);
            ts.expect_semi()?;
        } else if ts.peek().is_kw("by") {
            // BY processing recognized but deferred in v1.
            ts.next();
            ts.skip_to_semi();
            return Err(SasError::runtime(
                "BY processing is not yet implemented for PROC REG.",
            ));
        } else if ts.peek().is_kw("test") {
            // TEST statement recognized but deferred in v1.
            ts.next();
            ts.skip_to_semi();
        } else {
            // Unknown sub-statement (VAR, PLOT, ...): skip (recovery).
            ts.skip_to_semi();
        }
    }

    if models.is_empty() {
        return Err(SasError::runtime(
            "PROC REG requires at least one MODEL statement.",
        ));
    }

    Ok(RegAst {
        data,
        outest,
        alpha,
        models,
        output,
    })
}

/// Parse a `MODEL y = x1 x2 ... </ noint clb>` body (after `model` consumed,
/// up to but not including the terminating `;`).
fn parse_model(ts: &mut StatementStream) -> Result<ModelSpec> {
    // Dependent variable.
    let dep_tok = ts.peek().clone();
    let Some(depvar) = dep_tok.ident().map(str::to_string) else {
        return Err(SasError::parse(
            "expected a dependent variable name after MODEL",
            dep_tok.span,
        ));
    };
    ts.next();

    if ts.peek().kind != TokenKind::Eq {
        return Err(SasError::parse(
            "expected '=' in MODEL statement",
            ts.peek().span,
        ));
    }
    ts.next();

    // Regressor list, until ';' or '/'.
    let mut regressors: Vec<String> = Vec::new();
    loop {
        if ts.peek().kind == TokenKind::Semi
            || ts.peek().kind == TokenKind::Slash
            || ts.peek().kind == TokenKind::Eof
        {
            break;
        }
        let tok = ts.peek().clone();
        match tok.ident().map(str::to_string) {
            Some(name) => {
                regressors.push(name);
                ts.next();
            }
            None => {
                return Err(SasError::parse(
                    "expected a regressor variable name in MODEL statement",
                    tok.span,
                ));
            }
        }
    }

    if regressors.is_empty() {
        return Err(SasError::parse(
            "MODEL statement requires at least one regressor on the right-hand side",
            ts.peek().span,
        ));
    }

    // Options after '/'.
    let mut noint = false;
    let mut clb = false;
    if ts.peek().kind == TokenKind::Slash {
        ts.next();
        loop {
            if ts.peek().kind == TokenKind::Semi || ts.peek().kind == TokenKind::Eof {
                break;
            }
            if ts.peek().is_kw("noint") {
                ts.next();
                noint = true;
            } else if ts.peek().is_kw("clb") {
                ts.next();
                clb = true;
            } else if let Some(name) = ts.peek().ident().map(str::to_string) {
                let span = ts.peek().span;
                return Err(SasError::parse(
                    format!(
                        "Unexpected MODEL option '{}'.",
                        name.to_uppercase()
                    ),
                    span,
                ));
            } else {
                let span = ts.peek().span;
                return Err(SasError::parse("Unexpected token in MODEL options.", span));
            }
        }
    }

    Ok(ModelSpec {
        depvar,
        regressors,
        noint,
        clb,
    })
}

/// Parse an `OUTPUT OUT=ds P=name R=name` body (after `output` consumed, up to
/// but not including the terminating `;`).
fn parse_output(ts: &mut StatementStream) -> Result<OutputSpec> {
    let mut out: Option<DatasetRef> = None;
    let mut predicted: Option<String> = None;
    let mut residual: Option<String> = None;

    loop {
        if ts.peek().kind == TokenKind::Semi || ts.peek().kind == TokenKind::Eof {
            break;
        }
        if ts.peek().is_kw("out") {
            ts.next();
            expect_eq(ts, "OUT")?;
            out = Some(ts.parse_dataset_ref()?);
        } else if ts.peek().is_kw("p") || ts.peek().is_kw("predicted") || ts.peek().is_kw("pred") {
            ts.next();
            expect_eq(ts, "P")?;
            predicted = Some(parse_ident(ts, "P=")?);
        } else if ts.peek().is_kw("r") || ts.peek().is_kw("residual") {
            ts.next();
            expect_eq(ts, "R")?;
            residual = Some(parse_ident(ts, "R=")?);
        } else if let Some(name) = ts.peek().ident().map(str::to_string) {
            let span = ts.peek().span;
            return Err(SasError::parse(
                format!(
                    "Unexpected OUTPUT option '{}'.",
                    name.to_uppercase()
                ),
                span,
            ));
        } else {
            let span = ts.peek().span;
            return Err(SasError::parse("Unexpected token in OUTPUT statement.", span));
        }
    }

    let out = out.ok_or_else(|| {
        SasError::runtime("The OUTPUT statement of PROC REG requires OUT=.")
    })?;

    Ok(OutputSpec {
        out,
        predicted,
        residual,
    })
}

/// Read a single identifier (a new variable name) for OUTPUT keyword values.
fn parse_ident(ts: &mut StatementStream, opt: &str) -> Result<String> {
    let tok = ts.peek().clone();
    match tok.ident().map(str::to_string) {
        Some(name) => {
            ts.next();
            Ok(name)
        }
        None => Err(SasError::parse(
            format!("expected a variable name after {opt}"),
            tok.span,
        )),
    }
}

// ───────────────────────── numeric core ─────────────────────────

/// The numeric result of fitting an OLS model. `beta[j]` is the coefficient
/// for design-matrix column j (j=0 is the intercept when present). `names[j]`
/// labels each coefficient ("Intercept" or a regressor name).
#[derive(Debug, Clone)]
pub struct RegFit {
    /// Coefficient labels (parallel to beta/se/t/p).
    pub names: Vec<String>,
    /// Coefficient estimates.
    pub beta: Vec<f64>,
    /// Standard errors of the estimates.
    pub se: Vec<f64>,
    /// t statistics.
    pub t: Vec<f64>,
    /// Two-sided p-values.
    pub p: Vec<f64>,
    /// Number of observations used (listwise-complete).
    pub n: usize,
    /// Number of parameters (design columns).
    pub p_params: usize,
    /// Number of regressors (excludes the intercept).
    pub k: usize,
    /// Whether an intercept column was included.
    pub has_intercept: bool,
    pub sse: f64,
    pub sst: f64,
    pub ssr: f64,
    pub df_model: f64,
    pub df_error: f64,
    pub df_total: f64,
    pub mse: f64,
    pub root_mse: f64,
    pub r_square: f64,
    pub adj_r_square: f64,
    pub f_value: f64,
    pub f_pvalue: f64,
    pub dep_mean: f64,
    /// Coefficient of variation = 100·RootMSE/DependentMean.
    pub coeff_var: f64,
    /// Fitted values (one per used observation, in design-row order).
    pub fitted: Vec<f64>,
    /// Residuals (one per used observation, in design-row order).
    pub residuals: Vec<f64>,
}

/// Fit OLS given a design matrix `x` (n×p, intercept column already included if
/// desired), the response `y` (length n), and whether the design has an
/// intercept column (column 0). Returns the full set of estimates and ANOVA
/// quantities. Errors on degenerate input (n ≤ p, singular X'X, zero df_error).
///
/// `names` labels each design column (parallel to x's columns) and is carried
/// into the result for listing.
pub fn fit_ols(x: &[Vec<f64>], y: &[f64], has_intercept: bool, names: &[String]) -> Result<RegFit> {
    let n = x.len();
    if n == 0 || y.len() != n {
        return Err(SasError::runtime(
            "PROC REG: empty or mismatched design matrix.",
        ));
    }
    let p = x[0].len();
    if p == 0 {
        return Err(SasError::runtime("PROC REG: model has no parameters."));
    }
    if n <= p {
        return Err(SasError::runtime(format!(
            "PROC REG: not enough observations ({n}) for {p} parameters."
        )));
    }

    // Solve β via QR least squares.
    let beta = least_squares(x, y)?;

    // Fitted values and residuals.
    let mut fitted = vec![0.0; n];
    let mut residuals = vec![0.0; n];
    let mut sse = 0.0;
    for i in 0..n {
        let mut yhat = 0.0;
        for j in 0..p {
            yhat += x[i][j] * beta[j];
        }
        fitted[i] = yhat;
        let e = y[i] - yhat;
        residuals[i] = e;
        sse += e * e;
    }

    let nf = n as f64;
    let dep_mean = y.iter().sum::<f64>() / nf;

    // Corrected total sum of squares (implemented faithfully for the intercept
    // case; under NOINT SAS uses the uncorrected Σy² — documented simplification
    // where we keep the corrected form).
    let sst: f64 = y.iter().map(|&yi| (yi - dep_mean) * (yi - dep_mean)).sum();
    let ssr = (sst - sse).max(0.0);

    // Degrees of freedom. df_model = number of regressors (excludes intercept).
    let k = if has_intercept { p - 1 } else { p };
    let df_model = k as f64;
    let df_error = (n - p) as f64;
    let df_total = (n - 1) as f64;

    if df_error <= 0.0 {
        return Err(SasError::runtime(
            "PROC REG: zero error degrees of freedom (n equals the number of parameters).",
        ));
    }

    let mse = sse / df_error;
    let root_mse = mse.sqrt();

    let r_square = if sst > 0.0 { 1.0 - sse / sst } else { 0.0 };
    let adj_r_square = if df_error > 0.0 {
        1.0 - (1.0 - r_square) * df_total / df_error
    } else {
        0.0
    };

    let f_value = if df_model > 0.0 && mse > 0.0 {
        (ssr / df_model) / mse
    } else {
        0.0
    };
    let f_pvalue = if df_model > 0.0 && f_value > 0.0 {
        (1.0 - f_cdf(f_value, df_model, df_error)).clamp(0.0, 1.0)
    } else {
        1.0
    };

    // (X'X)⁻¹ for coefficient variances. Build X'X manually then invert.
    let mut xtx = vec![vec![0.0; p]; p];
    for a in 0..p {
        for b in 0..p {
            let mut s = 0.0;
            for row in x.iter() {
                s += row[a] * row[b];
            }
            xtx[a][b] = s;
        }
    }
    let xtx_inv = invert_matrix(&xtx)?;

    let mut se = vec![0.0; p];
    let mut tvals = vec![0.0; p];
    let mut pvals = vec![1.0; p];
    for j in 0..p {
        let var_j = mse * xtx_inv[j][j];
        let se_j = if var_j > 0.0 { var_j.sqrt() } else { 0.0 };
        se[j] = se_j;
        if se_j > 0.0 {
            let t = beta[j] / se_j;
            tvals[j] = t;
            pvals[j] = (2.0 * (1.0 - student_t_cdf(t.abs(), df_error))).clamp(0.0, 1.0);
        } else {
            tvals[j] = 0.0;
            pvals[j] = 1.0;
        }
    }

    let coeff_var = if dep_mean != 0.0 {
        100.0 * root_mse / dep_mean
    } else {
        0.0
    };

    Ok(RegFit {
        names: names.to_vec(),
        beta,
        se,
        t: tvals,
        p: pvals,
        n,
        p_params: p,
        k,
        has_intercept,
        sse,
        sst,
        ssr,
        df_model,
        df_error,
        df_total,
        mse,
        root_mse,
        r_square,
        adj_r_square,
        f_value,
        f_pvalue,
        dep_mean,
        coeff_var,
        fitted,
        residuals,
    })
}

/// Student-t quantile (inverse CDF): the value `q` with `P(T_df <= q) = pr`,
/// for `0 < pr < 1` and `df > 0`. Solved by bisection on the monotone t-CDF.
/// Kept private here — see PLAN: do NOT add a new pub fn to `stat`. Mirrors
/// `ttest::t_quantile`.
fn t_quantile(pr: f64, df: f64) -> f64 {
    if !(0.0..=1.0).contains(&pr) {
        return f64::NAN;
    }
    if pr == 0.5 {
        return 0.0;
    }
    let upper = pr > 0.5;
    let target = if upper { pr } else { 1.0 - pr };
    let mut lo = 0.0_f64;
    let mut hi = 1.0_f64;
    while student_t_cdf(hi, df) < target && hi < 1e12 {
        hi *= 2.0;
    }
    for _ in 0..200 {
        let mid = 0.5 * (lo + hi);
        let c = student_t_cdf(mid, df);
        if c < target {
            lo = mid;
        } else {
            hi = mid;
        }
        if (hi - lo) <= 1e-12 * (1.0 + hi.abs()) {
            break;
        }
    }
    let q = 0.5 * (lo + hi);
    if upper {
        q
    } else {
        -q
    }
}

// ───────────────────────── formatting ─────────────────────────

/// Format a numeric statistic SAS-style (best width 12).
fn fmt_num(v: f64) -> String {
    format_best(v, 12)
}

/// Format an F or t value to 2 decimals (SAS).
fn fmt_2dp(v: f64) -> String {
    format!("{v:.2}")
}

/// Format a p-value SAS-style: `<.0001`, else 4 decimals.
fn fmt_p(p: f64) -> String {
    if p < 0.0001 {
        "<.0001".to_string()
    } else {
        format!("{p:.4}")
    }
}

/// Write a centered line within LINESIZE (mirrors corr/ttest::centered).
fn centered(session: &mut Session, text: &str) {
    let ls = session.listing.ls();
    let pad = ls.saturating_sub(text.len()) / 2;
    session
        .listing
        .write_line(&format!("{}{}", " ".repeat(pad), text));
}

// ───────────────────────── execute ─────────────────────────

/// Resolve `data=` or `_LAST_` into a concrete DatasetRef (mirrors
/// corr/ttest::resolve_input).
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

/// Execute PROC REG: fit each MODEL, emit the listing, and (optionally) the
/// OUTPUT OUT= dataset for the last model.
pub fn execute(ast: &RegAst, session: &mut Session) -> Result<()> {
    let in_ref = resolve_input(ast, session)?;
    let in_libref = in_ref.libref_or_work();
    let in_table = in_ref.name.to_uppercase();

    let provider = session.libs.get(&in_libref)?;
    let (ds, notes) = provider.read(&in_table)?;
    for note in notes {
        session.log.forward(&note);
    }

    let n_obs = ds.n_obs();

    // Resolve a numeric variable name to a column index (clean error otherwise).
    let resolve_num = |nm: &str| -> Result<usize> {
        let i = ds
            .vars
            .iter()
            .position(|m| m.name.eq_ignore_ascii_case(nm))
            .ok_or_else(|| {
                SasError::runtime(format!("Variable {} not found.", nm.to_uppercase()))
            })?;
        if ds.vars[i].ty != VarType::Num {
            return Err(SasError::runtime(format!(
                "Variable {} is not numeric.",
                nm.to_uppercase()
            )));
        }
        Ok(i)
    };

    session.listing.page_header();
    centered(session, "The REG Procedure");
    session.listing.blank();

    // Fit each MODEL; remember the last fit + its row mask for OUTPUT.
    let mut last_fit_rows: Option<(RegFit, Vec<Option<usize>>)> = None;

    for (mi, model) in ast.models.iter().enumerate() {
        // Resolve & decode the dependent and regressor columns once each.
        let dep_idx = resolve_num(&model.depvar)?;
        let reg_idx: Vec<usize> = model
            .regressors
            .iter()
            .map(|nm| resolve_num(nm))
            .collect::<Result<_>>()?;

        let dep_vals = decode_column(&ds, dep_idx)?;
        let reg_vals: Vec<Vec<Value>> = reg_idx
            .iter()
            .map(|&c| decode_column(&ds, c))
            .collect::<Result<_>>()?;

        // Build listwise-complete design matrix. `row_map[i]` (length used) maps
        // the i-th used design row to its original observation index, so OUTPUT
        // can scatter predicted/residual back into place.
        let mut x: Vec<Vec<f64>> = Vec::new();
        let mut y: Vec<f64> = Vec::new();
        let mut used_orig: Vec<usize> = Vec::new();
        for r in 0..n_obs {
            let yv = match value_to_num(&dep_vals[r]) {
                Some(v) if !v.is_nan() => v,
                _ => continue,
            };
            let mut row: Vec<f64> = Vec::with_capacity(reg_idx.len() + 1);
            if !model.noint {
                row.push(1.0);
            }
            let mut complete = true;
            for col in &reg_vals {
                match value_to_num(&col[r]) {
                    Some(v) if !v.is_nan() => row.push(v),
                    _ => {
                        complete = false;
                        break;
                    }
                }
            }
            if !complete {
                continue;
            }
            x.push(row);
            y.push(yv);
            used_orig.push(r);
        }

        // Coefficient labels.
        let mut names: Vec<String> = Vec::new();
        if !model.noint {
            names.push("Intercept".to_string());
        }
        for &c in &reg_idx {
            names.push(ds.vars[c].name.clone());
        }

        let fit = fit_ols(&x, &y, !model.noint, &names)?;

        emit_model_listing(session, model, mi + 1, &ds.vars[dep_idx].name, &fit, ast.alpha);

        // Build the per-original-observation row map for OUTPUT (used row index
        // for originals that were in the fit, None otherwise).
        if ast.output.is_some() {
            let mut row_for_orig: Vec<Option<usize>> = vec![None; n_obs];
            for (used_i, &orig) in used_orig.iter().enumerate() {
                row_for_orig[orig] = Some(used_i);
            }
            last_fit_rows = Some((fit, row_for_orig));
        }
    }

    // --- OUTPUT OUT= (predicted / residual), bound to the last model ---
    if let Some(out_spec) = &ast.output {
        if let Some((fit, row_for_orig)) = &last_fit_rows {
            let out_ds = build_output_dataset(&ds, fit, row_for_orig, out_spec)?;
            write_dataset(session, &out_spec.out, out_ds)?;
        }
    }

    // OUTEST= emission deferred in v1 (parsed/recorded only).
    let _ = &ast.outest;

    Ok(())
}

/// Emit the listing for one fitted model: ANOVA, fit summary, Parameter
/// Estimates.
fn emit_model_listing(
    session: &mut Session,
    model: &ModelSpec,
    model_no: usize,
    depname: &str,
    fit: &RegFit,
    alpha: f64,
) {
    centered(session, &format!("Model: MODEL{model_no}"));
    centered(session, &format!("Dependent Variable: {depname}"));
    session.listing.blank();

    // --- Analysis of Variance ---
    centered(session, "Analysis of Variance");
    session.listing.blank();
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
    let ms_model = if fit.df_model > 0.0 {
        fit.ssr / fit.df_model
    } else {
        0.0
    };
    let rows: Vec<Vec<String>> = vec![
        vec![
            "Model".into(),
            fmt_df(fit.df_model),
            fmt_num(fit.ssr),
            fmt_num(ms_model),
            fmt_2dp(fit.f_value),
            fmt_p(fit.f_pvalue),
        ],
        vec![
            "Error".into(),
            fmt_df(fit.df_error),
            fmt_num(fit.sse),
            fmt_num(fit.mse),
            String::new(),
            String::new(),
        ],
        vec![
            "Corrected Total".into(),
            fmt_df(fit.df_total),
            fmt_num(fit.sst),
            String::new(),
            String::new(),
            String::new(),
        ],
    ];
    session.listing.write_table(&headers, &aligns, &rows);
    session.listing.blank();

    // --- Fit-summary block (two stacked two-column tables) ---
    let sum_aligns = vec![Align::Left, Align::Right];
    let left_rows: Vec<Vec<String>> = vec![
        vec!["Root MSE".into(), fmt_num(fit.root_mse)],
        vec!["Dependent Mean".into(), fmt_num(fit.dep_mean)],
        vec!["Coeff Var".into(), fmt_num(fit.coeff_var)],
    ];
    session
        .listing
        .write_table(&[String::new(), String::new()], &sum_aligns, &left_rows);
    let right_rows: Vec<Vec<String>> = vec![
        vec!["R-Square".into(), format!("{:.4}", fit.r_square)],
        vec!["Adj R-Sq".into(), format!("{:.4}", fit.adj_r_square)],
    ];
    session
        .listing
        .write_table(&[String::new(), String::new()], &sum_aligns, &right_rows);
    session.listing.blank();

    // --- Parameter Estimates ---
    centered(session, "Parameter Estimates");
    session.listing.blank();
    let mut pheaders: Vec<String> = vec![
        "Variable".into(),
        "DF".into(),
        "Parameter Estimate".into(),
        "Standard Error".into(),
        "t Value".into(),
        "Pr > |t|".into(),
    ];
    let mut paligns = vec![
        Align::Left,
        Align::Right,
        Align::Right,
        Align::Right,
        Align::Right,
        Align::Right,
    ];
    if model.clb {
        // Confidence-limit columns. Header reflects the 100(1-α)% level.
        pheaders.push("Lower CL".into());
        pheaders.push("Upper CL".into());
        paligns.push(Align::Right);
        paligns.push(Align::Right);
    }

    // Critical t for the 100(1-α)% confidence limits (CLB only).
    let tcrit = t_quantile(1.0 - alpha / 2.0, fit.df_error);
    let mut prows: Vec<Vec<String>> = Vec::with_capacity(fit.p_params);
    for j in 0..fit.p_params {
        let mut row = vec![
            fit.names[j].clone(),
            "1".to_string(),
            fmt_num(fit.beta[j]),
            fmt_num(fit.se[j]),
            fmt_2dp(fit.t[j]),
            fmt_p(fit.p[j]),
        ];
        if model.clb {
            let lower = fit.beta[j] - tcrit * fit.se[j];
            let upper = fit.beta[j] + tcrit * fit.se[j];
            row.push(fmt_num(lower));
            row.push(fmt_num(upper));
        }
        prows.push(row);
    }
    session.listing.write_table(&pheaders, &paligns, &prows);
    session.listing.blank();
}

/// Format degrees of freedom: integer when whole, else 2 decimals.
fn fmt_df(df: f64) -> String {
    if df.fract().abs() < 1e-9 {
        format!("{}", df.round() as i64)
    } else {
        format!("{df:.2}")
    }
}

// ───────────────────────── OUTPUT OUT= dataset ─────────────────────────

/// Build the OUTPUT OUT= dataset: all input columns plus the requested P=/R=
/// columns (one row per original observation; rows excluded from the fit get
/// missing predicted/residual). Mirrors corr/ttest dataset-construction idioms.
fn build_output_dataset(
    ds: &SasDataset,
    fit: &RegFit,
    row_for_orig: &[Option<usize>],
    out_spec: &OutputSpec,
) -> Result<SasDataset> {
    let mut columns: Vec<Column> = Vec::new();
    let mut vars: Vec<VarMeta> = Vec::new();

    // Copy every input column verbatim.
    for (i, c) in ds.df.get_columns().iter().enumerate() {
        columns.push(c.clone());
        vars.push(ds.vars[i].clone());
    }

    // Predicted column.
    if let Some(pname) = &out_spec.predicted {
        let col: Vec<Option<f64>> = row_for_orig
            .iter()
            .map(|m| m.map(|i| fit.fitted[i]))
            .collect();
        columns.push(Series::new(pname.as_str().into(), col).into());
        vars.push(num_var_meta(pname));
    }
    // Residual column.
    if let Some(rname) = &out_spec.residual {
        let col: Vec<Option<f64>> = row_for_orig
            .iter()
            .map(|m| m.map(|i| fit.residuals[i]))
            .collect();
        columns.push(Series::new(rname.as_str().into(), col).into());
        vars.push(num_var_meta(rname));
    }

    let df = DataFrame::new(columns)?;
    Ok(SasDataset { df, vars })
}

/// Persist a built dataset to `target`, update `_LAST_`, emit the creation
/// NOTE (mirrors corr/ttest::write_out_dataset).
fn write_dataset(session: &mut Session, target: &DatasetRef, out_ds: SasDataset) -> Result<()> {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dataset::{SasDataset, VarMeta};
    use crate::session::Session;
    use crate::source::SourceFile;
    use crate::value::VarType;
    use polars::df;
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

    fn write_dataset_t(session: &mut Session, table: &str, ds: SasDataset) {
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

    /// Build a design matrix from a single regressor with an intercept column.
    fn design_simple(xs: &[f64]) -> Vec<Vec<f64>> {
        xs.iter().map(|&x| vec![1.0, x]).collect()
    }

    /// The 19 sashelp.class (height, weight) pairs (Alfred .. William).
    fn class_height_weight() -> (Vec<f64>, Vec<f64>) {
        let height = vec![
            69.0, 56.5, 65.3, 62.8, 63.5, 57.3, 59.8, 62.5, 62.5, 59.0, 51.3, 64.3, 56.3, 66.5,
            72.0, 64.8, 67.0, 57.5, 66.5,
        ];
        let weight = vec![
            112.5, 84.0, 98.0, 102.5, 102.5, 83.0, 84.5, 112.5, 84.0, 99.5, 50.5, 90.0, 77.0,
            112.0, 150.0, 128.0, 133.0, 85.0, 112.0,
        ];
        (height, weight)
    }

    fn class_age() -> Vec<f64> {
        vec![
            14.0, 13.0, 13.0, 14.0, 14.0, 12.0, 12.0, 15.0, 13.0, 12.0, 11.0, 14.0, 12.0, 15.0,
            16.0, 12.0, 15.0, 11.0, 15.0,
        ]
    }

    // ───────────── parse tests ─────────────

    #[test]
    fn parse_minimal_model() {
        let ast = parse_reg("proc reg data=a; model y = x; run;").unwrap();
        assert_eq!(ast.data.as_ref().unwrap().name, "a");
        assert_eq!(ast.models.len(), 1);
        assert_eq!(ast.models[0].depvar, "y");
        assert_eq!(ast.models[0].regressors, vec!["x"]);
        assert!(!ast.models[0].noint);
        assert!(!ast.models[0].clb);
        assert!(ast.output.is_none());
    }

    #[test]
    fn parse_multiple_regressors_and_options() {
        let ast =
            parse_reg("proc reg data=a alpha=0.10; model y = x1 x2 x3 / noint clb; run;").unwrap();
        assert!((ast.alpha - 0.10).abs() < 1e-12);
        assert_eq!(ast.models[0].regressors, vec!["x1", "x2", "x3"]);
        assert!(ast.models[0].noint);
        assert!(ast.models[0].clb);
    }

    #[test]
    fn parse_output_p_r() {
        let ast =
            parse_reg("proc reg data=a; model y = x; output out=b p=yhat r=resid; run;").unwrap();
        let out = ast.output.as_ref().unwrap();
        assert_eq!(out.out.name, "b");
        assert_eq!(out.predicted.as_deref(), Some("yhat"));
        assert_eq!(out.residual.as_deref(), Some("resid"));
    }

    #[test]
    fn parse_output_aliases() {
        let ast = parse_reg(
            "proc reg data=a; model y = x; output out=b predicted=ph residual=rr; run;",
        )
        .unwrap();
        let out = ast.output.as_ref().unwrap();
        assert_eq!(out.predicted.as_deref(), Some("ph"));
        assert_eq!(out.residual.as_deref(), Some("rr"));
    }

    #[test]
    fn parse_unknown_proc_option_errors() {
        let r = parse_reg("proc reg data=a bogus; model y = x; run;");
        assert!(r.is_err());
        assert!(r.err().unwrap().to_string().contains("BOGUS"));
    }

    #[test]
    fn parse_unknown_model_option_errors() {
        let r = parse_reg("proc reg data=a; model y = x / bogus; run;");
        assert!(r.is_err());
        assert!(r.err().unwrap().to_string().contains("BOGUS"));
    }

    #[test]
    fn parse_requires_a_model() {
        let r = parse_reg("proc reg data=a; run;");
        assert!(r.is_err());
        assert!(r
            .err()
            .unwrap()
            .to_string()
            .contains("at least one MODEL"));
    }

    #[test]
    fn parse_multiple_models() {
        let ast =
            parse_reg("proc reg data=a; model y = x; model z = w v; run;").unwrap();
        assert_eq!(ast.models.len(), 2);
        assert_eq!(ast.models[1].depvar, "z");
        assert_eq!(ast.models[1].regressors, vec!["w", "v"]);
    }

    // ───────────── Oracle 1: perfect linear fit ─────────────

    #[test]
    fn oracle_perfect_fit() {
        // y = 2x + 1 exactly.
        let x = [1.0, 2.0, 3.0, 4.0];
        let y = vec![3.0, 5.0, 7.0, 9.0];
        let design = design_simple(&x);
        let names = vec!["Intercept".into(), "x".into()];
        let fit = fit_ols(&design, &y, true, &names).unwrap();
        assert!((fit.beta[0] - 1.0).abs() < 1e-9, "intercept={}", fit.beta[0]);
        assert!((fit.beta[1] - 2.0).abs() < 1e-9, "slope={}", fit.beta[1]);
        assert!((fit.r_square - 1.0).abs() < 1e-9, "R2={}", fit.r_square);
        assert!(fit.sse.abs() < 1e-9, "SSE={}", fit.sse);
        assert_eq!(fit.n, 4);
        assert_eq!(fit.df_model, 1.0);
        assert_eq!(fit.df_error, 2.0);
    }

    // ───────────── Oracle 2: sashelp.class weight on height ─────────────

    #[test]
    fn oracle_class_weight_on_height() {
        let (height, weight) = class_height_weight();
        let design = design_simple(&height);
        let names = vec!["Intercept".into(), "Height".into()];
        let fit = fit_ols(&design, &weight, true, &names).unwrap();

        // Documented SAS estimates.
        assert!(
            (fit.beta[0] - (-143.02692)).abs() < 1e-3,
            "intercept={}",
            fit.beta[0]
        );
        assert!((fit.beta[1] - 3.89903).abs() < 1e-3, "slope={}", fit.beta[1]);
        assert!((fit.se[0] - 32.27459).abs() < 1e-2, "se0={}", fit.se[0]);
        assert!((fit.se[1] - 0.51609).abs() < 1e-3, "se1={}", fit.se[1]);
        assert!((fit.t[1] - 7.55).abs() < 1e-2, "t1={}", fit.t[1]);
        assert!((fit.r_square - 0.7705).abs() < 1e-3, "R2={}", fit.r_square);
        assert!(
            (fit.adj_r_square - 0.7570).abs() < 1e-3,
            "AdjR2={}",
            fit.adj_r_square
        );
        assert!(
            (fit.root_mse - 11.22625).abs() < 1e-3,
            "RootMSE={}",
            fit.root_mse
        );
        assert!(
            (fit.dep_mean - 100.02632).abs() < 1e-3,
            "DepMean={}",
            fit.dep_mean
        );
        assert!((fit.f_value - 57.08).abs() < 1e-1, "F={}", fit.f_value);

        // ANOVA sums of squares.
        assert!((fit.ssr - 7193.24912).abs() < 1e-1, "SSR={}", fit.ssr);
        assert!((fit.sse - 2142.48772).abs() < 1e-1, "SSE={}", fit.sse);
        assert!((fit.sst - 9335.73684).abs() < 1e-1, "SST={}", fit.sst);

        // Internal consistency.
        assert!((fit.ssr + fit.sse - fit.sst).abs() < 1e-6);
        assert_eq!(fit.df_model, 1.0);
        assert_eq!(fit.df_error, 17.0);
        assert_eq!(fit.df_total, 18.0);

        // Two-sided p-values.
        assert!((fit.p[0] - 0.0004).abs() < 5e-4, "p0={}", fit.p[0]);
        assert!(fit.p[1] < 0.0001, "p1={}", fit.p[1]);
    }

    // ───────────── Oracle 3: multiple regression ─────────────

    #[test]
    fn oracle_multiple_regression() {
        let (height, weight) = class_height_weight();
        let age = class_age();
        // Design: intercept, height, age.
        let design: Vec<Vec<f64>> = (0..height.len())
            .map(|i| vec![1.0, height[i], age[i]])
            .collect();
        let names = vec!["Intercept".into(), "Height".into(), "Age".into()];
        let fit = fit_ols(&design, &weight, true, &names).unwrap();
        assert_eq!(fit.n, 19);
        assert_eq!(fit.df_model, 2.0);
        assert_eq!(fit.df_error, 16.0);
        assert!(
            fit.r_square > 0.77 && fit.r_square < 0.80,
            "R2={}",
            fit.r_square
        );
        // Internal consistency.
        assert!((fit.ssr + fit.sse - fit.sst).abs() < 1e-6);
    }

    #[test]
    fn t_quantile_matches_known() {
        // 97.5th percentile of t_17 ≈ 2.10982.
        let q = t_quantile(0.975, 17.0);
        assert!((q - 2.10982).abs() < 1e-3, "q={q}");
    }

    #[test]
    fn fit_errors_on_too_few_obs() {
        // 2 obs, 2 parameters → n <= p.
        let x = vec![vec![1.0, 1.0], vec![1.0, 2.0]];
        let y = vec![1.0, 2.0];
        let names = vec!["Intercept".into(), "x".into()];
        assert!(fit_ols(&x, &y, true, &names).is_err());
    }

    // ───────────── execute-level test ─────────────

    #[test]
    fn execute_listing_and_output() {
        let mut session = make_session();
        let (height, weight) = class_height_weight();
        let df = df![
            "height" => height.clone(),
            "weight" => weight.clone(),
        ]
        .unwrap();
        let ds = SasDataset {
            df,
            vars: vec![num_meta("height"), num_meta("weight")],
        };
        write_dataset_t(&mut session, "CLASS", ds);

        let ast = parse_reg(
            "proc reg data=class; model weight = height; output out=pred p=yhat r=resid; run;",
        )
        .unwrap();
        execute(&ast, &mut session).unwrap();

        let listing = session.listing.into_string();
        assert!(
            listing.contains("Parameter Estimates"),
            "listing: {listing}"
        );
        assert!(listing.contains("Root MSE"), "listing: {listing}");
        assert!(listing.contains("Analysis of Variance"), "listing: {listing}");
        // The slope value should appear (to 5 dp via format_best).
        assert!(listing.contains("3.89903"), "listing: {listing}");
        assert!(listing.contains("The REG Procedure"), "listing: {listing}");

        // OUTPUT OUT= dataset has the P=/R= columns.
        let (out_ds, _notes) = session.libs.get("WORK").unwrap().read("PRED").unwrap();
        let names: Vec<String> = out_ds.vars.iter().map(|v| v.name.clone()).collect();
        assert!(names.contains(&"yhat".to_string()), "vars: {names:?}");
        assert!(names.contains(&"resid".to_string()), "vars: {names:?}");
        assert_eq!(out_ds.n_obs(), 19);

        let log = session.log.into_string();
        assert!(
            log.contains("The data set WORK.PRED has 19 observations"),
            "log: {log}"
        );
    }

    #[test]
    fn execute_missing_excluded_rows_get_missing_output() {
        let mut session = make_session();
        // One row has a missing regressor → dropped from the fit, missing P/R.
        let df = df![
            "x" => [1.0_f64, 2.0, 3.0, 4.0, f64::NAN],
            "y" => [3.0_f64, 5.0, 7.0, 9.0, 100.0],
        ]
        .unwrap();
        let ds = SasDataset {
            df,
            vars: vec![num_meta("x"), num_meta("y")],
        };
        write_dataset_t(&mut session, "D", ds);

        let ast = parse_reg(
            "proc reg data=d; model y = x; output out=o p=ph r=rr; run;",
        )
        .unwrap();
        execute(&ast, &mut session).unwrap();

        let (out_ds, _notes) = session.libs.get("WORK").unwrap().read("O").unwrap();
        assert_eq!(out_ds.n_obs(), 5);
        let ph_idx = out_ds
            .vars
            .iter()
            .position(|v| v.name == "ph")
            .unwrap();
        let ph = decode_column(&out_ds, ph_idx).unwrap();
        // First four predicted values present, last is missing.
        assert!(value_to_num(&ph[0]).map(|v| !v.is_nan()).unwrap_or(false));
        assert!(ph[4].is_missing());
    }
}
