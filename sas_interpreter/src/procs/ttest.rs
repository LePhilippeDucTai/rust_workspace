//! PROC TTEST — t-tests for hypothesis testing (M24.2).
//!
//! ## Plan d'implémentation (M24.2 — Opus, élevé)
//!
//! One-sample, two-sample (independent/paired), CLASS statement, Satterthwaite
//! test for unequal variances. Confidence intervals for means. Output listing
//! with test statistics, p-values, degrees of freedom, pooled/separate variance
//! estimates. ODS OUTPUT → datasets (Paired, TTest, ConfLimits, Equality).
//!
//! ### Architecture
//!
//! `TTestAst { data_options, proc_options, var_vars, class_var, paired_vars, output_options }`
//! with separate execution paths for 1-sample, 2-independent (CLASS), and paired.
//!
//! ### One-sample t-test
//! - Statement: `PROC TTEST DATA=ds H0=value ALPHA=p;` (default H0=0, ALPHA=0.05)
//! - VAR statement (required): `var x y z;` → test each vs H0
//! - t = (mean - H0) / (s / √n), df = n-1
//! - 2-tailed p-value: Pr(|T| > |t|) via `stat::student_t_cdf`
//! - 100(1-α)% CI for mean: mean ± t_{α/2,df} · s/√n
//! - Output row per variable: Variable, DF, Mean, StdDev, StdErr, Minimum, Maximum,
//!   t, Pr>|t|, 95% Lower/Upper CL Mean
//!
//! Listing section: "One-Sample t Tests"
//! Output dataset Ttest (if ODS OUTPUT ttest=ds):
//!   - Variable (char), N (num), Mean, StdDev, StdErr, DF, t, Probt
//!
//! ### Two-sample t-test (CLASS)
//! - Statement: `CLASS var;` where var has 2 distinct non-missing values
//! - Error if >2 classes (reject gracefully with NOTE)
//! - Assumption: groups A, B with n_A, n_B, s²_A, s²_B, x̄_A, x̄_B
//! - **Pooled variance** (default): s²_p = [(n_A-1)s²_A + (n_B-1)s²_B] / (n_A+n_B-2)
//!   t_pool = (x̄_A - x̄_B) / √(s²_p·(1/n_A + 1/n_B)), df = n_A+n_B-2
//! - **Satterthwaite** (ALPHA=p with EQUAL=NO): unequal variance correction
//!   t_satt = (x̄_A - x̄_B) / √(s²_A/n_A + s²_B/n_B)
//!   df = (s²_A/n_A + s²_B/n_B)² / [(s²_A/n_A)²/(n_A-1) + (s²_B/n_B)²/(n_B-1)]
//!   (Welch-Satterthwaite formula)
//! - **Folding F-test for equal variances** (optional): printed under listing
//!   F = max(s²_A, s²_B) / min(s²_A, s²_B), df1 = n_max-1, df2 = n_min-1
//!   Prob > F = 2 · min(F_CDF, 1 - F_CDF) for 2-tailed
//! - p-value: Pr(|T| > |t|) using pooled or Satterthwaite df
//! - CI: (x̄_A - x̄_B) ± t_{α/2,df} · SE(diff)
//!
//! Listing section: "Two-Sample t Tests" with columns:
//!   - Variable, Class1, Class2, Method (pooled/Satterthwaite), N, Mean, StdDev, SE, t, Pr>|t|, 95% CL
//! Subscript: "Equality of Variances" row if requested (F test)
//!
//! Output dataset Ttest (if ODS):
//!   - Variable (char), Class (char), N, Mean, StdDev, StdErr, DF, t, Probt, LowerCLMean, UpperCLMean
//!
//! ### Paired t-test
//! - Statement: `PAIRED x*y z*w;` → paired differences per VAR pair
//! - Error if datasets have different n (one pair is NA → exclude obs, WARN)
//! - Differences: d_i = x_i - y_i (per pair specification)
//! - One-sample t test on differences: t = d̄ / (s_d / √n), df = n-1
//! - p-value: 2-tailed Pr(|T| > |t|)
//! - CI: d̄ ± t_{α/2,df} · s_d/√n
//! - Also output: N_pairs, Mean_diff, StdDev_diff, StdErr_diff, t, Pr>|t|, CL
//!
//! Listing section: "Paired t Tests" with columns per pair:
//!   - Variable, N, Mean Diff, StdDev, StdErr, DF, t, Pr>|t|, 95% CL Diff
//!
//! Output dataset Ttest (if ODS):
//!   - Variable (char), N, MeanDiff, StdDevDiff, StdErrDiff, DF, t, Probt, LowerCLDiff, UpperCLDiff
//!
//! ### Options
//! - **ALPHA=** (default 0.05): significance level for CIs
//! - **H0=** (default 0): null hypothesis mean (1-sample)
//! - **EQUAL=YES|NO** (default YES for pooled): Satterthwaite test
//! - **CI=** (default 95): confidence level as percentage (e.g., CI=90 → α=0.10)
//! - **SIDES=2|U|L** (2-tailed, upper, lower; default 2)
//! - **CLASS** var (2-level classification, automatic detection)
//! - **PAIRED** pair1*pair2 ... (paired comparisons)
//! - **OUT=** dataset (ODS OUTPUT TTEST equivalent; also write Paired, ConfLimits, Equality if applicable)
//! - **VAR** variables (default = all numeric)
//!
//! ### Parsers
//! - `parse_ttest(ts: &mut TokenStream) -> Result<TTestAst>` : top-level proc parser
//! - `parse_ttest_options` : ALPHA, H0, EQUAL, etc.
//! - `parse_class` : CLASS var; single var, required ≥2 distinct values
//! - `parse_paired` : PAIRED x*y z*w; VAR-like syntax but pairs
//! - `parse_var` : VAR variables; defaults to all numeric if omitted
//!
//! ### Execution
//! - `execute_ttest(ast: TTestAst, session: &mut Session) -> Result<Option<LastDataset>>`
//! - Read DATA= dataset via `provider.read()` + `SasDataset`
//! - Filter VAR to numeric columns
//! - If CLASS: validate 2 distinct values, split into groups, compute pooled/Satt stats
//! - If PAIRED: align pairs, compute differences, execute 1-sample test
//! - Otherwise: execute 1-sample test on each VAR
//! - Write listing via `listing.page_header()`, `listing.write_table()`
//! - If OUT=: create ODS output dataset(s) with stats
//! - Emit NOTEs: "N observations read...", "The TTEST Procedure", variable counts
//!
//! ### Error handling
//! - Missing values → excluded from analysis (note "NMiss" in listing)
//! - Constant variable (s²=0) → note "StdDev=.", t=missing, p=missing (no test)
//! - Non-numeric VAR → note and skip
//! - CLASS with >2 values → error "Class variable must have exactly 2 levels"
//! - PAIRED with length mismatch → error or exclude incomplete pairs
//! - ALPHA not in (0, 1) → error "ALPHA must be in (0, 1)"
//!
//! ### Special cases (documented as limits)
//! - BY groups (M24.2) : parser recognizes BY but defers (note "not yet implemented")
//! - Multiple CLASS variables → parser rejects (requires single var)
//! - Confidence level asymmetry (lower/upper CI separate) → differed
//! - Non-parametric alternatives (WILCOXON) → M24.3 (PROC NPAR1WAY)
//!
//! ### Tests
//! - 1-sample: t=0 (mean=H0), t>0, t<0, CI coverage
//! - 2-sample: pooled, Satterthwaite, F test, CI
//! - Paired: simple pair, multiple pairs, missing-value handling
//! - Edge cases: n=1, n=2, s²=0, all missing, alpha edge
//! - Listing format verification (columns, headers, rounding)
//! - ODS output dataset structure (column names, types, values)

use crate::ast::DatasetRef;
use crate::dataset::{SasDataset, VarMeta};
use crate::error::{Result, SasError};
use crate::listing::Align;
use crate::parser::StatementStream;
use crate::procs::common::{decode_column, partition_numeric, sample_std};
use crate::session::Session;
use crate::stat::{f_cdf, student_t_cdf};
use crate::token::TokenKind;
use crate::value::{format_best, Value, VarType};
use polars::prelude::{Column, DataFrame, NamedFrom, Series};

#[derive(Debug, Clone)]
pub struct TTestAst {
    pub data_options: TTestDataOptions,
    pub proc_options: TTestProcOptions,
    pub var_vars: Vec<String>,
    pub class_var: Option<String>,
    pub paired_vars: Vec<(String, String)>,
}

#[derive(Debug, Clone)]
pub struct TTestDataOptions {
    pub input: Option<DatasetRef>,
    pub output: Option<DatasetRef>,
}

#[derive(Debug, Clone)]
pub struct TTestProcOptions {
    pub h0: f64,
    pub alpha: f64,
    pub ci: f64,
    pub equal: bool,
    pub sides: TTestSides,
}

#[derive(Debug, Clone, Copy)]
pub enum TTestSides {
    TwoTailed,
    Upper,
    Lower,
}

impl Default for TTestProcOptions {
    fn default() -> Self {
        TTestProcOptions {
            h0: 0.0,
            alpha: 0.05,
            ci: 95.0,
            equal: true,
            sides: TTestSides::TwoTailed,
        }
    }
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

/// Read a numeric option value (`H0=`, `ALPHA=`, `CI=`). Accepts an optional
/// leading sign for H0 (e.g. `H0=-1`).
fn parse_number(ts: &mut StatementStream, opt: &str) -> Result<f64> {
    let mut sign = 1.0;
    if ts.peek().kind == TokenKind::Minus {
        sign = -1.0;
        ts.next();
    } else if ts.peek().kind == TokenKind::Plus {
        ts.next();
    }
    let tok = ts.peek().clone();
    match tok.kind {
        TokenKind::Num(f) => {
            ts.next();
            Ok(sign * f)
        }
        _ => Err(SasError::parse(
            format!("expected a number after {opt}="),
            tok.span,
        )),
    }
}

/// Parse PROC TTEST statement and its options. Called AFTER "proc ttest" was
/// consumed; consumes through `run;`/`quit;`. Mirrors the structure of
/// `corr::parse`.
pub fn parse(ts: &mut StatementStream) -> Result<TTestAst> {
    let mut input: Option<DatasetRef> = None;
    let mut output: Option<DatasetRef> = None;
    let mut opts = TTestProcOptions::default();
    // Track whether the user explicitly set CI= (so EQUAL/SIDES default holds)
    // and whether ALPHA= or CI= won the precedence for the listing α.
    let mut ci_set = false;
    let mut alpha_set = false;

    // --- PROC TTEST statement options, until `;` ---
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
            input = Some(ts.parse_dataset_ref()?);
        } else if ts.peek().is_kw("out") {
            ts.next();
            expect_eq(ts, "OUT")?;
            output = Some(ts.parse_dataset_ref()?);
        } else if ts.peek().is_kw("h0") {
            ts.next();
            expect_eq(ts, "H0")?;
            opts.h0 = parse_number(ts, "H0")?;
        } else if ts.peek().is_kw("alpha") {
            ts.next();
            expect_eq(ts, "ALPHA")?;
            let val = parse_number(ts, "ALPHA")?;
            if !(val > 0.0 && val < 1.0) {
                return Err(SasError::runtime(format!(
                    "The ALPHA= value {val} must be between 0 and 1."
                )));
            }
            opts.alpha = val;
            opts.ci = 100.0 * (1.0 - val);
            alpha_set = true;
        } else if ts.peek().is_kw("ci") {
            ts.next();
            expect_eq(ts, "CI")?;
            let val = parse_number(ts, "CI")?;
            if !(val > 0.0 && val < 100.0) {
                return Err(SasError::runtime(format!(
                    "The CI= value {val} must be between 0 and 100."
                )));
            }
            opts.ci = val;
            ci_set = true;
            if !alpha_set {
                opts.alpha = 1.0 - val / 100.0;
            }
        } else if ts.peek().is_kw("equal") {
            ts.next();
            expect_eq(ts, "EQUAL")?;
            let tok = ts.peek().clone();
            match tok.ident().map(|s| s.to_ascii_lowercase()) {
                Some(ref s) if s == "yes" => opts.equal = true,
                Some(ref s) if s == "no" => opts.equal = false,
                _ => {
                    return Err(SasError::parse(
                        "expected YES or NO after EQUAL=",
                        tok.span,
                    ));
                }
            }
            ts.next();
        } else if ts.peek().is_kw("sides") {
            ts.next();
            expect_eq(ts, "SIDES")?;
            let tok = ts.peek().clone();
            // SIDES accepts 2, U, L (and the numeric 2 lexes as Num).
            let spec = match &tok.kind {
                TokenKind::Num(f) if *f == 2.0 => Some(TTestSides::TwoTailed),
                TokenKind::Ident(s) if s.eq_ignore_ascii_case("u") => Some(TTestSides::Upper),
                TokenKind::Ident(s) if s.eq_ignore_ascii_case("l") => Some(TTestSides::Lower),
                _ => None,
            };
            match spec {
                Some(s) => {
                    opts.sides = s;
                    ts.next();
                }
                None => {
                    return Err(SasError::parse(
                        "expected 2, U or L after SIDES=",
                        tok.span,
                    ));
                }
            }
        } else if let Some(name) = ts.peek().ident().map(str::to_string) {
            let span = ts.peek().span;
            return Err(SasError::parse(
                format!(
                    "Unexpected option '{}' on PROC TTEST statement.",
                    name.to_uppercase()
                ),
                span,
            ));
        } else {
            let span = ts.peek().span;
            return Err(SasError::parse(
                "Unexpected token on PROC TTEST statement.",
                span,
            ));
        }
    }
    let _ = ci_set;

    // --- sub-statements until run;/quit; ---
    let mut var_vars: Vec<String> = Vec::new();
    let mut class_var: Option<String> = None;
    let mut paired_vars: Vec<(String, String)> = Vec::new();

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

        if ts.peek().is_kw("var") {
            ts.next();
            var_vars = ts.parse_name_list()?;
            ts.expect_semi()?;
        } else if ts.peek().is_kw("class") {
            ts.next();
            let names = ts.parse_name_list()?;
            ts.expect_semi()?;
            if names.len() != 1 {
                return Err(SasError::runtime(
                    "The CLASS statement of PROC TTEST accepts exactly one variable.",
                ));
            }
            class_var = Some(names.into_iter().next().unwrap());
        } else if ts.peek().is_kw("paired") {
            ts.next();
            paired_vars = parse_paired(ts)?;
            ts.expect_semi()?;
        } else if ts.peek().is_kw("by") {
            // BY processing is recognized but deferred in v1.
            ts.next();
            ts.skip_to_semi();
            return Err(SasError::runtime(
                "BY processing is not yet implemented for PROC TTEST.",
            ));
        } else {
            // Unknown sub-statement: skip it (recovery, like corr/means).
            ts.skip_to_semi();
        }
    }

    Ok(TTestAst {
        data_options: TTestDataOptions { input, output },
        proc_options: opts,
        var_vars,
        class_var,
        paired_vars,
    })
}

/// Parse a `PAIRED a*b c*d;` list (after `paired` was consumed, up to but not
/// including the terminating `;`). Each pair is `name * name`.
fn parse_paired(ts: &mut StatementStream) -> Result<Vec<(String, String)>> {
    let mut pairs = Vec::new();
    loop {
        let a_tok = ts.peek().clone();
        let Some(a) = a_tok.ident().map(str::to_string) else {
            break;
        };
        ts.next();
        if ts.peek().kind != TokenKind::Star {
            return Err(SasError::parse(
                "expected '*' between PAIRED variables",
                ts.peek().span,
            ));
        }
        ts.next(); // '*'
        let b_tok = ts.peek().clone();
        let Some(b) = b_tok.ident().map(str::to_string) else {
            return Err(SasError::parse(
                "expected a variable name after '*' in PAIRED",
                b_tok.span,
            ));
        };
        ts.next();
        pairs.push((a, b));
    }
    if pairs.is_empty() {
        return Err(SasError::parse(
            "expected at least one PAIRED specification (a*b)",
            ts.peek().span,
        ));
    }
    Ok(pairs)
}

// ───────────────────────── numeric core ─────────────────────────

/// Summary statistics of a single numeric sample.
#[derive(Debug, Clone, Copy)]
struct SampleStats {
    n: usize,
    mean: Option<f64>,
    std: Option<f64>,
    stderr: Option<f64>,
    min: Option<f64>,
    max: Option<f64>,
}

/// Compute the basic descriptive statistics of a numeric sample (missing
/// already excluded). std/stderr are None when n < 2 or the sample is
/// constant (zero variance → `sample_std` returns Some(0.0); we keep 0.0 to
/// match SAS which prints 0). mean/min/max are None only when n == 0.
fn sample_summary(xs: &[f64]) -> SampleStats {
    let n = xs.len();
    if n == 0 {
        return SampleStats {
            n: 0,
            mean: None,
            std: None,
            stderr: None,
            min: None,
            max: None,
        };
    }
    let nf = n as f64;
    let mean = xs.iter().sum::<f64>() / nf;
    let std = sample_std(xs);
    let stderr = std.map(|s| s / nf.sqrt());
    let min = xs.iter().cloned().fold(f64::INFINITY, f64::min);
    let max = xs.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    SampleStats {
        n,
        mean: Some(mean),
        std,
        stderr,
        min: Some(min),
        max: Some(max),
    }
}

/// Result of a one-sample (or paired-difference) t-test.
#[derive(Debug, Clone, Copy)]
struct OneSampleTest {
    df: f64,
    t: Option<f64>,
    p: Option<f64>,
    /// Confidence limits for the mean (lower, upper).
    cl: Option<(f64, f64)>,
}

/// One-sample t-test of `xs` against `h0` at level `alpha`, with the requested
/// `sides`. t = (mean − h0)/stderr, df = n−1. Returns t/p/CI = None when the
/// test is undefined (n < 2 or constant sample → stderr 0).
fn one_sample_stats(
    xs: &[f64],
    h0: f64,
    alpha: f64,
    sides: TTestSides,
) -> (SampleStats, OneSampleTest) {
    let s = sample_summary(xs);
    let df = if s.n >= 1 { (s.n - 1) as f64 } else { 0.0 };
    let test = match (s.mean, s.stderr) {
        (Some(mean), Some(se)) if se > 0.0 && s.n >= 2 => {
            let t = (mean - h0) / se;
            let p = Some(t_pvalue(t, df, sides));
            let cl = ci_for_mean(mean, se, df, alpha, sides);
            OneSampleTest { df, t: Some(t), p, cl: Some(cl) }
        }
        _ => OneSampleTest { df, t: None, p: None, cl: None },
    };
    (s, test)
}

/// Differences of a PAIRED specification over pairwise-complete observations:
/// d_i = a_i − b_i for rows where both a and b are non-missing.
fn paired_diff(acol: &[Value], bcol: &[Value]) -> Vec<f64> {
    let mut d = Vec::new();
    let n_rows = acol.len().min(bcol.len());
    for i in 0..n_rows {
        match (crate::missing::value_to_num(&acol[i]), crate::missing::value_to_num(&bcol[i])) {
            (Some(a), Some(b)) if !a.is_nan() && !b.is_nan() => d.push(a - b),
            _ => {}
        }
    }
    d
}

/// Result of a two-sample (independent groups) t-test, both methods.
#[derive(Debug, Clone, Copy)]
struct TwoSampleTest {
    diff: Option<f64>,
    /// Pooled-variance method.
    pooled_df: f64,
    pooled_t: Option<f64>,
    pooled_p: Option<f64>,
    pooled_cl: Option<(f64, f64)>,
    /// Satterthwaite (unequal variance) method.
    satt_df: Option<f64>,
    satt_t: Option<f64>,
    satt_p: Option<f64>,
    satt_cl: Option<(f64, f64)>,
    /// Folding F-test for equality of variances: (F, df1, df2, p).
    f_test: Option<(f64, f64, f64, f64)>,
}

/// Two-sample t-test of group A vs group B (both pooled and Satterthwaite),
/// plus the folding F-test for equal variances. `diff = mean_A − mean_B`.
fn pooled_two_sample(
    a: &[f64],
    b: &[f64],
    alpha: f64,
    sides: TTestSides,
) -> (SampleStats, SampleStats, TwoSampleTest) {
    let sa = sample_summary(a);
    let sb = sample_summary(b);
    let na = sa.n;
    let nb = sb.n;

    // Undefined unless both groups have at least 2 observations.
    if na < 2 || nb < 2 || sa.mean.is_none() || sb.mean.is_none() {
        let test = TwoSampleTest {
            diff: match (sa.mean, sb.mean) {
                (Some(ma), Some(mb)) => Some(ma - mb),
                _ => None,
            },
            pooled_df: (na + nb).saturating_sub(2) as f64,
            pooled_t: None,
            pooled_p: None,
            pooled_cl: None,
            satt_df: None,
            satt_t: None,
            satt_p: None,
            satt_cl: None,
            f_test: None,
        };
        return (sa, sb, test);
    }

    let ma = sa.mean.unwrap();
    let mb = sb.mean.unwrap();
    let va = sa.std.unwrap().powi(2);
    let vb = sb.std.unwrap().powi(2);
    let naf = na as f64;
    let nbf = nb as f64;
    let diff = ma - mb;

    // Pooled.
    let pooled_df = naf + nbf - 2.0;
    let sp2 = ((naf - 1.0) * va + (nbf - 1.0) * vb) / pooled_df;
    let se_pool = (sp2 * (1.0 / naf + 1.0 / nbf)).sqrt();
    let (pooled_t, pooled_p, pooled_cl) = if se_pool > 0.0 {
        let t = diff / se_pool;
        (
            Some(t),
            Some(t_pvalue(t, pooled_df, sides)),
            Some(ci_for_mean(diff, se_pool, pooled_df, alpha, sides)),
        )
    } else {
        (None, None, None)
    };

    // Satterthwaite.
    let se_satt = (va / naf + vb / nbf).sqrt();
    let (satt_df, satt_t, satt_p, satt_cl) = if se_satt > 0.0 {
        let num = (va / naf + vb / nbf).powi(2);
        let den = (va / naf).powi(2) / (naf - 1.0) + (vb / nbf).powi(2) / (nbf - 1.0);
        let dfw = num / den;
        let t = diff / se_satt;
        (
            Some(dfw),
            Some(t),
            Some(t_pvalue(t, dfw, sides)),
            Some(ci_for_mean(diff, se_satt, dfw, alpha, sides)),
        )
    } else {
        (None, None, None, None)
    };

    // Folding F-test for equality of variances: F = max var / min var,
    // df numerator = (n of the larger variance) − 1, etc.
    let f_test = if va > 0.0 && vb > 0.0 {
        let (fnum, fden, df1, df2) = if va >= vb {
            (va, vb, naf - 1.0, nbf - 1.0)
        } else {
            (vb, va, nbf - 1.0, naf - 1.0)
        };
        let f = fnum / fden;
        // Two-sided p: 2 * P(F > f) = 2 * (1 - F_CDF(f)).
        let p = (2.0 * (1.0 - f_cdf(f, df1, df2))).min(1.0);
        Some((f, df1, df2, p))
    } else {
        None
    };

    let test = TwoSampleTest {
        diff: Some(diff),
        pooled_df,
        pooled_t,
        pooled_p,
        pooled_cl,
        satt_df,
        satt_t,
        satt_p,
        satt_cl,
        f_test,
    };
    (sa, sb, test)
}

/// p-value for a t statistic given df and the test sidedness. Two-sided:
/// 2·(1 − CDF(|t|)). Upper: 1 − CDF(t). Lower: CDF(t).
fn t_pvalue(t: f64, df: f64, sides: TTestSides) -> f64 {
    match sides {
        TTestSides::TwoTailed => (2.0 * (1.0 - student_t_cdf(t.abs(), df))).min(1.0),
        TTestSides::Upper => 1.0 - student_t_cdf(t, df),
        TTestSides::Lower => student_t_cdf(t, df),
    }
}

/// Confidence limits for a mean (or mean difference) given its point estimate,
/// standard error, df and α, honoring sidedness. For one-sided tests SAS
/// reports a one-sided interval (the unbounded side as ±∞).
fn ci_for_mean(est: f64, se: f64, df: f64, alpha: f64, sides: TTestSides) -> (f64, f64) {
    match sides {
        TTestSides::TwoTailed => {
            let tcrit = t_quantile(1.0 - alpha / 2.0, df);
            (est - tcrit * se, est + tcrit * se)
        }
        TTestSides::Upper => {
            // Lower confidence bound; upper is +∞.
            let tcrit = t_quantile(1.0 - alpha, df);
            (est - tcrit * se, f64::INFINITY)
        }
        TTestSides::Lower => {
            // Upper confidence bound; lower is −∞.
            let tcrit = t_quantile(1.0 - alpha, df);
            (f64::NEG_INFINITY, est + tcrit * se)
        }
    }
}

/// Student-t quantile (inverse CDF): the value `q` with `P(T_df <= q) = p`,
/// for `0 < p < 1` and `df > 0`. Solved by bisection on the monotone t-CDF
/// (robust, derivative-free). Kept private here — see PLAN: do NOT add a new
/// pub fn to `stat`. Mirrors `procs::common::t_quantile`.
fn t_quantile(p: f64, df: f64) -> f64 {
    if !(0.0..=1.0).contains(&p) {
        return f64::NAN;
    }
    if p == 0.5 {
        return 0.0;
    }
    let upper = p > 0.5;
    let target = if upper { p } else { 1.0 - p };
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

/// Format an optional numeric statistic SAS-style (best width 12). None → ".".
fn fmt_opt(v: Option<f64>) -> String {
    match v {
        Some(f) => format_best(f, 12),
        None => ".".to_string(),
    }
}

/// Format degrees of freedom: integer when whole (pooled/one-sample), else
/// 2 decimals (Satterthwaite).
fn fmt_df(df: f64) -> String {
    if (df.fract()).abs() < 1e-9 {
        format!("{}", df.round() as i64)
    } else {
        format!("{df:.2}")
    }
}

/// Format a t value to 2 decimals (SAS "t Value"). None → ".".
fn fmt_t(t: Option<f64>) -> String {
    match t {
        Some(v) => format!("{v:.2}"),
        None => ".".to_string(),
    }
}

/// Format one confidence limit (the lower or upper element of a CL pair).
/// ±∞ (one-sided open ends) render as SAS does, "-Infty"/"Infty". None → ".".
fn fmt_cl(bound: Option<f64>) -> String {
    match bound {
        None => ".".to_string(),
        Some(v) if v == f64::INFINITY => "Infty".to_string(),
        Some(v) if v == f64::NEG_INFINITY => "-Infty".to_string(),
        Some(v) => format_best(v, 12),
    }
}

/// Format a p-value SAS-style: `<.0001`, else 4 decimals. None → ".".
fn fmt_p(p: Option<f64>) -> String {
    match p {
        None => ".".to_string(),
        Some(v) => {
            if v < 0.0001 {
                "<.0001".to_string()
            } else {
                format!("{v:.4}")
            }
        }
    }
}

/// Write a centered line within LINESIZE (mirrors corr::centered).
fn centered(session: &mut Session, text: &str) {
    let ls = session.listing.ls();
    let pad = ls.saturating_sub(text.len()) / 2;
    session
        .listing
        .write_line(&format!("{}{}", " ".repeat(pad), text));
}

// ───────────────────────── execute ─────────────────────────

/// Resolve `data=` or `_LAST_` into a concrete DatasetRef (mirrors
/// corr::resolve_input).
fn resolve_input(ast: &TTestAst, session: &Session) -> Result<DatasetRef> {
    match &ast.data_options.input {
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

/// Execute PROC TTEST and produce the listing (+ optional OUT= dataset).
pub fn execute(ast: &TTestAst, session: &mut Session) -> Result<()> {
    let in_ref = resolve_input(ast, session)?;
    let in_libref = in_ref.libref_or_work();
    let in_table = in_ref.name.to_uppercase();

    let provider = session.libs.get(&in_libref)?;
    let (ds, notes) = provider.read(&in_table)?;
    for note in notes {
        session.log.forward(&note);
    }

    let n_obs = ds.n_obs();

    // Helper: resolve a single variable name to a column index (any type).
    let resolve_var = |nm: &str| -> Result<usize> {
        ds.vars
            .iter()
            .position(|m| m.name.eq_ignore_ascii_case(nm))
            .ok_or_else(|| {
                SasError::runtime(format!("Variable {} not found.", nm.to_uppercase()))
            })
    };
    // Helper: resolve a numeric variable name to a column index.
    let resolve_num = |nm: &str| -> Result<usize> {
        let i = resolve_var(nm)?;
        if ds.vars[i].ty != VarType::Num {
            return Err(SasError::runtime(format!(
                "Variable {} is not numeric.",
                nm.to_uppercase()
            )));
        }
        Ok(i)
    };

    session.listing.page_header();
    centered(session, "The TTEST Procedure");
    session.listing.blank();

    // Dispatch: PAIRED > CLASS > one-sample.
    let out_rows: Vec<OutRow> = if !ast.paired_vars.is_empty() {
        execute_paired(ast, session, &ds, &resolve_num)?
    } else if let Some(cv) = &ast.class_var {
        execute_two_sample(ast, session, &ds, cv, &resolve_var, &resolve_num, n_obs)?
    } else {
        execute_one_sample(ast, session, &ds, &resolve_num)?
    };

    // --- OUT= dataset (simplified Ttest table) ---
    if let Some(target) = &ast.data_options.output {
        let out_ds = build_out_dataset(&out_rows)?;
        write_out_dataset(session, target, out_ds)?;
    }

    Ok(())
}

/// A row of the simplified OUT= "Ttest" dataset.
struct OutRow {
    variable: String,
    n: usize,
    mean: Option<f64>,
    std: Option<f64>,
    stderr: Option<f64>,
    df: f64,
    t: Option<f64>,
    probt: Option<f64>,
}

/// Resolve the analysis VAR list: explicit, else all numeric columns in order.
fn var_columns(ds: &SasDataset, ast: &TTestAst, resolve_num: &dyn Fn(&str) -> Result<usize>) -> Result<Vec<usize>> {
    if !ast.var_vars.is_empty() {
        ast.var_vars.iter().map(|nm| resolve_num(nm)).collect()
    } else {
        let cols: Vec<usize> = (0..ds.vars.len())
            .filter(|&i| ds.vars[i].ty == VarType::Num)
            .collect();
        if cols.is_empty() {
            return Err(SasError::runtime(
                "No numeric variables found for PROC TTEST analysis.",
            ));
        }
        Ok(cols)
    }
}

/// One-sample path: a test of each VAR against H0.
fn execute_one_sample(
    ast: &TTestAst,
    session: &mut Session,
    ds: &SasDataset,
    resolve_num: &dyn Fn(&str) -> Result<usize>,
) -> Result<Vec<OutRow>> {
    let cols = var_columns(ds, ast, resolve_num)?;
    let all_rows: Vec<usize> = (0..ds.n_obs()).collect();
    let opts = &ast.proc_options;

    // Compute per-variable statistics first (decode each column once).
    let mut summaries: Vec<(String, SampleStats, OneSampleTest)> = Vec::with_capacity(cols.len());
    for &c in &cols {
        let col = decode_column(ds, c)?;
        let (xs, _nmiss) = partition_numeric(&col, &all_rows);
        let (s, test) = one_sample_stats(&xs, opts.h0, opts.alpha, opts.sides);
        summaries.push((ds.vars[c].name.clone(), s, test));
    }

    // --- Statistics table ---
    let headers: Vec<String> = vec![
        "Variable".into(),
        "N".into(),
        "Mean".into(),
        "Std Dev".into(),
        "Std Err".into(),
        "Minimum".into(),
        "Maximum".into(),
    ];
    let aligns = vec![Align::Left, Align::Right, Align::Right, Align::Right, Align::Right, Align::Right, Align::Right];
    let mut rows: Vec<Vec<String>> = Vec::with_capacity(summaries.len());
    for (name, s, _t) in &summaries {
        rows.push(vec![
            name.clone(),
            format!("{}", s.n),
            fmt_opt(s.mean),
            fmt_opt(s.std),
            fmt_opt(s.stderr),
            fmt_opt(s.min),
            fmt_opt(s.max),
        ]);
    }
    session.listing.write_table(&headers, &aligns, &rows);
    session.listing.blank();

    // --- T-Tests table ---
    centered(session, "T-Tests");
    session.listing.blank();
    let theaders: Vec<String> = vec![
        "Variable".into(),
        "DF".into(),
        "t Value".into(),
        "Pr > |t|".into(),
        "Lower CL Mean".into(),
        "Upper CL Mean".into(),
    ];
    let taligns = vec![Align::Left, Align::Right, Align::Right, Align::Right, Align::Right, Align::Right];
    let mut trows: Vec<Vec<String>> = Vec::with_capacity(summaries.len());
    for (name, _s, t) in &summaries {
        let (lo, hi) = match t.cl {
            Some((l, h)) => (Some(l), Some(h)),
            None => (None, None),
        };
        trows.push(vec![
            name.clone(),
            fmt_df(t.df),
            fmt_t(t.t),
            fmt_p(t.p),
            fmt_cl(lo),
            fmt_cl(hi),
        ]);
    }
    session.listing.write_table(&theaders, &taligns, &trows);
    session.listing.blank();

    Ok(summaries
        .into_iter()
        .map(|(name, s, t)| OutRow {
            variable: name,
            n: s.n,
            mean: s.mean,
            std: s.std,
            stderr: s.stderr,
            df: t.df,
            t: t.t,
            probt: t.p,
        })
        .collect())
}

/// Two-sample path (CLASS with exactly 2 non-missing levels).
#[allow(clippy::too_many_arguments)]
fn execute_two_sample(
    ast: &TTestAst,
    session: &mut Session,
    ds: &SasDataset,
    class_name: &str,
    resolve_var: &dyn Fn(&str) -> Result<usize>,
    resolve_num: &dyn Fn(&str) -> Result<usize>,
    n_obs: usize,
) -> Result<Vec<OutRow>> {
    let class_col = resolve_var(class_name)?;
    let class_vals = decode_column(ds, class_col)?;

    // Group row indices by class value (non-missing only), ordered by sas_cmp.
    let mut groups: Vec<(Value, Vec<usize>)> = Vec::new();
    for r in 0..n_obs {
        let v = &class_vals[r];
        if v.is_missing() {
            continue;
        }
        match groups.iter_mut().find(|(k, _)| k.sas_cmp(v) == std::cmp::Ordering::Equal) {
            Some((_, idx)) => idx.push(r),
            None => groups.push((v.clone(), vec![r])),
        }
    }
    groups.sort_by(|(a, _), (b, _)| a.sas_cmp(b));

    if groups.len() != 2 {
        return Err(SasError::runtime(format!(
            "The CLASS variable {} must have exactly two levels; found {}.",
            class_name.to_uppercase(),
            groups.len()
        )));
    }

    let label = |v: &Value| -> String {
        match v {
            Value::Char(s) => s.trim_end().to_string(),
            Value::Num(f) => format_best(*f, 12),
            Value::Missing(k) => k.display(),
        }
    };
    let c1 = label(&groups[0].0);
    let c2 = label(&groups[1].0);
    let rows1 = &groups[0].1;
    let rows2 = &groups[1].1;

    let cols = var_columns(ds, ast, resolve_num)?;
    let opts = &ast.proc_options;

    let mut out_rows: Vec<OutRow> = Vec::new();

    for &c in &cols {
        let vname = ds.vars[c].name.clone();
        let col = decode_column(ds, c)?;
        let (a, _) = partition_numeric(&col, rows1);
        let (b, _) = partition_numeric(&col, rows2);
        let (sa, sb, test) = pooled_two_sample(&a, &b, opts.alpha, opts.sides);

        // --- Statistics table (one block per variable) ---
        centered(session, &format!("Variable: {vname}"));
        session.listing.blank();
        let headers: Vec<String> = vec![
            class_name.to_string(),
            "N".into(),
            "Mean".into(),
            "Std Dev".into(),
            "Std Err".into(),
            "Minimum".into(),
            "Maximum".into(),
        ];
        let aligns = vec![Align::Left, Align::Right, Align::Right, Align::Right, Align::Right, Align::Right, Align::Right];
        let stat_row = |lbl: &str, s: &SampleStats| -> Vec<String> {
            vec![
                lbl.to_string(),
                format!("{}", s.n),
                fmt_opt(s.mean),
                fmt_opt(s.std),
                fmt_opt(s.stderr),
                fmt_opt(s.min),
                fmt_opt(s.max),
            ]
        };
        let diff_summary = SampleStats {
            n: sa.n + sb.n,
            mean: test.diff,
            std: None,
            stderr: None,
            min: None,
            max: None,
        };
        let rows = vec![
            stat_row(&c1, &sa),
            stat_row(&c2, &sb),
            stat_row("Diff (1-2)", &diff_summary),
        ];
        session.listing.write_table(&headers, &aligns, &rows);
        session.listing.blank();

        // --- T-Tests table: pooled + Satterthwaite ---
        centered(session, "T-Tests");
        session.listing.blank();
        let theaders: Vec<String> = vec![
            "Method".into(),
            "Variances".into(),
            "DF".into(),
            "t Value".into(),
            "Pr > |t|".into(),
            "Lower CL Mean".into(),
            "Upper CL Mean".into(),
        ];
        let taligns = vec![Align::Left, Align::Left, Align::Right, Align::Right, Align::Right, Align::Right, Align::Right];
        let pooled_cl = test.pooled_cl.map(|(l, h)| (Some(l), Some(h))).unwrap_or((None, None));
        let satt_cl = test.satt_cl.map(|(l, h)| (Some(l), Some(h))).unwrap_or((None, None));
        let trows: Vec<Vec<String>> = vec![
            vec![
                "Pooled".into(),
                "Equal".into(),
                fmt_df(test.pooled_df),
                fmt_t(test.pooled_t),
                fmt_p(test.pooled_p),
                fmt_cl(pooled_cl.0),
                fmt_cl(pooled_cl.1),
            ],
            vec![
                "Satterthwaite".into(),
                "Unequal".into(),
                test.satt_df.map(fmt_df).unwrap_or_else(|| ".".into()),
                fmt_t(test.satt_t),
                fmt_p(test.satt_p),
                fmt_cl(satt_cl.0),
                fmt_cl(satt_cl.1),
            ],
        ];
        session.listing.write_table(&theaders, &taligns, &trows);
        session.listing.blank();

        // --- Equality of Variances (folding F-test) ---
        centered(session, "Equality of Variances");
        session.listing.blank();
        let fheaders: Vec<String> = vec![
            "Method".into(),
            "Num DF".into(),
            "Den DF".into(),
            "F Value".into(),
            "Pr > F".into(),
        ];
        let faligns = vec![Align::Left, Align::Right, Align::Right, Align::Right, Align::Right];
        let frow = match test.f_test {
            Some((f, df1, df2, p)) => vec![
                "Folded F".into(),
                fmt_df(df1),
                fmt_df(df2),
                format!("{f:.2}"),
                fmt_p(Some(p)),
            ],
            None => vec!["Folded F".into(), ".".into(), ".".into(), ".".into(), ".".into()],
        };
        session.listing.write_table(&fheaders, &faligns, &[frow]);
        session.listing.blank();

        // OUT= row: the pooled difference test, labelled by the variable.
        out_rows.push(OutRow {
            variable: vname,
            n: sa.n + sb.n,
            mean: test.diff,
            std: None,
            stderr: None,
            df: test.pooled_df,
            t: test.pooled_t,
            probt: test.pooled_p,
        });
    }

    Ok(out_rows)
}

/// Paired path: a one-sample test on the differences of each pair.
fn execute_paired(
    ast: &TTestAst,
    session: &mut Session,
    ds: &SasDataset,
    resolve_num: &dyn Fn(&str) -> Result<usize>,
) -> Result<Vec<OutRow>> {
    let opts = &ast.proc_options;

    // Compute per-pair statistics first.
    let mut summaries: Vec<(String, SampleStats, OneSampleTest)> = Vec::new();
    for (a, b) in &ast.paired_vars {
        let ca = resolve_num(a)?;
        let cb = resolve_num(b)?;
        let acol = decode_column(ds, ca)?;
        let bcol = decode_column(ds, cb)?;
        let d = paired_diff(&acol, &bcol);
        let (s, test) = one_sample_stats(&d, 0.0, opts.alpha, opts.sides);
        let label = format!("{} - {}", ds.vars[ca].name, ds.vars[cb].name);
        summaries.push((label, s, test));
    }

    // --- Statistics table ---
    let headers: Vec<String> = vec![
        "Difference".into(),
        "N".into(),
        "Mean".into(),
        "Std Dev".into(),
        "Std Err".into(),
        "Minimum".into(),
        "Maximum".into(),
    ];
    let aligns = vec![Align::Left, Align::Right, Align::Right, Align::Right, Align::Right, Align::Right, Align::Right];
    let mut rows: Vec<Vec<String>> = Vec::with_capacity(summaries.len());
    for (label, s, _t) in &summaries {
        rows.push(vec![
            label.clone(),
            format!("{}", s.n),
            fmt_opt(s.mean),
            fmt_opt(s.std),
            fmt_opt(s.stderr),
            fmt_opt(s.min),
            fmt_opt(s.max),
        ]);
    }
    session.listing.write_table(&headers, &aligns, &rows);
    session.listing.blank();

    // --- T-Tests table ---
    centered(session, "T-Tests");
    session.listing.blank();
    let theaders: Vec<String> = vec![
        "Difference".into(),
        "DF".into(),
        "t Value".into(),
        "Pr > |t|".into(),
        "Lower CL Mean".into(),
        "Upper CL Mean".into(),
    ];
    let taligns = vec![Align::Left, Align::Right, Align::Right, Align::Right, Align::Right, Align::Right];
    let mut trows: Vec<Vec<String>> = Vec::with_capacity(summaries.len());
    for (label, _s, t) in &summaries {
        let (lo, hi) = match t.cl {
            Some((l, h)) => (Some(l), Some(h)),
            None => (None, None),
        };
        trows.push(vec![
            label.clone(),
            fmt_df(t.df),
            fmt_t(t.t),
            fmt_p(t.p),
            fmt_cl(lo),
            fmt_cl(hi),
        ]);
    }
    session.listing.write_table(&theaders, &taligns, &trows);
    session.listing.blank();

    Ok(summaries
        .into_iter()
        .map(|(label, s, t)| OutRow {
            variable: label,
            n: s.n,
            mean: s.mean,
            std: s.std,
            stderr: s.stderr,
            df: t.df,
            t: t.t,
            probt: t.p,
        })
        .collect())
}

// ───────────────────────── OUT= dataset ─────────────────────────

/// Build a simplified Ttest OUT= dataset. ODS OUTPUT table-name routing
/// (TTEST=, CONFLIMITS=, EQUALITY=) is deferred in v1; OUT= produces a single
/// table: Variable, N, Mean, StdDev, StdErr, DF, tValue, Probt. Mirrors
/// corr::build_out_dataset's column-assembly idioms.
fn build_out_dataset(rows: &[OutRow]) -> Result<SasDataset> {
    let var_col: Vec<Option<String>> = rows.iter().map(|r| Some(r.variable.clone())).collect();
    let n_col: Vec<Option<f64>> = rows.iter().map(|r| Some(r.n as f64)).collect();
    let mean_col: Vec<Option<f64>> = rows.iter().map(|r| r.mean).collect();
    let std_col: Vec<Option<f64>> = rows.iter().map(|r| r.std).collect();
    let stderr_col: Vec<Option<f64>> = rows.iter().map(|r| r.stderr).collect();
    let df_col: Vec<Option<f64>> = rows.iter().map(|r| Some(r.df)).collect();
    let t_col: Vec<Option<f64>> = rows.iter().map(|r| r.t).collect();
    let p_col: Vec<Option<f64>> = rows.iter().map(|r| r.probt).collect();

    let columns: Vec<Column> = vec![
        Series::new("Variable".into(), var_col).into(),
        Series::new("N".into(), n_col).into(),
        Series::new("Mean".into(), mean_col).into(),
        Series::new("StdDev".into(), std_col).into(),
        Series::new("StdErr".into(), stderr_col).into(),
        Series::new("DF".into(), df_col).into(),
        Series::new("tValue".into(), t_col).into(),
        Series::new("Probt".into(), p_col).into(),
    ];
    let vars: Vec<VarMeta> = vec![
        char_var_meta("Variable", 32),
        num_var_meta("N"),
        num_var_meta("Mean"),
        num_var_meta("StdDev"),
        num_var_meta("StdErr"),
        num_var_meta("DF"),
        num_var_meta("tValue"),
        num_var_meta("Probt"),
    ];

    let df = DataFrame::new(columns)?;
    Ok(SasDataset { df, vars })
}

/// Persist a built dataset to `target`, update `_LAST_`, emit the creation
/// NOTE (mirrors corr::write_out_dataset).
fn write_out_dataset(session: &mut Session, target: &DatasetRef, out_ds: SasDataset) -> Result<()> {
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

fn char_var_meta(name: &str, length: usize) -> VarMeta {
    VarMeta {
        name: name.to_string(),
        ty: VarType::Char,
        length,
        format: None,
        label: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dataset::{SasDataset, VarMeta};
    use crate::missing::value_to_num;
    use crate::session::Session;
    use crate::source::SourceFile;
    use crate::value::VarType;
    use polars::df;
    use std::path::PathBuf;

    fn make_session() -> Session {
        Session::new(None, PathBuf::from("."), true).unwrap()
    }

    fn num_meta(name: &str) -> VarMeta {
        VarMeta { name: name.to_string(), ty: VarType::Num, length: 8, format: None, label: None }
    }

    fn char_meta(name: &str) -> VarMeta {
        VarMeta { name: name.to_string(), ty: VarType::Char, length: 4, format: None, label: None }
    }

    fn write_dataset(session: &mut Session, table: &str, ds: SasDataset) {
        session.libs.get("WORK").unwrap().write(table, &ds).unwrap();
        session.last_dataset = Some(format!("WORK.{}", table.to_uppercase()));
    }

    fn parse_ttest(src: &str) -> Result<TTestAst> {
        let source = SourceFile::new(src);
        let mut ts = StatementStream::new(&source).unwrap();
        ts.next(); // "proc"
        ts.next(); // "ttest"
        parse(&mut ts)
    }

    fn work_ref(name: &str) -> DatasetRef {
        DatasetRef { libref: Some("WORK".into()), name: name.into() }
    }

    // ───────────── parse tests ─────────────

    #[test]
    fn parse_minimal() {
        let ast = parse_ttest("proc ttest data=a; run;").unwrap();
        assert_eq!(ast.data_options.input.as_ref().unwrap().name, "a");
        assert_eq!(ast.proc_options.h0, 0.0);
        assert!((ast.proc_options.alpha - 0.05).abs() < 1e-12);
        assert!(ast.proc_options.equal);
        assert!(ast.var_vars.is_empty());
        assert!(ast.class_var.is_none());
        assert!(ast.paired_vars.is_empty());
    }

    #[test]
    fn parse_options() {
        let ast =
            parse_ttest("proc ttest data=a h0=5 alpha=0.10 equal=no sides=u; var x y; run;").unwrap();
        assert_eq!(ast.proc_options.h0, 5.0);
        assert!((ast.proc_options.alpha - 0.10).abs() < 1e-12);
        assert!(!ast.proc_options.equal);
        assert!(matches!(ast.proc_options.sides, TTestSides::Upper));
        assert_eq!(ast.var_vars, vec!["x", "y"]);
    }

    #[test]
    fn parse_class_statement() {
        let ast = parse_ttest("proc ttest data=a; class grp; var x; run;").unwrap();
        assert_eq!(ast.class_var.as_deref(), Some("grp"));
        assert_eq!(ast.var_vars, vec!["x"]);
    }

    #[test]
    fn parse_paired_statement() {
        let ast = parse_ttest("proc ttest data=a; paired pre*post hi*lo; run;").unwrap();
        assert_eq!(
            ast.paired_vars,
            vec![("pre".into(), "post".into()), ("hi".into(), "lo".into())]
        );
    }

    #[test]
    fn parse_negative_h0() {
        let ast = parse_ttest("proc ttest data=a h0=-2.5; run;").unwrap();
        assert_eq!(ast.proc_options.h0, -2.5);
    }

    #[test]
    fn parse_unknown_option_errors() {
        let r = parse_ttest("proc ttest data=a bogus; run;");
        assert!(r.is_err());
        assert!(r.err().unwrap().to_string().contains("BOGUS"));
    }

    #[test]
    fn parse_multiple_class_errors() {
        let r = parse_ttest("proc ttest data=a; class g1 g2; run;");
        assert!(r.is_err());
    }

    #[test]
    fn parse_by_deferred() {
        let r = parse_ttest("proc ttest data=a; by g; var x; run;");
        assert!(r.is_err());
        assert!(r.err().unwrap().to_string().contains("BY processing"));
    }

    #[test]
    fn parse_bad_alpha_errors() {
        assert!(parse_ttest("proc ttest data=a alpha=1.5; run;").is_err());
        assert!(parse_ttest("proc ttest data=a alpha=0; run;").is_err());
    }

    // ───────────── numeric core tests ─────────────

    #[test]
    fn one_sample_oracle() {
        // x=[1,2,3,4,5], H0=0: mean=3, s=sqrt(2.5)=1.5811388, n=5,
        // stderr=0.7071068, t=4.2426407, df=4, two-sided p≈0.013333.
        let xs = [1.0, 2.0, 3.0, 4.0, 5.0];
        let (s, t) = one_sample_stats(&xs, 0.0, 0.05, TTestSides::TwoTailed);
        assert_eq!(s.n, 5);
        assert!((s.mean.unwrap() - 3.0).abs() < 1e-9);
        assert!((s.std.unwrap() - 1.5811388300841898).abs() < 1e-5);
        assert!((s.stderr.unwrap() - 0.7071067811865476).abs() < 1e-5);
        assert!((t.df - 4.0).abs() < 1e-12);
        assert!((t.t.unwrap() - 4.242640687119285).abs() < 1e-5);
        let p = t.p.unwrap();
        assert!(p > 0.005 && p < 0.02, "p={p}");
        assert!((p - 0.013333).abs() < 1e-4, "p={p}");
    }

    #[test]
    fn one_sample_constant_is_missing() {
        // Constant sample: std=0, no test (t/p None), no panic.
        let xs = [4.0, 4.0, 4.0, 4.0];
        let (s, t) = one_sample_stats(&xs, 0.0, 0.05, TTestSides::TwoTailed);
        assert_eq!(s.n, 4);
        assert!(t.t.is_none());
        assert!(t.p.is_none());
    }

    #[test]
    fn one_sample_empty_no_panic() {
        let (s, t) = one_sample_stats(&[], 0.0, 0.05, TTestSides::TwoTailed);
        assert_eq!(s.n, 0);
        assert!(s.mean.is_none());
        assert!(t.t.is_none());
    }

    #[test]
    fn two_sample_pooled_oracle() {
        // A=[1..5] (mean3,var2.5), B=[6..10] (mean8,var2.5): pooled var=2.5,
        // SE=1.0, t=(3-8)/1.0=-5.0, df=8.
        let a = [1.0, 2.0, 3.0, 4.0, 5.0];
        let b = [6.0, 7.0, 8.0, 9.0, 10.0];
        let (sa, sb, test) = pooled_two_sample(&a, &b, 0.05, TTestSides::TwoTailed);
        assert_eq!(sa.n, 5);
        assert_eq!(sb.n, 5);
        assert!((test.diff.unwrap() + 5.0).abs() < 1e-9);
        assert!((test.pooled_df - 8.0).abs() < 1e-12);
        assert!((test.pooled_t.unwrap() + 5.0).abs() < 1e-9, "t={:?}", test.pooled_t);
        // Equal variances → Satterthwaite df ≈ 8 too; F=1.0.
        let (f, _, _, _) = test.f_test.unwrap();
        assert!((f - 1.0).abs() < 1e-9, "F={f}");
        assert!(test.pooled_p.unwrap() < 0.01, "p={:?}", test.pooled_p);
    }

    #[test]
    fn two_sample_satterthwaite_present() {
        // Unequal variances exercise the Satterthwaite df formula.
        let a = [1.0, 2.0, 3.0, 4.0, 5.0];
        let b = [10.0, 20.0, 30.0, 40.0, 50.0];
        let (_, _, test) = pooled_two_sample(&a, &b, 0.05, TTestSides::TwoTailed);
        let dfw = test.satt_df.unwrap();
        // Satterthwaite df lies between min(n-1)=4 and pooled df=8.
        assert!(dfw > 3.9 && dfw < 8.01, "dfw={dfw}");
        assert!(test.satt_t.is_some());
    }

    #[test]
    fn paired_oracle() {
        // a=[5,6,7,8], b=[3,3,4,5]: d=[2,3,3,3], dbar=2.75, s_d=0.5, n=4,
        // stderr=0.25, t=11.0, df=3.
        let a: Vec<Value> = [5.0, 6.0, 7.0, 8.0].iter().map(|v| Value::Num(*v)).collect();
        let b: Vec<Value> = [3.0, 3.0, 4.0, 5.0].iter().map(|v| Value::Num(*v)).collect();
        let d = paired_diff(&a, &b);
        assert_eq!(d, vec![2.0, 3.0, 3.0, 3.0]);
        let (s, t) = one_sample_stats(&d, 0.0, 0.05, TTestSides::TwoTailed);
        assert_eq!(s.n, 4);
        assert!((s.mean.unwrap() - 2.75).abs() < 1e-9);
        assert!((s.std.unwrap() - 0.5).abs() < 1e-9);
        assert!((s.stderr.unwrap() - 0.25).abs() < 1e-9);
        assert!((t.df - 3.0).abs() < 1e-12);
        assert!((t.t.unwrap() - 11.0).abs() < 1e-9, "t={:?}", t.t);
    }

    #[test]
    fn paired_excludes_incomplete() {
        let a = vec![Value::Num(5.0), Value::missing(), Value::Num(7.0)];
        let b = vec![Value::Num(3.0), Value::Num(2.0), Value::missing()];
        // Only row 0 is complete.
        let d = paired_diff(&a, &b);
        assert_eq!(d, vec![2.0]);
    }

    #[test]
    fn t_quantile_matches_known() {
        // t_{0.975, 10} ≈ 2.228139.
        assert!((t_quantile(0.975, 10.0) - 2.228138852).abs() < 1e-6);
        // Symmetric median.
        assert!(t_quantile(0.5, 5.0).abs() < 1e-9);
    }

    #[test]
    fn fmt_helpers() {
        assert_eq!(fmt_p(Some(0.00001)), "<.0001");
        assert_eq!(fmt_p(Some(0.1234)), "0.1234");
        assert_eq!(fmt_p(None), ".");
        assert_eq!(fmt_t(Some(-5.0)), "-5.00");
        assert_eq!(fmt_t(None), ".");
        assert_eq!(fmt_df(8.0), "8");
        assert_eq!(fmt_df(7.96), "7.96");
    }

    // ───────────── execute tests ─────────────

    #[test]
    fn execute_one_sample_listing() {
        let mut session = make_session();
        let df = df!["x" => [1.0_f64, 2.0, 3.0, 4.0, 5.0]].unwrap();
        let ds = SasDataset { df, vars: vec![num_meta("x")] };
        write_dataset(&mut session, "T", ds);

        let mut ast = TTestAst {
            data_options: TTestDataOptions { input: Some(work_ref("T")), output: None },
            proc_options: TTestProcOptions::default(),
            var_vars: vec![],
            class_var: None,
            paired_vars: vec![],
        };
        ast.proc_options.h0 = 0.0;
        execute(&ast, &mut session).unwrap();

        let listing = session.listing.into_string();
        assert!(listing.contains("The TTEST Procedure"), "{listing}");
        assert!(listing.contains("T-Tests"), "{listing}");
        // t = 4.24, df = 4.
        assert!(listing.contains("4.24"), "{listing}");
    }

    #[test]
    fn execute_two_sample_listing() {
        let mut session = make_session();
        let df = df![
            "g" => ["A", "A", "A", "A", "A", "B", "B", "B", "B", "B"],
            "x" => [1.0_f64, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0]
        ]
        .unwrap();
        let ds = SasDataset { df, vars: vec![char_meta("g"), num_meta("x")] };
        write_dataset(&mut session, "T", ds);

        let ast = TTestAst {
            data_options: TTestDataOptions { input: Some(work_ref("T")), output: None },
            proc_options: TTestProcOptions::default(),
            var_vars: vec!["x".into()],
            class_var: Some("g".into()),
            paired_vars: vec![],
        };
        execute(&ast, &mut session).unwrap();

        let listing = session.listing.into_string();
        assert!(listing.contains("Pooled"), "{listing}");
        assert!(listing.contains("Satterthwaite"), "{listing}");
        assert!(listing.contains("Equality of Variances"), "{listing}");
        // t = -5.00.
        assert!(listing.contains("-5.00"), "{listing}");
    }

    #[test]
    fn execute_two_sample_too_many_levels_errors() {
        let mut session = make_session();
        let df = df![
            "g" => ["A", "B", "C"],
            "x" => [1.0_f64, 2.0, 3.0]
        ]
        .unwrap();
        let ds = SasDataset { df, vars: vec![char_meta("g"), num_meta("x")] };
        write_dataset(&mut session, "T", ds);

        let ast = TTestAst {
            data_options: TTestDataOptions { input: Some(work_ref("T")), output: None },
            proc_options: TTestProcOptions::default(),
            var_vars: vec!["x".into()],
            class_var: Some("g".into()),
            paired_vars: vec![],
        };
        let r = execute(&ast, &mut session);
        assert!(r.is_err());
        assert!(r.err().unwrap().to_string().contains("exactly two levels"));
    }

    #[test]
    fn execute_paired_listing() {
        let mut session = make_session();
        let df = df![
            "a" => [5.0_f64, 6.0, 7.0, 8.0],
            "b" => [3.0_f64, 3.0, 4.0, 5.0]
        ]
        .unwrap();
        let ds = SasDataset { df, vars: vec![num_meta("a"), num_meta("b")] };
        write_dataset(&mut session, "T", ds);

        let ast = TTestAst {
            data_options: TTestDataOptions { input: Some(work_ref("T")), output: None },
            proc_options: TTestProcOptions::default(),
            var_vars: vec![],
            class_var: None,
            paired_vars: vec![("a".into(), "b".into())],
        };
        execute(&ast, &mut session).unwrap();

        let listing = session.listing.into_string();
        assert!(listing.contains("a - b"), "{listing}");
        // t = 11.00.
        assert!(listing.contains("11.00"), "{listing}");
    }

    #[test]
    fn execute_out_dataset() {
        let mut session = make_session();
        let df = df!["x" => [1.0_f64, 2.0, 3.0, 4.0, 5.0]].unwrap();
        let ds = SasDataset { df, vars: vec![num_meta("x")] };
        write_dataset(&mut session, "T", ds);

        let ast = TTestAst {
            data_options: TTestDataOptions {
                input: Some(work_ref("T")),
                output: Some(work_ref("OUT")),
            },
            proc_options: TTestProcOptions::default(),
            var_vars: vec![],
            class_var: None,
            paired_vars: vec![],
        };
        execute(&ast, &mut session).unwrap();

        let (out, _) = session.libs.get("WORK").unwrap().read("OUT").unwrap();
        let names: Vec<String> = out.vars.iter().map(|v| v.name.clone()).collect();
        assert_eq!(
            names,
            vec!["Variable", "N", "Mean", "StdDev", "StdErr", "DF", "tValue", "Probt"]
        );
        assert_eq!(out.n_obs(), 1);
        let tcol = decode_column(&out, 6).unwrap();
        assert!((value_to_num(&tcol[0]).unwrap() - 4.242640687).abs() < 1e-5);
        assert_eq!(session.last_dataset.as_deref(), Some("WORK.OUT"));
    }

    #[test]
    fn execute_default_var_all_numeric() {
        let mut session = make_session();
        let df = df![
            "x" => [1.0_f64, 2.0, 3.0],
            "g" => ["a", "b", "c"],
            "y" => [3.0_f64, 2.0, 1.0]
        ]
        .unwrap();
        let ds = SasDataset { df, vars: vec![num_meta("x"), char_meta("g"), num_meta("y")] };
        write_dataset(&mut session, "T", ds);

        let ast = TTestAst {
            data_options: TTestDataOptions { input: Some(work_ref("T")), output: None },
            proc_options: TTestProcOptions::default(),
            var_vars: vec![],
            class_var: None,
            paired_vars: vec![],
        };
        execute(&ast, &mut session).unwrap();
        let listing = session.listing.into_string();
        // Both numeric vars analyzed; char g excluded (no panic).
        assert!(listing.contains("x"), "{listing}");
        assert!(listing.contains("y"), "{listing}");
    }
}
