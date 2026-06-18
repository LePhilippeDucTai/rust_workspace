//! PROC NPAR1WAY — non-parametric one-way tests (M24.3).
//!
//! ## Plan d'implémentation (M24.3 — Opus, moyen-élevé)
//!
//! Non-parametric alternatives to ANOVA for testing equality of k>1 populations.
//! Wilcoxon rank-sum (2 samples), Kruskal-Wallis (k>1), score test methods.
//! CLASS required (exact 1, multi-level). VAR optional (default all numeric).
//! Output: test statistics, p-values, ranks (optionally), sample statistics.
//!
//! ### Architecture
//!
//! `NparAst { data_options, proc_options, var_vars, class_var, test_options }`
//! - Single CLASS variable (k levels, n total obs)
//! - Multiple VAR (analyzed separately)
//! - Tests: WILCOXON (2-sample), KRUSKAL, SCORES (normal/van-der-Waerden/Savage/median)
//!
//! ### Wilcoxon test (2-sample: rank-sum)
//! - Samples A (n_A obs) vs B (n_B obs), k=2
//! - Null hypothesis: distributions identical (shift alternative)
//! - Procedure:
//!   1. Pool all n = n_A + n_B observations, sort by VAR value
//!   2. Assign ranks 1..n (midrank ties), missing → excluded + NOTE "N pairs"
//!   3. W = sum of ranks in group A (by convention, reference = first level)
//!   4. Expected W_0 = n_A(n+1)/2, Var(W) tie-corrected exact form
//!   5. Z = (W - W_0) / √Var(W) ≈ N(0,1) for large n
//!   6. 2-tailed p: 2·(1 − Φ(|z|)) via `stat::probnorm`
//!   7. Exact p-value (if n ≤ 20) via permutation enumeration (expensive, defer v1)
//! - Output: Z, Pr>|Z|, plus the χ²-approx (z², df=1)
//!
//! ### Kruskal-Wallis test (k-sample: rank-sum)
//! - k≥2 groups with sizes n_1, ..., n_k, total n
//! - Null: all k distributions identical
//! - Procedure:
//!   1. Pool, rank 1..n (midrank ties)
//!   2. R_i = sum of ranks in group i
//!   3. H = [12 / (n(n+1))] · Σ R_i² / n_i - 3(n+1) (Kruskal-Wallis statistic)
//!   4. H/tie ≈ χ²(df=k-1) under null (tie-corrected)
//!   5. p-value: 1 − chisq_cdf(H, df)
//! - Output: H statistic, DF (k-1), Pr>H (chi-square tail probability)
//!
//! ### Score tests (van der Waerden, Savage, median) — DEFERRED v1
//! - NOTE: implementation deferred; SAVAGE/MEDIAN parse only, error at execute.
//!
//! ### Options
//! - **CLASS** var (required; exactly one; multi-level automatic)
//! - **VAR** variables (default = all numeric except the CLASS var)
//! - **WILCOXON** / **KRUSKAL** (auto-selected by k otherwise)
//! - **ALPHA=** (default 0.05; not used for p-value in v1)
//! - **OUT=** dataset (optional small results dataset)
//! - Scoring methods: NORMAL (default), SAVAGE, MEDIAN (NOT IN v1; parse but error)
//! - (Deferred) **BY** support
//!
//! ### Error handling
//! - CLASS with <2 levels → error "Class variable must have at least 2 levels"
//! - Missing CLASS statement → error
//! - SAVAGE/MEDIAN scoring → error "Only NORMAL/Wilcoxon scores are implemented in v1."
//! - BY support → deferred error/NOTE
//!
//! ### v1 deferrals (documented for the orchestrator)
//! - Exact (permutation) Wilcoxon p-value (n ≤ 20) → deferred.
//! - SAVAGE / MEDIAN / van der Waerden normal scores → deferred (parse, reject).
//! - Confidence intervals, ALPHA= usage → deferred.
//! - BY-group processing → deferred.
//! - Wilcoxon continuity correction: NOT applied in v1 (z uses the raw
//!   (W − E[W])/√Var deviation). Documented at `wilcoxon` below.

use crate::ast::DatasetRef;
use crate::dataset::{SasDataset, VarMeta};
use crate::error::{Result, SasError};
use crate::listing::Align;
use crate::missing::value_to_num;
use crate::parser::StatementStream;
use crate::procs::common::decode_column;
use crate::session::Session;
use crate::stat::{chisq_cdf, probnorm};
use crate::token::TokenKind;
use crate::value::{format_best, Value, VarType};
use polars::prelude::{Column, DataFrame, NamedFrom, Series};
use std::cmp::Ordering;

#[derive(Debug, Clone)]
pub struct NparAst {
    pub data_options: NparDataOptions,
    pub proc_options: NparProcOptions,
    pub var_vars: Vec<String>,
    pub class_var: String,
    pub test_options: NparTestOptions,
}

#[derive(Debug, Clone)]
pub struct NparDataOptions {
    pub input: Option<DatasetRef>,
    pub output: Option<DatasetRef>,
}

#[derive(Debug, Clone)]
pub struct NparProcOptions {
    pub alpha: f64,
}

#[derive(Debug, Clone)]
pub struct NparTestOptions {
    pub wilcoxon: bool,
    pub kruskal: bool,
    pub scores: NparScores,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NparScores {
    Normal,
    Savage,
    Median,
}

impl Default for NparProcOptions {
    fn default() -> Self {
        NparProcOptions { alpha: 0.05 }
    }
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

/// Parse `proc npar1way [data=a] [wilcoxon] [kruskal] [alpha=x] [out=b];
/// class g; [var ...;] ... run;`. Called AFTER "proc npar1way" was consumed.
/// Consumes through `run;` / `quit;`.
pub fn parse(ts: &mut StatementStream) -> Result<NparAst> {
    let mut input: Option<DatasetRef> = None;
    let mut output: Option<DatasetRef> = None;
    let mut alpha = 0.05_f64;
    let mut wilcoxon = false;
    let mut kruskal = false;
    let mut scores = NparScores::Normal;

    // --- PROC NPAR1WAY statement options, until `;` ---
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
        } else if ts.peek().is_kw("wilcoxon") {
            ts.next();
            wilcoxon = true;
        } else if ts.peek().is_kw("kruskal") || ts.peek().is_kw("kruskalwallis") {
            ts.next();
            kruskal = true;
        } else if ts.peek().is_kw("normal") {
            ts.next();
            scores = NparScores::Normal;
        } else if ts.peek().is_kw("savage") {
            ts.next();
            scores = NparScores::Savage;
        } else if ts.peek().is_kw("median") {
            ts.next();
            scores = NparScores::Median;
        } else if ts.peek().is_kw("alpha") {
            ts.next();
            expect_eq(ts, "ALPHA")?;
            let tok = ts.peek().clone();
            alpha = match tok.kind {
                TokenKind::Num(f) => f,
                _ => {
                    return Err(SasError::parse("expected a number after ALPHA=", tok.span));
                }
            };
            ts.next();
        } else if let Some(name) = ts.peek().ident().map(str::to_string) {
            let span = ts.peek().span;
            return Err(SasError::parse(
                format!(
                    "Unexpected option '{}' on PROC NPAR1WAY statement.",
                    name.to_uppercase()
                ),
                span,
            ));
        } else {
            let span = ts.peek().span;
            return Err(SasError::parse(
                "Unexpected token on PROC NPAR1WAY statement.",
                span,
            ));
        }
    }

    // --- sub-statements until run;/quit; ---
    let mut var_vars: Vec<String> = Vec::new();
    let mut class_var: Option<String> = None;

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
            let names = ts.parse_name_list()?;
            ts.expect_semi()?;
            if names.len() != 1 {
                return Err(SasError::runtime(
                    "The CLASS statement of PROC NPAR1WAY accepts exactly one variable.",
                ));
            }
            class_var = Some(names.into_iter().next().unwrap());
        } else if ts.peek().is_kw("var") {
            ts.next();
            var_vars = ts.parse_name_list()?;
            ts.expect_semi()?;
        } else {
            // Unknown sub-statement: skip it (recovery, like corr/means).
            ts.skip_to_semi();
        }
    }

    let class_var = class_var.ok_or_else(|| {
        SasError::runtime("PROC NPAR1WAY requires a CLASS statement with one variable.")
    })?;

    Ok(NparAst {
        data_options: NparDataOptions { input, output },
        proc_options: NparProcOptions { alpha },
        var_vars,
        class_var,
        test_options: NparTestOptions {
            wilcoxon,
            kruskal,
            scores,
        },
    })
}

/// Resolve `data=` or `_LAST_` into a concrete DatasetRef.
fn resolve_input(ast: &NparAst, session: &Session) -> Result<DatasetRef> {
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

// ───────────────────────── numeric core ─────────────────────────

/// Mean (midrank) ranks of a slice: ties receive the average of the ranks they
/// would otherwise occupy (1-based). Mirrors `corr::mean_ranks`.
fn midranks(xs: &[f64]) -> Vec<f64> {
    let n = xs.len();
    let mut idx: Vec<usize> = (0..n).collect();
    idx.sort_by(|&a, &b| xs[a].partial_cmp(&xs[b]).unwrap_or(Ordering::Equal));
    let mut ranks = vec![0.0_f64; n];
    let mut i = 0;
    while i < n {
        let mut j = i + 1;
        while j < n && xs[idx[j]] == xs[idx[i]] {
            j += 1;
        }
        // Ranks i+1 .. j (1-based) averaged.
        let avg = ((i + 1 + j) as f64) / 2.0; // sum (i+1..=j) / count
        for &k in &idx[i..j] {
            ranks[k] = avg;
        }
        i = j;
    }
    ranks
}

/// Tie correction factor `1 − Σ(t³ − t) / (n³ − n)`, where the `t` are the
/// sizes of groups of equal values in `xs`. Equals 1.0 when there are no ties
/// (or n < 2). Applied as a divisor to the Kruskal-Wallis H statistic and as a
/// factor inside the Wilcoxon variance.
fn tie_correction(xs: &[f64]) -> f64 {
    let n = xs.len();
    if n < 2 {
        return 1.0;
    }
    let mut v: Vec<f64> = xs.to_vec();
    v.sort_by(|a, b| a.partial_cmp(b).unwrap_or(Ordering::Equal));
    let mut sum_t3_t = 0.0_f64;
    let mut i = 0;
    while i < n {
        let mut j = i + 1;
        while j < n && v[j] == v[i] {
            j += 1;
        }
        let t = (j - i) as f64;
        sum_t3_t += t * t * t - t;
        i = j;
    }
    let nf = n as f64;
    let denom = nf * nf * nf - nf;
    if denom == 0.0 {
        1.0
    } else {
        1.0 - sum_t3_t / denom
    }
}

/// Result of a Wilcoxon rank-sum (two-sample) test.
struct WilcoxonResult {
    /// Sum of scores (ranks) in the reference group A.
    w: f64,
    /// Expected sum under H0: n_A·(n+1)/2.
    expected: f64,
    /// Variance of W under H0 (tie-corrected exact form).
    variance: f64,
    /// Standard deviation = √variance.
    std_dev: f64,
    /// Standardized statistic z = (W − E[W]) / √Var.
    z: f64,
    /// Two-sided normal-approximation p-value.
    p_normal: f64,
    /// χ²-approximation statistic (z²) and its one-sided χ² p-value.
    chisq: f64,
    p_chisq: f64,
}

/// Wilcoxon rank-sum test from the already-pooled midranks. `ranks` are the
/// pooled midranks of all n observations; `is_a` flags membership in the
/// reference group A (its rank sum is W). `n_a` = |A|, n = ranks.len().
///
/// Variance uses the tie-corrected exact form
///   Var = (n_A·n_B / (n(n−1))) · (Σ r_i² − n(n+1)²/4),
/// which already accounts for ties through Σ r_i². No continuity correction is
/// applied (v1 choice — see file header); z is the raw standardized deviation.
fn wilcoxon(ranks: &[f64], is_a: &[bool], n_a: usize) -> WilcoxonResult {
    let n = ranks.len();
    let nf = n as f64;
    let naf = n_a as f64;
    let nbf = nf - naf;

    let w: f64 = ranks
        .iter()
        .zip(is_a)
        .filter(|&(_, &a)| a)
        .map(|(r, _)| *r)
        .sum();

    let expected = naf * (nf + 1.0) / 2.0;

    let sum_r2: f64 = ranks.iter().map(|r| r * r).sum();
    // Tie-corrected exact variance of the rank sum.
    let variance = if n > 1 {
        (naf * nbf / (nf * (nf - 1.0))) * (sum_r2 - nf * (nf + 1.0) * (nf + 1.0) / 4.0)
    } else {
        0.0
    };
    let std_dev = if variance > 0.0 { variance.sqrt() } else { 0.0 };

    let z = if std_dev > 0.0 {
        (w - expected) / std_dev
    } else {
        0.0
    };
    let p_normal = 2.0 * (1.0 - probnorm(z.abs()));
    let chisq = z * z;
    let p_chisq = 1.0 - chisq_cdf(chisq, 1.0);

    WilcoxonResult {
        w,
        expected,
        variance,
        std_dev,
        z,
        p_normal,
        chisq,
        p_chisq,
    }
}

/// Result of a Kruskal-Wallis test.
struct KruskalResult {
    /// H statistic, tie-corrected.
    h: f64,
    /// Degrees of freedom (k − 1).
    df: f64,
    /// p-value = 1 − chisq_cdf(H, df).
    p: f64,
}

/// Kruskal-Wallis test from pooled midranks. `ranks` are the pooled midranks of
/// all n observations; `rank_sums[i]` / `group_n[i]` are the rank sum and size
/// of group i (k groups). `tie` is `tie_correction(pooled_values)`.
fn kruskal_wallis(ranks: &[f64], rank_sums: &[f64], group_n: &[usize], tie: f64) -> KruskalResult {
    let n = ranks.len();
    let nf = n as f64;
    let k = rank_sums.len();

    let mut acc = 0.0_f64;
    for i in 0..k {
        let ni = group_n[i] as f64;
        if ni > 0.0 {
            acc += rank_sums[i] * rank_sums[i] / ni;
        }
    }
    let h_raw = 12.0 / (nf * (nf + 1.0)) * acc - 3.0 * (nf + 1.0);
    let h = if tie != 0.0 { h_raw / tie } else { h_raw };
    let df = (k as f64) - 1.0;
    let p = 1.0 - chisq_cdf(h, df);
    KruskalResult { h, df, p }
}

// ───────────────────────── formatting ─────────────────────────

/// Format a p-value SAS-style: `<.0001`, else 4 decimals.
fn fmt_p(p: f64) -> String {
    if p < 0.0001 {
        "<.0001".to_string()
    } else {
        format!("{p:.4}")
    }
}

/// Write a centered line within LINESIZE (mirrors `corr::centered`).
fn centered(session: &mut Session, text: &str) {
    let ls = session.listing.ls();
    let pad = ls.saturating_sub(text.len()) / 2;
    session
        .listing
        .write_line(&format!("{}{}", " ".repeat(pad), text));
}

/// Display label for a class-level key value (numeric or char).
fn class_label(v: &Value) -> String {
    match v {
        Value::Num(f) => format_best(*f, 12),
        Value::Missing(k) => k.display(),
        Value::Char(s) => s.trim_end().to_string(),
    }
}

// ───────────────────────── execute ─────────────────────────

/// Per-VAR analysis outcome, used both for the listing and the OUT= dataset.
struct VarAnalysis {
    var_name: String,
    /// Class level labels in `sas_cmp` order.
    level_labels: Vec<String>,
    /// Sum of scores (rank sums) per level.
    sum_scores: Vec<f64>,
    /// Per-level size.
    level_n: Vec<usize>,
    /// Total non-missing pairs analyzed.
    n: usize,
    wilcoxon: Option<WilcoxonResult>,
    kruskal: Option<KruskalResult>,
}

pub fn execute(ast: &NparAst, session: &mut Session) -> Result<()> {
    // Reject deferred scoring methods up front.
    if ast.test_options.scores != NparScores::Normal {
        return Err(SasError::runtime(
            "Only NORMAL/Wilcoxon scores are implemented in v1.",
        ));
    }

    let in_ref = resolve_input(ast, session)?;
    let in_libref = in_ref.libref_or_work();
    let in_table = in_ref.name.to_uppercase();

    let provider = session.libs.get(&in_libref)?;
    let (ds, notes) = provider.read(&in_table)?;
    for note in notes {
        session.log.forward(&note);
    }

    let n_obs = ds.n_obs();

    // Resolve the CLASS column (any type).
    let class_idx = ds
        .vars
        .iter()
        .position(|m| m.name.eq_ignore_ascii_case(&ast.class_var))
        .ok_or_else(|| {
            SasError::runtime(format!(
                "Variable {} not found.",
                ast.class_var.to_uppercase()
            ))
        })?;

    // Resolve VAR list: explicit numeric, else all numeric except the CLASS var.
    let var_cols: Vec<usize> = if !ast.var_vars.is_empty() {
        let mut out = Vec::with_capacity(ast.var_vars.len());
        for nm in &ast.var_vars {
            match ds.vars.iter().position(|m| m.name.eq_ignore_ascii_case(nm)) {
                Some(i) => {
                    if ds.vars[i].ty != VarType::Num {
                        return Err(SasError::runtime(format!(
                            "Variable {} in the VAR list is not numeric.",
                            nm.to_uppercase()
                        )));
                    }
                    out.push(i);
                }
                None => {
                    return Err(SasError::runtime(format!(
                        "Variable {} not found.",
                        nm.to_uppercase()
                    )));
                }
            }
        }
        out
    } else {
        (0..ds.vars.len())
            .filter(|&i| ds.vars[i].ty == VarType::Num && i != class_idx)
            .collect()
    };

    if var_cols.is_empty() {
        return Err(SasError::runtime(
            "No numeric variables found for PROC NPAR1WAY analysis.",
        ));
    }

    // Decode the CLASS column once and every analysis column once.
    let class_vals = decode_column(&ds, class_idx)?;
    let mut decoded: std::collections::HashMap<usize, Vec<Value>> =
        std::collections::HashMap::new();
    for &c in &var_cols {
        decoded.insert(c, decode_column(&ds, c)?);
    }

    // --- listing header ---
    session.listing.page_header();
    centered(session, "The NPAR1WAY Procedure");
    session.listing.blank();

    let mut analyses: Vec<VarAnalysis> = Vec::with_capacity(var_cols.len());

    for &c in &var_cols {
        let analysis = analyze_var(
            &ds.vars[c].name,
            &decoded[&c],
            &class_vals,
            n_obs,
            &ast.test_options,
        )?;
        emit_var_listing(session, &analysis);
        analyses.push(analysis);
    }

    // --- OUT= (optional results dataset) ---
    if let Some(target) = &ast.data_options.output {
        let out_ds = build_out_dataset(&analyses)?;
        write_out_dataset(session, target, out_ds)?;
    }

    Ok(())
}

/// Run the non-parametric analysis for one VAR against the CLASS variable.
fn analyze_var(
    var_name: &str,
    var_vals: &[Value],
    class_vals: &[Value],
    n_obs: usize,
    opts: &NparTestOptions,
) -> Result<VarAnalysis> {
    // Build the (value, class) list excluding rows where either is missing.
    let mut values: Vec<f64> = Vec::with_capacity(n_obs);
    let mut classes: Vec<Value> = Vec::with_capacity(n_obs);
    for i in 0..n_obs {
        if class_vals[i].is_missing() {
            continue;
        }
        match value_to_num(&var_vals[i]) {
            Some(v) if !v.is_nan() => {
                values.push(v);
                classes.push(class_vals[i].clone());
            }
            _ => {}
        }
    }

    // Distinct class levels, ordered by sas_cmp.
    let mut levels: Vec<Value> = Vec::new();
    for c in &classes {
        if !levels.iter().any(|l| l.sas_cmp(c) == Ordering::Equal) {
            levels.push(c.clone());
        }
    }
    levels.sort_by(|a, b| a.sas_cmp(b));
    let k = levels.len();
    if k < 2 {
        return Err(SasError::runtime(
            "Class variable must have at least 2 levels.",
        ));
    }

    let n = values.len();
    // Map each observation to its level index.
    let level_of = |c: &Value| -> usize {
        levels
            .iter()
            .position(|l| l.sas_cmp(c) == Ordering::Equal)
            .unwrap()
    };

    // Pooled midranks and tie correction.
    let ranks = midranks(&values);
    let tie = tie_correction(&values);

    // Per-level rank sums and sizes.
    let mut sum_scores = vec![0.0_f64; k];
    let mut level_n = vec![0usize; k];
    for i in 0..n {
        let li = level_of(&classes[i]);
        sum_scores[li] += ranks[i];
        level_n[li] += 1;
    }

    let level_labels: Vec<String> = levels.iter().map(class_label).collect();

    // Decide which test(s) to run. SAS auto-selects by k; honor explicit flags.
    // k==2 → Wilcoxon (always shown); k>2 → Kruskal-Wallis.
    let want_wilcoxon = k == 2 || opts.wilcoxon;
    let want_kruskal = k > 2 || opts.kruskal;

    let wilcoxon_res = if want_wilcoxon {
        // Reference group A = first level (index 0).
        let is_a: Vec<bool> = classes.iter().map(|c| level_of(c) == 0).collect();
        let n_a = level_n[0];
        Some(wilcoxon(&ranks, &is_a, n_a))
    } else {
        None
    };

    let kruskal_res = if want_kruskal {
        Some(kruskal_wallis(&ranks, &sum_scores, &level_n, tie))
    } else {
        None
    };

    Ok(VarAnalysis {
        var_name: var_name.to_string(),
        level_labels,
        sum_scores,
        level_n,
        n,
        wilcoxon: wilcoxon_res,
        kruskal: kruskal_res,
    })
}

/// Emit the listing for one VAR's analysis.
fn emit_var_listing(session: &mut Session, a: &VarAnalysis) {
    centered(
        session,
        &format!("Wilcoxon Scores (Rank Sums) for Variable {}", a.var_name),
    );
    centered(session, "Classified by Variable CLASS");
    session.listing.blank();

    let nf = a.n as f64;
    let headers: Vec<String> = vec![
        "Class".into(),
        "N".into(),
        "Sum of Scores".into(),
        "Expected Under H0".into(),
        "Std Dev Under H0".into(),
        "Mean Score".into(),
    ];
    let aligns = vec![
        Align::Left,
        Align::Right,
        Align::Right,
        Align::Right,
        Align::Right,
        Align::Right,
    ];

    // Per-level expected sum and std dev under H0 (Wilcoxon scores).
    // E_i = n_i (n+1)/2; Std_i = sqrt(n_i (n − n_i)(n+1)/12) (no tie corr in the
    // per-level display, matching SAS's summary table).
    let mut rows: Vec<Vec<String>> = Vec::with_capacity(a.level_labels.len());
    for i in 0..a.level_labels.len() {
        let ni = a.level_n[i] as f64;
        let expected = ni * (nf + 1.0) / 2.0;
        let var_i = if a.n > 1 {
            ni * (nf - ni) * (nf + 1.0) / 12.0
        } else {
            0.0
        };
        let std_i = var_i.max(0.0).sqrt();
        let mean_score = if ni > 0.0 {
            a.sum_scores[i] / ni
        } else {
            0.0
        };
        rows.push(vec![
            a.level_labels[i].clone(),
            format!("{}", a.level_n[i]),
            format_best(a.sum_scores[i], 12),
            format_best(expected, 12),
            format_best(std_i, 12),
            format_best(mean_score, 12),
        ]);
    }
    session.listing.write_table(&headers, &aligns, &rows);
    session.listing.blank();

    // Wilcoxon two-sample block.
    if let Some(w) = &a.wilcoxon {
        centered(session, "Wilcoxon Two-Sample Test");
        session.listing.blank();
        session
            .listing
            .write_line(&format!("Statistic (S)            {}", format_best(w.w, 12)));
        session.listing.write_line(&format!(
            "Normal Approximation (Z) {}",
            format_best(w.z, 12)
        ));
        session
            .listing
            .write_line(&format!("Two-Sided Pr > |Z|       {}", fmt_p(w.p_normal)));
        session.listing.write_line(&format!(
            "Chi-Square               {}",
            format_best(w.chisq, 12)
        ));
        session
            .listing
            .write_line("DF                       1");
        session
            .listing
            .write_line(&format!("Pr > Chi-Square          {}", fmt_p(w.p_chisq)));
        // Silence unused-field warnings while keeping the struct expressive.
        let _ = (w.expected, w.variance, w.std_dev);
        session.listing.blank();
    }

    // Kruskal-Wallis block.
    if let Some(kk) = &a.kruskal {
        centered(session, "Kruskal-Wallis Test");
        session.listing.blank();
        session.listing.write_line(&format!(
            "Chi-Square      {}",
            format_best(kk.h, 12)
        ));
        session
            .listing
            .write_line(&format!("DF              {}", kk.df as i64));
        session
            .listing
            .write_line(&format!("Pr > Chi-Square {}", fmt_p(kk.p)));
        session.listing.blank();
    }
}

// ───────────────────────── OUT= dataset ─────────────────────────

/// Build a small results dataset, one row per VAR, holding the headline test
/// statistic, DF and p-value. KW vars report H/DF/p; Wilcoxon-only (k==2 with
/// no KW) vars report the χ²-approx (z², df=1) and its p-value, with the Z
/// statistic carried in `_Z_`.
fn build_out_dataset(analyses: &[VarAnalysis]) -> Result<SasDataset> {
    let mut var_col: Vec<Option<String>> = Vec::with_capacity(analyses.len());
    let mut test_col: Vec<Option<String>> = Vec::with_capacity(analyses.len());
    let mut n_col: Vec<Option<f64>> = Vec::with_capacity(analyses.len());
    let mut stat_col: Vec<Option<f64>> = Vec::with_capacity(analyses.len());
    let mut z_col: Vec<Option<f64>> = Vec::with_capacity(analyses.len());
    let mut df_col: Vec<Option<f64>> = Vec::with_capacity(analyses.len());
    let mut p_col: Vec<Option<f64>> = Vec::with_capacity(analyses.len());

    for a in analyses {
        var_col.push(Some(a.var_name.clone()));
        n_col.push(Some(a.n as f64));
        if let Some(kk) = &a.kruskal {
            test_col.push(Some("KRUSKAL_WALLIS".into()));
            stat_col.push(Some(kk.h));
            z_col.push(None);
            df_col.push(Some(kk.df));
            p_col.push(Some(kk.p));
        } else if let Some(w) = &a.wilcoxon {
            test_col.push(Some("WILCOXON".into()));
            stat_col.push(Some(w.chisq));
            z_col.push(Some(w.z));
            df_col.push(Some(1.0));
            p_col.push(Some(w.p_normal));
        } else {
            test_col.push(Some("NONE".into()));
            stat_col.push(None);
            z_col.push(None);
            df_col.push(None);
            p_col.push(None);
        }
    }

    let columns: Vec<Column> = vec![
        Series::new("_VAR_".into(), var_col).into(),
        Series::new("_TEST_".into(), test_col).into(),
        Series::new("_N_".into(), n_col).into(),
        Series::new("_STAT_".into(), stat_col).into(),
        Series::new("_Z_".into(), z_col).into(),
        Series::new("_DF_".into(), df_col).into(),
        Series::new("P_VALUE".into(), p_col).into(),
    ];
    let vars: Vec<VarMeta> = vec![
        char_var_meta("_VAR_", 32),
        char_var_meta("_TEST_", 16),
        num_var_meta("_N_"),
        num_var_meta("_STAT_"),
        num_var_meta("_Z_"),
        num_var_meta("_DF_"),
        num_var_meta("P_VALUE"),
    ];

    let df = DataFrame::new(columns)?;
    Ok(SasDataset { df, vars })
}

/// Persist a built results dataset to `target`, update `_LAST_`, and emit the
/// SAS creation NOTE.
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
            length: 4,
            format: None,
            label: None,
        }
    }

    fn write_dataset(session: &mut Session, table: &str, ds: SasDataset) {
        session.libs.get("WORK").unwrap().write(table, &ds).unwrap();
        session.last_dataset = Some(format!("WORK.{}", table.to_uppercase()));
    }

    fn parse_npar(src: &str) -> Result<NparAst> {
        let source = SourceFile::new(src);
        let mut ts = StatementStream::new(&source).unwrap();
        ts.next(); // "proc"
        ts.next(); // "npar1way"
        parse(&mut ts)
    }

    // ───────────── parse tests ─────────────

    #[test]
    fn parse_minimal() {
        let ast = parse_npar("proc npar1way data=a; class g; run;").unwrap();
        assert_eq!(ast.data_options.input.as_ref().unwrap().name, "a");
        assert_eq!(ast.class_var, "g");
        assert!(ast.var_vars.is_empty());
        assert!(!ast.test_options.wilcoxon && !ast.test_options.kruskal);
    }

    #[test]
    fn parse_options_and_statements() {
        let ast = parse_npar(
            "proc npar1way data=a wilcoxon kruskal alpha=0.1 out=b; class grp; var x y; run;",
        )
        .unwrap();
        assert!(ast.test_options.wilcoxon && ast.test_options.kruskal);
        assert!((ast.proc_options.alpha - 0.1).abs() < 1e-12);
        assert_eq!(ast.data_options.output.as_ref().unwrap().name, "b");
        assert_eq!(ast.class_var, "grp");
        assert_eq!(ast.var_vars, vec!["x", "y"]);
    }

    #[test]
    fn parse_kruskalwallis_alias() {
        let ast = parse_npar("proc npar1way kruskalwallis; class g; run;").unwrap();
        assert!(ast.test_options.kruskal);
    }

    #[test]
    fn parse_missing_class_errors() {
        let r = parse_npar("proc npar1way data=a; var x; run;");
        assert!(r.is_err());
        assert!(r.err().unwrap().to_string().contains("CLASS"));
    }

    #[test]
    fn parse_unknown_option_errors() {
        let r = parse_npar("proc npar1way data=a bogus; class g; run;");
        assert!(r.is_err());
        assert!(r.err().unwrap().to_string().contains("BOGUS"));
    }

    #[test]
    fn parse_scores_keywords() {
        let ast = parse_npar("proc npar1way savage; class g; run;").unwrap();
        assert_eq!(ast.test_options.scores, NparScores::Savage);
        let ast = parse_npar("proc npar1way median; class g; run;").unwrap();
        assert_eq!(ast.test_options.scores, NparScores::Median);
    }

    // ───────────── numeric core tests ─────────────

    #[test]
    fn test_ties_correction() {
        // data [1,2,2,3,3,3], n=6. Tie groups sizes {1,2,3}.
        // Σ(t³−t) = 0 + (8−2) + (27−3) = 6 + 24 = 30; n³−n = 210.
        // tie = 1 − 30/210 = 1 − 1/7 = 0.857142857.
        let xs = [1.0, 2.0, 2.0, 3.0, 3.0, 3.0];
        let tie = tie_correction(&xs);
        assert!((tie - 0.8571429).abs() < 1e-6, "tie={tie}");

        // midranks of [1,2,2,3,3,3] = [1, 2.5, 2.5, 5, 5, 5].
        let r = midranks(&xs);
        assert_eq!(r, vec![1.0, 2.5, 2.5, 5.0, 5.0, 5.0]);

        // No ties → tie = 1.0.
        assert!((tie_correction(&[1.0, 2.0, 3.0, 4.0]) - 1.0).abs() < 1e-12);
    }

    #[test]
    fn test_wilcoxon_basic() {
        // groupA=[1,2,3], groupB=[4,5,6], no ties, n=6. Pooled ranks 1..6.
        // W_A = 1+2+3 = 6, E[W_A] = 3·7/2 = 10.5,
        // Var = n_A·n_B·(n+1)/12 = 3·3·7/12 = 5.25, z = (6−10.5)/√5.25 ≈ −1.9640.
        // NO continuity correction applied (v1 choice, see file header).
        let values = [1.0, 2.0, 3.0, 4.0, 5.0, 6.0];
        let ranks = midranks(&values);
        // A = first three, B = last three.
        let is_a = [true, true, true, false, false, false];
        let res = wilcoxon(&ranks, &is_a, 3);
        assert!((res.w - 6.0).abs() < 1e-12, "W={}", res.w);
        assert!((res.expected - 10.5).abs() < 1e-12, "E={}", res.expected);
        // Tie-corrected exact variance reduces to n_A·n_B·(n+1)/12 with no ties.
        assert!((res.variance - 5.25).abs() < 1e-9, "Var={}", res.variance);
        assert!((res.z.abs() - 1.9640).abs() < 1e-3, "z={}", res.z);
    }

    #[test]
    fn test_kruskal_three_groups() {
        // g1=[1,2,3], g2=[4,5,6], g3=[7,8,9], n=9, no ties.
        // Rank sums R=[6,15,24], n_i=3 each.
        // H = [12/(9·10)]·(36+225+576)/3 − 3·10 = (12/90)·279 − 30 = 37.2 − 30 = 7.2.
        // df=2, p = 1 − chisq_cdf(7.2,2) = exp(−3.6) ≈ 0.027324.
        let values: Vec<f64> = (1..=9).map(|v| v as f64).collect();
        let ranks = midranks(&values);
        let rank_sums = vec![6.0, 15.0, 24.0];
        let group_n = vec![3usize, 3, 3];
        let tie = tie_correction(&values);
        let res = kruskal_wallis(&ranks, &rank_sums, &group_n, tie);
        assert!((res.h - 7.2).abs() < 1e-9, "H={}", res.h);
        assert!((res.df - 2.0).abs() < 1e-12);
        assert!((res.p - 0.0273).abs() < 1e-3, "p={}", res.p);
    }

    #[test]
    fn test_tie_corrected_kruskal() {
        // With ties, H is inflated by dividing by tie (<1). Sanity: tie-corrected
        // H >= raw H when tie < 1.
        let values = [1.0, 2.0, 2.0, 3.0, 4.0, 4.0];
        let ranks = midranks(&values);
        // Two groups of 3 (just exercising the divisor).
        let rank_sums = vec![
            ranks[0] + ranks[1] + ranks[2],
            ranks[3] + ranks[4] + ranks[5],
        ];
        let tie = tie_correction(&values);
        assert!(tie < 1.0);
        let res = kruskal_wallis(&ranks, &rank_sums, &[3usize, 3], tie);
        let raw = kruskal_wallis(&ranks, &rank_sums, &[3usize, 3], 1.0);
        assert!(res.h >= raw.h, "corrected {} raw {}", res.h, raw.h);
    }

    // ───────────── execute tests ─────────────

    #[test]
    fn execute_wilcoxon_two_groups() {
        let mut session = make_session();
        let df = df![
            "x" => [1.0_f64, 2.0, 3.0, 4.0, 5.0, 6.0],
            "g" => ["a", "a", "a", "b", "b", "b"]
        ]
        .unwrap();
        let ds = SasDataset {
            df,
            vars: vec![num_meta("x"), char_meta("g")],
        };
        write_dataset(&mut session, "T", ds);

        let ast = NparAst {
            data_options: NparDataOptions {
                input: Some(DatasetRef { libref: Some("WORK".into()), name: "T".into() }),
                output: None,
            },
            proc_options: NparProcOptions::default(),
            var_vars: vec![],
            class_var: "g".into(),
            test_options: NparTestOptions { wilcoxon: false, kruskal: false, scores: NparScores::Normal },
        };
        execute(&ast, &mut session).unwrap();

        let listing = session.listing.into_string();
        assert!(listing.contains("The NPAR1WAY Procedure"), "{listing}");
        assert!(listing.contains("Wilcoxon Scores (Rank Sums)"), "{listing}");
        assert!(listing.contains("Wilcoxon Two-Sample Test"), "{listing}");
    }

    #[test]
    fn execute_kruskal_three_groups() {
        let mut session = make_session();
        let df = df![
            "x" => [1.0_f64, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0],
            "g" => [1.0_f64, 1.0, 1.0, 2.0, 2.0, 2.0, 3.0, 3.0, 3.0]
        ]
        .unwrap();
        let ds = SasDataset {
            df,
            vars: vec![num_meta("x"), num_meta("g")],
        };
        write_dataset(&mut session, "T", ds);

        let ast = NparAst {
            data_options: NparDataOptions {
                input: Some(DatasetRef { libref: Some("WORK".into()), name: "T".into() }),
                output: Some(DatasetRef { libref: Some("WORK".into()), name: "R".into() }),
            },
            proc_options: NparProcOptions::default(),
            var_vars: vec!["x".into()],
            class_var: "g".into(),
            test_options: NparTestOptions { wilcoxon: false, kruskal: false, scores: NparScores::Normal },
        };
        execute(&ast, &mut session).unwrap();

        let listing = session.listing.into_string();
        assert!(listing.contains("Kruskal-Wallis Test"), "{listing}");
        assert!(listing.contains("7.2"), "{listing}");

        // OUT= dataset has one row with the KW statistic.
        let (out, _) = session.libs.get("WORK").unwrap().read("R").unwrap();
        assert_eq!(out.n_obs(), 1);
        let stat = decode_column(&out, 3).unwrap();
        assert!((value_to_num(&stat[0]).unwrap() - 7.2).abs() < 1e-6);
        let p = decode_column(&out, 6).unwrap();
        assert!((value_to_num(&p[0]).unwrap() - 0.0273).abs() < 1e-3);
        assert_eq!(session.last_dataset.as_deref(), Some("WORK.R"));
    }

    #[test]
    fn execute_one_level_errors() {
        let mut session = make_session();
        let df = df![
            "x" => [1.0_f64, 2.0, 3.0],
            "g" => ["a", "a", "a"]
        ]
        .unwrap();
        let ds = SasDataset {
            df,
            vars: vec![num_meta("x"), char_meta("g")],
        };
        write_dataset(&mut session, "T", ds);

        let ast = NparAst {
            data_options: NparDataOptions {
                input: Some(DatasetRef { libref: Some("WORK".into()), name: "T".into() }),
                output: None,
            },
            proc_options: NparProcOptions::default(),
            var_vars: vec![],
            class_var: "g".into(),
            test_options: NparTestOptions { wilcoxon: false, kruskal: false, scores: NparScores::Normal },
        };
        let r = execute(&ast, &mut session);
        assert!(r.is_err());
        assert!(r.err().unwrap().to_string().contains("at least 2 levels"));
    }

    #[test]
    fn execute_savage_rejected() {
        let mut session = make_session();
        let df = df![
            "x" => [1.0_f64, 2.0, 3.0, 4.0],
            "g" => ["a", "a", "b", "b"]
        ]
        .unwrap();
        let ds = SasDataset {
            df,
            vars: vec![num_meta("x"), char_meta("g")],
        };
        write_dataset(&mut session, "T", ds);

        let ast = NparAst {
            data_options: NparDataOptions {
                input: Some(DatasetRef { libref: Some("WORK".into()), name: "T".into() }),
                output: None,
            },
            proc_options: NparProcOptions::default(),
            var_vars: vec![],
            class_var: "g".into(),
            test_options: NparTestOptions { wilcoxon: false, kruskal: false, scores: NparScores::Savage },
        };
        let r = execute(&ast, &mut session);
        assert!(r.is_err());
        assert!(r.err().unwrap().to_string().contains("v1"));
    }

    #[test]
    fn execute_missing_excluded() {
        let mut session = make_session();
        // One row missing the value, one missing the class.
        let df = df![
            "x" => [Some(1.0_f64), Some(2.0), None, Some(4.0), Some(5.0), Some(6.0)],
            "g" => [Some("a"), Some("a"), Some("a"), Some("b"), Some("b"), None]
        ]
        .unwrap();
        let ds = SasDataset {
            df,
            vars: vec![num_meta("x"), char_meta("g")],
        };
        write_dataset(&mut session, "T", ds);

        let ast = NparAst {
            data_options: NparDataOptions {
                input: Some(DatasetRef { libref: Some("WORK".into()), name: "T".into() }),
                output: Some(DatasetRef { libref: Some("WORK".into()), name: "R".into() }),
            },
            proc_options: NparProcOptions::default(),
            var_vars: vec![],
            class_var: "g".into(),
            test_options: NparTestOptions { wilcoxon: false, kruskal: false, scores: NparScores::Normal },
        };
        execute(&ast, &mut session).unwrap();
        // 4 usable rows: x∈{1,2,4,5}, g∈{a,a,b,b}.
        let (out, _) = session.libs.get("WORK").unwrap().read("R").unwrap();
        let n = decode_column(&out, 2).unwrap();
        assert!((value_to_num(&n[0]).unwrap() - 4.0).abs() < 1e-12);
    }
}
