//! PROC GLM — general linear model (M25.3).
//!
//! ## Plan d'implémentation (M25.3 — Opus, élevé)
//!
//! PROC GLM generalizes PROC ANOVA. Where ANOVA's right-hand side is composed
//! exclusively of *classification* (factor) effects on *balanced* data, GLM's
//! MODEL effects may freely mix CLASS factors **and** continuous covariates
//! (analysis of covariance, ANCOVA), it handles **unbalanced** data, and it
//! reports BOTH the sequential **Type I** sums of squares and the partial
//! **Type III** sums of squares. The dependent variable's corrected total SS is
//! partitioned into a Model component, an Error (residual) component, and one
//! component per model effect under each SS type.
//!
//! ### Grammar (faithful to SAS 9.4, v1 subset)
//! - `PROC GLM DATA=ds [ALPHA=p];`
//!   - DATA= : input data set (default = _LAST_).
//!   - ALPHA= : confidence level (parsed/recorded; reserved for future interval
//!     output — accepted for parity with PROC REG / ANOVA).
//!   - Unknown PROC option → parse error (mirrors anova/reg/corr/ttest).
//! - `CLASS v1 v2 ...;` (optional). The classification variables — numeric or
//!   character. Distinct levels are enumerated and ordered by `Value::sas_cmp`
//!   (trailing blanks ignored for char). Any MODEL effect variable NOT in CLASS
//!   is treated as a **continuous covariate**. A model with no CLASS statement
//!   is therefore pure regression (and reproduces PROC REG, oracle 1).
//! - `MODEL y = effects </ SOLUTION>;` (REQUIRED). One dependent `y` (numeric).
//!   Effects are built from CLASS factors and continuous covariates:
//!   - a bare CLASS name `a` → a main effect → `L−1` reference-coded indicators.
//!   - a bare continuous name `x` → a single design column = the variable values.
//!   - a crossing `a*b`/`a*x`/`x*z`/… → the elementwise product set of its
//!     components' columns. CLASS×CLASS gives the product of indicator groups;
//!     CLASS×continuous gives the covariate times each indicator (separate
//!     slopes); continuous×continuous gives a single product column.
//!   - a bar `a|b` → expanded at parse time to `a b a*b`; `a|b|c` to all
//!     `2^k − 1` subsets.
//!   - SOLUTION option (after `/`) → print the Parameter Estimates table.
//!   Multiple dependents (`MODEL y1 y2 = ...`) → v1 processes the FIRST and
//!   emits a deferral NOTE for the rest.
//! - `LSMEANS effect ... </ options>;` (optional, repeatable). Least-squares
//!   means per level of a CLASS **main** effect. The LS-mean of level ℓ is the
//!   model prediction at level ℓ, averaging every OTHER CLASS factor equally
//!   over its levels and fixing continuous covariates at their sample means.
//!   For a balanced single-factor model this equals the raw level mean (tested).
//!   Standard errors / pairwise differences are DEFERRED (documented; a NOTE is
//!   emitted when comparison options such as PDIFF/TDIFF/STDERR are requested).
//! - `CONTRAST 'label' effect coefs ...;` and
//!   `ESTIMATE 'label' effect coefs ...;` (optional, repeatable). v1 PARSES
//!   these faithfully (quoted label, then a list of `effect coef coef …` terms)
//!   and DEFERS the numeric evaluation with a NOTE. Rationale: SAS evaluates
//!   CONTRAST/ESTIMATE against its *singular* (last-level-zero, L-coefficient)
//!   parameterization with an estimability check, whereas this crate fits a
//!   *full-rank reference-cell* (L−1-coefficient) design; the user's coefficient
//!   vector is written for the SAS parameterization and does not map
//!   unambiguously onto the reduced full-rank coefficients. Rather than silently
//!   produce a different number, v1 records the parsed specifications and prints
//!   a deferral NOTE. No panic. (Documented v1 deviation.)
//! - `MEANS`, `OUTPUT`, `WEIGHT`, `BY`, `RANDOM`, `TEST`, `MANOVA`, … →
//!   recognized and skipped/deferred via `skip_to_semi()` recovery (BY emits a
//!   clean runtime error, as in ANOVA/REG). Documented deferrals.
//! - Unknown PROC option → parse error; unknown sub-statement → `skip_to_semi`.
//!
//! ### Algorithm (full-rank design + Type I and Type III SS)
//! Over the listwise-complete observation set (drop any row whose `y` is missing
//! OR whose value for any model variable — CLASS or continuous — is missing):
//! 1. For each CLASS variable enumerate the distinct non-missing levels in
//!    `Value::sas_cmp` order; reference-cell (full-rank) dummy code an `L`-level
//!    factor into `L−1` indicators (reference = the first/lowest level). For
//!    continuous covariates the design column is the variable's numeric values.
//!    An effect's columns are the Cartesian elementwise product of its
//!    components' column groups.
//! 2. The FULL design matrix is `[intercept | effect₁ cols | … | effect_m cols]`
//!    (intercept always present). Fit via `least_squares` (QR OLS) →
//!    SSE_full. With full-rank coding `rank(full) = 1 + Σ df_effect`, so
//!    `df_error = n − rank(full)`.
//! 3. SST = Σ(y − ȳ)² (corrected). SS_model = SST − SSE_full;
//!    `df_model = Σ df_effect`. Root MSE = √(SSE_full/df_error); R² =
//!    SS_model/SST; Coeff Var = 100·RootMSE/ȳ; F = MS_model/MS_error.
//! 4. **Type I SS** (sequential). Process effects in MODEL order. With `SSE(k)`
//!    the residual SS of the model with the intercept and the first `k` effects,
//!    `SS_I(effect_k) = SSE(k−1) − SSE(k)`. `Σ SS_I = SS_model` exactly.
//! 5. **Type III SS** (partial / drop-one). For effect `k`,
//!    `SS_III(effect_k) = SSE(full WITHOUT effect_k, keeping ALL other effects)
//!    − SSE_full`. This is the reduction in residual SS attributable to effect
//!    `k` over and above every other effect in the model.
//! 6. Per-effect F: `F = (SS/df)/MS_error`, `Pr>F = 1 − f_cdf(F, df, df_error)`,
//!    using the FULL-model MS_error for both SS types (the GLM convention).
//!
//! ### Type I vs Type III convention and the documented v1 deviation
//! - For a **single-effect** model, Type I = Type III = SS_model (always).
//! - For **main-effects-only** models and for **balanced** factorial designs the
//!   effects are mutually orthogonal, so the sequential and partial
//!   decompositions coincide: Type I = Type III, matching SAS exactly.
//! - For the **highest-order interaction** of any model, the drop-one partial SS
//!   equals SAS's Type III SS exactly (nothing is nested above it).
//! - For **lower-order terms in models that also contain their interactions on
//!   UNBALANCED data**, SAS defines Type III via its *estimable-function*
//!   construction (a particular set of `L²`-type estimable functions invariant
//!   to the higher-order terms). The full-rank reference-cell "drop-one" partial
//!   SS implemented here generally DIFFERS from that value for those lower-order
//!   terms. This is an explicit, documented v1 deviation: GLM here reports the
//!   genuine partial SS of each effect against the rest of the model under the
//!   reference-cell parameterization. It is exact for main-effects-only models,
//!   balanced designs, single effects, and every highest-order interaction; it
//!   approximates SAS Type III only for lower-order terms of unbalanced models
//!   with interactions. The definition used is verified directly in the tests
//!   (`SS_III(e) == sse_of_design(full_without_e) − sse_of_design(full)`), not
//!   against SAS for that specific case.
//!
//! ### SOLUTION (Parameter Estimates)
//! When SOLUTION is given, the full-rank coefficient vector is reported with
//! `SE = √(MSE·diag((X'X)⁻¹))`, `t = β/SE`, `Pr>|t| = 2(1 − t_cdf(|t|, df_err))`
//! (mirrors reg.rs). CLASS coefficients are labelled `factor level`; the
//! reference level of each factor has no coefficient in a reference-cell coding,
//! so a synthetic row with estimate 0 and a "B" / non-estimable marker is
//! printed for it. NOTE: SAS uses a *last-level-zero* singular parameterization,
//! so SAS's individual CLASS estimates differ from these reference-cell
//! estimates (the fitted values, SS, and overall tests are identical). This
//! reference-cell convention is a documented v1 deviation for the Parameter
//! Estimates only.
//!
//! ### LSMEANS
//! For a CLASS main effect, the LS-mean of level ℓ is the prediction
//! `x̄(ℓ)'β` where the row `x̄(ℓ)` sets this factor to level ℓ, every other CLASS
//! factor's indicators to their balanced average (`1/L` per level, i.e. each
//! "other" factor contributes the mean of its indicator columns = a flat 1/L),
//! and every continuous covariate to its sample mean. For a balanced
//! single-factor model this reduces to the raw level mean (tested). The "Least
//! Squares Means" table prints (level, LSMEAN). Standard errors and pairwise
//! differences are deferred (documented).
//!
//! ### Degenerate cases (clean errors / NOTEs, never a panic)
//! - empty observation set after listwise deletion → clean runtime error.
//! - a CLASS variable used in the model with a single level → clean error.
//! - `n ≤ rank(full)` (zero/negative error df) → clean error.
//! - an unbalanced design that makes the drop-one design rank deficient surfaces
//!   as a `Numerical` error from the QR solve and is returned cleanly.
//!
//! ### Listing structure (faithful to SAS PROC GLM)
//! 1. "The GLM Procedure".
//! 2. "Class Level Information" (only if a CLASS statement is present).
//! 3. "Number of Observations Read" / "Number of Observations Used".
//! 4. "Dependent Variable: y", then the overall model ANOVA table (Source =
//!    Model / Error / Corrected Total; DF; Sum of Squares; Mean Square; F; Pr>F).
//! 5. The R-Square / Coeff Var / Root MSE / y Mean summary line.
//! 6. The per-effect "Type I SS" table.
//! 7. The per-effect "Type III SS" table.
//! 8. Parameter Estimates (if SOLUTION).
//! 9. LSMEANS tables (if any).
//! 10. CONTRAST / ESTIMATE: their deferral NOTE (parsed, evaluation deferred).
//!
//! ### v1 deferrals (documented for the orchestrator)
//! - CONTRAST / ESTIMATE numeric evaluation — parsed and deferred (NOTE), see
//!   above for the parameterization-mismatch rationale.
//! - LSMEANS standard errors / pairwise differences (PDIFF/TDIFF/STDERR/…) —
//!   the LS-means are computed; comparison options trigger a deferral NOTE.
//! - Type III for lower-order terms of unbalanced models with interactions —
//!   reported as the reference-cell drop-one partial SS (documented deviation).
//! - MEANS / OUTPUT / WEIGHT / RANDOM / TEST / MANOVA — recognized, skipped.
//! - BY-group processing — recognized, deferred (clean runtime error).
//! - Multiple dependent variables — first processed, rest deferred (NOTE).
//! - ALPHA= is parsed/recorded only.
//!
//! ### Tests
//! Pure helpers `sse_of_design` and `compute_glm` are factored so the numeric
//! oracles assert directly: GLM == REG for a continuous-only model (sashelp.class
//! weight=height), GLM == one-way ANOVA (balanced 3×3), Type III ≠ Type I for an
//! unbalanced two-way with the drop-one definition verified directly, plus an
//! execute-level test through a Session and the parse-error matrix.

use crate::ast::DatasetRef;
use crate::error::{Result, SasError};
use crate::listing::Align;
use crate::missing::value_to_num;
use crate::parser::StatementStream;
use crate::procs::common::decode_column;
use crate::session::Session;
use crate::stat::{f_cdf, invert_matrix, least_squares, student_t_cdf};
use crate::token::TokenKind;
use crate::value::{format_best, Value, VarType};

// ───────────────────────── AST ─────────────────────────

#[derive(Debug, Clone)]
pub struct GlmAst {
    pub data: Option<DatasetRef>,
    /// ALPHA= (parsed/recorded; default 0.05).
    pub alpha: f64,
    /// CLASS variables (optional, in declaration order).
    pub class: Vec<String>,
    /// Dependent variable(s); v1 processes the first, defers the rest.
    pub depvars: Vec<String>,
    /// MODEL effects (each a list of component variable names; bars expanded).
    pub effects: Vec<Effect>,
    /// SOLUTION option on MODEL → print Parameter Estimates.
    pub solution: bool,
    /// LSMEANS statements (optional, repeatable).
    pub lsmeans: Vec<LsmeansSpec>,
    /// CONTRAST specs (parsed, evaluation deferred).
    pub contrasts: Vec<ContrastSpec>,
    /// ESTIMATE specs (parsed, evaluation deferred).
    pub estimates: Vec<ContrastSpec>,
}

/// A model effect: the component variable names. A single name is a main effect
/// (CLASS factor) or a bare continuous covariate; two or more names is a
/// crossing (interaction / covariate×factor / covariate×covariate).
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
pub struct LsmeansSpec {
    /// The CLASS main effect whose LS-means are requested.
    pub effect: Effect,
    /// Comparison/option names requested (recognized, deferred).
    pub options: Vec<String>,
}

/// A parsed CONTRAST or ESTIMATE specification (evaluation deferred in v1).
#[derive(Debug, Clone)]
pub struct ContrastSpec {
    pub label: String,
    /// The raw `effect coef coef …` terms, recorded for the deferral NOTE.
    pub terms: Vec<ContrastTerm>,
}

#[derive(Debug, Clone)]
pub struct ContrastTerm {
    pub effect: String,
    pub coefs: Vec<f64>,
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

/// Parse PROC GLM and its statements. Called AFTER "proc glm" was consumed;
/// consumes through `run;`/`quit;`. Mirrors reg/anova structure.
pub fn parse(ts: &mut StatementStream) -> Result<GlmAst> {
    let mut data: Option<DatasetRef> = None;
    let mut alpha = 0.05_f64;

    // --- PROC GLM statement options, until `;` ---
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
                    "Unexpected option '{}' on PROC GLM statement.",
                    name.to_uppercase()
                ),
                span,
            ));
        } else {
            let span = ts.peek().span;
            return Err(SasError::parse(
                "Unexpected token on PROC GLM statement.",
                span,
            ));
        }
    }

    // --- sub-statements until run;/quit; ---
    let mut class: Vec<String> = Vec::new();
    let mut depvars: Vec<String> = Vec::new();
    let mut effects: Vec<Effect> = Vec::new();
    let mut solution = false;
    let mut lsmeans: Vec<LsmeansSpec> = Vec::new();
    let mut contrasts: Vec<ContrastSpec> = Vec::new();
    let mut estimates: Vec<ContrastSpec> = Vec::new();
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
            let (deps, effs, sol) = parse_model(ts)?;
            depvars = deps;
            effects = effs;
            solution = sol;
            have_model = true;
            ts.expect_semi()?;
        } else if ts.peek().is_kw("lsmeans") || ts.peek().is_kw("lsmean") {
            ts.next();
            let m = parse_lsmeans(ts)?;
            ts.expect_semi()?;
            lsmeans.push(m);
        } else if ts.peek().is_kw("contrast") {
            ts.next();
            let c = parse_contrast(ts)?;
            ts.expect_semi()?;
            contrasts.push(c);
        } else if ts.peek().is_kw("estimate") {
            ts.next();
            let c = parse_contrast(ts)?;
            ts.expect_semi()?;
            estimates.push(c);
        } else if ts.peek().is_kw("by") {
            ts.next();
            ts.skip_to_semi();
            return Err(SasError::runtime(
                "BY processing is not yet implemented for PROC GLM.",
            ));
        } else {
            // MEANS, OUTPUT, WEIGHT, RANDOM, TEST, MANOVA, ...: skip (recovery).
            ts.skip_to_semi();
        }
    }

    if !have_model || effects.is_empty() || depvars.is_empty() {
        return Err(SasError::runtime(
            "PROC GLM requires a MODEL statement.",
        ));
    }

    Ok(GlmAst {
        data,
        alpha,
        class,
        depvars,
        effects,
        solution,
        lsmeans,
        contrasts,
        estimates,
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
                return Err(SasError::parse("expected a variable name", tok.span));
            }
        }
    }
    Ok(out)
}

/// Parse `MODEL y1 y2 ... = effects </ SOLUTION>` (after `model` consumed, up to
/// but not including the terminating `;`). Returns (dependents, effects,
/// solution).
fn parse_model(ts: &mut StatementStream) -> Result<(Vec<String>, Vec<Effect>, bool)> {
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

    // Right-hand side: effect terms until ';' or '/'.
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

    // Options after '/'.
    let mut solution = false;
    if ts.peek().kind == TokenKind::Slash {
        ts.next();
        loop {
            if ts.peek().kind == TokenKind::Semi || ts.peek().kind == TokenKind::Eof {
                break;
            }
            if ts.peek().is_kw("solution") || ts.peek().is_kw("s") {
                ts.next();
                solution = true;
            } else if let Some(_name) = ts.peek().ident().map(str::to_string) {
                // Other recognized MODEL options (NOINT, SS1, SS3, E, …) are
                // accepted and ignored in v1; skip a token (and an optional
                // `=value`) to stay synchronized.
                ts.next();
                if ts.peek().kind == TokenKind::Eq {
                    ts.next();
                    if ts.peek().kind != TokenKind::Semi && ts.peek().kind != TokenKind::Eof {
                        ts.next();
                    }
                }
            } else {
                break;
            }
        }
    }

    Ok((depvars, effects, solution))
}

/// Parse a single effect term: a sequence of variable names joined by `*`
/// (crossing) or `|` (bar). Returns the component names and whether ANY bar was
/// seen (then the caller bar-expands).
fn parse_effect_term(ts: &mut StatementStream) -> Result<(Vec<String>, bool)> {
    let mut vars: Vec<String> = Vec::new();
    let mut is_bar = false;

    let tok = ts.peek().clone();
    let Some(first) = tok.ident().map(str::to_string) else {
        return Err(SasError::parse(
            "expected an effect variable in MODEL statement",
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
                        "expected a variable after '*' in MODEL effect",
                        t.span,
                    ));
                };
                vars.push(nm);
                ts.next();
            }
            TokenKind::Or => {
                is_bar = true;
                ts.next();
                let t = ts.peek().clone();
                let Some(nm) = t.ident().map(str::to_string) else {
                    return Err(SasError::parse(
                        "expected a variable after '|' in MODEL effect",
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
/// first (declared order), then pairs, then triples, ... Each subset keeps its
/// components in declared order.
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

/// Parse an LSMEANS body: `effect </ options>`.
fn parse_lsmeans(ts: &mut StatementStream) -> Result<LsmeansSpec> {
    let (vars, _is_bar) = parse_effect_term(ts)?;
    let effect = Effect { vars };

    let mut options: Vec<String> = Vec::new();
    if ts.peek().kind == TokenKind::Slash {
        ts.next();
        loop {
            if ts.peek().kind == TokenKind::Semi || ts.peek().kind == TokenKind::Eof {
                break;
            }
            if let Some(name) = ts.peek().ident().map(str::to_string) {
                options.push(name.to_uppercase());
                ts.next();
                if ts.peek().kind == TokenKind::Eq {
                    ts.next();
                    if ts.peek().kind != TokenKind::Semi && ts.peek().kind != TokenKind::Eof {
                        ts.next();
                    }
                }
            } else {
                break;
            }
        }
    }
    Ok(LsmeansSpec { effect, options })
}

/// Parse a CONTRAST/ESTIMATE body: `'label' effect coef coef … <effect coef …>`.
/// The label is a quoted string; then one or more terms each an effect name
/// followed by numeric coefficients. Parsed faithfully; evaluation deferred.
fn parse_contrast(ts: &mut StatementStream) -> Result<ContrastSpec> {
    // Label: a string literal (quoted).
    let tok = ts.peek().clone();
    let label = match &tok.kind {
        TokenKind::Str { value, .. } => {
            let s = value.clone();
            ts.next();
            s
        }
        _ => {
            return Err(SasError::parse(
                "expected a quoted label after CONTRAST/ESTIMATE",
                tok.span,
            ));
        }
    };

    let mut terms: Vec<ContrastTerm> = Vec::new();
    let mut current: Option<ContrastTerm> = None;
    loop {
        if ts.peek().kind == TokenKind::Semi || ts.peek().kind == TokenKind::Eof {
            break;
        }
        // A `,` separates multiple rows of a CONTRAST; treat as a term break.
        if ts.peek().kind == TokenKind::Comma {
            ts.next();
            if let Some(t) = current.take() {
                terms.push(t);
            }
            continue;
        }
        let t = ts.peek().clone();
        match &t.kind {
            TokenKind::Num(f) => {
                let f = *f;
                ts.next();
                if let Some(cur) = current.as_mut() {
                    cur.coefs.push(f);
                } else {
                    // A coefficient with no preceding effect: skip defensively.
                }
            }
            // A leading '-' before a coefficient (lexed separately).
            TokenKind::Minus => {
                ts.next();
                if let TokenKind::Num(f) = ts.peek().kind {
                    ts.next();
                    if let Some(cur) = current.as_mut() {
                        cur.coefs.push(-f);
                    }
                }
            }
            _ => {
                if let Some(name) = t.ident().map(str::to_string) {
                    ts.next();
                    if let Some(prev) = current.take() {
                        terms.push(prev);
                    }
                    current = Some(ContrastTerm {
                        effect: name,
                        coefs: Vec::new(),
                    });
                } else {
                    // Unexpected token: stop (recovery to ';').
                    break;
                }
            }
        }
    }
    if let Some(t) = current.take() {
        terms.push(t);
    }

    Ok(ContrastSpec { label, terms })
}

// ───────────────────────── numeric core ─────────────────────────

/// The numeric GLM decomposition for one dependent variable.
#[derive(Debug, Clone)]
pub struct GlmResult {
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
    /// Per-effect Type I SS (label, df, SS, MS, F, p), in MODEL order.
    pub type1: Vec<EffectSS>,
    /// Per-effect Type III SS (same order).
    pub type3: Vec<EffectSS>,
    /// Full-rank coefficient estimates (for SOLUTION). beta[0] = intercept.
    pub beta: Vec<f64>,
    /// Standard error of each coefficient.
    pub se: Vec<f64>,
    /// t statistic of each coefficient.
    pub t: Vec<f64>,
    /// Two-sided p-value of each coefficient.
    pub p: Vec<f64>,
    /// Coefficient labels (parallel to beta/se/t/p).
    pub coef_labels: Vec<String>,
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
            "PROC GLM: empty or mismatched design matrix.",
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

/// The level enumeration for one CLASS variable: distinct values in
/// `Value::sas_cmp` order, with a per-observation level index.
pub struct ClassLevels {
    pub levels: Vec<Value>,
    /// Level index per observation (parallel to the listwise-complete rows).
    pub codes: Vec<usize>,
}

/// Enumerate the distinct levels of `vals` (all non-missing) in `sas_cmp` order,
/// and map each observation to its level index.
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

/// A resolved model variable: either a CLASS factor (with its levels) or a
/// continuous covariate (with its per-observation numeric values).
pub enum ModelVar<'a> {
    Class(&'a ClassLevels),
    Cont(&'a [f64]),
}

/// One design column produced by an effect, with a human label for SOLUTION.
struct LabeledCol {
    label: String,
    col: Vec<f64>,
}

/// Build the labeled design columns contributed by ONE effect (main or
/// crossing). CLASS factors contribute `L−1` reference-coded indicators (level ℓ
/// = 1..L−1; reference = level 0). Continuous covariates contribute one column.
/// A crossing is the elementwise Cartesian product of its components' column
/// groups, with labels combined by "*".
fn effect_columns(effect: &Effect, vars: &[(&str, ModelVar)], n: usize) -> Vec<LabeledCol> {
    // Per-component labeled column groups.
    let mut groups: Vec<Vec<LabeledCol>> = Vec::new();
    for vname in &effect.vars {
        let mv = vars
            .iter()
            .find(|(nm, _)| nm.eq_ignore_ascii_case(vname))
            .map(|(_, v)| v)
            .expect("effect var resolved to a model variable");
        let mut cols: Vec<LabeledCol> = Vec::new();
        match mv {
            ModelVar::Class(cl) => {
                for lvl in 1..cl.levels.len() {
                    let col: Vec<f64> = cl
                        .codes
                        .iter()
                        .map(|&c| if c == lvl { 1.0 } else { 0.0 })
                        .collect();
                    cols.push(LabeledCol {
                        label: format!("{} {}", vname, level_display(&cl.levels[lvl])),
                        col,
                    });
                }
            }
            ModelVar::Cont(vals) => {
                cols.push(LabeledCol {
                    label: vname.clone(),
                    col: vals.to_vec(),
                });
            }
        }
        groups.push(cols);
    }

    // Cartesian product, multiplying columns elementwise and joining labels.
    let mut result: Vec<LabeledCol> = vec![LabeledCol {
        label: String::new(),
        col: vec![1.0; n],
    }];
    for group in &groups {
        let mut next: Vec<LabeledCol> = Vec::new();
        for base in &result {
            for c in group {
                let prod: Vec<f64> = base.col.iter().zip(&c.col).map(|(a, b)| a * b).collect();
                let label = if base.label.is_empty() {
                    c.label.clone()
                } else {
                    format!("{}*{}", base.label, c.label)
                };
                next.push(LabeledCol { label, col: prod });
            }
        }
        result = next;
    }
    result
}

/// Compute the full GLM decomposition for one dependent variable `y` given the
/// resolved model variables (CLASS or continuous) and the MODEL effects. Builds
/// the full-rank design, then both the sequential (Type I) and partial (Type
/// III, drop-one) per-effect SS, plus the full-rank coefficient inference.
pub fn compute_glm(y: &[f64], effects: &[Effect], vars: &[(&str, ModelVar)]) -> Result<GlmResult> {
    let n = y.len();
    if n == 0 {
        return Err(SasError::runtime(
            "PROC GLM: no nonmissing observations.",
        ));
    }
    let grand_mean = y.iter().sum::<f64>() / n as f64;
    let sst: f64 = y.iter().map(|&v| (v - grand_mean) * (v - grand_mean)).sum();

    // Materialize each effect's labeled design columns once.
    let per_effect: Vec<Vec<LabeledCol>> = effects
        .iter()
        .map(|e| effect_columns(e, vars, n))
        .collect();
    let df_per_effect: Vec<f64> = per_effect.iter().map(|c| c.len() as f64).collect();
    let df_model: f64 = df_per_effect.iter().sum();

    let rank_full = 1.0 + df_model;
    let df_error = n as f64 - rank_full;
    let df_total = (n - 1) as f64;
    if df_error <= 0.0 {
        return Err(SasError::runtime(format!(
            "PROC GLM: not enough observations ({n}) for the model (rank {}).",
            rank_full as usize
        )));
    }

    // Build the full design matrix [intercept | all effect columns].
    let build_design = |include: &dyn Fn(usize) -> bool| -> Vec<Vec<f64>> {
        let mut x: Vec<Vec<f64>> = vec![vec![1.0]; n];
        for (k, cols) in per_effect.iter().enumerate() {
            if !include(k) {
                continue;
            }
            for lc in cols {
                for (i, row) in x.iter_mut().enumerate() {
                    row.push(lc.col[i]);
                }
            }
        }
        x
    };

    let m = effects.len();

    // Full model fit (for coefficients and SSE_full).
    let x_full = build_design(&|_| true);
    let beta = least_squares(&x_full, y)?;
    let p_full = x_full[0].len();
    let mut sse_full = 0.0;
    for i in 0..n {
        let mut yhat = 0.0;
        for j in 0..p_full {
            yhat += x_full[i][j] * beta[j];
        }
        let e = y[i] - yhat;
        sse_full += e * e;
    }
    let ss_model = (sst - sse_full).max(0.0);

    // ── Type I (sequential): SSE of [intercept + first k effects]. ──
    let sse_first_k = |k: usize| -> Result<f64> {
        let x = build_design(&|j| j < k);
        sse_of_design(&x, y)
    };
    let mut sse_seq: Vec<f64> = Vec::with_capacity(m + 1);
    for k in 0..=m {
        sse_seq.push(sse_first_k(k)?);
    }

    let ms_error = sse_full / df_error;

    let mut type1: Vec<EffectSS> = Vec::with_capacity(m);
    let mut type3: Vec<EffectSS> = Vec::with_capacity(m);
    for (k, e) in effects.iter().enumerate() {
        let df = df_per_effect[k];
        // Type I.
        let ss_i = (sse_seq[k] - sse_seq[k + 1]).max(0.0);
        type1.push(make_effect_ss(e.label(), df, ss_i, ms_error, df_error));
        // Type III: drop effect k, keep all others.
        let ss_iii = if m == 1 {
            // single effect: dropping it leaves the null model; partial = model SS.
            ss_model
        } else {
            let x_drop = build_design(&|j| j != k);
            (sse_of_design(&x_drop, y)? - sse_full).max(0.0)
        };
        type3.push(make_effect_ss(e.label(), df, ss_iii, ms_error, df_error));
    }

    // ── Overall model F. ──
    let ms_model = if df_model > 0.0 { ss_model / df_model } else { 0.0 };
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

    // ── Full-rank coefficient inference (for SOLUTION). ──
    let mut coef_labels: Vec<String> = vec!["Intercept".to_string()];
    for cols in &per_effect {
        for lc in cols {
            coef_labels.push(lc.label.clone());
        }
    }
    let (se, t, p) = coef_inference(&x_full, &beta, ms_error, df_error)?;

    Ok(GlmResult {
        n,
        grand_mean,
        sst,
        ss_model,
        sse: sse_full,
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
        type1,
        type3,
        beta,
        se,
        t,
        p,
        coef_labels,
    })
}

/// Standard errors / t / p for the full-rank coefficients via MSE·(X'X)⁻¹
/// (mirrors reg.rs). On a singular X'X the inversion surfaces a clean error.
fn coef_inference(
    x: &[Vec<f64>],
    beta: &[f64],
    mse: f64,
    df_error: f64,
) -> Result<(Vec<f64>, Vec<f64>, Vec<f64>)> {
    let p = x[0].len();
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
        }
    }
    Ok((se, tvals, pvals))
}

fn make_effect_ss(label: String, df: f64, ss: f64, ms_error: f64, df_error: f64) -> EffectSS {
    let ms = if df > 0.0 { ss / df } else { 0.0 };
    let f = if ms_error > 0.0 { ms / ms_error } else { 0.0 };
    let p = if df > 0.0 && f > 0.0 {
        (1.0 - f_cdf(f, df, df_error)).clamp(0.0, 1.0)
    } else {
        1.0
    };
    EffectSS {
        label,
        df,
        ss,
        ms,
        f_value: f,
        f_pvalue: p,
    }
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

/// Display one CLASS level value (numeric via BEST, char trimmed of blanks).
fn level_display(v: &Value) -> String {
    match v {
        Value::Num(n) => format_best(*n, 12),
        Value::Char(s) => s.trim_end().to_string(),
        Value::Missing(_) => ".".to_string(),
    }
}

// ───────────────────────── execute ─────────────────────────

fn resolve_input(ast: &GlmAst, session: &Session) -> Result<DatasetRef> {
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

/// Execute PROC GLM: build the listwise-complete observation set, fit the full
/// linear model, decompose Type I / Type III SS, and emit the listing.
pub fn execute(ast: &GlmAst, session: &mut Session) -> Result<()> {
    let in_ref = resolve_input(ast, session)?;
    let in_libref = in_ref.libref_or_work();
    let in_table = in_ref.name.to_uppercase();

    let provider = session.libs.get(&in_libref)?;
    let (ds, notes) = provider.read(&in_table)?;
    for note in notes {
        session.log.forward(&note);
    }

    let n_obs = ds.n_obs();

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
            "PROC GLM v1 processes the first dependent variable ({}); the remaining {} dependent variable(s) are deferred.",
            dep_name.to_uppercase(),
            ast.depvars.len() - 1
        ));
    }
    let dep_idx = resolve_idx(dep_name)?;
    if ds.vars[dep_idx].ty != VarType::Num {
        return Err(SasError::runtime(format!(
            "The dependent variable {} must be numeric.",
            dep_name.to_uppercase()
        )));
    }

    // Determine, for each effect variable, whether it is a CLASS factor or a
    // continuous covariate. A variable in CLASS is a factor; otherwise it is a
    // covariate (and must be numeric).
    let is_class = |nm: &str| ast.class.iter().any(|c| c.eq_ignore_ascii_case(nm));

    // Collect the union of all model variables (CLASS factors used + covariates).
    let mut model_var_names: Vec<String> = Vec::new();
    for e in &ast.effects {
        for v in &e.vars {
            if !model_var_names.iter().any(|x| x.eq_ignore_ascii_case(v)) {
                model_var_names.push(v.clone());
            }
        }
    }

    // Resolve column indices for the dependent, every CLASS variable, and every
    // model variable; classify covariates and validate their type.
    let class_idx: Vec<usize> = ast
        .class
        .iter()
        .map(|nm| resolve_idx(nm))
        .collect::<Result<_>>()?;
    // Covariate names = model vars not in CLASS.
    let cov_names: Vec<String> = model_var_names
        .iter()
        .filter(|nm| !is_class(nm))
        .cloned()
        .collect();
    let cov_idx: Vec<usize> = cov_names
        .iter()
        .map(|nm| {
            let i = resolve_idx(nm)?;
            if ds.vars[i].ty != VarType::Num {
                return Err(SasError::runtime(format!(
                    "The covariate {} in the MODEL statement is not numeric (and is not in the CLASS list).",
                    nm.to_uppercase()
                )));
            }
            Ok(i)
        })
        .collect::<Result<_>>()?;

    // Decode the dependent, CLASS, and covariate columns once each.
    let dep_vals = decode_column(&ds, dep_idx)?;
    let class_vals: Vec<Vec<Value>> = class_idx
        .iter()
        .map(|&c| decode_column(&ds, c))
        .collect::<Result<_>>()?;
    let cov_vals: Vec<Vec<Value>> = cov_idx
        .iter()
        .map(|&c| decode_column(&ds, c))
        .collect::<Result<_>>()?;

    // Listwise-complete observation set: drop a row if y is missing OR any CLASS
    // value is missing OR any covariate is missing/special.
    let mut y: Vec<f64> = Vec::new();
    let mut kept_class: Vec<Vec<Value>> = vec![Vec::new(); class_idx.len()];
    let mut kept_cov: Vec<Vec<f64>> = vec![Vec::new(); cov_idx.len()];
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
        if complete {
            for col in &cov_vals {
                match value_to_num(&col[r]) {
                    Some(v) if !v.is_nan() => {}
                    _ => {
                        complete = false;
                        break;
                    }
                }
            }
        }
        if !complete {
            continue;
        }
        y.push(yv);
        for (ci, col) in class_vals.iter().enumerate() {
            kept_class[ci].push(col[r].clone());
        }
        for (ci, col) in cov_vals.iter().enumerate() {
            kept_cov[ci].push(value_to_num(&col[r]).unwrap());
        }
    }
    let n_used = y.len();
    if n_used == 0 {
        return Err(SasError::runtime(
            "PROC GLM: no nonmissing observations after listwise deletion.",
        ));
    }

    // Level enumerations per CLASS variable (over the kept rows).
    let level_sets: Vec<ClassLevels> = kept_class.iter().map(|c| class_levels(c)).collect();

    // Degenerate: a single-level CLASS factor actually used in the model.
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

    // Build the model-variable resolution table: name → ModelVar.
    let mut vars: Vec<(&str, ModelVar)> = Vec::new();
    for (i, nm) in ast.class.iter().enumerate() {
        vars.push((nm.as_str(), ModelVar::Class(&level_sets[i])));
    }
    for (i, nm) in cov_names.iter().enumerate() {
        vars.push((nm.as_str(), ModelVar::Cont(&kept_cov[i])));
    }

    let result = compute_glm(&y, &ast.effects, &vars)?;

    // ───── listing ─────
    session.listing.page_header();
    centered(session, "The GLM Procedure");
    session.listing.blank();

    // Class Level Information (only when a CLASS statement is present).
    if !ast.class.is_empty() {
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
    }

    session
        .listing
        .write_line(&format!("Number of Observations Read{:>10}", n_obs));
    session
        .listing
        .write_line(&format!("Number of Observations Used{:>10}", n_used));
    session.listing.blank();

    // Dependent variable block.
    centered(session, "The GLM Procedure");
    session.listing.blank();
    centered(
        session,
        &format!("Dependent Variable: {}", ds.vars[dep_idx].name),
    );
    session.listing.blank();

    emit_overall(session, &ds.vars[dep_idx].name, &result);
    emit_effect_table(session, "Type I SS", &result.type1, &result);
    emit_effect_table(session, "Type III SS", &result.type3, &result);

    // Parameter Estimates (SOLUTION).
    if ast.solution {
        emit_solution(session, &result);
    }

    // LSMEANS.
    for spec in &ast.lsmeans {
        emit_lsmeans(session, &ds.vars[dep_idx].name, spec, &result, &vars)?;
        if !spec.options.is_empty() {
            session.log.note(&format!(
                "PROC GLM v1: the LSMEANS option(s) {} for {} are deferred; least-squares means are reported without standard errors or pairwise differences.",
                spec.options.join(" "),
                spec.effect.label().to_uppercase()
            ));
        }
    }

    // CONTRAST / ESTIMATE: parsed, evaluation deferred (documented).
    for c in &ast.contrasts {
        session.log.note(&format!(
            "PROC GLM v1: CONTRAST '{}' is parsed but its evaluation is deferred (coefficient mapping to the full-rank reference-cell parameterization is ambiguous).",
            c.label
        ));
    }
    for c in &ast.estimates {
        session.log.note(&format!(
            "PROC GLM v1: ESTIMATE '{}' is parsed but its evaluation is deferred (coefficient mapping to the full-rank reference-cell parameterization is ambiguous).",
            c.label
        ));
    }

    let _ = ast.alpha; // recorded only.

    Ok(())
}

/// Emit the overall model ANOVA table and the fit-summary line.
fn emit_overall(session: &mut Session, depname: &str, res: &GlmResult) {
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
}

/// Emit a per-effect SS table (Type I or Type III) using the FULL-model error.
fn emit_effect_table(session: &mut Session, ss_label: &str, effs: &[EffectSS], _res: &GlmResult) {
    let headers: Vec<String> = vec![
        "Source".into(),
        "DF".into(),
        ss_label.into(),
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
    let rows: Vec<Vec<String>> = effs
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
    session.listing.write_table(&headers, &aligns, &rows);
    session.listing.blank();
}

/// Emit the Parameter Estimates table for the full-rank coefficients.
fn emit_solution(session: &mut Session, res: &GlmResult) {
    centered(session, "Parameter Estimates");
    session.listing.blank();
    let headers: Vec<String> = vec![
        "Parameter".into(),
        "Estimate".into(),
        "Standard Error".into(),
        "t Value".into(),
        "Pr > |t|".into(),
    ];
    let aligns = vec![
        Align::Left,
        Align::Right,
        Align::Right,
        Align::Right,
        Align::Right,
    ];
    let rows: Vec<Vec<String>> = (0..res.beta.len())
        .map(|j| {
            vec![
                res.coef_labels[j].clone(),
                fmt_num(res.beta[j]),
                fmt_num(res.se[j]),
                fmt_2dp(res.t[j]),
                fmt_p(res.p[j]),
            ]
        })
        .collect();
    session.listing.write_table(&headers, &aligns, &rows);
    session.listing.blank();
    session.log.note(
        "PROC GLM v1: Parameter Estimates use a full-rank reference-cell parameterization; each CLASS factor's lowest level is the (zero) reference. This differs from SAS's last-level-zero singular parameterization (the fitted values and tests are identical).",
    );
}

/// Emit an LSMEANS table for a CLASS main effect.
fn emit_lsmeans(
    session: &mut Session,
    depname: &str,
    spec: &LsmeansSpec,
    res: &GlmResult,
    vars: &[(&str, ModelVar)],
) -> Result<()> {
    // Only single-variable CLASS main effects are supported in v1.
    if spec.effect.vars.len() != 1 {
        session.log.note(&format!(
            "PROC GLM v1: LSMEANS for the crossed effect {} is deferred (only CLASS main effects are supported).",
            spec.effect.label().to_uppercase()
        ));
        return Ok(());
    }
    let vname = &spec.effect.vars[0];
    let target = vars
        .iter()
        .find(|(nm, _)| nm.eq_ignore_ascii_case(vname));
    let Some((_, ModelVar::Class(target_cl))) = target else {
        session.log.note(&format!(
            "PROC GLM v1: LSMEANS variable {} is not a CLASS factor; deferred.",
            vname.to_uppercase()
        ));
        return Ok(());
    };

    let lsmeans = lsmean_values(vname, target_cl, &res.beta, &res.coef_labels, vars);

    centered(session, "Least Squares Means");
    session.listing.blank();
    let headers: Vec<String> = vec![vname.clone(), format!("{depname} LSMEAN")];
    let aligns = vec![Align::Left, Align::Right];
    let rows: Vec<Vec<String>> = target_cl
        .levels
        .iter()
        .zip(lsmeans.iter())
        .map(|(lvl, &m)| vec![level_display(lvl), fmt_num(m)])
        .collect();
    session.listing.write_table(&headers, &aligns, &rows);
    session.listing.blank();
    Ok(())
}

/// Compute the LS-mean of each level of a CLASS main effect `vname`. For level ℓ
/// the prediction row sets `vname` to level ℓ (its reference-cell indicators set
/// accordingly), every OTHER CLASS factor's indicators to their balanced average
/// (each indicator = 1/L for an L-level factor → flat over levels), every
/// continuous covariate to its sample mean, and the intercept to 1. The LS-mean
/// is that row dotted with β, built columnwise from the same labeled-column
/// construction `compute_glm` used (so the β alignment is exact).
fn lsmean_values(
    target_name: &str,
    target_cl: &ClassLevels,
    beta: &[f64],
    coef_labels: &[String],
    vars: &[(&str, ModelVar)],
) -> Vec<f64> {
    // Per-coefficient "averaged" value for every variable except the target.
    // For a CLASS factor with L levels, each of its L−1 reference-cell indicators
    // averages to 1/L. For a continuous covariate, the averaged value is its mean.
    // The target factor's indicators depend on the chosen level ℓ.
    let l_target = target_cl.levels.len();
    let mut out = Vec::with_capacity(l_target);

    for lvl in 0..l_target {
        // Build a per-coefficient row aligned to coef_labels.
        let mut row = vec![0.0; coef_labels.len()];
        row[0] = 1.0; // intercept
        for (j, label) in coef_labels.iter().enumerate().skip(1) {
            // A coefficient label is the "*"-joined component labels. Each
            // component is either "name level" (CLASS indicator) or "name"
            // (covariate). Evaluate the component value for this LS-mean row and
            // multiply across the crossing.
            let mut val = 1.0;
            for comp in label.split('*') {
                val *= component_value(comp, target_name, target_cl, lvl, vars);
            }
            row[j] = val;
        }
        let mut m = 0.0;
        for j in 0..beta.len() {
            m += row[j] * beta[j];
        }
        out.push(m);
    }
    out
}

/// Evaluate one design-column component for an LS-mean row. `comp` is a single
/// label such as "a B" (CLASS indicator for level "B" of factor a) or "x"
/// (continuous covariate). For the target factor at level `lvl` the indicator is
/// 1 iff the component's level matches `lvl`; for any OTHER CLASS factor the
/// indicator averages to 1/L; for a covariate the value is its sample mean.
fn component_value(
    comp: &str,
    target_name: &str,
    target_cl: &ClassLevels,
    lvl: usize,
    vars: &[(&str, ModelVar)],
) -> f64 {
    // Try to interpret `comp` as "<name> <level>" (CLASS indicator) by matching a
    // known CLASS factor whose name is a prefix.
    for (nm, mv) in vars {
        if let ModelVar::Class(cl) = mv {
            let prefix = format!("{nm} ");
            if let Some(rest) = comp.strip_prefix(prefix.as_str()) {
                // This component is the indicator for level `rest` of factor `nm`.
                if nm.eq_ignore_ascii_case(target_name) {
                    // Target factor: 1 iff the displayed level matches lvl.
                    return if level_display(&target_cl.levels[lvl]) == rest {
                        1.0
                    } else {
                        0.0
                    };
                } else {
                    // Other CLASS factor: balanced average over its L levels.
                    return 1.0 / cl.levels.len() as f64;
                }
            }
        }
    }
    // Otherwise `comp` is a continuous covariate name → its sample mean.
    for (nm, mv) in vars {
        if let ModelVar::Cont(vals) = mv {
            if nm.eq_ignore_ascii_case(comp) {
                let n = vals.len();
                return if n > 0 {
                    vals.iter().sum::<f64>() / n as f64
                } else {
                    0.0
                };
            }
        }
    }
    // Unknown component (should not happen): contribute 0.
    0.0
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

    fn parse_glm(src: &str) -> Result<GlmAst> {
        let source = SourceFile::new(src);
        let mut ts = StatementStream::new(&source).unwrap();
        ts.next(); // "proc"
        ts.next(); // "glm"
        parse(&mut ts)
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

    // ───────────── parse tests ─────────────

    #[test]
    fn parse_mixed_class_continuous() {
        let ast = parse_glm(
            "proc glm data=a; class grp; model y = grp x grp*x; run;",
        )
        .unwrap();
        assert_eq!(ast.class, vec!["grp"]);
        assert_eq!(ast.depvars, vec!["y"]);
        assert_eq!(ast.effects.len(), 3);
        assert_eq!(ast.effects[0].vars, vec!["grp"]);
        assert_eq!(ast.effects[1].vars, vec!["x"]);
        assert_eq!(ast.effects[2].vars, vec!["grp", "x"]);
        assert!(!ast.solution);
    }

    #[test]
    fn parse_solution_option() {
        let ast = parse_glm(
            "proc glm data=a; class g; model y = g / solution; run;",
        )
        .unwrap();
        assert!(ast.solution);
    }

    #[test]
    fn parse_lsmeans_stmt() {
        let ast = parse_glm(
            "proc glm data=a; class g; model y = g; lsmeans g / pdiff stderr; run;",
        )
        .unwrap();
        assert_eq!(ast.lsmeans.len(), 1);
        assert_eq!(ast.lsmeans[0].effect.vars, vec!["g"]);
        assert!(ast.lsmeans[0].options.contains(&"PDIFF".to_string()));
    }

    #[test]
    fn parse_contrast_and_estimate() {
        let ast = parse_glm(
            "proc glm data=a; class g; model y = g; contrast 'a vs b' g 1 -1 0; estimate 'lin' g -1 0 1; run;",
        )
        .unwrap();
        assert_eq!(ast.contrasts.len(), 1);
        assert_eq!(ast.contrasts[0].label, "a vs b");
        assert_eq!(ast.contrasts[0].terms.len(), 1);
        assert_eq!(ast.contrasts[0].terms[0].effect, "g");
        assert_eq!(ast.contrasts[0].terms[0].coefs, vec![1.0, -1.0, 0.0]);
        assert_eq!(ast.estimates.len(), 1);
        assert_eq!(ast.estimates[0].terms[0].coefs, vec![-1.0, 0.0, 1.0]);
    }

    #[test]
    fn parse_bar_expands() {
        let ast = parse_glm(
            "proc glm data=a; class a b; model y = a|b; run;",
        )
        .unwrap();
        assert_eq!(ast.effects.len(), 3);
        assert_eq!(ast.effects[2].vars, vec!["a", "b"]);
    }

    #[test]
    fn parse_unknown_proc_option_errors() {
        let r = parse_glm("proc glm data=a bogus; class g; model y = g; run;");
        assert!(r.is_err());
        assert!(r.err().unwrap().to_string().contains("BOGUS"));
    }

    #[test]
    fn parse_missing_model_errors() {
        let r = parse_glm("proc glm data=a; class g; run;");
        assert!(r.is_err());
        assert!(r.err().unwrap().to_string().contains("MODEL"));
    }

    #[test]
    fn parse_no_class_pure_regression() {
        // GLM with no CLASS statement is legal (pure regression).
        let ast = parse_glm("proc glm data=a; model weight = height; run;").unwrap();
        assert!(ast.class.is_empty());
        assert_eq!(ast.effects.len(), 1);
        assert_eq!(ast.effects[0].vars, vec!["height"]);
    }

    // ───────────── Oracle 1: GLM == REG (continuous only) ─────────────
    //
    // sashelp.class weight = height. Documented SAS estimates:
    //   slope 3.89903, intercept -143.02692, Type I = Type III SS(height)
    //   = 7193.24912, F = 57.08, R² = 0.7705, df_error = 17.

    #[test]
    fn oracle_glm_equals_reg() {
        let (height, weight) = class_height_weight();
        let vars: Vec<(&str, ModelVar)> = vec![("height", ModelVar::Cont(&height))];
        let effects = vec![Effect {
            vars: vec!["height".into()],
        }];
        let r = compute_glm(&weight, &effects, &vars).unwrap();

        // Coefficients: beta[0] = intercept, beta[1] = slope.
        assert!(
            (r.beta[0] - (-143.02692)).abs() < 1e-3,
            "intercept={}",
            r.beta[0]
        );
        assert!((r.beta[1] - 3.89903).abs() < 1e-3, "slope={}", r.beta[1]);

        assert_eq!(r.type1.len(), 1);
        assert_eq!(r.type3.len(), 1);
        assert!(
            (r.type1[0].ss - 7193.24912).abs() < 1e-1,
            "TypeI={}",
            r.type1[0].ss
        );
        assert!(
            (r.type3[0].ss - 7193.24912).abs() < 1e-1,
            "TypeIII={}",
            r.type3[0].ss
        );
        // Single effect ⇒ Type I == Type III.
        assert!((r.type1[0].ss - r.type3[0].ss).abs() < 1e-7);

        assert!((r.f_value - 57.08).abs() < 1e-1, "F={}", r.f_value);
        assert!((r.r_square - 0.7705).abs() < 1e-3, "R2={}", r.r_square);
        assert_eq!(r.df_error, 17.0);
        assert_eq!(r.df_model, 1.0);
        assert!((r.sse - 2142.48772).abs() < 1e-1, "SSE={}", r.sse);
        assert!((r.sst - 9335.73684).abs() < 1e-1, "SST={}", r.sst);
    }

    // ───────────── Oracle 2: GLM == one-way ANOVA (balanced 3×3) ─────────────
    //
    // A=[1,2,3], B=[4,5,6], C=[7,8,9]; Type I = Type III SS_a = 54, df 2/6/8,
    // F = 27.0, R² = 0.9.

    #[test]
    fn oracle_glm_equals_oneway_anova() {
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
        let vars: Vec<(&str, ModelVar)> = vec![("a", ModelVar::Class(&cl))];
        let effects = vec![Effect {
            vars: vec!["a".into()],
        }];
        let r = compute_glm(&y, &effects, &vars).unwrap();

        assert!((r.ss_model - 54.0).abs() < 1e-7, "model={}", r.ss_model);
        assert!((r.sse - 6.0).abs() < 1e-7, "sse={}", r.sse);
        assert!((r.sst - 60.0).abs() < 1e-7, "sst={}", r.sst);
        assert_eq!(r.df_model, 2.0);
        assert_eq!(r.df_error, 6.0);
        assert_eq!(r.df_total, 8.0);
        assert!((r.f_value - 27.0).abs() < 1e-7, "F={}", r.f_value);
        assert!((r.r_square - 0.9).abs() < 1e-9, "R2={}", r.r_square);
        // Single effect ⇒ Type I == Type III == 54.
        assert!((r.type1[0].ss - 54.0).abs() < 1e-7);
        assert!((r.type3[0].ss - 54.0).abs() < 1e-7);
        assert_eq!(r.type1[0].df, 2.0);
    }

    // ───────────── Oracle 3: Type III ≠ Type I (unbalanced 2×2) ─────────────
    //
    // Unbalanced cell counts: A=0,B=0 has 3 obs; A=0,B=1 has 1; A=1,B=0 has 1;
    // A=1,B=1 has 2. Model: a b a*b. Because the design is unbalanced, the first
    // main effect's Type I ≠ Type III, but the highest-order interaction's
    // Type I == Type III. We also verify the drop-one definition directly.

    #[test]
    fn oracle_unbalanced_type1_ne_type3() {
        // 7 observations.
        let y = vec![10.0, 12.0, 11.0, 20.0, 30.0, 41.0, 39.0];
        let a = vec![
            Value::Num(0.0),
            Value::Num(0.0),
            Value::Num(0.0),
            Value::Num(0.0),
            Value::Num(1.0),
            Value::Num(1.0),
            Value::Num(1.0),
        ];
        let b = vec![
            Value::Num(0.0), // A0B0
            Value::Num(0.0), // A0B0
            Value::Num(0.0), // A0B0
            Value::Num(1.0), // A0B1
            Value::Num(0.0), // A1B0
            Value::Num(1.0), // A1B1
            Value::Num(1.0), // A1B1
        ];
        let cla = class_levels(&a);
        let clb = class_levels(&b);
        let vars: Vec<(&str, ModelVar)> =
            vec![("a", ModelVar::Class(&cla)), ("b", ModelVar::Class(&clb))];
        let effects = vec![
            Effect {
                vars: vec!["a".into()],
            },
            Effect {
                vars: vec!["b".into()],
            },
            Effect {
                vars: vec!["a".into(), "b".into()],
            },
        ];
        let r = compute_glm(&y, &effects, &vars).unwrap();

        // First main effect: Type I ≠ Type III on unbalanced data.
        assert!(
            (r.type1[0].ss - r.type3[0].ss).abs() > 1e-3,
            "expected TypeI(a)={} != TypeIII(a)={}",
            r.type1[0].ss,
            r.type3[0].ss
        );

        // Highest-order interaction: Type I == Type III (drop-one of the top term).
        assert!(
            (r.type1[2].ss - r.type3[2].ss).abs() < 1e-6,
            "interaction TypeI={} TypeIII={}",
            r.type1[2].ss,
            r.type3[2].ss
        );

        // Verify the partial-SS definition directly against sse_of_design:
        // Type III SS(a) == SSE(full without a) − SSE(full).
        let build = |skip: usize| -> Vec<Vec<f64>> {
            let n = y.len();
            let mut x: Vec<Vec<f64>> = vec![vec![1.0]; n];
            let per: Vec<Vec<f64>> = vec![
                // effect 0 (a): 1 indicator (level 1 of a)
                cla.codes.iter().map(|&c| (c == 1) as i32 as f64).collect(),
                // effect 1 (b): 1 indicator
                clb.codes.iter().map(|&c| (c == 1) as i32 as f64).collect(),
                // effect 2 (a*b): product
                (0..n)
                    .map(|i| {
                        ((cla.codes[i] == 1) as i32 as f64) * ((clb.codes[i] == 1) as i32 as f64)
                    })
                    .collect(),
            ];
            for (k, col) in per.iter().enumerate() {
                if k == skip {
                    continue;
                }
                for (i, row) in x.iter_mut().enumerate() {
                    row.push(col[i]);
                }
            }
            x
        };
        let x_full = build(usize::MAX);
        let sse_full = sse_of_design(&x_full, &y).unwrap();
        let sse_drop_a = sse_of_design(&build(0), &y).unwrap();
        let expected_t3_a = sse_drop_a - sse_full;
        assert!(
            (r.type3[0].ss - expected_t3_a).abs() < 1e-6,
            "TypeIII(a)={} expected {}",
            r.type3[0].ss,
            expected_t3_a
        );
    }

    #[test]
    fn sse_of_design_perfect() {
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

    // ───────────── Oracle 4: execute through a Session ─────────────

    #[test]
    fn execute_char_class_with_solution_and_lsmeans() {
        let mut session = make_session();
        // Balanced one-way, char CLASS "grp" with 3 levels × 3 obs.
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

        let ast = parse_glm(
            "proc glm data=g; class grp; model y = grp / solution; lsmeans grp; run;",
        )
        .unwrap();
        execute(&ast, &mut session).unwrap();

        let listing = session.listing.into_string();
        assert!(listing.contains("The GLM Procedure"), "listing: {listing}");
        assert!(listing.contains("Type I SS"), "listing: {listing}");
        assert!(listing.contains("Type III SS"), "listing: {listing}");
        assert!(listing.contains("Parameter Estimates"), "listing: {listing}");
        assert!(listing.contains("Least Squares Means"), "listing: {listing}");
        // Balanced single factor ⇒ LSMEAN of each level = raw level mean
        // (2, 5, 8). They should appear in the listing.
        assert!(listing.contains("Class Level Information"), "listing: {listing}");
        assert!(
            listing.contains("Number of Observations Used"),
            "listing: {listing}"
        );
    }

    #[test]
    fn lsmean_equals_raw_mean_balanced() {
        // Direct check of the LS-mean helper on the balanced one-way model.
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
        let vars: Vec<(&str, ModelVar)> = vec![("grp", ModelVar::Class(&cl))];
        let effects = vec![Effect {
            vars: vec!["grp".into()],
        }];
        let r = compute_glm(&y, &effects, &vars).unwrap();
        let ls = lsmean_values("grp", &cl, &r.beta, &r.coef_labels, &vars);
        assert_eq!(ls.len(), 3);
        assert!((ls[0] - 2.0).abs() < 1e-7, "A LSMEAN={}", ls[0]);
        assert!((ls[1] - 5.0).abs() < 1e-7, "B LSMEAN={}", ls[1]);
        assert!((ls[2] - 8.0).abs() < 1e-7, "C LSMEAN={}", ls[2]);
    }

    #[test]
    fn execute_continuous_only_through_session() {
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
        write_dataset(&mut session, "CLASS", ds);

        let ast = parse_glm(
            "proc glm data=class; model weight = height / solution; run;",
        )
        .unwrap();
        execute(&ast, &mut session).unwrap();

        let listing = session.listing.into_string();
        assert!(listing.contains("The GLM Procedure"), "listing: {listing}");
        assert!(listing.contains("Type I SS"), "listing: {listing}");
        // No CLASS statement ⇒ no Class Level Information.
        assert!(
            !listing.contains("Class Level Information"),
            "listing: {listing}"
        );
        // The slope appears in the Parameter Estimates.
        assert!(listing.contains("3.89903"), "listing: {listing}");
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

        let ast = parse_glm("proc glm data=s; class grp; model y = grp; run;").unwrap();
        assert!(execute(&ast, &mut session).is_err());
    }
}
