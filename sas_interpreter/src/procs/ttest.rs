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
//!
//! ### Two-sample t-test (CLASS)
//! - Statement: `CLASS var;` where var has 2 distinct non-missing values
//! - Error if >2 classes (reject gracefully)
//! - **Pooled variance** (default): s²_p = [(n_A-1)s²_A + (n_B-1)s²_B] / (n_A+n_B-2)
//!   t_pool = (x̄_A - x̄_B) / √(s²_p·(1/n_A + 1/n_B)), df = n_A+n_B-2
//! - **Satterthwaite** (unequal variance correction):
//!   t_satt = (x̄_A - x̄_B) / √(s²_A/n_A + s²_B/n_B)
//!   df = (s²_A/n_A + s²_B/n_B)² / [(s²_A/n_A)²/(n_A-1) + (s²_B/n_B)²/(n_B-1)]
//! - **Folded F-test for equal variances**:
//!   F = max(s²_A, s²_B) / min(s²_A, s²_B), Prob > F = 2 · min(F_cdf tail).
//!
//! ### Paired t-test
//! - Statement: `PAIRED x*y z*w;` → paired differences per pair
//! - Differences: d_i = x_i - y_i, one-sample test on differences.
//!
//! ### Options
//! - **ALPHA=** (default 0.05): significance level for CIs (must be in (0,1)).
//! - **H0=** (default 0): null hypothesis mean (1-sample / paired).
//! - **EQUAL** : kept in the AST; two-sample always prints both methods (SAS).
//! - **CI=** (default 95): confidence level as percentage.
//! - **SIDES=2|U|L**: parsed; CI/p-value computed two-sided in this increment.
//!
//! ### Error handling
//! - Missing values → excluded from analysis (special missings decode to NaN
//!   via `value_to_num` and are excluded), counted as NMiss.
//! - Constant variable (s=0) → t/p missing.
//! - n<2 → cannot compute std → handled gracefully (no panic).
//! - CLASS with >2 distinct values → error.
//! - CLASS and PAIRED mutually exclusive (rejected at parse time).
//! - ALPHA not in (0, 1) → error.
//!
//! ### Documented deferrals
//! - BY processing: parser recognizes BY and returns a clean error.
//! - One-sided (SIDES=U/L) p-values/CIs: parsed but computed two-sided.
//! - Exact / non-parametric alternatives: out of scope (PROC NPAR1WAY).

use crate::ast::DatasetRef;
use crate::dataset::{SasDataset, VarMeta};
use crate::error::{Result, SasError};
use crate::listing::Align;
use crate::missing::value_to_num;
use crate::parser::StatementStream;
use crate::procs::common::{decode_column, partition_numeric, sample_std, t_quantile};
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

// ─────────────────────────────── parsing ───────────────────────────────

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

/// Read a numeric value (after `=`), accepting a leading sign.
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
/// consumed. Consumes through `run;` / `quit;`.
pub fn parse(ts: &mut StatementStream) -> Result<TTestAst> {
    let mut input: Option<DatasetRef> = None;
    let mut output: Option<DatasetRef> = None;
    let mut opts = TTestProcOptions::default();

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
            opts.alpha = parse_number(ts, "ALPHA")?;
        } else if ts.peek().is_kw("ci") {
            ts.next();
            expect_eq(ts, "CI")?;
            opts.ci = parse_number(ts, "CI")?;
        } else if ts.peek().is_kw("sides") {
            ts.next();
            expect_eq(ts, "SIDES")?;
            let tok = ts.peek().clone();
            let sval = match &tok.kind {
                TokenKind::Num(f) if *f == 2.0 => {
                    ts.next();
                    TTestSides::TwoTailed
                }
                TokenKind::Ident(s) if s.eq_ignore_ascii_case("u") => {
                    ts.next();
                    TTestSides::Upper
                }
                TokenKind::Ident(s) if s.eq_ignore_ascii_case("l") => {
                    ts.next();
                    TTestSides::Lower
                }
                _ => {
                    return Err(SasError::parse(
                        "expected 2, U or L after SIDES=",
                        tok.span,
                    ));
                }
            };
            opts.sides = sval;
        } else if ts.peek().is_kw("equal") {
            ts.next();
            opts.equal = true;
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
            paired_vars = parse_paired_list(ts)?;
            ts.expect_semi()?;
        } else if ts.peek().is_kw("by") {
            return Err(SasError::runtime(
                "BY processing not yet implemented in PROC TTEST.",
            ));
        } else {
            // Unknown sub-statement: recovery, like other procs.
            ts.skip_to_semi();
        }
    }

    // Validation: CLASS and PAIRED are mutually exclusive.
    if class_var.is_some() && !paired_vars.is_empty() {
        return Err(SasError::runtime(
            "The CLASS and PAIRED statements cannot be used together in PROC TTEST.",
        ));
    }
    // ALPHA must be in (0, 1).
    if !(opts.alpha > 0.0 && opts.alpha < 1.0) {
        return Err(SasError::runtime(format!(
            "The ALPHA= value {} must be between 0 and 1.",
            opts.alpha
        )));
    }

    Ok(TTestAst {
        data_options: TTestDataOptions { input, output },
        proc_options: opts,
        var_vars,
        class_var,
        paired_vars,
    })
}

/// Parse a PAIRED statement body's pairs: `v1*v2 v3*v4 ...` (up to `;`).
fn parse_paired_list(ts: &mut StatementStream) -> Result<Vec<(String, String)>> {
    let mut pairs: Vec<(String, String)> = Vec::new();
    loop {
        if ts.peek().kind == TokenKind::Semi || ts.peek().kind == TokenKind::Eof {
            break;
        }
        let lhs = match ts.peek().ident().map(str::to_string) {
            Some(n) => {
                ts.next();
                n
            }
            None => {
                return Err(SasError::parse(
                    "expected a variable name in the PAIRED statement",
                    ts.peek().span,
                ));
            }
        };
        if ts.peek().kind != TokenKind::Star {
            return Err(SasError::parse(
                "expected '*' between paired variables in the PAIRED statement",
                ts.peek().span,
            ));
        }
        ts.next(); // '*'
        let rhs = match ts.peek().ident().map(str::to_string) {
            Some(n) => {
                ts.next();
                n
            }
            None => {
                return Err(SasError::parse(
                    "expected a variable name after '*' in the PAIRED statement",
                    ts.peek().span,
                ));
            }
        };
        pairs.push((lhs, rhs));
    }
    if pairs.is_empty() {
        return Err(SasError::runtime(
            "The PAIRED statement of PROC TTEST requires at least one pair.",
        ));
    }
    Ok(pairs)
}

// ─────────────────────────── statistics core ───────────────────────────

/// Summary statistics over the non-missing numeric values of a sample.
#[derive(Clone, Copy, Debug)]
struct Summary {
    n: usize,
    mean: Option<f64>,
    std: Option<f64>,
    stderr: Option<f64>,
    min: Option<f64>,
    max: Option<f64>,
}

/// Compute summary stats from already-extracted non-missing values and the
/// missing count.
fn summarize(xs: &[f64], _nmiss: usize) -> Summary {
    let n = xs.len();
    let mean = if n > 0 {
        Some(xs.iter().sum::<f64>() / n as f64)
    } else {
        None
    };
    let std = sample_std(xs);
    let stderr = match std {
        Some(s) if n >= 1 => Some(s / (n as f64).sqrt()),
        _ => None,
    };
    let min = if n > 0 {
        Some(xs.iter().cloned().fold(f64::INFINITY, f64::min))
    } else {
        None
    };
    let max = if n > 0 {
        Some(xs.iter().cloned().fold(f64::NEG_INFINITY, f64::max))
    } else {
        None
    };
    Summary {
        n,
        mean,
        std,
        stderr,
        min,
        max,
    }
}

/// One-sample / paired t-test result against `h0`.
#[derive(Clone, Copy, Debug)]
struct OneSampleTest {
    df: Option<f64>,
    t: Option<f64>,
    p: Option<f64>,
    lower_cl: Option<f64>,
    upper_cl: Option<f64>,
}

/// Two-sided p-value for Student t: P(|T_df| > |t|) = 2·(1 - CDF(|t|)).
fn two_sided_p(t: f64, df: f64) -> f64 {
    (2.0 * (1.0 - student_t_cdf(t.abs(), df))).clamp(0.0, 1.0)
}

/// One-sample t-test for a summary vs `h0`, with a `100(1-alpha)%` CI for the
/// mean. Undefined components (n<2, constant) → None.
fn one_sample_test(s: &Summary, h0: f64, alpha: f64) -> OneSampleTest {
    let (mean, std, stderr) = match (s.mean, s.std, s.stderr) {
        (Some(m), Some(sd), Some(se)) if s.n >= 2 && sd > 0.0 && se > 0.0 => (m, sd, se),
        _ => {
            // CI may still be reported as missing; t/p undefined.
            return OneSampleTest {
                df: if s.n >= 2 { Some((s.n - 1) as f64) } else { None },
                t: None,
                p: None,
                lower_cl: None,
                upper_cl: None,
            };
        }
    };
    let _ = std;
    let df = (s.n - 1) as f64;
    let t = (mean - h0) / stderr;
    let p = two_sided_p(t, df);
    let tcrit = t_quantile(1.0 - alpha / 2.0, df);
    let half = tcrit * stderr;
    OneSampleTest {
        df: Some(df),
        t: Some(t),
        p: Some(p),
        lower_cl: Some(mean - half),
        upper_cl: Some(mean + half),
    }
}

/// Two-sample pooled / Satterthwaite results.
#[derive(Clone, Copy, Debug)]
struct TwoSampleTest {
    diff: Option<f64>,
    // Pooled.
    pooled_df: Option<f64>,
    pooled_t: Option<f64>,
    pooled_p: Option<f64>,
    pooled_lcl: Option<f64>,
    pooled_ucl: Option<f64>,
    // Satterthwaite.
    satt_df: Option<f64>,
    satt_t: Option<f64>,
    satt_p: Option<f64>,
    satt_lcl: Option<f64>,
    satt_ucl: Option<f64>,
    // Folded F test for equality of variances.
    f_value: Option<f64>,
    f_num_df: Option<f64>,
    f_den_df: Option<f64>,
    f_p: Option<f64>,
}

/// Compute pooled and Satterthwaite two-sample tests plus the folded F-test.
/// `a` is the first class level, `b` the second; diff = mean_a - mean_b.
fn two_sample_test(a: &Summary, b: &Summary, alpha: f64) -> TwoSampleTest {
    let mut out = TwoSampleTest {
        diff: None,
        pooled_df: None,
        pooled_t: None,
        pooled_p: None,
        pooled_lcl: None,
        pooled_ucl: None,
        satt_df: None,
        satt_t: None,
        satt_p: None,
        satt_lcl: None,
        satt_ucl: None,
        f_value: None,
        f_num_df: None,
        f_den_df: None,
        f_p: None,
    };

    let (ma, mb) = match (a.mean, b.mean) {
        (Some(ma), Some(mb)) => (ma, mb),
        _ => return out,
    };
    out.diff = Some(ma - mb);

    // Need n>=2 in both groups and defined std for the tests.
    let (na, nb) = (a.n as f64, b.n as f64);
    let (sa, sb) = match (a.std, b.std) {
        (Some(sa), Some(sb)) if a.n >= 2 && b.n >= 2 => (sa, sb),
        _ => return out,
    };
    let (va, vb) = (sa * sa, sb * sb);

    // --- Pooled ---
    let pooled_df = na + nb - 2.0;
    if pooled_df > 0.0 {
        let sp2 = ((na - 1.0) * va + (nb - 1.0) * vb) / pooled_df;
        let se = (sp2 * (1.0 / na + 1.0 / nb)).sqrt();
        out.pooled_df = Some(pooled_df);
        if se > 0.0 {
            let t = (ma - mb) / se;
            out.pooled_t = Some(t);
            out.pooled_p = Some(two_sided_p(t, pooled_df));
            let tcrit = t_quantile(1.0 - alpha / 2.0, pooled_df);
            let half = tcrit * se;
            out.pooled_lcl = Some((ma - mb) - half);
            out.pooled_ucl = Some((ma - mb) + half);
        }
    }

    // --- Satterthwaite ---
    let ua = va / na;
    let ub = vb / nb;
    let se = (ua + ub).sqrt();
    if se > 0.0 {
        let num = (ua + ub) * (ua + ub);
        let den = (ua * ua) / (na - 1.0) + (ub * ub) / (nb - 1.0);
        if den > 0.0 {
            let df = num / den;
            out.satt_df = Some(df);
            let t = (ma - mb) / se;
            out.satt_t = Some(t);
            out.satt_p = Some(two_sided_p(t, df));
            let tcrit = t_quantile(1.0 - alpha / 2.0, df);
            let half = tcrit * se;
            out.satt_lcl = Some((ma - mb) - half);
            out.satt_ucl = Some((ma - mb) + half);
        }
    }

    // --- Folded F test for equal variances ---
    if va > 0.0 && vb > 0.0 {
        let (fmax, dfn, fmin, dfd) = if va >= vb {
            (va, na - 1.0, vb, nb - 1.0)
        } else {
            (vb, nb - 1.0, va, na - 1.0)
        };
        let f = fmax / fmin;
        out.f_value = Some(f);
        out.f_num_df = Some(dfn);
        out.f_den_df = Some(dfd);
        // Two-tailed folded F p-value: 2·P(F > f) (upper tail since f>=1).
        let upper = 1.0 - f_cdf(f, dfn, dfd);
        out.f_p = Some((2.0 * upper).clamp(0.0, 1.0));
    }

    out
}

// ─────────────────────────────── formatting ───────────────────────────────

fn fmt_num(v: Option<f64>) -> String {
    match v {
        Some(f) => format_best(f, 12),
        None => ".".to_string(),
    }
}

fn fmt_p_cell(p: Option<f64>) -> String {
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

fn fmt_int(v: Option<f64>) -> String {
    match v {
        Some(f) => format!("{}", f.round() as i64),
        None => ".".to_string(),
    }
}

fn cl_percent(alpha: f64) -> String {
    let pct = 100.0 * (1.0 - alpha);
    let rounded = (pct * 1e6).round() / 1e6;
    if (rounded - rounded.round()).abs() < 1e-9 {
        format!("{}", rounded.round() as i64)
    } else {
        format!("{rounded}")
    }
}

// ─────────────────────────────── execute ───────────────────────────────

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

/// Find a column index by name (case-insensitive), erroring if absent.
fn find_col(ds: &SasDataset, name: &str) -> Result<usize> {
    ds.vars
        .iter()
        .position(|m| m.name.eq_ignore_ascii_case(name))
        .ok_or_else(|| SasError::runtime(format!("Variable {} not found.", name.to_uppercase())))
}

fn require_numeric(ds: &SasDataset, idx: usize) -> Result<()> {
    if ds.vars[idx].ty != VarType::Num {
        return Err(SasError::runtime(format!(
            "Variable {} in the analysis is not numeric.",
            ds.vars[idx].name.to_uppercase()
        )));
    }
    Ok(())
}

fn centered(session: &mut Session, text: &str) {
    let ls = session.listing.ls();
    let pad = ls.saturating_sub(text.len()) / 2;
    session
        .listing
        .write_line(&format!("{}{}", " ".repeat(pad), text));
}

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
    let alpha = ast.proc_options.alpha;

    session.listing.page_header();
    centered(session, "The TTEST Procedure");
    session.listing.blank();

    // Collect ODS / OUT= output rows here for a single TTest output dataset.
    let mut out_rows: Vec<TtestOutRow> = Vec::new();

    if !ast.paired_vars.is_empty() {
        execute_paired(ast, &ds, n_obs, alpha, session, &mut out_rows)?;
    } else if let Some(class_name) = &ast.class_var {
        execute_two_sample(ast, &ds, n_obs, alpha, class_name, session, &mut out_rows)?;
    } else {
        execute_one_sample(ast, &ds, n_obs, alpha, session, &mut out_rows)?;
    }

    // OUT= / ODS OUTPUT TTest dataset.
    let target = ast
        .data_options
        .output
        .clone()
        .or_else(|| session.ods_output_target("TTest"));
    if let Some(target) = target {
        write_out_dataset(session, &target, &out_rows)?;
    }

    Ok(())
}

/// Resolve the VAR list: explicit `var`, else all numeric variables (excluding
/// the CLASS variable when present), in dataset order.
fn resolve_var_cols(ast: &TTestAst, ds: &SasDataset, class_col: Option<usize>) -> Result<Vec<usize>> {
    if !ast.var_vars.is_empty() {
        let mut v = Vec::with_capacity(ast.var_vars.len());
        for name in &ast.var_vars {
            let idx = find_col(ds, name)?;
            require_numeric(ds, idx)?;
            v.push(idx);
        }
        Ok(v)
    } else {
        Ok((0..ds.vars.len())
            .filter(|&i| ds.vars[i].ty == VarType::Num && Some(i) != class_col)
            .collect())
    }
}

// ───────────────────────────── one-sample ─────────────────────────────

fn execute_one_sample(
    ast: &TTestAst,
    ds: &SasDataset,
    n_obs: usize,
    alpha: f64,
    session: &mut Session,
    out_rows: &mut Vec<TtestOutRow>,
) -> Result<()> {
    let var_cols = resolve_var_cols(ast, ds, None)?;
    if var_cols.is_empty() {
        return Err(SasError::runtime(
            "No numeric variables found for PROC TTEST analysis.",
        ));
    }
    let h0 = ast.proc_options.h0;
    let all_rows: Vec<usize> = (0..n_obs).collect();
    let pct = cl_percent(alpha);

    // Statistics table.
    let stat_headers = vec![
        "Variable".into(),
        "N".into(),
        "Mean".into(),
        "Std Dev".into(),
        "Std Err".into(),
        "Minimum".into(),
        "Maximum".into(),
    ];
    let stat_aligns = vec![
        Align::Left,
        Align::Right,
        Align::Right,
        Align::Right,
        Align::Right,
        Align::Right,
        Align::Right,
    ];

    let t_headers = vec![
        "Variable".into(),
        "DF".into(),
        "t Value".into(),
        "Pr > |t|".into(),
        format!("{pct}% CL Lower"),
        format!("{pct}% CL Upper"),
    ];
    let t_aligns = vec![
        Align::Left,
        Align::Right,
        Align::Right,
        Align::Right,
        Align::Right,
        Align::Right,
    ];

    let mut stat_rows: Vec<Vec<String>> = Vec::new();
    let mut t_rows: Vec<Vec<String>> = Vec::new();

    for &c in &var_cols {
        let col = decode_column(ds, c)?;
        let (xs, nmiss) = partition_numeric(&col, &all_rows);
        let s = summarize(&xs, nmiss);
        let test = one_sample_test(&s, h0, alpha);
        let name = ds.vars[c].name.clone();

        stat_rows.push(vec![
            name.clone(),
            format!("{}", s.n),
            fmt_num(s.mean),
            fmt_num(s.std),
            fmt_num(s.stderr),
            fmt_num(s.min),
            fmt_num(s.max),
        ]);
        t_rows.push(vec![
            name.clone(),
            fmt_int(test.df),
            fmt_num(test.t),
            fmt_p_cell(test.p),
            fmt_num(test.lower_cl),
            fmt_num(test.upper_cl),
        ]);

        out_rows.push(TtestOutRow {
            variable: name,
            class: None,
            n: s.n as f64,
            mean: s.mean,
            std_dev: s.std,
            std_err: s.stderr,
            df: test.df,
            t: test.t,
            probt: test.p,
            lower_cl: test.lower_cl,
            upper_cl: test.upper_cl,
        });
    }

    session.listing.write_table(&stat_headers, &stat_aligns, &stat_rows);
    session.listing.blank();
    centered(session, &format!("T-Tests (H0: Mean = {})", format_best(h0, 12)));
    session.listing.blank();
    session.listing.write_table(&t_headers, &t_aligns, &t_rows);
    session.listing.blank();

    session.log.note(&format!(
        "PROC TTEST analyzed {} variables.",
        var_cols.len()
    ));

    Ok(())
}

// ───────────────────────────── two-sample ─────────────────────────────

fn execute_two_sample(
    ast: &TTestAst,
    ds: &SasDataset,
    n_obs: usize,
    alpha: f64,
    class_name: &str,
    session: &mut Session,
    out_rows: &mut Vec<TtestOutRow>,
) -> Result<()> {
    let class_col = find_col(ds, class_name)?;
    let var_cols = resolve_var_cols(ast, ds, Some(class_col))?;
    if var_cols.is_empty() {
        return Err(SasError::runtime(
            "No numeric variables found for PROC TTEST analysis.",
        ));
    }

    // Determine the (exactly two) distinct non-missing class levels in
    // sas_cmp order.
    let class_vals = decode_column(ds, class_col)?;
    let mut levels: Vec<Value> = Vec::new();
    for v in &class_vals {
        if v.is_missing() {
            continue;
        }
        if !levels.iter().any(|l| l.sas_cmp(v) == std::cmp::Ordering::Equal) {
            levels.push(v.clone());
        }
    }
    levels.sort_by(|a, b| a.sas_cmp(b));
    if levels.len() != 2 {
        return Err(SasError::runtime(format!(
            "The CLASS variable {} must have exactly 2 levels (found {}).",
            class_name.to_uppercase(),
            levels.len()
        )));
    }

    // Row indices per level.
    let rows_for = |lvl: &Value| -> Vec<usize> {
        (0..n_obs)
            .filter(|&r| class_vals[r].sas_cmp(lvl) == std::cmp::Ordering::Equal)
            .collect()
    };
    let rows_a = rows_for(&levels[0]);
    let rows_b = rows_for(&levels[1]);
    let label_a = class_cell(&levels[0]);
    let label_b = class_cell(&levels[1]);
    let pct = cl_percent(alpha);

    for &c in &var_cols {
        let col = decode_column(ds, c)?;
        let (xa, na_miss) = partition_numeric(&col, &rows_a);
        let (xb, nb_miss) = partition_numeric(&col, &rows_b);
        let sa = summarize(&xa, na_miss);
        let sb = summarize(&xb, nb_miss);
        let test = two_sample_test(&sa, &sb, alpha);
        let name = ds.vars[c].name.clone();

        // Statistics table: one row per class level + a "Diff" row.
        let stat_headers = vec![
            class_name.to_string(),
            "N".into(),
            "Mean".into(),
            "Std Dev".into(),
            "Std Err".into(),
            "Minimum".into(),
            "Maximum".into(),
        ];
        let stat_aligns = vec![
            Align::Left,
            Align::Right,
            Align::Right,
            Align::Right,
            Align::Right,
            Align::Right,
            Align::Right,
        ];
        let stat_rows = vec![
            vec![
                label_a.clone(),
                format!("{}", sa.n),
                fmt_num(sa.mean),
                fmt_num(sa.std),
                fmt_num(sa.stderr),
                fmt_num(sa.min),
                fmt_num(sa.max),
            ],
            vec![
                label_b.clone(),
                format!("{}", sb.n),
                fmt_num(sb.mean),
                fmt_num(sb.std),
                fmt_num(sb.stderr),
                fmt_num(sb.min),
                fmt_num(sb.max),
            ],
            vec![
                "Diff (1-2)".into(),
                String::new(),
                fmt_num(test.diff),
                String::new(),
                String::new(),
                String::new(),
                String::new(),
            ],
        ];

        centered(
            session,
            &format!("Variable: {} (Class {} = {} vs {})", name, class_name, label_a, label_b),
        );
        session.listing.blank();
        session.listing.write_table(&stat_headers, &stat_aligns, &stat_rows);
        session.listing.blank();

        // T-Tests table: pooled then Satterthwaite.
        let t_headers = vec![
            "Method".into(),
            "Variances".into(),
            "DF".into(),
            "t Value".into(),
            "Pr > |t|".into(),
            format!("{pct}% CL Lower"),
            format!("{pct}% CL Upper"),
        ];
        let t_aligns = vec![
            Align::Left,
            Align::Left,
            Align::Right,
            Align::Right,
            Align::Right,
            Align::Right,
            Align::Right,
        ];
        let t_rows = vec![
            vec![
                "Pooled".into(),
                "Equal".into(),
                fmt_int(test.pooled_df),
                fmt_num(test.pooled_t),
                fmt_p_cell(test.pooled_p),
                fmt_num(test.pooled_lcl),
                fmt_num(test.pooled_ucl),
            ],
            vec![
                "Satterthwaite".into(),
                "Unequal".into(),
                fmt_num(test.satt_df),
                fmt_num(test.satt_t),
                fmt_p_cell(test.satt_p),
                fmt_num(test.satt_lcl),
                fmt_num(test.satt_ucl),
            ],
        ];
        session.listing.write_table(&t_headers, &t_aligns, &t_rows);
        session.listing.blank();

        // Equality of Variances (folded F).
        centered(session, "Equality of Variances");
        session.listing.blank();
        let f_headers = vec![
            "Method".into(),
            "Num DF".into(),
            "Den DF".into(),
            "F Value".into(),
            "Pr > F".into(),
        ];
        let f_aligns = vec![
            Align::Left,
            Align::Right,
            Align::Right,
            Align::Right,
            Align::Right,
        ];
        let f_rows = vec![vec![
            "Folded F".into(),
            fmt_int(test.f_num_df),
            fmt_int(test.f_den_df),
            fmt_num(test.f_value),
            fmt_p_cell(test.f_p),
        ]];
        session.listing.write_table(&f_headers, &f_aligns, &f_rows);
        session.listing.blank();

        // ODS / OUT= rows: one per class level + pooled diff row.
        out_rows.push(TtestOutRow {
            variable: name.clone(),
            class: Some(label_a.clone()),
            n: sa.n as f64,
            mean: sa.mean,
            std_dev: sa.std,
            std_err: sa.stderr,
            df: None,
            t: None,
            probt: None,
            lower_cl: None,
            upper_cl: None,
        });
        out_rows.push(TtestOutRow {
            variable: name.clone(),
            class: Some(label_b.clone()),
            n: sb.n as f64,
            mean: sb.mean,
            std_dev: sb.std,
            std_err: sb.stderr,
            df: None,
            t: None,
            probt: None,
            lower_cl: None,
            upper_cl: None,
        });
        out_rows.push(TtestOutRow {
            variable: name.clone(),
            class: Some("Diff (1-2)".into()),
            n: (sa.n + sb.n) as f64,
            mean: test.diff,
            std_dev: None,
            std_err: None,
            df: test.pooled_df,
            t: test.pooled_t,
            probt: test.pooled_p,
            lower_cl: test.pooled_lcl,
            upper_cl: test.pooled_ucl,
        });
    }

    session.log.note(&format!(
        "PROC TTEST analyzed {} variables.",
        var_cols.len()
    ));

    Ok(())
}

// ─────────────────────────────── paired ───────────────────────────────

fn execute_paired(
    ast: &TTestAst,
    ds: &SasDataset,
    n_obs: usize,
    alpha: f64,
    session: &mut Session,
    out_rows: &mut Vec<TtestOutRow>,
) -> Result<()> {
    let h0 = ast.proc_options.h0;
    let pct = cl_percent(alpha);

    let stat_headers = vec![
        "Difference".into(),
        "N".into(),
        "Mean".into(),
        "Std Dev".into(),
        "Std Err".into(),
        "Minimum".into(),
        "Maximum".into(),
    ];
    let stat_aligns = vec![
        Align::Left,
        Align::Right,
        Align::Right,
        Align::Right,
        Align::Right,
        Align::Right,
        Align::Right,
    ];
    let t_headers = vec![
        "Difference".into(),
        "DF".into(),
        "t Value".into(),
        "Pr > |t|".into(),
        format!("{pct}% CL Lower"),
        format!("{pct}% CL Upper"),
    ];
    let t_aligns = vec![
        Align::Left,
        Align::Right,
        Align::Right,
        Align::Right,
        Align::Right,
        Align::Right,
    ];

    let mut stat_rows: Vec<Vec<String>> = Vec::new();
    let mut t_rows: Vec<Vec<String>> = Vec::new();

    for (lhs, rhs) in &ast.paired_vars {
        let li = find_col(ds, lhs)?;
        let ri = find_col(ds, rhs)?;
        require_numeric(ds, li)?;
        require_numeric(ds, ri)?;
        let lcol = decode_column(ds, li)?;
        let rcol = decode_column(ds, ri)?;

        // Differences over rows where both are non-missing.
        let mut diffs: Vec<f64> = Vec::new();
        let mut nmiss = 0usize;
        for r in 0..n_obs {
            match (value_to_num(&lcol[r]), value_to_num(&rcol[r])) {
                (Some(a), Some(b)) if !a.is_nan() && !b.is_nan() => diffs.push(a - b),
                _ => nmiss += 1,
            }
        }
        let s = summarize(&diffs, nmiss);
        let test = one_sample_test(&s, h0, alpha);
        let label = format!("{} - {}", ds.vars[li].name, ds.vars[ri].name);

        stat_rows.push(vec![
            label.clone(),
            format!("{}", s.n),
            fmt_num(s.mean),
            fmt_num(s.std),
            fmt_num(s.stderr),
            fmt_num(s.min),
            fmt_num(s.max),
        ]);
        t_rows.push(vec![
            label.clone(),
            fmt_int(test.df),
            fmt_num(test.t),
            fmt_p_cell(test.p),
            fmt_num(test.lower_cl),
            fmt_num(test.upper_cl),
        ]);

        out_rows.push(TtestOutRow {
            variable: label,
            class: None,
            n: s.n as f64,
            mean: s.mean,
            std_dev: s.std,
            std_err: s.stderr,
            df: test.df,
            t: test.t,
            probt: test.p,
            lower_cl: test.lower_cl,
            upper_cl: test.upper_cl,
        });
    }

    session.listing.write_table(&stat_headers, &stat_aligns, &stat_rows);
    session.listing.blank();
    centered(session, &format!("Paired T-Tests (H0: Mean = {})", format_best(h0, 12)));
    session.listing.blank();
    session.listing.write_table(&t_headers, &t_aligns, &t_rows);
    session.listing.blank();

    session.log.note(&format!(
        "PROC TTEST analyzed {} variables.",
        ast.paired_vars.len()
    ));

    Ok(())
}

/// Render a class-value cell for the listing / labels.
fn class_cell(v: &Value) -> String {
    match v {
        Value::Num(f) => format_best(*f, 12),
        Value::Missing(k) => k.display(),
        Value::Char(s) => s.trim_end().to_string(),
    }
}

// ─────────────────────────── output dataset ───────────────────────────

/// One row of the TTest output dataset.
struct TtestOutRow {
    variable: String,
    class: Option<String>,
    n: f64,
    mean: Option<f64>,
    std_dev: Option<f64>,
    std_err: Option<f64>,
    df: Option<f64>,
    t: Option<f64>,
    probt: Option<f64>,
    lower_cl: Option<f64>,
    upper_cl: Option<f64>,
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

/// Persist the TTest output dataset (OUT= / ODS OUTPUT TTest), update _LAST_,
/// and emit the SAS creation NOTE. Includes a Class column iff any row has one.
fn write_out_dataset(
    session: &mut Session,
    target: &DatasetRef,
    rows: &[TtestOutRow],
) -> Result<()> {
    let has_class = rows.iter().any(|r| r.class.is_some());

    let var_len = rows.iter().map(|r| r.variable.len()).max().unwrap_or(8).max(8);

    let mut columns: Vec<Column> = Vec::new();
    let mut vars: Vec<VarMeta> = Vec::new();

    let var_col: Vec<Option<String>> =
        rows.iter().map(|r| Some(r.variable.clone())).collect();
    columns.push(Series::new("Variable".into(), var_col).into());
    vars.push(char_var_meta("Variable", var_len));

    if has_class {
        let class_len = rows
            .iter()
            .filter_map(|r| r.class.as_ref().map(|c| c.len()))
            .max()
            .unwrap_or(8)
            .max(8);
        let class_col: Vec<Option<String>> = rows.iter().map(|r| r.class.clone()).collect();
        columns.push(Series::new("Class".into(), class_col).into());
        vars.push(char_var_meta("Class", class_len));
    }

    let n_col: Vec<Option<f64>> = rows.iter().map(|r| Some(r.n)).collect();
    columns.push(Series::new("N".into(), n_col).into());
    vars.push(num_var_meta("N"));

    macro_rules! num_col {
        ($name:expr, $field:ident) => {{
            let vals: Vec<Option<f64>> = rows.iter().map(|r| r.$field).collect();
            columns.push(Series::new($name.into(), vals).into());
            vars.push(num_var_meta($name));
        }};
    }
    num_col!("Mean", mean);
    num_col!("StdDev", std_dev);
    num_col!("StdErr", std_err);
    num_col!("DF", df);
    num_col!("tValue", t);
    num_col!("Probt", probt);
    num_col!("LowerCLMean", lower_cl);
    num_col!("UpperCLMean", upper_cl);

    let df = DataFrame::new(columns)?;
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

    fn parse_ttest(src: &str) -> Result<TTestAst> {
        let source = SourceFile::new(src);
        let mut ts = StatementStream::new(&source).unwrap();
        ts.next(); // "proc"
        ts.next(); // "ttest"
        parse(&mut ts)
    }

    fn dref(name: &str) -> DatasetRef {
        DatasetRef {
            libref: Some("WORK".into()),
            name: name.into(),
        }
    }

    fn approx(a: f64, b: f64, tol: f64) -> bool {
        (a - b).abs() <= tol
    }

    // ───────────── parse tests ─────────────

    #[test]
    fn parse_minimal() {
        let ast = parse_ttest("proc ttest data=a; var x; run;").unwrap();
        assert_eq!(ast.data_options.input.as_ref().unwrap().name, "a");
        assert_eq!(ast.var_vars, vec!["x"]);
        assert_eq!(ast.proc_options.h0, 0.0);
        assert!((ast.proc_options.alpha - 0.05).abs() < 1e-12);
    }

    #[test]
    fn parse_options_and_class() {
        let ast =
            parse_ttest("proc ttest data=a h0=10 alpha=0.1 sides=2; var x y; class g; run;")
                .unwrap();
        assert_eq!(ast.proc_options.h0, 10.0);
        assert!((ast.proc_options.alpha - 0.1).abs() < 1e-12);
        assert_eq!(ast.var_vars, vec!["x", "y"]);
        assert_eq!(ast.class_var.as_deref(), Some("g"));
    }

    #[test]
    fn parse_paired_pairs() {
        let ast = parse_ttest("proc ttest data=a; paired pre*post b1*b2; run;").unwrap();
        assert_eq!(
            ast.paired_vars,
            vec![
                ("pre".to_string(), "post".to_string()),
                ("b1".to_string(), "b2".to_string())
            ]
        );
    }

    #[test]
    fn parse_negative_h0() {
        let ast = parse_ttest("proc ttest data=a h0=-2.5; var x; run;").unwrap();
        assert_eq!(ast.proc_options.h0, -2.5);
    }

    #[test]
    fn parse_class_paired_conflict_errors() {
        let r = parse_ttest("proc ttest data=a; class g; paired x*y; run;");
        assert!(r.is_err());
    }

    #[test]
    fn parse_alpha_out_of_range_errors() {
        let r = parse_ttest("proc ttest data=a alpha=1.5; var x; run;");
        assert!(r.is_err());
        assert!(r.err().unwrap().to_string().contains("ALPHA"));
    }

    #[test]
    fn parse_class_two_vars_errors() {
        let r = parse_ttest("proc ttest data=a; class g h; run;");
        assert!(r.is_err());
    }

    #[test]
    fn parse_by_deferred_errors() {
        let r = parse_ttest("proc ttest data=a; var x; by g; run;");
        assert!(r.is_err());
        assert!(r
            .err()
            .unwrap()
            .to_string()
            .contains("BY processing not yet implemented"));
    }

    #[test]
    fn parse_unknown_option_errors() {
        let r = parse_ttest("proc ttest data=a bogus; run;");
        assert!(r.is_err());
        assert!(r.err().unwrap().to_string().contains("BOGUS"));
    }

    // ───────────── statistics core tests ─────────────

    #[test]
    fn summary_basic() {
        let xs = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let s = summarize(&xs, 0);
        assert_eq!(s.n, 5);
        assert!((s.mean.unwrap() - 3.0).abs() < 1e-12);
        assert!((s.std.unwrap() - 1.5811388300841898).abs() < 1e-9);
        assert!((s.min.unwrap() - 1.0).abs() < 1e-12);
        assert!((s.max.unwrap() - 5.0).abs() < 1e-12);
    }

    #[test]
    fn one_sample_known_t() {
        // mean=3, std≈1.581139, n=5, h0=0 → t = 3/(1.581139/sqrt(5)) = 4.2426.
        let xs = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let s = summarize(&xs, 0);
        let test = one_sample_test(&s, 0.0, 0.05);
        assert!(approx(test.t.unwrap(), 4.242640687, 1e-6), "t={:?}", test.t);
        assert_eq!(test.df.unwrap(), 4.0);
        // p two-sided around 0.0132.
        assert!(approx(test.p.unwrap(), 0.013228, 1e-4), "p={:?}", test.p);
    }

    #[test]
    fn one_sample_constant_is_missing() {
        let xs = vec![5.0, 5.0, 5.0];
        let s = summarize(&xs, 0);
        let test = one_sample_test(&s, 0.0, 0.05);
        assert!(test.t.is_none());
        assert!(test.p.is_none());
    }

    #[test]
    fn one_sample_n_lt_2_no_panic() {
        let xs = vec![5.0];
        let s = summarize(&xs, 0);
        let test = one_sample_test(&s, 0.0, 0.05);
        assert!(test.t.is_none());
        assert!(s.std.is_none());
    }

    #[test]
    fn two_sample_pooled_and_satterthwaite() {
        // a = [1,2,3,4,5] mean 3 var 2.5; b = [2,4,6,8,10] mean 6 var 10.
        let a = summarize(&[1.0, 2.0, 3.0, 4.0, 5.0], 0);
        let b = summarize(&[2.0, 4.0, 6.0, 8.0, 10.0], 0);
        let test = two_sample_test(&a, &b, 0.05);
        assert!((test.diff.unwrap() - (-3.0)).abs() < 1e-12);
        assert_eq!(test.pooled_df.unwrap(), 8.0);
        // sp2 = (4*2.5 + 4*10)/8 = 6.25; se = sqrt(6.25*0.4)=1.5811; t=-1.897.
        assert!(approx(test.pooled_t.unwrap(), -1.897367, 1e-5), "{:?}", test.pooled_t);
        // Satterthwaite: ua=0.5, ub=2.0, se=sqrt(2.5)=1.5811; t=-1.897.
        assert!(approx(test.satt_t.unwrap(), -1.897367, 1e-5), "{:?}", test.satt_t);
        // df = (2.5)^2 / (0.25/4 + 4/4) = 6.25/1.0625 = 5.882.
        assert!(approx(test.satt_df.unwrap(), 5.882353, 1e-4), "{:?}", test.satt_df);
        // Folded F = 10/2.5 = 4.
        assert!(approx(test.f_value.unwrap(), 4.0, 1e-9));
    }

    #[test]
    fn folded_f_symmetric_in_ratio() {
        // Swap groups: F should still be max/min = 4 (>=1).
        let a = summarize(&[2.0, 4.0, 6.0, 8.0, 10.0], 0);
        let b = summarize(&[1.0, 2.0, 3.0, 4.0, 5.0], 0);
        let test = two_sample_test(&a, &b, 0.05);
        assert!(approx(test.f_value.unwrap(), 4.0, 1e-9));
    }

    // ───────────── execute: one-sample ─────────────

    #[test]
    fn execute_one_sample_listing() {
        let mut session = make_session();
        let df = df!["x" => [1.0_f64, 2.0, 3.0, 4.0, 5.0]].unwrap();
        let ds = SasDataset { df, vars: vec![num_meta("x")] };
        write_dataset(&mut session, "T", ds);

        let ast = TTestAst {
            data_options: TTestDataOptions { input: Some(dref("T")), output: None },
            proc_options: TTestProcOptions::default(),
            var_vars: vec!["x".into()],
            class_var: None,
            paired_vars: vec![],
        };
        execute(&ast, &mut session).unwrap();
        let listing = session.listing.into_string();
        assert!(listing.contains("The TTEST Procedure"), "{listing}");
        assert!(listing.contains("t Value"), "{listing}");
        assert!(listing.contains("Pr > |t|"), "{listing}");
        assert!(listing.contains("x"), "{listing}");
    }

    #[test]
    fn execute_one_sample_sashelp_class_height() {
        // SAS reference: Height in sashelp.class — N=19, Mean=62.337,
        // StdDev=5.127, StdErr=1.176, t(H0=0)=53.01, Pr>|t|<.0001.
        let heights = [
            69.0_f64, 56.5, 65.3, 62.8, 63.5, 57.3, 59.8, 62.5, 62.5, 59.0, 51.3, 64.3, 56.3,
            66.5, 72.0, 64.8, 67.0, 57.5, 66.5,
        ];
        let s = summarize(&heights, 0);
        assert_eq!(s.n, 19);
        assert!(approx(s.mean.unwrap(), 62.336842, 1e-4), "mean={:?}", s.mean);
        assert!(approx(s.std.unwrap(), 5.127075, 1e-4), "std={:?}", s.std);
        assert!(approx(s.stderr.unwrap(), 1.176232, 1e-4), "se={:?}", s.stderr);
        let test = one_sample_test(&s, 0.0, 0.05);
        assert!(approx(test.t.unwrap(), 53.0058, 1e-2), "t={:?}", test.t);
        assert!(test.p.unwrap() < 0.0001, "p={:?}", test.p);
    }

    #[test]
    fn execute_one_sample_missing_values() {
        let mut session = make_session();
        let df = df!["x" => [Some(1.0_f64), Some(2.0), None, Some(3.0), Some(4.0), Some(5.0)]]
            .unwrap();
        let ds = SasDataset { df, vars: vec![num_meta("x")] };
        write_dataset(&mut session, "T", ds);

        let ast = TTestAst {
            data_options: TTestDataOptions { input: Some(dref("T")), output: Some(dref("O")) },
            proc_options: TTestProcOptions::default(),
            var_vars: vec!["x".into()],
            class_var: None,
            paired_vars: vec![],
        };
        execute(&ast, &mut session).unwrap();
        // Output dataset: N should be 5 (one missing excluded).
        let (out, _) = session.libs.get("WORK").unwrap().read("O").unwrap();
        let ncol = decode_column(&out, out.vars.iter().position(|v| v.name == "N").unwrap())
            .unwrap();
        assert!((value_to_num(&ncol[0]).unwrap() - 5.0).abs() < 1e-12);
    }

    // ───────────── execute: two-sample ─────────────

    #[test]
    fn execute_two_sample_listing_and_out() {
        let mut session = make_session();
        let df = df![
            "g" => ["a", "a", "a", "b", "b", "b"],
            "y" => [1.0_f64, 2.0, 3.0, 4.0, 6.0, 8.0]
        ]
        .unwrap();
        let ds = SasDataset { df, vars: vec![char_meta("g"), num_meta("y")] };
        write_dataset(&mut session, "T", ds);

        let ast = TTestAst {
            data_options: TTestDataOptions { input: Some(dref("T")), output: Some(dref("O")) },
            proc_options: TTestProcOptions::default(),
            var_vars: vec!["y".into()],
            class_var: Some("g".into()),
            paired_vars: vec![],
        };
        execute(&ast, &mut session).unwrap();
        let listing = session.listing.into_string();
        assert!(listing.contains("Pooled"), "{listing}");
        assert!(listing.contains("Satterthwaite"), "{listing}");
        assert!(listing.contains("Equality of Variances"), "{listing}");
        assert!(listing.contains("Pr > F"), "{listing}");

        // Output dataset has a Class column.
        let (out, _) = session.libs.get("WORK").unwrap().read("O").unwrap();
        assert!(out.vars.iter().any(|v| v.name == "Class"));
        // Three rows: a, b, Diff.
        assert_eq!(out.n_obs(), 3);
    }

    #[test]
    fn execute_two_sample_more_than_two_levels_errors() {
        let mut session = make_session();
        let df = df![
            "g" => ["a", "b", "c", "a"],
            "y" => [1.0_f64, 2.0, 3.0, 4.0]
        ]
        .unwrap();
        let ds = SasDataset { df, vars: vec![char_meta("g"), num_meta("y")] };
        write_dataset(&mut session, "T", ds);

        let ast = TTestAst {
            data_options: TTestDataOptions { input: Some(dref("T")), output: None },
            proc_options: TTestProcOptions::default(),
            var_vars: vec!["y".into()],
            class_var: Some("g".into()),
            paired_vars: vec![],
        };
        let r = execute(&ast, &mut session);
        assert!(r.is_err());
        assert!(r.err().unwrap().to_string().contains("exactly 2 levels"));
    }

    // ───────────── execute: paired ─────────────

    #[test]
    fn execute_paired_listing() {
        let mut session = make_session();
        let df = df![
            "pre" => [10.0_f64, 12.0, 14.0, 16.0, 18.0],
            "post" => [11.0_f64, 14.0, 15.0, 19.0, 20.0]
        ]
        .unwrap();
        let ds = SasDataset { df, vars: vec![num_meta("pre"), num_meta("post")] };
        write_dataset(&mut session, "T", ds);

        let ast = TTestAst {
            data_options: TTestDataOptions { input: Some(dref("T")), output: Some(dref("O")) },
            proc_options: TTestProcOptions::default(),
            var_vars: vec![],
            class_var: None,
            paired_vars: vec![("pre".into(), "post".into())],
        };
        execute(&ast, &mut session).unwrap();
        let listing = session.listing.into_string();
        assert!(listing.contains("Paired T-Tests"), "{listing}");
        assert!(listing.contains("pre - post"), "{listing}");

        // Differences: [-1,-2,-1,-3,-2] mean=-1.8, test against 0.
        let (out, _) = session.libs.get("WORK").unwrap().read("O").unwrap();
        let mcol = decode_column(&out, out.vars.iter().position(|v| v.name == "Mean").unwrap())
            .unwrap();
        assert!((value_to_num(&mcol[0]).unwrap() - (-1.8)).abs() < 1e-9);
    }

    #[test]
    fn execute_paired_missing_excluded() {
        let mut session = make_session();
        let df = df![
            "a" => [Some(1.0_f64), Some(2.0), None, Some(4.0)],
            "b" => [Some(0.0_f64), None, Some(3.0), Some(1.0)]
        ]
        .unwrap();
        let ds = SasDataset { df, vars: vec![num_meta("a"), num_meta("b")] };
        write_dataset(&mut session, "T", ds);

        let ast = TTestAst {
            data_options: TTestDataOptions { input: Some(dref("T")), output: Some(dref("O")) },
            proc_options: TTestProcOptions::default(),
            var_vars: vec![],
            class_var: None,
            paired_vars: vec![("a".into(), "b".into())],
        };
        execute(&ast, &mut session).unwrap();
        // Only rows 0 and 3 have both → N=2.
        let (out, _) = session.libs.get("WORK").unwrap().read("O").unwrap();
        let ncol = decode_column(&out, out.vars.iter().position(|v| v.name == "N").unwrap())
            .unwrap();
        assert!((value_to_num(&ncol[0]).unwrap() - 2.0).abs() < 1e-12);
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
        let ds = SasDataset {
            df,
            vars: vec![num_meta("x"), char_meta("g"), num_meta("y")],
        };
        write_dataset(&mut session, "T", ds);

        let ast = TTestAst {
            data_options: TTestDataOptions { input: Some(dref("T")), output: Some(dref("O")) },
            proc_options: TTestProcOptions::default(),
            var_vars: vec![],
            class_var: None,
            paired_vars: vec![],
        };
        execute(&ast, &mut session).unwrap();
        // x and y analyzed (char g excluded) → 2 output rows.
        let (out, _) = session.libs.get("WORK").unwrap().read("O").unwrap();
        assert_eq!(out.n_obs(), 2);
    }
}
