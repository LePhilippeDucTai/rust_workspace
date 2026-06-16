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
//!   3. W = sum of ranks in the group whose CLASS value sorts first (SAS
//!      convention: the first level in the collating sequence)
//!   4. Expected W_0 = n_A(n+1)/2, Var(W) = n_A·n_B·(n+1)/12 (adjust for ties)
//!   5. Z = (W - W_0 - c) / √Var(W) where c is the continuity correction
//!      ±0.5 toward the mean (SAS default CORRECT=YES, documented below)
//!   6. 2-tailed p: 2·(1 − Φ(|z|)) via `stat::probnorm`; 1-sided reported too
//! - Output: Z, one-sided Pr>Z, two-sided Pr>|Z|. Also Kruskal-Wallis (SAS
//!   prints both for k=2).
//!
//! ### Kruskal-Wallis test (k-sample: rank-sum)
//! - k≥2 groups with sizes n_1, ..., n_k, total n
//! - Null: all k distributions identical
//! - Procedure:
//!   1. Pool, rank 1..n (midrank ties)
//!   2. R_i = sum of ranks in group i
//!   3. H = [12 / (n(n+1))] · Σ R_i² / n_i - 3(n+1) (Kruskal-Wallis statistic)
//!   4. H ≈ χ²(df=k-1) under null (divide by tie correction 1 − Σ(t³−t)/(n³−n))
//!   5. p-value: Pr(χ² > H) via `stat::chisq_cdf` (p = 1 − chisq_cdf(H, k-1))
//! - Output: H statistic, DF (k-1), Pr>H (chi-square tail probability)
//!
//! ### Continuity correction (documented choice)
//! SAS PROC NPAR1WAY applies a continuity correction of 0.5 to the Wilcoxon
//! two-sample normal approximation by default (the historical behaviour; the
//! CORRECT= option can turn it off). We follow the default: the numerator
//! `W − E[W]` is shrunk toward zero by 0.5 before dividing by the standard
//! deviation. This matches SAS's printed Z and Pr values.
//!
//! ### Score tests (van der Waerden, Savage, median)
//! - Generalization: assign scores φ(ranks); rank scores are the default and
//!   the only method implemented in v1. SAVAGE / MEDIAN / VW (normal scores)
//!   parse but defer with a clean "score method not yet implemented" error.
//!
//! ### Options
//! - **CLASS** var (required; multi-level automatic)
//! - **VAR** variables (default = all numeric)
//! - **WILCOXON** / **KRUSKAL** : statistics requested. When k=2 both blocks
//!   print; when k>2 Kruskal-Wallis is the relevant test.
//! - **ALPHA=** (default 0.05; not used for p-values, only for CI — deferred)
//! - **OUT=** dataset (ODS-like): one row per VAR with the test statistic and
//!   p-value.
//! - Scoring methods: WILCOXON rank scores (default). VW/SAVAGE/MEDIAN deferred.
//! - (Deferred) **BY** support.
//!
//! ### Error handling
//! - Missing CLASS → error "The CLASS statement is required ..."
//! - CLASS with <2 levels → error "CLASS variable G has fewer than two levels"
//! - Non-numeric VAR → skip with NOTE
//! - Scoring method ≠ default → error "score method not yet implemented"
//! - BY → error "BY processing is not yet supported in PROC NPAR1WAY"
//!
//! ### Tests
//! - Wilcoxon: group A {1,2,3,4} vs B {5,6,7,8} (hand-computable W, Z, p)
//! - Kruskal-Wallis: textbook 3-group example (H, p validated, source cited)
//! - Tie correction: dataset with ties (variance / H adjustment verified)
//! - Error paths: single-level CLASS, missing CLASS, deferred scores/BY.

use crate::ast::DatasetRef;
use crate::dataset::{SasDataset, VarMeta};
use crate::error::{Result, SasError};
use crate::listing::Align;
use crate::missing::value_to_num;
use crate::parser::StatementStream;
use crate::procs::common::decode_column;
use crate::session::Session;
use crate::stat::dists::{chisq_cdf, probnorm};
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

// ───────────────────────────── parsing ─────────────────────────────

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

/// Read a numeric literal token (for ALPHA=).
fn parse_number(ts: &mut StatementStream) -> Result<f64> {
    match ts.peek().kind.clone() {
        TokenKind::Num(v) => {
            ts.next();
            Ok(v)
        }
        _ => Err(SasError::parse("expected a number", ts.peek().span)),
    }
}

/// Parse PROC NPAR1WAY statement and its options. Called AFTER `proc npar1way`
/// has been consumed. Consumes through `run;` / `quit;`.
pub fn parse(ts: &mut StatementStream) -> Result<NparAst> {
    let mut input: Option<DatasetRef> = None;
    let mut output: Option<DatasetRef> = None;
    let mut alpha = 0.05;
    let mut wilcoxon = false;
    let mut kruskal = false;
    let mut scores = NparScores::Normal;
    // Track whether a non-default (non rank-score) scoring method was requested.
    let mut non_default_scores: Option<&'static str> = None;

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
        } else if ts.peek().is_kw("alpha") {
            ts.next();
            expect_eq(ts, "ALPHA")?;
            alpha = parse_number(ts)?;
        } else if ts.peek().is_kw("wilcoxon") {
            ts.next();
            wilcoxon = true;
        } else if ts.peek().is_kw("kruskal") || ts.peek().is_kw("kruskalwallis") {
            ts.next();
            kruskal = true;
        } else if ts.peek().is_kw("savage") {
            ts.next();
            scores = NparScores::Savage;
            non_default_scores = Some("SAVAGE");
        } else if ts.peek().is_kw("median") {
            ts.next();
            scores = NparScores::Median;
            non_default_scores = Some("MEDIAN");
        } else if ts.peek().is_kw("vw") || ts.peek().is_kw("normal") {
            // van der Waerden / normal scores.
            ts.next();
            non_default_scores = Some("VW (normal scores)");
        } else if ts.peek().is_kw("anova")
            || ts.peek().is_kw("edf")
            || ts.peek().is_kw("scores")
            || ts.peek().is_kw("correct")
        {
            // Recognized but unsupported modifiers: skip the option (and any
            // `=value`) gracefully.
            ts.next();
            if ts.peek().kind == TokenKind::Eq {
                ts.next();
                // consume the value token (ident or number)
                if !matches!(ts.peek().kind, TokenKind::Semi | TokenKind::Eof) {
                    ts.next();
                }
            }
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
    let mut has_by = false;

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
        } else if ts.peek().is_kw("by") {
            ts.next();
            ts.skip_to_semi();
            has_by = true;
        } else {
            // Unknown sub-statement: skip it (recovery).
            ts.skip_to_semi();
        }
    }

    if has_by {
        return Err(SasError::runtime(
            "BY processing is not yet supported in PROC NPAR1WAY.",
        ));
    }
    if let Some(method) = non_default_scores {
        return Err(SasError::runtime(format!(
            "The {method} score method is not yet implemented in PROC NPAR1WAY."
        )));
    }
    let class_var = class_var.ok_or_else(|| {
        SasError::runtime("The CLASS statement is required in PROC NPAR1WAY.")
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

// ───────────────────────────── numeric core ─────────────────────────────

/// One per-group summary for the Wilcoxon scores table.
#[derive(Clone)]
struct GroupScores {
    /// CLASS level label.
    label: String,
    /// Number of non-missing observations in the group.
    n: usize,
    /// Sum of (rank) scores in the group.
    sum: f64,
    /// Expected sum of scores under H0 = n_i * (N+1)/2.
    expected: f64,
    /// Standard deviation of the sum of scores under H0.
    std_dev: f64,
    /// Mean score = sum / n_i.
    mean: f64,
}

/// Result of the non-parametric analysis for one VAR.
struct NparResult {
    /// Per-group score summaries (collation order).
    groups: Vec<GroupScores>,
    /// Number of distinct CLASS levels.
    k: usize,
    /// Wilcoxon two-sample statistics (only when k == 2).
    wilcoxon: Option<WilcoxonStats>,
    /// Kruskal-Wallis statistics (always computed).
    kruskal: KruskalStats,
}

struct WilcoxonStats {
    /// Sum of scores W in the first-collating group.
    w: f64,
    /// Standardized statistic Z (with continuity correction).
    z: f64,
    /// One-sided p-value Pr > |Z| in the direction of Z.
    p_one_sided: f64,
    /// Two-sided p-value 2·(1 − Φ(|Z|)).
    p_two_sided: f64,
}

struct KruskalStats {
    /// Chi-square statistic H (tie-corrected).
    h: f64,
    /// Degrees of freedom k − 1.
    df: usize,
    /// p-value Pr > χ².
    p: f64,
}

/// Assign mid-ranks (1-based) to the pooled values. Ties receive the average of
/// the ranks they would otherwise occupy. Returns (ranks parallel to `vals`,
/// tie-correction sum Σ(t³−t) over tie groups).
fn midranks(vals: &[f64]) -> (Vec<f64>, f64) {
    let n = vals.len();
    let mut idx: Vec<usize> = (0..n).collect();
    idx.sort_by(|&a, &b| vals[a].partial_cmp(&vals[b]).unwrap_or(Ordering::Equal));
    let mut ranks = vec![0.0_f64; n];
    let mut tie_sum = 0.0_f64;
    let mut i = 0;
    while i < n {
        let mut j = i + 1;
        while j < n && vals[idx[j]] == vals[idx[i]] {
            j += 1;
        }
        // Ranks i+1 ..= j (1-based) averaged.
        let avg = ((i + 1 + j) as f64) / 2.0; // sum(i+1..=j) / count
        for &k in &idx[i..j] {
            ranks[k] = avg;
        }
        let t = (j - i) as f64;
        if t > 1.0 {
            tie_sum += t * t * t - t;
        }
        i = j;
    }
    (ranks, tie_sum)
}

/// Run the Wilcoxon-score (rank) analysis. `values` are the non-missing VAR
/// observations; `group_of` maps each observation to a group index 0..k;
/// `labels` are the k group labels in collation order.
fn analyze(values: &[f64], group_of: &[usize], labels: &[String]) -> NparResult {
    let n = values.len();
    let nf = n as f64;
    let k = labels.len();
    let (ranks, tie_sum) = midranks(values);

    // Per-group counts and rank sums.
    let mut counts = vec![0usize; k];
    let mut rank_sums = vec![0.0_f64; k];
    for (i, &g) in group_of.iter().enumerate() {
        counts[g] += 1;
        rank_sums[g] += ranks[i];
    }

    // Tie correction factor C = 1 − Σ(t³−t)/(n³−n) (used by both tests).
    let denom = nf * nf * nf - nf;
    let tie_factor = if denom > 0.0 {
        1.0 - tie_sum / denom
    } else {
        1.0
    };

    // Per-group score summaries.
    let mut groups = Vec::with_capacity(k);
    for g in 0..k {
        let ni = counts[g] as f64;
        let expected = ni * (nf + 1.0) / 2.0;
        // Var(sum of scores for group i) under H0 for rank scores:
        //   Var = ni*(N-ni)/(N*(N-1)) * Σ(rank_j - mean_rank)²
        // With the tie-corrected sum of squared deviations this equals
        //   ni*(N-ni)/(N-1) * [ (N+1)/12 * tie_factor ] * ... ; we compute the
        // SS directly for fidelity with ties.
        let ss: f64 = ranks
            .iter()
            .map(|r| {
                let d = r - (nf + 1.0) / 2.0;
                d * d
            })
            .sum();
        let var = if n > 1 {
            ni * (nf - ni) / (nf * (nf - 1.0)) * ss
        } else {
            0.0
        };
        let std_dev = var.max(0.0).sqrt();
        let mean = if counts[g] > 0 {
            rank_sums[g] / ni
        } else {
            0.0
        };
        groups.push(GroupScores {
            label: labels[g].clone(),
            n: counts[g],
            sum: rank_sums[g],
            expected,
            std_dev,
            mean,
        });
    }

    // Kruskal-Wallis (always).
    let mut h = 0.0;
    for g in 0..k {
        if counts[g] > 0 {
            h += rank_sums[g] * rank_sums[g] / counts[g] as f64;
        }
    }
    h = 12.0 / (nf * (nf + 1.0)) * h - 3.0 * (nf + 1.0);
    if tie_factor > 0.0 {
        h /= tie_factor;
    }
    let df = k.saturating_sub(1);
    let kw_p = if df >= 1 {
        1.0 - chisq_cdf(h, df as f64)
    } else {
        1.0
    };
    let kruskal = KruskalStats { h, df, p: kw_p };

    // Wilcoxon two-sample (k == 2). W = sum of scores in the first group.
    let wilcoxon = if k == 2 {
        let n1 = counts[0] as f64;
        let n2 = counts[1] as f64;
        let w = rank_sums[0];
        let e_w = n1 * (nf + 1.0) / 2.0;
        // Var(W) = n1*n2*(N+1)/12 * tie_factor.
        let var_w = n1 * n2 * (nf + 1.0) / 12.0 * tie_factor;
        let diff = w - e_w;
        // Continuity correction of 0.5 toward the mean (SAS default).
        let corrected = if diff > 0.0 {
            diff - 0.5
        } else if diff < 0.0 {
            diff + 0.5
        } else {
            0.0
        };
        let z = if var_w > 0.0 {
            corrected / var_w.sqrt()
        } else {
            0.0
        };
        let p_two_sided = 2.0 * (1.0 - probnorm(z.abs()));
        let p_one_sided = 1.0 - probnorm(z.abs());
        Some(WilcoxonStats {
            w,
            z,
            p_one_sided,
            p_two_sided,
        })
    } else {
        None
    };

    NparResult {
        groups,
        k,
        wilcoxon,
        kruskal,
    }
}

// ───────────────────────────── formatting ─────────────────────────────

/// Format a p-value SAS-style: `<.0001`, else 4 decimals.
fn fmt_p(p: f64) -> String {
    if p < 0.0001 {
        "<.0001".to_string()
    } else {
        format!("{p:.4}")
    }
}

/// Render a CLASS level for display.
fn fmt_class(v: &Value) -> String {
    match v {
        Value::Num(f) => format_best(*f, 12),
        Value::Missing(k) => k.display(),
        Value::Char(s) => s.trim_end().to_string(),
    }
}

fn centered(session: &mut Session, text: &str) {
    let ls = session.listing.ls();
    let pad = ls.saturating_sub(text.len()) / 2;
    session
        .listing
        .write_line(&format!("{}{}", " ".repeat(pad), text));
}

// ───────────────────────────── execute ─────────────────────────────

pub fn execute(ast: &NparAst, session: &mut Session) -> Result<()> {
    let in_ref = resolve_input(ast, session)?;
    let in_libref = in_ref.libref_or_work();
    let in_table = in_ref.name.to_uppercase();

    let provider = session.libs.get(&in_libref)?;
    let (ds, notes) = provider.read(&in_table)?;
    for note in notes {
        session.log.forward(&note);
    }

    let n_obs = ds.n_obs();

    // Resolve the CLASS column.
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
    let class_name = ds.vars[class_idx].name.clone();
    let class_vals = decode_column(&ds, class_idx)?;

    // Resolve VAR list: explicit, else all numeric variables except CLASS.
    let var_cols: Vec<usize> = if !ast.var_vars.is_empty() {
        let mut out = Vec::with_capacity(ast.var_vars.len());
        for nm in &ast.var_vars {
            match ds.vars.iter().position(|m| m.name.eq_ignore_ascii_case(nm)) {
                Some(i) => out.push(i),
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
            .filter(|&i| i != class_idx && ds.vars[i].ty == VarType::Num)
            .collect()
    };

    if var_cols.is_empty() {
        return Err(SasError::runtime(
            "No numeric VAR variables found for PROC NPAR1WAY analysis.",
        ));
    }

    // --- listing header ---
    session.listing.page_header();
    centered(session, "The NPAR1WAY Procedure");
    session.listing.blank();

    // Collect OUT= rows.
    let mut out_rows: Vec<OutRow> = Vec::new();

    for &vc in &var_cols {
        let var_name = ds.vars[vc].name.clone();

        if ds.vars[vc].ty != VarType::Num {
            session.log.note(&format!(
                "Variable {} is character and is skipped by PROC NPAR1WAY.",
                var_name.to_uppercase()
            ));
            continue;
        }

        let var_vals = decode_column(&ds, vc)?;

        // Exclude rows where VAR or CLASS is missing; collect distinct levels in
        // sas_cmp collation order.
        let mut values: Vec<f64> = Vec::new();
        let mut group_keys: Vec<Value> = Vec::new(); // parallel to values
        let mut nmiss = 0usize;
        for r in 0..n_obs {
            let v = value_to_num(&var_vals[r]);
            let class_missing = class_vals[r].is_missing();
            match v {
                Some(f) if !f.is_nan() && !class_missing => {
                    values.push(f);
                    group_keys.push(class_vals[r].clone());
                }
                _ => nmiss += 1,
            }
        }

        if values.is_empty() {
            session.log.note(&format!(
                "Variable {} has no non-missing observations and is skipped.",
                var_name.to_uppercase()
            ));
            continue;
        }

        // Distinct levels in collation order.
        let mut labels_vals: Vec<Value> = Vec::new();
        for key in &group_keys {
            if !labels_vals
                .iter()
                .any(|x| x.sas_cmp(key) == Ordering::Equal)
            {
                labels_vals.push(key.clone());
            }
        }
        labels_vals.sort_by(|a, b| a.sas_cmp(b));
        let k = labels_vals.len();

        if k < 2 {
            return Err(SasError::runtime(format!(
                "CLASS variable {} has fewer than two levels.",
                class_name.to_uppercase()
            )));
        }

        // Map each observation to its group index.
        let group_of: Vec<usize> = group_keys
            .iter()
            .map(|key| {
                labels_vals
                    .iter()
                    .position(|x| x.sas_cmp(key) == Ordering::Equal)
                    .unwrap()
            })
            .collect();
        let labels: Vec<String> = labels_vals.iter().map(fmt_class).collect();

        let result = analyze(&values, &group_of, &labels);

        if nmiss > 0 {
            session.log.note(&format!(
                "PROC NPAR1WAY excluded {} observations with missing values for variable {}.",
                nmiss,
                var_name.to_uppercase()
            ));
        }

        emit_var_listing(session, &var_name, &class_name, &result);

        // OUT= row: choose the relevant test (Wilcoxon when k==2, else KW).
        if result.k == 2 {
            if let Some(w) = &result.wilcoxon {
                out_rows.push(OutRow {
                    var: var_name.clone(),
                    test: "Wilcoxon".to_string(),
                    statistic: w.z,
                    df: None,
                    p_value: w.p_two_sided,
                });
            }
        } else {
            out_rows.push(OutRow {
                var: var_name.clone(),
                test: "Kruskal-Wallis".to_string(),
                statistic: result.kruskal.h,
                df: Some(result.kruskal.df),
                p_value: result.kruskal.p,
            });
        }
    }

    // --- OUT= dataset ---
    if let Some(target) = &ast.data_options.output {
        let out_ds = build_out_dataset(&out_rows)?;
        write_out_dataset(session, target, out_ds)?;
    }

    Ok(())
}

fn emit_var_listing(
    session: &mut Session,
    var_name: &str,
    class_name: &str,
    result: &NparResult,
) {
    // Wilcoxon Scores table.
    centered(
        session,
        &format!(
            "Wilcoxon Scores (Rank Sums) for Variable {} Classified by Variable {}",
            var_name, class_name
        ),
    );
    session.listing.blank();

    let headers: Vec<String> = vec![
        class_name.to_string(),
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
    let mut rows: Vec<Vec<String>> = Vec::with_capacity(result.groups.len());
    for g in &result.groups {
        rows.push(vec![
            g.label.clone(),
            format!("{}", g.n),
            format_best(g.sum, 12),
            format_best(g.expected, 12),
            format_best(g.std_dev, 12),
            format_best(g.mean, 12),
        ]);
    }
    session.listing.write_table(&headers, &aligns, &rows);
    session.listing.blank();

    // Wilcoxon two-sample test block (k == 2).
    if let Some(w) = &result.wilcoxon {
        centered(session, "Wilcoxon Two-Sample Test");
        session.listing.blank();
        session
            .listing
            .write_line(&format!("Statistic (S)            {}", format_best(w.w, 12)));
        session
            .listing
            .write_line(&format!("Z                        {}", format_best(w.z, 12)));
        session
            .listing
            .write_line(&format!("One-Sided Pr >  Z        {}", fmt_p(w.p_one_sided)));
        session
            .listing
            .write_line(&format!("Two-Sided Pr > |Z|       {}", fmt_p(w.p_two_sided)));
        session.listing.blank();
    }

    // Kruskal-Wallis test block.
    centered(session, "Kruskal-Wallis Test");
    session.listing.blank();
    session.listing.write_line(&format!(
        "Chi-Square               {}",
        format_best(result.kruskal.h, 12)
    ));
    session
        .listing
        .write_line(&format!("DF                       {}", result.kruskal.df));
    session.listing.write_line(&format!(
        "Pr > Chi-Square          {}",
        fmt_p(result.kruskal.p)
    ));
    session.listing.blank();
}

// ───────────────────────────── OUT= dataset ─────────────────────────────

struct OutRow {
    var: String,
    test: String,
    statistic: f64,
    df: Option<usize>,
    p_value: f64,
}

/// Build the OUT= dataset: one row per analyzed VAR with the variable name, the
/// test, the test statistic, DF (missing for Wilcoxon Z), and the p-value.
fn build_out_dataset(rows: &[OutRow]) -> Result<SasDataset> {
    let var_col: Vec<Option<String>> = rows.iter().map(|r| Some(r.var.clone())).collect();
    let test_col: Vec<Option<String>> = rows.iter().map(|r| Some(r.test.clone())).collect();
    let stat_col: Vec<Option<f64>> = rows.iter().map(|r| Some(r.statistic)).collect();
    let df_col: Vec<Option<f64>> = rows.iter().map(|r| r.df.map(|d| d as f64)).collect();
    let p_col: Vec<Option<f64>> = rows.iter().map(|r| Some(r.p_value)).collect();

    let columns: Vec<Column> = vec![
        Series::new("_VAR_".into(), var_col).into(),
        Series::new("_TEST_".into(), test_col).into(),
        Series::new("_STAT_".into(), stat_col).into(),
        Series::new("_DF_".into(), df_col).into(),
        Series::new("P_VALUE".into(), p_col).into(),
    ];
    let vars = vec![
        char_var_meta("_VAR_", 32),
        char_var_meta("_TEST_", 20),
        num_var_meta("_STAT_"),
        num_var_meta("_DF_"),
        num_var_meta("P_VALUE"),
    ];
    let df = DataFrame::new(columns)?;
    Ok(SasDataset { df, vars })
}

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
            length: 8,
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
        let ast = parse_npar("proc npar1way data=a wilcoxon; class g; var x; run;").unwrap();
        assert_eq!(ast.data_options.input.as_ref().unwrap().name, "a");
        assert!(ast.test_options.wilcoxon);
        assert_eq!(ast.class_var, "g");
        assert_eq!(ast.var_vars, vec!["x"]);
        assert_eq!(ast.proc_options.alpha, 0.05);
    }

    #[test]
    fn parse_alpha_and_kruskal() {
        let ast =
            parse_npar("proc npar1way data=a kruskal alpha=0.10; class g; var x y; run;").unwrap();
        assert!(ast.test_options.kruskal);
        assert!((ast.proc_options.alpha - 0.10).abs() < 1e-12);
        assert_eq!(ast.var_vars, vec!["x", "y"]);
    }

    #[test]
    fn parse_out_dataset() {
        let ast =
            parse_npar("proc npar1way data=a out=res wilcoxon; class g; var x; run;").unwrap();
        assert_eq!(ast.data_options.output.as_ref().unwrap().name, "res");
    }

    #[test]
    fn parse_missing_class_errors() {
        let r = parse_npar("proc npar1way data=a wilcoxon; var x; run;");
        assert!(r.is_err());
        assert!(r.err().unwrap().to_string().contains("CLASS statement is required"));
    }

    #[test]
    fn parse_savage_scores_deferred() {
        let r = parse_npar("proc npar1way data=a savage; class g; var x; run;");
        assert!(r.is_err());
        assert!(r
            .err()
            .unwrap()
            .to_string()
            .to_lowercase()
            .contains("not yet implemented"));
    }

    #[test]
    fn parse_by_deferred() {
        let r = parse_npar("proc npar1way data=a wilcoxon; class g; var x; by b; run;");
        assert!(r.is_err());
        assert!(r.err().unwrap().to_string().contains("BY processing"));
    }

    #[test]
    fn parse_multiple_class_errors() {
        let r = parse_npar("proc npar1way data=a; class g h; var x; run;");
        assert!(r.is_err());
        assert!(r.err().unwrap().to_string().contains("exactly one variable"));
    }

    // ───────────── numeric core tests ─────────────

    #[test]
    fn midranks_no_ties() {
        let (r, tie) = midranks(&[10.0, 30.0, 20.0]);
        assert_eq!(r, vec![1.0, 3.0, 2.0]);
        assert_eq!(tie, 0.0);
    }

    #[test]
    fn midranks_with_ties() {
        // values: 1,1,2,3 → ranks 1.5,1.5,3,4 ; tie group of 2 → t³−t = 6.
        let (r, tie) = midranks(&[1.0, 1.0, 2.0, 3.0]);
        assert_eq!(r, vec![1.5, 1.5, 3.0, 4.0]);
        assert_eq!(tie, 6.0);
    }

    #[test]
    fn wilcoxon_basic() {
        // Group A {1,2,3,4} vs Group B {5,6,7,8}. Pooled ranks 1..8.
        // W (group A, sorts first) = 1+2+3+4 = 10. n1=n2=4, N=8.
        // E[W] = 4*9/2 = 18. Var(W) = 4*4*9/12 = 12 (no ties).
        // diff = 10-18 = -8; continuity-corrected = -7.5.
        // Z = -7.5/sqrt(12) = -2.16506...
        let values = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0];
        let group_of = vec![0, 0, 0, 0, 1, 1, 1, 1];
        let labels = vec!["A".to_string(), "B".to_string()];
        let res = analyze(&values, &group_of, &labels);
        let w = res.wilcoxon.as_ref().unwrap();
        assert!((w.w - 10.0).abs() < 1e-12, "W={}", w.w);
        assert!((res.groups[0].expected - 18.0).abs() < 1e-12);
        assert!((w.z + 2.165063509).abs() < 1e-6, "Z={}", w.z);
        // Two-sided p = 2*(1-Phi(2.16506)) ≈ 0.03038.
        assert!((w.p_two_sided - 0.030380).abs() < 1e-4, "p={}", w.p_two_sided);
        // Std Dev Under H0 for group A = sqrt(Var(W)) = sqrt(12) = 3.4641016.
        assert!((res.groups[0].std_dev - 12f64.sqrt()).abs() < 1e-9);
    }

    #[test]
    fn kruskal_wallis_three_groups() {
        // Textbook example (Daniel, Biostatistics; widely reproduced):
        // group1 = {68,70,72,74,76}? Use the classic Conover example instead.
        // Source: R kruskal.test on the three groups below.
        //   g1 = {2.9, 3.0, 2.5, 2.6, 3.2}
        //   g2 = {3.8, 2.7, 4.0, 2.4}
        //   g3 = {2.8, 3.4, 3.7, 2.2, 2.0}
        // R kruskal.test reports H = 0.7714, df = 2, p = 0.6798.
        let values = vec![
            2.9, 3.0, 2.5, 2.6, 3.2, // g1
            3.8, 2.7, 4.0, 2.4, // g2
            2.8, 3.4, 3.7, 2.2, 2.0, // g3
        ];
        let group_of = vec![0, 0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2, 2];
        let labels = vec!["1".to_string(), "2".to_string(), "3".to_string()];
        let res = analyze(&values, &group_of, &labels);
        assert_eq!(res.k, 3);
        assert_eq!(res.kruskal.df, 2);
        assert!((res.kruskal.h - 0.7714).abs() < 1e-3, "H={}", res.kruskal.h);
        assert!((res.kruskal.p - 0.6798).abs() < 1e-3, "p={}", res.kruskal.p);
    }

    #[test]
    fn kruskal_wallis_known_h() {
        // Hollander & Wolfe style 3-group example reproduced via R kruskal.test:
        //   A = {1, 2, 3}, B = {4, 5, 6}, C = {7, 8, 9} (perfect separation).
        // Pooled ranks: A=1,2,3 (sum 6); B=4,5,6 (sum 15); C=7,8,9 (sum 24).
        // H = 12/(9*10)*(6^2/3 + 15^2/3 + 24^2/3) - 3*10
        //   = (12/90)*(12 + 75 + 192) - 30 = 0.133333*279 - 30 = 37.2 - 30 = 7.2.
        // df=2, p = 1 - chisq_cdf(7.2, 2) = 0.0273.
        let values = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0];
        let group_of = vec![0, 0, 0, 1, 1, 1, 2, 2, 2];
        let labels = vec!["A".into(), "B".into(), "C".into()];
        let res = analyze(&values, &group_of, &labels);
        assert!((res.kruskal.h - 7.2).abs() < 1e-9, "H={}", res.kruskal.h);
        assert!((res.kruskal.p - 0.0273).abs() < 1e-3, "p={}", res.kruskal.p);
    }

    #[test]
    fn ties_correction() {
        // Two groups with ties. g1={1,1,2}, g2={2,3,3}. Pooled values
        //   1,1,2,2,3,3 → ranks 1.5,1.5,3.5,3.5,5.5,5.5.
        // Tie groups: three of size 2 → Σ(t³−t)=3*6=18. N=6, N³−N=210.
        // tie_factor = 1 - 18/210 = 0.914286.
        // Verify H uses the tie correction: uncorrected H divided by 0.914286.
        let values = vec![1.0, 1.0, 2.0, 2.0, 3.0, 3.0];
        let group_of = vec![0, 0, 0, 1, 1, 1];
        let labels = vec!["g1".into(), "g2".into()];
        let res = analyze(&values, &group_of, &labels);
        // Rank sums: g1 = 1.5+1.5+3.5 = 6.5; g2 = 3.5+5.5+5.5 = 14.5.
        // Uncorrected H = 12/(6*7)*(6.5^2/3 + 14.5^2/3) - 3*7
        //   = (12/42)*(14.0833 + 70.0833) - 21 = 0.285714*84.1667 - 21
        //   = 24.0476 - 21 = 3.0476.
        // Corrected: 3.0476 / 0.914286 = 3.33333.
        assert!((res.kruskal.h - 3.33333).abs() < 1e-3, "H={}", res.kruskal.h);
        // And Wilcoxon Var(W) uses the same tie factor.
        let w = res.wilcoxon.as_ref().unwrap();
        // Var(W)=n1*n2*(N+1)/12*tie = 3*3*7/12*0.914286 = 5.25*0.914286=4.8.
        // Std Dev Under H0 for group g1 = sqrt(4.8)=2.190890.
        assert!(
            (res.groups[0].std_dev - 4.8f64.sqrt()).abs() < 1e-6,
            "sd={}",
            res.groups[0].std_dev
        );
        assert!(w.z.is_finite());
    }

    // ───────────── execute tests ─────────────

    #[test]
    fn execute_wilcoxon_listing() {
        let mut session = make_session();
        let df = df![
            "x" => [1.0_f64, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0],
            "g" => ["A", "A", "A", "A", "B", "B", "B", "B"]
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
            var_vars: vec!["x".into()],
            class_var: "g".into(),
            test_options: NparTestOptions {
                wilcoxon: true,
                kruskal: false,
                scores: NparScores::Normal,
            },
        };
        execute(&ast, &mut session).unwrap();

        let listing = session.listing.into_string();
        assert!(listing.contains("The NPAR1WAY Procedure"), "{listing}");
        assert!(
            listing.contains("Wilcoxon Scores (Rank Sums) for Variable x Classified by Variable g"),
            "{listing}"
        );
        assert!(listing.contains("Wilcoxon Two-Sample Test"), "{listing}");
        assert!(listing.contains("Kruskal-Wallis Test"), "{listing}");
        assert!(listing.contains("Sum of Scores"), "{listing}");
    }

    #[test]
    fn execute_kruskal_three_groups_listing() {
        let mut session = make_session();
        let df = df![
            "x" => [1.0_f64, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0],
            "g" => ["A", "A", "A", "B", "B", "B", "C", "C", "C"]
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
            var_vars: vec!["x".into()],
            class_var: "g".into(),
            test_options: NparTestOptions {
                wilcoxon: false,
                kruskal: true,
                scores: NparScores::Normal,
            },
        };
        execute(&ast, &mut session).unwrap();

        let listing = session.listing.into_string();
        assert!(listing.contains("Kruskal-Wallis Test"), "{listing}");
        assert!(listing.contains("Chi-Square"), "{listing}");
        // No Wilcoxon two-sample block for 3 groups.
        assert!(!listing.contains("Wilcoxon Two-Sample Test"), "{listing}");

        // OUT= dataset: one row, Kruskal-Wallis, H=7.2, df=2.
        let (out, _) = session.libs.get("WORK").unwrap().read("R").unwrap();
        assert_eq!(out.n_obs(), 1);
        let stat = decode_column(&out, 2).unwrap();
        assert!((value_to_num(&stat[0]).unwrap() - 7.2).abs() < 1e-6);
        let dfc = decode_column(&out, 3).unwrap();
        assert!((value_to_num(&dfc[0]).unwrap() - 2.0).abs() < 1e-12);
        assert_eq!(session.last_dataset.as_deref(), Some("WORK.R"));
    }

    #[test]
    fn execute_single_level_class_errors() {
        let mut session = make_session();
        let df = df![
            "x" => [1.0_f64, 2.0, 3.0],
            "g" => ["A", "A", "A"]
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
            var_vars: vec!["x".into()],
            class_var: "g".into(),
            test_options: NparTestOptions {
                wilcoxon: true,
                kruskal: false,
                scores: NparScores::Normal,
            },
        };
        let r = execute(&ast, &mut session);
        assert!(r.is_err());
        assert!(r
            .err()
            .unwrap()
            .to_string()
            .contains("fewer than two levels"));
    }

    #[test]
    fn execute_missing_values_excluded() {
        let mut session = make_session();
        // Two rows with a missing VAR / missing CLASS should be excluded.
        let df = df![
            "x" => [Some(1.0_f64), Some(2.0), None, Some(4.0), Some(5.0), Some(6.0)],
            "g" => [Some("A"), Some("A"), Some("A"), Some("B"), None, Some("B")]
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
            var_vars: vec!["x".into()],
            class_var: "g".into(),
            test_options: NparTestOptions {
                wilcoxon: true,
                kruskal: false,
                scores: NparScores::Normal,
            },
        };
        execute(&ast, &mut session).unwrap();
        // Should run without panic; 4 usable obs (rows 0,1,3,5).
        let listing = session.listing.into_string();
        assert!(listing.contains("Wilcoxon Scores"), "{listing}");
    }
}
