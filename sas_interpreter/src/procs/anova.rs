//! PROC ANOVA — analysis of variance for balanced designs (M25.2).
//!
//! ## Plan d'implémentation (M25.2 — Opus, élevé)
//!
//! PROC ANOVA fits a linear model whose right-hand side is composed entirely of
//! *classification* (factor) effects — main effects, their crossings (`a*b`),
//! and bars (`a|b`) — and partitions the corrected total sum of squares of a
//! numeric dependent variable into a Model component, an Error (residual)
//! component, and one component per model effect. It is the balanced-design
//! sibling of PROC GLM: ANOVA assumes (and is only exact for) balanced data,
//! where every cell of the factorial design holds the same number of
//! observations. For balanced designs the sequential (Type I) and the
//! partial (Type III) sums of squares coincide, so a single decomposition
//! serves both. This crate reports the Type I (sequential) decomposition and
//! labels the per-effect column "Anova SS" exactly as SAS does, documenting
//! that for balanced data it equals Type III.
//!
//! ### Grammar (faithful to SAS 9.4, v1 subset)
//! - `PROC ANOVA DATA=ds [ALPHA=p];`
//!   - DATA= : input data set (default = _LAST_).
//!   - ALPHA= : confidence level (parsed/recorded; used only for future
//!     interval output — accepted for parity with PROC REG / GLM).
//!   - Unknown PROC option → parse error (mirrors corr/ttest/reg).
//! - `CLASS v1 v2 ...;` (REQUIRED). The classification variables. They may be
//!   numeric or character; their distinct levels are enumerated and ordered by
//!   `Value::sas_cmp` (the SAS total order; trailing blanks ignored for char).
//! - `MODEL y = effects;` (REQUIRED). One dependent `y` (numeric). The effects
//!   are built from CLASS variables:
//!   - a bare CLASS name `a` → a main effect.
//!   - `a*b` (and `a*b*c`, ...) → an interaction / crossed effect.
//!   - `a|b` → the bar operator, expanded to `a b a*b`; `a|b|c` expands to all
//!     2^k − 1 subsets. Bars are expanded at parse time.
//!   Multiple dependents (`MODEL y1 y2 = ...`) → v1 processes the FIRST
//!   dependent and emits a NOTE that the remaining dependents are deferred.
//! - `MEANS effect </ options>;` (optional, repeatable). Prints the mean of `y`
//!   for each level (main effect) or each cell (crossed effect) of the
//!   requested effect(s). Multiple-comparison options (TUKEY, BON, DUNCAN,
//!   SCHEFFE, SNK, LSD, ...) are recognized and DEFERRED: plain means are
//!   printed and a NOTE records the deferral. No panic.
//! - `BY` : recognized and deferred (clean runtime error).
//! - Unknown sub-statement → `skip_to_semi()` recovery.
//!
//! ### Algorithm (design matrix + sequential SS)
//! Over the listwise-complete observation set (drop any row whose `y` is
//! missing OR whose value for any CLASS variable used by the model is missing):
//! 1. For each CLASS variable, enumerate the distinct non-missing levels in
//!    `Value::sas_cmp` order. A factor with `L` levels is **reference-cell**
//!    (full-rank) dummy coded into `L − 1` indicator columns: the indicator for
//!    level `ℓ` (ℓ = 1..L−1, skipping the first/reference level) is 1 when the
//!    observation is at level ℓ. An interaction `a*b` contributes the
//!    `(La−1)(Lb−1)` pairwise products of its components' indicator columns
//!    (generalized to k-way crossings as the full product set).
//! 2. The FULL design matrix is `[intercept | effect₁ cols | … | effect_m cols]`
//!    (intercept always present). Fit via `least_squares` (QR OLS) and form the
//!    residual sum of squares SSE_full. `rank(full) = 1 + Σ df_effect` for a
//!    full-rank coding, so `df_error = n − rank(full)`.
//! 3. SST = Σ(y − ȳ)² (corrected). SS_model = SST − SSE_full;
//!    `df_model = Σ df_effect` (all model columns, excluding the intercept).
//! 4. **Per-effect Type I (sequential) SS.** Process effects in MODEL order.
//!    Let `SSE(k)` be the residual SS of the model containing the intercept and
//!    the first `k` effects. Then `SS_effect_k = SSE(k−1) − SSE(k)` — the
//!    reduction in residual SS from adding effect `k` to everything before it.
//!    `Σ SS_effect = SS_model` exactly. For balanced designs the effects are
//!    mutually orthogonal so this order-dependent decomposition is also the
//!    partial (Type III) decomposition.
//! 5. F statistics. Model: `F = MS_model / MS_error`,
//!    `Pr>F = 1 − f_cdf(F, df_model, df_error)`. Each effect:
//!    `F = MS_effect / MS_error` with `Pr>F = 1 − f_cdf(F, df_effect, df_error)`
//!    (MS_error is the FULL-model error mean square — the ANOVA convention).
//! 6. R² = SS_model / SST; Coeff Var = 100·RootMSE/ȳ; Root MSE = √MS_error.
//!
//! ### Degenerate cases (clean errors / NOTEs, never a panic)
//! - empty observation set, or a CLASS variable with a single level, or
//!   `n ≤ rank(full)` (zero/negative error df) → clean runtime error.
//! - An empty cell (a crossing whose product columns make the design rank
//!   deficient / unbalanced) surfaces as a `Numerical` error from the QR solve
//!   and is returned cleanly.
//!
//! ### Listing structure (faithful to SAS PROC ANOVA)
//! 1. "The ANOVA Procedure".
//! 2. "Class Level Information": Class / Levels / Values for each CLASS var.
//! 3. "Number of Observations Read" / "Number of Observations Used".
//! 4. Per dependent: "Dependent Variable: y", then
//!    - the overall ANOVA table (Source = Model / Error / Corrected Total; DF;
//!      Sum of Squares; Mean Square; F Value; Pr > F),
//!    - a summary line (R-Square, Coeff Var, Root MSE, y Mean),
//!    - the per-effect table (Source = each effect; DF; "Anova SS";
//!      Mean Square; F Value; Pr > F).
//! 5. MEANS tables if requested (Level(s) / N / Mean of y per cell).
//!
//! ### SS-type convention
//! Type I (sequential) SS are reported under the SAS column label "Anova SS".
//! For balanced designs Type I = Type III; this equality is documented here and
//! is the reason PROC ANOVA is restricted to balanced data.
//!
//! ### v1 deferrals (documented for the orchestrator)
//! - Multiple-comparison procedures (TUKEY/BON/DUNCAN/SCHEFFE/SNK/LSD/…) —
//!   recognized on MEANS, plain means printed, deferral NOTE.
//! - Unbalanced data / true Type III SS — out of scope (use PROC GLM); ANOVA
//!   assumes balance and reports Type I = Type III.
//! - The `a|b` bar is expanded; nested effects `a(b)` are NOT supported.
//! - Multiple dependent variables — first processed, rest deferred with a NOTE.
//! - BY-group processing — recognized, deferred (clean error).
//! - ALPHA= is parsed/recorded only.
//!
//! ### Tests
//! Pure helpers `oneway_anova` and `sse_of_design` are factored so the numeric
//! oracles assert directly, plus an execute-level test through a Session and
//! the parse-error matrix.

use crate::ast::DatasetRef;
use crate::error::{Result, SasError};
use crate::listing::Align;
use crate::missing::value_to_num;
use crate::parser::StatementStream;
use crate::procs::common::decode_column;
use crate::session::Session;
use crate::stat::{f_cdf, least_squares};
use crate::token::TokenKind;
use crate::value::{format_best, Value};

// ───────────────────────── AST ─────────────────────────

#[derive(Debug, Clone)]
pub struct AnovaAst {
    pub data: Option<DatasetRef>,
    /// ALPHA= (parsed/recorded; default 0.05).
    pub alpha: f64,
    /// CLASS variables (REQUIRED, in declaration order).
    pub class: Vec<String>,
    /// Dependent variable(s); v1 processes the first, defers the rest.
    pub depvars: Vec<String>,
    /// MODEL effects (each a list of CLASS variable names; length 1 = main
    /// effect, length ≥2 = crossing). In MODEL order, bars already expanded.
    pub effects: Vec<Effect>,
    /// MEANS statements (optional, repeatable).
    pub means: Vec<MeansSpec>,
}

/// A model effect: the component CLASS variable names. A single name is a main
/// effect; two or more names is a crossing (interaction).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Effect {
    pub vars: Vec<String>,
}

impl Effect {
    fn label(&self) -> String {
        self.vars.join("*")
    }
}

#[derive(Debug, Clone)]
pub struct MeansSpec {
    /// The effect whose cell means are requested.
    pub effect: Effect,
    /// Multiple-comparison option names requested (recognized, deferred).
    pub mc_options: Vec<String>,
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

/// Parse PROC ANOVA and its statements. Called AFTER "proc anova" was consumed;
/// consumes through `run;`/`quit;`. Mirrors reg/corr/ttest structure.
pub fn parse(ts: &mut StatementStream) -> Result<AnovaAst> {
    let mut data: Option<DatasetRef> = None;
    let mut alpha = 0.05_f64;

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
    let mut class: Vec<String> = Vec::new();
    let mut depvars: Vec<String> = Vec::new();
    let mut effects: Vec<Effect> = Vec::new();
    let mut means: Vec<MeansSpec> = Vec::new();
    let mut have_model = false;

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

        if ts.peek().is_kw("class") || ts.peek().is_kw("classes") {
            ts.next();
            class = parse_var_list(ts)?;
            ts.expect_semi()?;
        } else if ts.peek().is_kw("model") {
            ts.next();
            let (deps, effs) = parse_model(ts)?;
            depvars = deps;
            effects = effs;
            have_model = true;
            ts.expect_semi()?;
        } else if ts.peek().is_kw("means") {
            ts.next();
            let m = parse_means(ts)?;
            ts.expect_semi()?;
            means.push(m);
        } else if ts.peek().is_kw("by") {
            ts.next();
            ts.skip_to_semi();
            return Err(SasError::runtime(
                "BY processing is not yet implemented for PROC ANOVA.",
            ));
        } else {
            // Unknown sub-statement (TEST, MANOVA, ...): skip (recovery).
            ts.skip_to_semi();
        }
    }

    if class.is_empty() {
        return Err(SasError::runtime(
            "PROC ANOVA requires a CLASS statement.",
        ));
    }
    if !have_model || effects.is_empty() || depvars.is_empty() {
        return Err(SasError::runtime(
            "PROC ANOVA requires a MODEL statement.",
        ));
    }

    Ok(AnovaAst {
        data,
        alpha,
        class,
        depvars,
        effects,
        means,
    })
}

/// Parse a whitespace-separated list of identifiers up to `;`/EOF.
fn parse_var_list(ts: &mut StatementStream) -> Result<Vec<String>> {
    let mut out: Vec<String> = Vec::new();
    loop {
        if ts.peek().kind == TokenKind::Semi || ts.peek().kind == TokenKind::Eof {
            break;
        }
        let tok = ts.peek().clone();
        match tok.ident().map(str::to_string) {
            Some(name) => {
                out.push(name);
                ts.next();
            }
            None => {
                return Err(SasError::parse(
                    "expected a variable name",
                    tok.span,
                ));
            }
        }
    }
    Ok(out)
}

/// Parse `MODEL y1 y2 ... = effects` (after `model` consumed, up to but not
/// including the terminating `;`). Returns (dependents, effects).
fn parse_model(ts: &mut StatementStream) -> Result<(Vec<String>, Vec<Effect>)> {
    // Dependent variable(s) until '='.
    let mut depvars: Vec<String> = Vec::new();
    loop {
        if ts.peek().kind == TokenKind::Eq {
            break;
        }
        if ts.peek().kind == TokenKind::Semi || ts.peek().kind == TokenKind::Eof {
            return Err(SasError::parse(
                "expected '=' in MODEL statement",
                ts.peek().span,
            ));
        }
        let tok = ts.peek().clone();
        match tok.ident().map(str::to_string) {
            Some(name) => {
                depvars.push(name);
                ts.next();
            }
            None => {
                return Err(SasError::parse(
                    "expected a dependent variable name in MODEL statement",
                    tok.span,
                ));
            }
        }
    }
    if depvars.is_empty() {
        return Err(SasError::parse(
            "MODEL statement requires a dependent variable",
            ts.peek().span,
        ));
    }
    ts.next(); // consume '='

    // Right-hand side: parse effect terms (idents joined by '*' or '|'), until
    // ';' or '/'. We collect one "term" at a time, then expand bars.
    let mut effects: Vec<Effect> = Vec::new();
    loop {
        if ts.peek().kind == TokenKind::Semi
            || ts.peek().kind == TokenKind::Slash
            || ts.peek().kind == TokenKind::Eof
        {
            break;
        }
        let (term_vars, is_bar) = parse_effect_term(ts)?;
        if is_bar {
            for combo in bar_expand(&term_vars) {
                push_effect(&mut effects, Effect { vars: combo });
            }
        } else {
            push_effect(&mut effects, Effect { vars: term_vars });
        }
    }

    if effects.is_empty() {
        return Err(SasError::parse(
            "MODEL statement requires at least one effect on the right-hand side",
            ts.peek().span,
        ));
    }

    Ok((depvars, effects))
}

/// Parse a single effect term: a sequence of CLASS names joined by `*`
/// (crossing) or `|` (bar). Returns the component names and whether ANY bar was
/// seen (in which case the caller bar-expands). Mixed `a|b*c` is treated as a
/// bar term over the listed components — a documented simplification.
fn parse_effect_term(ts: &mut StatementStream) -> Result<(Vec<String>, bool)> {
    let mut vars: Vec<String> = Vec::new();
    let mut is_bar = false;

    // First component must be an identifier.
    let tok = ts.peek().clone();
    let Some(first) = tok.ident().map(str::to_string) else {
        return Err(SasError::parse(
            "expected an effect (CLASS variable) in MODEL statement",
            tok.span,
        ));
    };
    vars.push(first);
    ts.next();

    loop {
        match ts.peek().kind {
            TokenKind::Star => {
                ts.next();
                let t = ts.peek().clone();
                let Some(nm) = t.ident().map(str::to_string) else {
                    return Err(SasError::parse(
                        "expected a CLASS variable after '*' in MODEL effect",
                        t.span,
                    ));
                };
                vars.push(nm);
                ts.next();
            }
            // A single `|` lexes to `Or`; `a|b` is the bar operator.
            TokenKind::Or => {
                is_bar = true;
                ts.next();
                let t = ts.peek().clone();
                let Some(nm) = t.ident().map(str::to_string) else {
                    return Err(SasError::parse(
                        "expected a CLASS variable after '|' in MODEL effect",
                        t.span,
                    ));
                };
                vars.push(nm);
                ts.next();
            }
            _ => break,
        }
    }
    Ok((vars, is_bar))
}

/// Expand a bar list `a|b|c` to all non-empty subsets in SAS order: singletons
/// first (in declared order), then pairs, then triples, ... Each subset keeps
/// its components in declared order. Implemented by iterating bitmasks and
/// sorting by (popcount, mask) so the ordering matches SAS's term ordering.
fn bar_expand(vars: &[String]) -> Vec<Vec<String>> {
    let n = vars.len();
    let mut masks: Vec<u32> = (1u32..(1u32 << n)).collect();
    masks.sort_by_key(|m| (m.count_ones(), *m));
    masks
        .into_iter()
        .map(|m| {
            (0..n)
                .filter(|&i| m & (1 << i) != 0)
                .map(|i| vars[i].clone())
                .collect()
        })
        .collect()
}

/// Append an effect if not already present (dedupe, as SAS collapses repeats).
fn push_effect(effects: &mut Vec<Effect>, e: Effect) {
    if !effects.iter().any(|x| x.vars == e.vars) {
        effects.push(e);
    }
}

/// Parse a MEANS body: `effect </ options>` (after `means` consumed, up to but
/// not including the terminating `;`).
fn parse_means(ts: &mut StatementStream) -> Result<MeansSpec> {
    let (vars, _is_bar) = parse_effect_term(ts)?;
    let effect = Effect { vars };

    let mut mc_options: Vec<String> = Vec::new();
    if ts.peek().kind == TokenKind::Slash {
        ts.next();
        loop {
            if ts.peek().kind == TokenKind::Semi || ts.peek().kind == TokenKind::Eof {
                break;
            }
            if let Some(name) = ts.peek().ident().map(str::to_string) {
                // Recognized comparison/option keywords are recorded; '=' values
                // (e.g. ALPHA=) are skipped over.
                mc_options.push(name.to_uppercase());
                ts.next();
                if ts.peek().kind == TokenKind::Eq {
                    ts.next();
                    // skip the value token
                    if ts.peek().kind != TokenKind::Semi && ts.peek().kind != TokenKind::Eof {
                        ts.next();
                    }
                }
            } else {
                // Unexpected token in options: stop (recovery to ';').
                break;
            }
        }
    }

    Ok(MeansSpec { effect, mc_options })
}

// ───────────────────────── numeric core ─────────────────────────

/// The numeric ANOVA decomposition for one dependent variable.
#[derive(Debug, Clone)]
pub struct AnovaResult {
    pub n: usize,
    pub grand_mean: f64,
    pub sst: f64,
    pub ss_model: f64,
    pub sse: f64,
    pub df_model: f64,
    pub df_error: f64,
    pub df_total: f64,
    pub ms_model: f64,
    pub ms_error: f64,
    pub root_mse: f64,
    pub r_square: f64,
    pub coeff_var: f64,
    pub f_value: f64,
    pub f_pvalue: f64,
    /// Per-effect (label, df, Type-I SS, MS, F, p), in MODEL order.
    pub effects: Vec<EffectSS>,
}

#[derive(Debug, Clone)]
pub struct EffectSS {
    pub label: String,
    pub df: f64,
    pub ss: f64,
    pub ms: f64,
    pub f_value: f64,
    pub f_pvalue: f64,
}

/// Residual sum of squares of the OLS fit of `y` on design matrix `x`
/// (n×p, intercept column included by the caller). Returns SSE = Σ(y − ŷ)².
pub fn sse_of_design(x: &[Vec<f64>], y: &[f64]) -> Result<f64> {
    let n = x.len();
    if n == 0 || y.len() != n {
        return Err(SasError::runtime(
            "PROC ANOVA: empty or mismatched design matrix.",
        ));
    }
    let p = x[0].len();
    let beta = least_squares(x, y)?;
    let mut sse = 0.0;
    for i in 0..n {
        let mut yhat = 0.0;
        for j in 0..p {
            yhat += x[i][j] * beta[j];
        }
        let e = y[i] - yhat;
        sse += e * e;
    }
    Ok(sse)
}

/// One-way balanced/unbalanced ANOVA from raw group data: `groups[g]` holds the
/// observations of group g. Computes the single-factor decomposition directly
/// (SSB between groups = Model, SSW within = Error). A convenience pure helper
/// for the numeric oracle.
pub fn oneway_anova(groups: &[Vec<f64>]) -> Result<AnovaResult> {
    let n: usize = groups.iter().map(|g| g.len()).sum();
    let k = groups.iter().filter(|g| !g.is_empty()).count();
    if n == 0 || k < 2 {
        return Err(SasError::runtime(
            "PROC ANOVA: one-way analysis needs ≥2 non-empty groups.",
        ));
    }
    let all_sum: f64 = groups.iter().flatten().sum();
    let grand_mean = all_sum / n as f64;

    let mut ss_between = 0.0;
    let mut ss_within = 0.0;
    for g in groups {
        if g.is_empty() {
            continue;
        }
        let gm = g.iter().sum::<f64>() / g.len() as f64;
        ss_between += g.len() as f64 * (gm - grand_mean) * (gm - grand_mean);
        for &v in g {
            ss_within += (v - gm) * (v - gm);
        }
    }
    let sst = ss_between + ss_within;
    let df_model = (k - 1) as f64;
    let df_error = (n - k) as f64;
    let df_total = (n - 1) as f64;
    if df_error <= 0.0 {
        return Err(SasError::runtime(
            "PROC ANOVA: zero error degrees of freedom.",
        ));
    }

    let result = assemble(
        n,
        grand_mean,
        sst,
        ss_between,
        ss_within,
        df_model,
        df_error,
        df_total,
        vec![EffectSeq {
            label: "Group".to_string(),
            df: df_model,
            ss: ss_between,
        }],
    );
    Ok(result)
}

/// A per-effect Type I result before F/MS are computed.
struct EffectSeq {
    label: String,
    df: f64,
    ss: f64,
}

/// Assemble an `AnovaResult` from the raw sums of squares and the sequential
/// per-effect SS list. Shared by `oneway_anova` and the design-matrix path.
#[allow(clippy::too_many_arguments)]
fn assemble(
    n: usize,
    grand_mean: f64,
    sst: f64,
    ss_model: f64,
    sse: f64,
    df_model: f64,
    df_error: f64,
    df_total: f64,
    eff_seq: Vec<EffectSeq>,
) -> AnovaResult {
    let ms_model = if df_model > 0.0 { ss_model / df_model } else { 0.0 };
    let ms_error = if df_error > 0.0 { sse / df_error } else { 0.0 };
    let root_mse = ms_error.max(0.0).sqrt();
    let r_square = if sst > 0.0 { ss_model / sst } else { 0.0 };
    let coeff_var = if grand_mean != 0.0 {
        100.0 * root_mse / grand_mean
    } else {
        0.0
    };
    let f_value = if ms_error > 0.0 { ms_model / ms_error } else { 0.0 };
    let f_pvalue = if df_model > 0.0 && f_value > 0.0 {
        (1.0 - f_cdf(f_value, df_model, df_error)).clamp(0.0, 1.0)
    } else {
        1.0
    };

    let effects: Vec<EffectSS> = eff_seq
        .into_iter()
        .map(|e| {
            let ms = if e.df > 0.0 { e.ss / e.df } else { 0.0 };
            let f = if ms_error > 0.0 { ms / ms_error } else { 0.0 };
            let p = if e.df > 0.0 && f > 0.0 {
                (1.0 - f_cdf(f, e.df, df_error)).clamp(0.0, 1.0)
            } else {
                1.0
            };
            EffectSS {
                label: e.label,
                df: e.df,
                ss: e.ss,
                ms,
                f_value: f,
                f_pvalue: p,
            }
        })
        .collect();

    AnovaResult {
        n,
        grand_mean,
        sst,
        ss_model,
        sse,
        df_model,
        df_error,
        df_total,
        ms_model,
        ms_error,
        root_mse,
        r_square,
        coeff_var,
        f_value,
        f_pvalue,
        effects,
    }
}

/// The level enumeration for one CLASS variable: distinct values in
/// `Value::sas_cmp` order, with a per-observation level index (`None` for a
/// missing value).
struct ClassLevels {
    /// Distinct levels in sorted order.
    levels: Vec<Value>,
    /// Level index per observation (parallel to the listwise-complete rows).
    codes: Vec<usize>,
}

/// Enumerate the distinct levels of `vals` (already filtered to the
/// listwise-complete observation set, all non-missing) in `sas_cmp` order, and
/// map each observation to its level index.
fn class_levels(vals: &[Value]) -> ClassLevels {
    let mut levels: Vec<Value> = Vec::new();
    for v in vals {
        if !levels.iter().any(|l| l.sas_cmp(v) == std::cmp::Ordering::Equal) {
            levels.push(v.clone());
        }
    }
    levels.sort_by(|a, b| a.sas_cmp(b));
    let codes: Vec<usize> = vals
        .iter()
        .map(|v| {
            levels
                .iter()
                .position(|l| l.sas_cmp(v) == std::cmp::Ordering::Equal)
                .unwrap_or(0)
        })
        .collect();
    ClassLevels { levels, codes }
}

/// Build the design columns contributed by ONE effect (main or crossing) under
/// reference-cell coding, for `n` observations. Returns a Vec of columns (each
/// length n). A main effect on a factor with L levels → L−1 indicator columns
/// (levels 1..L−1, reference = level 0). A crossing → the product set of its
/// components' indicator columns.
fn effect_columns(effect: &Effect, class_codes: &[(&str, &ClassLevels)]) -> Vec<Vec<f64>> {
    // Per-component indicator column groups (each group: L-1 columns).
    let mut groups: Vec<Vec<Vec<f64>>> = Vec::new();
    let n = class_codes[0].1.codes.len();
    for vname in &effect.vars {
        let cl = class_codes
            .iter()
            .find(|(nm, _)| nm.eq_ignore_ascii_case(vname))
            .map(|(_, c)| *c)
            .expect("effect var resolved to a CLASS variable");
        let l = cl.levels.len();
        let mut cols: Vec<Vec<f64>> = Vec::new();
        for lvl in 1..l {
            let col: Vec<f64> = cl
                .codes
                .iter()
                .map(|&c| if c == lvl { 1.0 } else { 0.0 })
                .collect();
            cols.push(col);
        }
        groups.push(cols);
    }

    // Cartesian product of the groups, multiplying columns elementwise.
    let mut result: Vec<Vec<f64>> = vec![vec![1.0; n]];
    for group in &groups {
        let mut next: Vec<Vec<f64>> = Vec::new();
        for base in &result {
            for col in group {
                let prod: Vec<f64> = base.iter().zip(col).map(|(a, b)| a * b).collect();
                next.push(prod);
            }
        }
        result = next;
    }
    result
}

/// Compute the full ANOVA decomposition for one dependent variable `y` given
/// the CLASS level enumerations and the MODEL effects. Builds the full design
/// matrix, then the sequential (Type I) per-effect SS via incremental fits.
fn compute_anova(
    y: &[f64],
    effects: &[Effect],
    class_codes: &[(&str, &ClassLevels)],
) -> Result<AnovaResult> {
    let n = y.len();
    if n == 0 {
        return Err(SasError::runtime(
            "PROC ANOVA: no nonmissing observations.",
        ));
    }
    let grand_mean = y.iter().sum::<f64>() / n as f64;
    let sst: f64 = y.iter().map(|&v| (v - grand_mean) * (v - grand_mean)).sum();

    // Materialize each effect's design columns once.
    let per_effect_cols: Vec<Vec<Vec<f64>>> = effects
        .iter()
        .map(|e| effect_columns(e, class_codes))
        .collect();
    let df_per_effect: Vec<f64> = per_effect_cols.iter().map(|c| c.len() as f64).collect();
    let df_model: f64 = df_per_effect.iter().sum();

    // Helper: build a design matrix with intercept + the first `k` effects, and
    // return its SSE.
    let sse_first_k = |k: usize| -> Result<f64> {
        let mut x: Vec<Vec<f64>> = vec![vec![1.0]; n];
        for cols in per_effect_cols.iter().take(k) {
            for col in cols {
                for (i, row) in x.iter_mut().enumerate() {
                    row.push(col[i]);
                }
            }
        }
        sse_of_design(&x, y)
    };

    let rank_full = 1.0 + df_model;
    let df_error = n as f64 - rank_full;
    let df_total = (n - 1) as f64;
    if df_error <= 0.0 {
        return Err(SasError::runtime(format!(
            "PROC ANOVA: not enough observations ({n}) for the model (rank {}).",
            rank_full as usize
        )));
    }

    // Sequential SSE(0..m).
    let m = effects.len();
    let mut sse_seq: Vec<f64> = Vec::with_capacity(m + 1);
    for k in 0..=m {
        sse_seq.push(sse_first_k(k)?);
    }
    let sse_full = sse_seq[m];
    let ss_model = (sst - sse_full).max(0.0);

    // Per-effect Type I SS = SSE(k-1) - SSE(k).
    let mut eff_seq: Vec<EffectSeq> = Vec::with_capacity(m);
    for (k, e) in effects.iter().enumerate() {
        let ss = (sse_seq[k] - sse_seq[k + 1]).max(0.0);
        eff_seq.push(EffectSeq {
            label: e.label(),
            df: df_per_effect[k],
            ss,
        });
    }

    Ok(assemble(
        n, grand_mean, sst, ss_model, sse_full, df_model, df_error, df_total, eff_seq,
    ))
}

// ───────────────────────── formatting ─────────────────────────

fn fmt_num(v: f64) -> String {
    format_best(v, 12)
}

fn fmt_2dp(v: f64) -> String {
    format!("{v:.2}")
}

fn fmt_p(p: f64) -> String {
    if p < 0.0001 {
        "<.0001".to_string()
    } else {
        format!("{p:.4}")
    }
}

fn fmt_df(df: f64) -> String {
    if df.fract().abs() < 1e-9 {
        format!("{}", df.round() as i64)
    } else {
        format!("{df:.2}")
    }
}

fn centered(session: &mut Session, text: &str) {
    let ls = session.listing.ls();
    let pad = ls.saturating_sub(text.len()) / 2;
    session
        .listing
        .write_line(&format!("{}{}", " ".repeat(pad), text));
}

/// Display one CLASS level value (numeric via BEST, char trimmed of trailing
/// blanks per SAS).
fn level_display(v: &Value) -> String {
    match v {
        Value::Num(n) => format_best(*n, 12),
        Value::Char(s) => s.trim_end().to_string(),
        Value::Missing(_) => ".".to_string(),
    }
}

// ───────────────────────── execute ─────────────────────────

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

/// Execute PROC ANOVA: build the listwise-complete observation set, decompose
/// the variance, and emit the listing.
pub fn execute(ast: &AnovaAst, session: &mut Session) -> Result<()> {
    let in_ref = resolve_input(ast, session)?;
    let in_libref = in_ref.libref_or_work();
    let in_table = in_ref.name.to_uppercase();

    let provider = session.libs.get(&in_libref)?;
    let (ds, notes) = provider.read(&in_table)?;
    for note in notes {
        session.log.forward(&note);
    }

    let n_obs = ds.n_obs();

    // Resolve a variable name to a column index (clean error otherwise).
    let resolve_idx = |nm: &str| -> Result<usize> {
        ds.vars
            .iter()
            .position(|m| m.name.eq_ignore_ascii_case(nm))
            .ok_or_else(|| {
                SasError::runtime(format!("Variable {} not found.", nm.to_uppercase()))
            })
    };

    // Dependent: v1 processes the first; defer the rest with a NOTE.
    let dep_name = &ast.depvars[0];
    if ast.depvars.len() > 1 {
        session.log.note(&format!(
            "PROC ANOVA v1 processes the first dependent variable ({}); the remaining {} dependent variable(s) are deferred.",
            dep_name.to_uppercase(),
            ast.depvars.len() - 1
        ));
    }
    let dep_idx = resolve_idx(dep_name)?;
    if ds.vars[dep_idx].ty != crate::value::VarType::Num {
        return Err(SasError::runtime(format!(
            "The dependent variable {} must be numeric.",
            dep_name.to_uppercase()
        )));
    }

    // Validate that every effect references only declared CLASS variables.
    for e in &ast.effects {
        for v in &e.vars {
            if !ast.class.iter().any(|c| c.eq_ignore_ascii_case(v)) {
                return Err(SasError::runtime(format!(
                    "The effect variable {} in the MODEL statement is not in the CLASS list.",
                    v.to_uppercase()
                )));
            }
        }
    }

    // Resolve & decode the dependent and CLASS columns once each.
    let class_idx: Vec<usize> = ast
        .class
        .iter()
        .map(|nm| resolve_idx(nm))
        .collect::<Result<_>>()?;
    let dep_vals = decode_column(&ds, dep_idx)?;
    let class_vals: Vec<Vec<Value>> = class_idx
        .iter()
        .map(|&c| decode_column(&ds, c))
        .collect::<Result<_>>()?;

    // Build the listwise-complete observation set: drop a row if y is missing
    // OR any CLASS value is missing.
    let mut y: Vec<f64> = Vec::new();
    let mut kept_class: Vec<Vec<Value>> = vec![Vec::new(); class_idx.len()];
    for r in 0..n_obs {
        let yv = match value_to_num(&dep_vals[r]) {
            Some(v) if !v.is_nan() => v,
            _ => continue,
        };
        let mut complete = true;
        for col in &class_vals {
            if col[r].is_missing() {
                complete = false;
                break;
            }
        }
        if !complete {
            continue;
        }
        y.push(yv);
        for (ci, col) in class_vals.iter().enumerate() {
            kept_class[ci].push(col[r].clone());
        }
    }
    let n_used = y.len();

    // Level enumerations per CLASS variable (over the kept rows).
    let level_sets: Vec<ClassLevels> = kept_class.iter().map(|c| class_levels(c)).collect();

    // Degenerate: a single-level factor used in the model, or no observations.
    if n_used == 0 {
        return Err(SasError::runtime(
            "PROC ANOVA: no nonmissing observations after listwise deletion.",
        ));
    }
    for (i, ls) in level_sets.iter().enumerate() {
        let used_in_model = ast
            .effects
            .iter()
            .any(|e| e.vars.iter().any(|v| v.eq_ignore_ascii_case(&ast.class[i])));
        if used_in_model && ls.levels.len() < 2 {
            return Err(SasError::runtime(format!(
                "The CLASS variable {} has fewer than two levels.",
                ast.class[i].to_uppercase()
            )));
        }
    }

    // Map effect-name → its ClassLevels (by CLASS index).
    let class_codes: Vec<(&str, &ClassLevels)> = ast
        .class
        .iter()
        .map(|s| s.as_str())
        .zip(level_sets.iter())
        .collect();

    let result = compute_anova(&y, &ast.effects, &class_codes)?;

    // ───── listing ─────
    session.listing.page_header();
    centered(session, "The ANOVA Procedure");
    session.listing.blank();

    // Class Level Information.
    centered(session, "Class Level Information");
    session.listing.blank();
    let ci_headers: Vec<String> = vec!["Class".into(), "Levels".into(), "Values".into()];
    let ci_aligns = vec![Align::Left, Align::Right, Align::Left];
    let ci_rows: Vec<Vec<String>> = ast
        .class
        .iter()
        .zip(level_sets.iter())
        .map(|(name, ls)| {
            let values = ls
                .levels
                .iter()
                .map(level_display)
                .collect::<Vec<_>>()
                .join(" ");
            vec![name.clone(), format!("{}", ls.levels.len()), values]
        })
        .collect();
    session.listing.write_table(&ci_headers, &ci_aligns, &ci_rows);
    session.listing.blank();

    session
        .listing
        .write_line(&format!("Number of Observations Read{:>10}", n_obs));
    session
        .listing
        .write_line(&format!("Number of Observations Used{:>10}", n_used));
    session.listing.blank();

    // Dependent variable block.
    centered(session, "The ANOVA Procedure");
    session.listing.blank();
    centered(session, &format!("Dependent Variable: {}", ds.vars[dep_idx].name));
    session.listing.blank();

    emit_anova_tables(session, &ds.vars[dep_idx].name, &result);

    // MEANS tables (plain cell means; deferral NOTE for comparison options).
    for m in &ast.means {
        emit_means(session, &ds.vars[dep_idx].name, m, &y, &class_codes)?;
        if !m.mc_options.is_empty() {
            session.log.note(&format!(
                "PROC ANOVA v1: the multiple-comparison option(s) {} on the MEANS statement for {} are deferred; plain means are reported.",
                m.mc_options.join(" "),
                m.effect.label().to_uppercase()
            ));
        }
    }

    // ALPHA= recorded only.
    let _ = ast.alpha;

    Ok(())
}

/// Emit the overall ANOVA table, the summary line, and the per-effect table.
fn emit_anova_tables(session: &mut Session, depname: &str, res: &AnovaResult) {
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
    let rows: Vec<Vec<String>> = vec![
        vec![
            "Model".into(),
            fmt_df(res.df_model),
            fmt_num(res.ss_model),
            fmt_num(res.ms_model),
            fmt_2dp(res.f_value),
            fmt_p(res.f_pvalue),
        ],
        vec![
            "Error".into(),
            fmt_df(res.df_error),
            fmt_num(res.sse),
            fmt_num(res.ms_error),
            String::new(),
            String::new(),
        ],
        vec![
            "Corrected Total".into(),
            fmt_df(res.df_total),
            fmt_num(res.sst),
            String::new(),
            String::new(),
            String::new(),
        ],
    ];
    session.listing.write_table(&headers, &aligns, &rows);
    session.listing.blank();

    // Summary line.
    let sum_headers: Vec<String> = vec![
        "R-Square".into(),
        "Coeff Var".into(),
        "Root MSE".into(),
        format!("{depname} Mean"),
    ];
    let sum_aligns = vec![Align::Right, Align::Right, Align::Right, Align::Right];
    let sum_rows: Vec<Vec<String>> = vec![vec![
        format!("{:.6}", res.r_square),
        fmt_num(res.coeff_var),
        fmt_num(res.root_mse),
        fmt_num(res.grand_mean),
    ]];
    session.listing.write_table(&sum_headers, &sum_aligns, &sum_rows);
    session.listing.blank();

    // Per-effect table ("Anova SS" = Type I; = Type III for balanced designs).
    let e_headers: Vec<String> = vec![
        "Source".into(),
        "DF".into(),
        "Anova SS".into(),
        "Mean Square".into(),
        "F Value".into(),
        "Pr > F".into(),
    ];
    let e_rows: Vec<Vec<String>> = res
        .effects
        .iter()
        .map(|e| {
            vec![
                e.label.clone(),
                fmt_df(e.df),
                fmt_num(e.ss),
                fmt_num(e.ms),
                fmt_2dp(e.f_value),
                fmt_p(e.f_pvalue),
            ]
        })
        .collect();
    session.listing.write_table(&e_headers, &aligns, &e_rows);
    session.listing.blank();
}

/// Emit a MEANS table: the mean of `y` per level (main effect) or per cell
/// (crossing) of the requested effect. Cells are ordered by component level
/// indices (component CLASS levels are themselves sas_cmp-ordered).
fn emit_means(
    session: &mut Session,
    depname: &str,
    spec: &MeansSpec,
    y: &[f64],
    class_codes: &[(&str, &ClassLevels)],
) -> Result<()> {
    centered(
        session,
        &format!("Level of {}", spec.effect.label()),
    );
    session.listing.blank();

    // Resolve component ClassLevels for this effect.
    let comps: Vec<&ClassLevels> = spec
        .effect
        .vars
        .iter()
        .map(|v| {
            class_codes
                .iter()
                .find(|(nm, _)| nm.eq_ignore_ascii_case(v))
                .map(|(_, c)| *c)
                .ok_or_else(|| {
                    SasError::runtime(format!(
                        "The MEANS effect variable {} is not in the CLASS list.",
                        v.to_uppercase()
                    ))
                })
        })
        .collect::<Result<_>>()?;

    let n = y.len();
    // Build a composite cell key per observation: tuple of component codes.
    use std::collections::BTreeMap;
    let mut cells: BTreeMap<Vec<usize>, (usize, f64)> = BTreeMap::new();
    for i in 0..n {
        let key: Vec<usize> = comps.iter().map(|c| c.codes[i]).collect();
        let entry = cells.entry(key).or_insert((0, 0.0));
        entry.0 += 1;
        entry.1 += y[i];
    }

    // One column per component + N + Mean of dep.
    let mut headers: Vec<String> = spec.effect.vars.clone();
    headers.push("N".into());
    headers.push(format!("{depname} Mean"));
    let mut aligns: Vec<Align> = spec.effect.vars.iter().map(|_| Align::Left).collect();
    aligns.push(Align::Right);
    aligns.push(Align::Right);

    let rows: Vec<Vec<String>> = cells
        .iter()
        .map(|(key, (cnt, sum))| {
            let mut row: Vec<String> = key
                .iter()
                .enumerate()
                .map(|(j, &code)| level_display(&comps[j].levels[code]))
                .collect();
            row.push(format!("{cnt}"));
            row.push(fmt_num(sum / *cnt as f64));
            row
        })
        .collect();
    session.listing.write_table(&headers, &aligns, &rows);
    session.listing.blank();
    Ok(())
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

    fn char_meta(name: &str, len: usize) -> VarMeta {
        VarMeta {
            name: name.to_string(),
            ty: VarType::Char,
            length: len,
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

    // ───────────── parse tests ─────────────

    #[test]
    fn parse_class_model_means() {
        let ast = parse_anova(
            "proc anova data=a; class grp; model y = grp; means grp; run;",
        )
        .unwrap();
        assert_eq!(ast.data.as_ref().unwrap().name, "a");
        assert_eq!(ast.class, vec!["grp"]);
        assert_eq!(ast.depvars, vec!["y"]);
        assert_eq!(ast.effects.len(), 1);
        assert_eq!(ast.effects[0].vars, vec!["grp"]);
        assert_eq!(ast.means.len(), 1);
        assert_eq!(ast.means[0].effect.vars, vec!["grp"]);
    }

    #[test]
    fn parse_interaction_effect() {
        let ast = parse_anova(
            "proc anova data=a; class a b; model y = a b a*b; run;",
        )
        .unwrap();
        assert_eq!(ast.effects.len(), 3);
        assert_eq!(ast.effects[0].vars, vec!["a"]);
        assert_eq!(ast.effects[1].vars, vec!["b"]);
        assert_eq!(ast.effects[2].vars, vec!["a", "b"]);
    }

    #[test]
    fn parse_bar_expands() {
        let ast = parse_anova(
            "proc anova data=a; class a b; model y = a|b; run;",
        )
        .unwrap();
        // a|b → a, b, a*b.
        assert_eq!(ast.effects.len(), 3);
        assert_eq!(ast.effects[0].vars, vec!["a"]);
        assert_eq!(ast.effects[1].vars, vec!["b"]);
        assert_eq!(ast.effects[2].vars, vec!["a", "b"]);
    }

    #[test]
    fn parse_three_way_bar() {
        let ast = parse_anova(
            "proc anova data=a; class a b c; model y = a|b|c; run;",
        )
        .unwrap();
        // a|b|c → a b c a*b a*c b*c a*b*c (7 terms).
        assert_eq!(ast.effects.len(), 7);
        assert!(ast.effects.iter().any(|e| e.vars == vec!["a", "b", "c"]));
        assert!(ast.effects.iter().any(|e| e.vars == vec!["b", "c"]));
    }

    #[test]
    fn parse_means_with_deferred_options() {
        let ast = parse_anova(
            "proc anova data=a; class g; model y = g; means g / tukey alpha=0.05; run;",
        )
        .unwrap();
        assert_eq!(ast.means.len(), 1);
        assert!(ast.means[0].mc_options.contains(&"TUKEY".to_string()));
    }

    #[test]
    fn parse_unknown_proc_option_errors() {
        let r = parse_anova("proc anova data=a bogus; class g; model y = g; run;");
        assert!(r.is_err());
        assert!(r.err().unwrap().to_string().contains("BOGUS"));
    }

    #[test]
    fn parse_missing_class_errors() {
        let r = parse_anova("proc anova data=a; model y = g; run;");
        assert!(r.is_err());
        assert!(r.err().unwrap().to_string().contains("CLASS"));
    }

    #[test]
    fn parse_missing_model_errors() {
        let r = parse_anova("proc anova data=a; class g; run;");
        assert!(r.is_err());
        assert!(r.err().unwrap().to_string().contains("MODEL"));
    }

    // ───────────── Oracle 1: one-way balanced 3×3 ─────────────
    //
    // Groups A=[1,2,3], B=[4,5,6], C=[7,8,9]. Grand mean 5; group means 2/5/8.
    // SSB(Model) = 3·((2-5)²+(5-5)²+(8-5)²) = 3·(9+0+9) = 54.
    // SSW(Error) = 3 groups × ((-1)²+0²+1²) = 3·2 = 6.
    // SST = 60; df 2/6/8; MS 27/1; F = 27.0; R² = 54/60 = 0.9.

    #[test]
    fn oracle_oneway_3x3() {
        let groups = vec![
            vec![1.0, 2.0, 3.0],
            vec![4.0, 5.0, 6.0],
            vec![7.0, 8.0, 9.0],
        ];
        let r = oneway_anova(&groups).unwrap();
        assert!((r.ss_model - 54.0).abs() < 1e-9, "SSB={}", r.ss_model);
        assert!((r.sse - 6.0).abs() < 1e-9, "SSW={}", r.sse);
        assert!((r.sst - 60.0).abs() < 1e-9, "SST={}", r.sst);
        assert_eq!(r.df_model, 2.0);
        assert_eq!(r.df_error, 6.0);
        assert_eq!(r.df_total, 8.0);
        assert!((r.ms_model - 27.0).abs() < 1e-9);
        assert!((r.ms_error - 1.0).abs() < 1e-9);
        assert!((r.f_value - 27.0).abs() < 1e-9, "F={}", r.f_value);
        assert!((r.r_square - 0.9).abs() < 1e-9, "R2={}", r.r_square);
        let expected_p = 1.0 - f_cdf(27.0, 2.0, 6.0);
        assert!((r.f_pvalue - expected_p).abs() < 1e-12);
    }

    // Same data, but through the design-matrix path with a CLASS factor to
    // confirm the two engines agree.
    #[test]
    fn oracle_oneway_via_design() {
        // 3 groups coded as a single CLASS variable.
        let y = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0];
        let g = vec![
            Value::Char("A".into()),
            Value::Char("A".into()),
            Value::Char("A".into()),
            Value::Char("B".into()),
            Value::Char("B".into()),
            Value::Char("B".into()),
            Value::Char("C".into()),
            Value::Char("C".into()),
            Value::Char("C".into()),
        ];
        let cl = class_levels(&g);
        let class_codes: Vec<(&str, &ClassLevels)> = vec![("grp", &cl)];
        let effects = vec![Effect { vars: vec!["grp".into()] }];
        let r = compute_anova(&y, &effects, &class_codes).unwrap();
        assert!((r.ss_model - 54.0).abs() < 1e-7, "SSB={}", r.ss_model);
        assert!((r.sse - 6.0).abs() < 1e-7, "SSW={}", r.sse);
        assert!((r.sst - 60.0).abs() < 1e-7, "SST={}", r.sst);
        assert_eq!(r.df_model, 2.0);
        assert_eq!(r.df_error, 6.0);
        assert!((r.f_value - 27.0).abs() < 1e-7, "F={}", r.f_value);
        // Single effect: Anova SS == Model SS.
        assert_eq!(r.effects.len(), 1);
        assert!((r.effects[0].ss - 54.0).abs() < 1e-7);
        assert_eq!(r.effects[0].df, 2.0);
    }

    // ───────────── Oracle 2: two-way balanced 2×2, n=2 per cell ─────────────
    //
    // Factors A∈{0,1}, B∈{0,1}; cells of size 2. Cell values chosen so the
    // decomposition is clean:
    //   A=0,B=0: [1, 3]  (mean 2)
    //   A=0,B=1: [5, 7]  (mean 6)
    //   A=1,B=0: [4, 6]  (mean 5)
    //   A=1,B=1: [8, 10] (mean 9)
    // Cell means table:
    //              B=0   B=1   | A-mean
    //   A=0         2     6    |   4
    //   A=1         5     9    |   7
    //   B-mean      3.5   7.5  |  grand = 5.5
    // n=8, grand mean 5.5.
    //   SST = Σ(y-5.5)² : values 1,3,5,7,4,6,8,10
    //     deviations: -4.5,-2.5,-0.5,1.5,-1.5,0.5,2.5,4.5
    //     squares: 20.25,6.25,0.25,2.25,2.25,0.25,6.25,20.25 = 58.0
    //   SS_A (1 df) = 8·... main effect of A: A-means 4 and 7 (4 obs each)
    //     = 4·(4-5.5)² + 4·(7-5.5)² = 4·2.25 + 4·2.25 = 18.0
    //   SS_B (1 df): B-means 3.5 and 7.5
    //     = 4·(3.5-5.5)² + 4·(7.5-5.5)² = 4·4 + 4·4 = 32.0
    //   SS_AB (1 df): interaction. Cell-mean model SS:
    //     Σ n_cell·(cellmean-grand)² = 2·[(2-5.5)²+(6-5.5)²+(5-5.5)²+(9-5.5)²]
    //       = 2·[12.25+0.25+0.25+12.25] = 2·25 = 50.0 = SS_model
    //     SS_AB = SS_model - SS_A - SS_B = 50 - 18 - 32 = 0.0
    //   SSE = SST - SS_model = 58 - 50 = 8.0  (df_error = 8 - 4 = 4)
    //   df: A=1, B=1, AB=1, error=4, total=7.

    #[test]
    fn oracle_twoway_2x2() {
        let y = vec![1.0, 3.0, 5.0, 7.0, 4.0, 6.0, 8.0, 10.0];
        let a = vec![
            Value::Num(0.0), Value::Num(0.0), Value::Num(0.0), Value::Num(0.0),
            Value::Num(1.0), Value::Num(1.0), Value::Num(1.0), Value::Num(1.0),
        ];
        let b = vec![
            Value::Num(0.0), Value::Num(0.0), Value::Num(1.0), Value::Num(1.0),
            Value::Num(0.0), Value::Num(0.0), Value::Num(1.0), Value::Num(1.0),
        ];
        let cla = class_levels(&a);
        let clb = class_levels(&b);
        let class_codes: Vec<(&str, &ClassLevels)> = vec![("a", &cla), ("b", &clb)];
        let effects = vec![
            Effect { vars: vec!["a".into()] },
            Effect { vars: vec!["b".into()] },
            Effect { vars: vec!["a".into(), "b".into()] },
        ];
        let r = compute_anova(&y, &effects, &class_codes).unwrap();

        assert!((r.sst - 58.0).abs() < 1e-7, "SST={}", r.sst);
        assert!((r.ss_model - 50.0).abs() < 1e-7, "SSmodel={}", r.ss_model);
        assert!((r.sse - 8.0).abs() < 1e-7, "SSE={}", r.sse);

        assert_eq!(r.effects.len(), 3);
        let ss_a = r.effects[0].ss;
        let ss_b = r.effects[1].ss;
        let ss_ab = r.effects[2].ss;
        assert!((ss_a - 18.0).abs() < 1e-7, "SS_A={ss_a}");
        assert!((ss_b - 32.0).abs() < 1e-7, "SS_B={ss_b}");
        assert!(ss_ab.abs() < 1e-7, "SS_AB={ss_ab}");

        // Components sum to SST.
        assert!((ss_a + ss_b + ss_ab + r.sse - r.sst).abs() < 1e-7);

        // df add up: 1 + 1 + 1 + 4 = 7 total.
        assert_eq!(r.effects[0].df, 1.0);
        assert_eq!(r.effects[1].df, 1.0);
        assert_eq!(r.effects[2].df, 1.0);
        assert_eq!(r.df_error, 4.0);
        assert_eq!(r.df_total, 7.0);
        assert_eq!(r.df_model, 3.0);
    }

    #[test]
    fn sse_of_design_perfect() {
        // y = 1 + 2·x exactly → SSE = 0.
        let x = vec![
            vec![1.0, 1.0],
            vec![1.0, 2.0],
            vec![1.0, 3.0],
            vec![1.0, 4.0],
        ];
        let y = vec![3.0, 5.0, 7.0, 9.0];
        let sse = sse_of_design(&x, &y).unwrap();
        assert!(sse.abs() < 1e-9, "sse={sse}");
    }

    #[test]
    fn oneway_errors_on_single_group() {
        let groups = vec![vec![1.0, 2.0, 3.0]];
        assert!(oneway_anova(&groups).is_err());
    }

    // ───────────── Oracle 3: execute-level through a Session ─────────────

    #[test]
    fn execute_listing_char_class() {
        let mut session = make_session();
        // Balanced one-way, char CLASS variable "grp" with 3 levels × 3 obs.
        let df = df![
            "grp" => ["A", "A", "A", "B", "B", "B", "C", "C", "C"],
            "y" => [1.0_f64, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0],
        ]
        .unwrap();
        let ds = SasDataset {
            df,
            vars: vec![char_meta("grp", 1), num_meta("y")],
        };
        write_dataset(&mut session, "G", ds);

        let ast = parse_anova(
            "proc anova data=g; class grp; model y = grp; means grp; run;",
        )
        .unwrap();
        execute(&ast, &mut session).unwrap();

        let listing = session.listing.into_string();
        assert!(listing.contains("The ANOVA Procedure"), "listing: {listing}");
        assert!(
            listing.contains("Class Level Information"),
            "listing: {listing}"
        );
        assert!(listing.contains("Anova SS"), "listing: {listing}");
        // F value 27.00 should appear.
        assert!(listing.contains("27.00"), "listing: {listing}");
        // Number of observations.
        assert!(
            listing.contains("Number of Observations Used"),
            "listing: {listing}"
        );
    }

    #[test]
    fn execute_twoway_through_session() {
        let mut session = make_session();
        let df = df![
            "a" => [0.0_f64, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0],
            "b" => [0.0_f64, 0.0, 1.0, 1.0, 0.0, 0.0, 1.0, 1.0],
            "y" => [1.0_f64, 3.0, 5.0, 7.0, 4.0, 6.0, 8.0, 10.0],
        ]
        .unwrap();
        let ds = SasDataset {
            df,
            vars: vec![num_meta("a"), num_meta("b"), num_meta("y")],
        };
        write_dataset(&mut session, "TW", ds);

        let ast = parse_anova(
            "proc anova data=tw; class a b; model y = a b a*b; run;",
        )
        .unwrap();
        execute(&ast, &mut session).unwrap();

        let listing = session.listing.into_string();
        assert!(listing.contains("a*b"), "listing: {listing}");
        assert!(listing.contains("Anova SS"), "listing: {listing}");
    }

    #[test]
    fn execute_single_level_errors() {
        let mut session = make_session();
        let df = df![
            "grp" => ["A", "A", "A"],
            "y" => [1.0_f64, 2.0, 3.0],
        ]
        .unwrap();
        let ds = SasDataset {
            df,
            vars: vec![char_meta("grp", 1), num_meta("y")],
        };
        write_dataset(&mut session, "S", ds);

        let ast = parse_anova(
            "proc anova data=s; class grp; model y = grp; run;",
        )
        .unwrap();
        assert!(execute(&ast, &mut session).is_err());
    }
}
