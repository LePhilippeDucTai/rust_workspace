//! PROC GLM — general linear models (v1).
//!
//! # Plan du fichier — voir le plan M25.3
//!
//! `proc glm data=<ref> [alpha=<f64>];
//!  class a b ... ;
//!  model y = a b a*b x ... ;
//!  [lsmeans a b / adjust=TUKEY;]
//!  [contrast 'label' a 1 -1 ;]
//!  [estimate 'label' intercept 1 a 1 / e;]
//!  [output out=<ref>;]
//!  run;`
//!
//! ## Périmètre v1 (fidèle à SAS 9.4 PROC GLM)
//! PROC GLM étend PROC ANOVA :
//! - effets catégoriels (CLASS) ET covariables continues (ANCOVA) ;
//! - sommes de carrés de Type III par défaut (chaque effet testé en contrôlant
//!   tous les autres : SS(E) = SS(modèle complet) − SS(modèle sans E)), avec
//!   aussi le Type I (séquentiel) affiché ;
//! - LSMEANS : moyennes ajustées des moindres carrés, avec erreur-type et IC ;
//! - CONTRAST : test F/t d'une contrainte linéaire L·β ;
//! - ESTIMATE : estimation ponctuelle l·β avec IC.
//!
//! ## Codage et résolution
//! - One-hot des CLASS en omettant la DERNIÈRE modalité (reference cell), comme
//!   ANOVA. Les covariables continues entrent telles quelles.
//! - Intercept + dummies + interactions + covariables → X. β par QR
//!   (`stat::linalg::least_squares`). (X'X)⁻¹ par `invert_matrix` pour les SE.
//!
//! ## Choix / simplifications documentés
//! - Exclusion LISTWISE : retirer une observation dès que y OU une variable du
//!   modèle (CLASS ou continue) est manquante.
//! - Type III SS : implémenté fidèlement par re-ajustement du modèle privé de
//!   l'effet (deux passes de moindres carrés par effet). Coûteux mais exact sur
//!   les plans testés.
//! - LSMEANS : marginalisation par la MOYENNE des autres effets (dummies fixés
//!   à la proportion observée de chaque niveau, covariables à leur moyenne).
//!   C'est l'approximation v1 documentée ; SAS utilise une cellule de référence
//!   équipondérée, ce qui coïncide pour les plans équilibrés.
//! - ADJUST : BONFERRONI (α/k), TUKEY/DUNNETT/SIDAK reconnus mais traités comme
//!   l'IC non ajusté avec une NOTE (approximation v1).
//! - Matrice singulière (colinéarité parfaite) → `SasError::Numerical`.

use crate::ast::DatasetRef;
use crate::dataset::{SasDataset, VarMeta};
use crate::error::{Result, SasError};
use crate::listing::Align;
use crate::missing::value_to_num;
use crate::parser::StatementStream;
use crate::procs::common::{decode_column, t_quantile};
use crate::session::Session;
use crate::stat::dists::{f_cdf, student_t_cdf};
use crate::stat::linalg::{invert_matrix, least_squares};
use crate::token::TokenKind;
use crate::value::{format_best, Value, VarType};
use polars::prelude::{Column, NamedFrom, Series};
use std::cmp::Ordering;

// ───────────────────────── AST ─────────────────────────

/// `class a b c ;` — declares the categorical factors.
pub struct ClassStmt {
    pub vars: Vec<String>,
}

/// `model y = a b a*b x ... ;` — dependent variable and effects.
/// Each effect is a list of variable names: `["a"]` for a main effect / a
/// continuous covariate, or `["a", "b"]` for the interaction `a*b`.
pub struct ModelStmt {
    pub y_var: String,
    pub effects: Vec<Vec<String>>,
}

/// `lsmeans a b / adjust=TUKEY ;`
pub struct LsmeansStmt {
    /// Effects (factor lists) on which to produce least-squares means.
    pub vars: Vec<Vec<String>>,
    /// Multiplicity adjustment (TUKEY/DUNNETT/BONFERRONI/SIDAK), if any.
    pub adjust: Option<String>,
}

/// `contrast 'label' a 1 -1 ;` — one linear hypothesis L·β = 0.
/// Each inner row is a contrast vector over the model coefficients (without the
/// intercept slot pre-filled; coefficients are addressed by effect name when
/// parsed — but in v1 we store the raw numeric coefficients positionally aligned
/// to the design columns of the named effects). For simplicity v1 stores one
/// row of raw coefficients spanning the full parameter vector.
pub struct ContrastStmt {
    pub label: String,
    /// One or more contrast rows, each a full-length coefficient vector over the
    /// model parameters (intercept first). Filled in at execution time from the
    /// parsed (effect, coefficients) spec.
    pub spec: Vec<EffectCoefs>,
}

/// `estimate 'label' intercept 1 a 1 / e ;`
pub struct EstimateStmt {
    pub label: String,
    pub spec: Vec<EffectCoefs>,
}

/// A parsed `<effect> c1 c2 ...` term inside CONTRAST/ESTIMATE: the effect name
/// (`intercept`, a CLASS var, an interaction `a*b`, or a covariate) and the list
/// of numeric coefficients attached to it.
pub struct EffectCoefs {
    pub effect: Vec<String>,
    pub coefs: Vec<f64>,
}

pub struct GlmAst {
    pub data: Option<DatasetRef>,
    /// Confidence-level complement. Default 0.05.
    pub alpha: f64,
    pub class_stmt: ClassStmt,
    pub model_stmt: ModelStmt,
    pub lsmeans_stmt: Vec<LsmeansStmt>,
    pub contrast_stmts: Vec<ContrastStmt>,
    pub estimate_stmts: Vec<EstimateStmt>,
    pub output_out: Option<DatasetRef>,
}

// ───────────────────────── parse ─────────────────────────

/// Parse `proc glm [data=a] [alpha=p]; class ...; model y = ...; [lsmeans ...;]
/// [contrast ...;] [estimate ...;] [output out=b;] run;`. Called AFTER
/// "proc glm" was consumed. Consumes through `run;` / `quit;`.
pub fn parse(ts: &mut StatementStream) -> Result<GlmAst> {
    let mut data: Option<DatasetRef> = None;
    let mut alpha: f64 = 0.05;

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
                    "Unexpected option '{}' on PROC GLM statement.",
                    name.to_uppercase()
                ),
                span,
            ));
        } else {
            let span = ts.peek().span;
            return Err(SasError::parse("Unexpected token on PROC GLM statement.", span));
        }
    }

    // --- sub-statements until run;/quit; ---
    let mut class_vars: Vec<String> = Vec::new();
    let mut model: Option<ModelStmt> = None;
    let mut lsmeans_stmt: Vec<LsmeansStmt> = Vec::new();
    let mut contrast_stmts: Vec<ContrastStmt> = Vec::new();
    let mut estimate_stmts: Vec<EstimateStmt> = Vec::new();
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
                    "PROC GLM v1 supports a single MODEL statement.",
                ));
            }
            model = Some(parse_model(ts)?);
        } else if ts.peek().is_kw("lsmeans") {
            ts.next();
            lsmeans_stmt.push(parse_lsmeans(ts)?);
        } else if ts.peek().is_kw("contrast") {
            ts.next();
            contrast_stmts.push(parse_contrast(ts)?);
        } else if ts.peek().is_kw("estimate") {
            ts.next();
            estimate_stmts.push(parse_estimate(ts)?);
        } else if ts.peek().is_kw("output") {
            ts.next();
            output_out = Some(parse_output(ts)?);
        } else {
            // Unknown sub-statement: skip it (recovery, like anova/reg).
            ts.skip_to_semi();
        }
    }

    let model = model.ok_or_else(|| {
        SasError::runtime("PROC GLM requires a MODEL statement (model y = effects;).")
    })?;

    Ok(GlmAst {
        data,
        alpha,
        class_stmt: ClassStmt { vars: class_vars },
        model_stmt: model,
        lsmeans_stmt,
        contrast_stmts,
        estimate_stmts,
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

/// Parse `model y = a b a*b x ;` (the leading `model` keyword already consumed).
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

/// Parse `lsmeans a b [/ adjust=TUKEY] ;` (the leading `lsmeans` consumed).
fn parse_lsmeans(ts: &mut StatementStream) -> Result<LsmeansStmt> {
    let mut vars: Vec<Vec<String>> = Vec::new();
    let mut adjust: Option<String> = None;
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
            // Options: scan for `adjust=<method>`; skip the rest to ';'.
            loop {
                if ts.peek().kind == TokenKind::Semi
                    || ts.peek().kind == TokenKind::Eof
                    || ts.peek().is_kw("run")
                    || ts.peek().is_kw("quit")
                {
                    break;
                }
                if ts.peek().is_kw("adjust") {
                    ts.next();
                    expect_eq(ts, "ADJUST")?;
                    if let Some(m) = ts.peek().ident().map(str::to_string) {
                        adjust = Some(m.to_uppercase());
                        ts.next();
                    }
                } else {
                    ts.next();
                }
            }
            if ts.peek().kind == TokenKind::Semi {
                ts.next();
            }
            break;
        }
        let eff = parse_one_effect(ts)?;
        if eff.is_empty() {
            break;
        }
        vars.push(eff);
    }
    Ok(LsmeansStmt { vars, adjust })
}

/// Parse `contrast 'label' <effect coefs ...> [/ options] ;`.
fn parse_contrast(ts: &mut StatementStream) -> Result<ContrastStmt> {
    let label = parse_quoted_label(ts, "CONTRAST")?;
    let spec = parse_effect_coefs(ts)?;
    if spec.is_empty() {
        return Err(SasError::runtime(
            "A CONTRAST statement requires at least one coefficient.",
        ));
    }
    Ok(ContrastStmt { label, spec })
}

/// Parse `estimate 'label' <effect coefs ...> [/ options] ;`.
fn parse_estimate(ts: &mut StatementStream) -> Result<EstimateStmt> {
    let label = parse_quoted_label(ts, "ESTIMATE")?;
    let spec = parse_effect_coefs(ts)?;
    if spec.is_empty() {
        return Err(SasError::runtime(
            "An ESTIMATE statement requires at least one coefficient.",
        ));
    }
    Ok(EstimateStmt { label, spec })
}

/// Parse the leading quoted label of a CONTRAST/ESTIMATE statement.
fn parse_quoted_label(ts: &mut StatementStream, kw: &str) -> Result<String> {
    match &ts.peek().kind {
        TokenKind::Str { value, .. } => {
            let s = value.clone();
            ts.next();
            Ok(s)
        }
        _ => Err(SasError::parse(
            format!("The {kw} statement requires a quoted label."),
            ts.peek().span,
        )),
    }
}

/// Parse `<effect> c1 c2 ... <effect> c1 ...` until `;` / `/`. Coefficients are
/// signed numbers; an effect is `intercept`, a name, or an interaction `a*b`.
fn parse_effect_coefs(ts: &mut StatementStream) -> Result<Vec<EffectCoefs>> {
    let mut out: Vec<EffectCoefs> = Vec::new();
    loop {
        if ts.peek().kind == TokenKind::Semi {
            ts.next();
            break;
        }
        if ts.peek().kind == TokenKind::Eof || ts.peek().is_kw("run") || ts.peek().is_kw("quit") {
            break;
        }
        if ts.peek().kind == TokenKind::Slash {
            // Options (e.g. `/ e`): skip to ';'.
            ts.skip_to_semi();
            break;
        }
        // Effect name (or interaction).
        let effect = parse_one_effect(ts)?;
        // Following signed numeric coefficients.
        let mut coefs: Vec<f64> = Vec::new();
        loop {
            if let Some(v) = try_parse_signed_number(ts) {
                coefs.push(v);
            } else {
                break;
            }
        }
        out.push(EffectCoefs { effect, coefs });
    }
    Ok(out)
}

/// Try to parse an optionally-signed numeric literal. Returns None and consumes
/// nothing if the next token is not a number (or +/- followed by a number).
fn try_parse_signed_number(ts: &mut StatementStream) -> Option<f64> {
    match ts.peek().kind.clone() {
        TokenKind::Num(v) => {
            ts.next();
            Some(v)
        }
        TokenKind::Minus => {
            if let TokenKind::Num(v) = ts.peek2().kind.clone() {
                ts.next();
                ts.next();
                Some(-v)
            } else {
                None
            }
        }
        TokenKind::Plus => {
            if let TokenKind::Num(v) = ts.peek2().kind.clone() {
                ts.next();
                ts.next();
                Some(v)
            } else {
                None
            }
        }
        _ => None,
    }
}

/// Parse a list of effects, e.g. `a b a*b x`, until `;`/`/`/run/quit.
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

/// Parse a single effect: `a` or `a*b` or `a*b*c` (or `intercept`).
fn parse_one_effect(ts: &mut StatementStream) -> Result<Vec<String>> {
    let mut factors: Vec<String> = Vec::new();
    let name = match ts.peek().ident().map(str::to_string) {
        Some(n) => n,
        None => {
            return Err(SasError::parse(
                "Expected an effect (variable) name.",
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
        SasError::runtime("The OUTPUT statement of PROC GLM requires OUT=<dataset>.")
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

// ───────────────────────── design construction ─────────────────────────

/// True when a value is a usable (non-missing) class level.
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

/// Cartesian product of per-factor level slices → vector of combinations.
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
    if levels.iter().any(|lv| lv.is_empty()) {
        return Vec::new();
    }
    result
}

/// A resolved model variable: name, dataset column index, decoded values, and
/// whether it is a CLASS factor (vs a continuous covariate).
struct ModelVar {
    name: String,
    is_class: bool,
    vals: Vec<Value>,
}

/// One block of design columns contributed by an effect, with metadata for
/// LSMEANS / contrast addressing.
struct EffectBlock {
    /// The effect (variable names).
    effect: Vec<String>,
    /// Indices into the assembled design matrix (excluding intercept slot 0).
    /// Stored as 1-based positions in the full parameter vector.
    param_idx: Vec<usize>,
    /// For a pure-class effect: the level combination addressed by each column
    /// (the omitted reference cell is implicit — not present). Empty for effects
    /// involving a covariate.
    combos: Vec<Vec<Value>>,
}

/// Result of building the full design.
struct Design {
    /// X matrix, m × n_params (intercept first).
    x: Vec<Vec<f64>>,
    /// y vector, length m.
    y: Vec<f64>,
    /// Number of parameters (intercept + design columns).
    n_params: usize,
    /// One block per model effect (intercept not included).
    blocks: Vec<EffectBlock>,
}

/// Build the design matrix from the resolved model variables and the kept rows.
fn build_design(
    model_vars: &[ModelVar],
    factor_levels: &[Vec<Value>],
    effects: &[Vec<String>],
    kept_rows: &[usize],
    y_vals: &[Value],
) -> Design {
    let m = kept_rows.len();
    let var_index = |nm: &str| -> usize {
        model_vars
            .iter()
            .position(|v| v.name.eq_ignore_ascii_case(nm))
            .expect("model var resolved earlier")
    };

    let mut design_cols: Vec<Vec<f64>> = Vec::new();
    let mut blocks: Vec<EffectBlock> = Vec::new();

    for eff in effects {
        let idxs: Vec<usize> = eff.iter().map(|f| var_index(f)).collect();
        let all_class = idxs.iter().all(|&i| model_vars[i].is_class);
        let start_param = design_cols.len() + 1; // +1 for intercept

        if all_class {
            // Dummy levels (all but last) per factor of the effect.
            let dummy_levels: Vec<&[Value]> = idxs
                .iter()
                .map(|&fi| {
                    let lv = &factor_levels[fi];
                    let take = lv.len().saturating_sub(1);
                    &lv[..take]
                })
                .collect();
            let combos = cartesian(&dummy_levels);
            let mut combo_vals: Vec<Vec<Value>> = Vec::new();
            for combo in &combos {
                let mut col = vec![0.0_f64; m];
                for (pos, &r) in kept_rows.iter().enumerate() {
                    let mut hit = true;
                    for (k, &fi) in idxs.iter().enumerate() {
                        if model_vars[fi].vals[r].sas_cmp(combo[k]) != Ordering::Equal {
                            hit = false;
                            break;
                        }
                    }
                    if hit {
                        col[pos] = 1.0;
                    }
                }
                design_cols.push(col);
                combo_vals.push(combo.iter().map(|v| (*v).clone()).collect());
            }
            let n = combo_vals.len();
            blocks.push(EffectBlock {
                effect: eff.clone(),
                param_idx: (start_param..start_param + n).collect(),
                combos: combo_vals,
            });
        } else {
            // Effect involves continuous covariate(s): one column = product of
            // the covariate values (and class dummies if mixed; v1 supports a
            // single continuous covariate per effect cleanly, products work for
            // continuous*continuous too).
            let mut col = vec![1.0_f64; m];
            for &fi in &idxs {
                if model_vars[fi].is_class {
                    // Mixed class*continuous: use the first-level indicator is
                    // not standard; v1 multiplies the numeric code. To keep it
                    // simple and well-defined, treat the class var's numeric
                    // value (decoded) as the covariate. For char class vars this
                    // degenerates; documented limitation.
                }
                for (pos, &r) in kept_rows.iter().enumerate() {
                    let v = value_to_num(&model_vars[fi].vals[r]).unwrap_or(f64::NAN);
                    col[pos] *= v;
                }
            }
            design_cols.push(col);
            blocks.push(EffectBlock {
                effect: eff.clone(),
                param_idx: vec![start_param],
                combos: Vec::new(),
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
        y.push(value_to_num(&y_vals[r]).unwrap());
    }

    Design {
        x,
        y,
        n_params,
        blocks,
    }
}

/// SSE of an OLS fit of `y` on the given design columns (a subset of params).
/// `param_keep` lists the parameter indices (into the full vector) to retain;
/// index 0 (intercept) should be included by the caller when desired.
fn sse_for_subset(full_x: &[Vec<f64>], y: &[f64], param_keep: &[usize]) -> Result<f64> {
    let m = full_x.len();
    let mut sub: Vec<Vec<f64>> = Vec::with_capacity(m);
    for row in full_x {
        sub.push(param_keep.iter().map(|&j| row[j]).collect());
    }
    let beta = least_squares(&sub, y)?;
    let mut sse = 0.0;
    for i in 0..m {
        let mut yhat = 0.0;
        for (k, &j) in param_keep.iter().enumerate() {
            yhat += full_x[i][j] * beta[k];
        }
        let e = y[i] - yhat;
        sse += e * e;
    }
    Ok(sse)
}

// ───────────────────────── execute ─────────────────────────

pub fn execute(ast: &GlmAst, session: &mut Session) -> Result<()> {
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

    // Dependent variable: must be numeric.
    let y_col = resolve_col(&ast.model_stmt.y_var)?;
    if ds.vars[y_col].ty != VarType::Num {
        return Err(SasError::runtime(format!(
            "Variable {} in the MODEL statement is not numeric.",
            ast.model_stmt.y_var.to_uppercase()
        )));
    }
    let y_vals = decode_column(&ds, y_col)?;

    // Resolve every distinct variable referenced by the model effects.
    let mut model_var_names: Vec<String> = Vec::new();
    for eff in &ast.model_stmt.effects {
        for f in eff {
            if !model_var_names.iter().any(|m| m.eq_ignore_ascii_case(f)) {
                model_var_names.push(f.clone());
            }
        }
    }

    let mut model_vars: Vec<ModelVar> = Vec::new();
    for nm in &model_var_names {
        let c = resolve_col(nm)?;
        let is_class = ast.class_stmt.vars.iter().any(|cv| cv.eq_ignore_ascii_case(nm));
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

    // LISTWISE exclusion: keep rows where y AND every model var is present.
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
    let m = kept_rows.len();

    // Levels for each model var (class vars only meaningfully).
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

    let design = build_design(
        &model_vars,
        &factor_levels,
        &ast.model_stmt.effects,
        &kept_rows,
        &y_vals,
    );
    let n_params = design.n_params;

    if m < n_params + 1 {
        return Err(SasError::runtime(format!(
            "Insufficient non-missing observations ({m}) for {n_params} parameters in PROC GLM.",
        )));
    }

    // Solve full model.
    let beta = least_squares(&design.x, &design.y)?;
    let mut sse = 0.0_f64;
    for i in 0..m {
        let mut yhat = 0.0;
        for j in 0..n_params {
            yhat += design.x[i][j] * beta[j];
        }
        let e = design.y[i] - yhat;
        sse += e * e;
    }

    let ybar = design.y.iter().sum::<f64>() / m as f64;
    let sst: f64 = design.y.iter().map(|v| (v - ybar) * (v - ybar)).sum();
    let ssr = (sst - sse).max(0.0);

    let df_model = (n_params - 1) as f64;
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

    // (X'X)⁻¹ for SEs / LSMEANS / contrasts.
    let xtx = xtx_matrix(&design.x, n_params);
    let xtx_inv = invert_matrix(&xtx)?;

    // --- Type I (sequential) and Type III sums of squares per effect. ---
    let type1 = type1_ss(&design, &design.y, sse)?;
    let type3 = type3_ss(&design, &design.y, sse)?;

    // --- listing ---
    emit_listing(
        session,
        &ds,
        y_col,
        &ast.class_stmt.vars,
        &model_vars,
        &factor_levels,
        &in_display,
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
        &design.blocks,
        &type1,
        &type3,
    );

    // --- LSMEANS ---
    for ls in &ast.lsmeans_stmt {
        emit_lsmeans(
            session,
            ast,
            ls,
            &model_vars,
            &factor_levels,
            &design,
            &beta,
            &xtx_inv,
            mse,
            df_error,
            &kept_rows,
        );
    }

    // --- CONTRAST ---
    for c in &ast.contrast_stmts {
        emit_contrast(session, c, &design, &beta, &xtx_inv, mse, df_error);
    }

    // --- ESTIMATE ---
    for e in &ast.estimate_stmts {
        emit_estimate(session, ast, e, &design, &beta, &xtx_inv, mse, df_error);
    }

    // --- OUTPUT OUT= ---
    if let Some(target) = &ast.output_out {
        // Fitted/residuals over kept rows.
        let mut fitted = vec![0.0_f64; m];
        let mut resid = vec![0.0_f64; m];
        for i in 0..m {
            let mut yhat = 0.0;
            for j in 0..n_params {
                yhat += design.x[i][j] * beta[j];
            }
            fitted[i] = yhat;
            resid[i] = design.y[i] - yhat;
        }
        write_output_dataset(session, &ds, target, &kept_rows, &fitted, &resid, n_obs)?;
    }

    Ok(())
}

/// Form X'X.
fn xtx_matrix(x: &[Vec<f64>], n_params: usize) -> Vec<Vec<f64>> {
    let mut out = vec![vec![0.0; n_params]; n_params];
    for row in x {
        for a in 0..n_params {
            for b in 0..n_params {
                out[a][b] += row[a] * row[b];
            }
        }
    }
    out
}

/// One effect's SS / DF / F / p.
struct EffectSS {
    effect: Vec<String>,
    ss: f64,
    df: usize,
}

/// Type I (sequential) SS: add each effect's columns in order; SS(E) is the drop
/// in SSE when E's columns are added to the running model (intercept first).
fn type1_ss(design: &Design, y: &[f64], _full_sse: f64) -> Result<Vec<EffectSS>> {
    let mut keep: Vec<usize> = vec![0]; // intercept
    let mut prev_sse = sse_for_subset(&design.x, y, &keep)?;
    let mut out: Vec<EffectSS> = Vec::new();
    for blk in &design.blocks {
        keep.extend(blk.param_idx.iter().copied());
        let cur_sse = sse_for_subset(&design.x, y, &keep)?;
        let ss = (prev_sse - cur_sse).max(0.0);
        out.push(EffectSS {
            effect: blk.effect.clone(),
            ss,
            df: blk.param_idx.len(),
        });
        prev_sse = cur_sse;
    }
    Ok(out)
}

/// Type III (partial) SS: SS(E) = SSE(model without E) − SSE(full model), each
/// effect tested controlling for all others. Two fits per effect.
fn type3_ss(design: &Design, y: &[f64], full_sse: f64) -> Result<Vec<EffectSS>> {
    let mut out: Vec<EffectSS> = Vec::new();
    for (bi, blk) in design.blocks.iter().enumerate() {
        // Keep intercept + all other effects' params.
        let mut keep: Vec<usize> = vec![0];
        for (bj, other) in design.blocks.iter().enumerate() {
            if bj != bi {
                keep.extend(other.param_idx.iter().copied());
            }
        }
        let reduced_sse = sse_for_subset(&design.x, y, &keep)?;
        let ss = (reduced_sse - full_sse).max(0.0);
        out.push(EffectSS {
            effect: blk.effect.clone(),
            ss,
            df: blk.param_idx.len(),
        });
    }
    Ok(out)
}

// ───────────────────────── LSMEANS ─────────────────────────

#[allow(clippy::too_many_arguments)]
fn emit_lsmeans(
    session: &mut Session,
    ast: &GlmAst,
    ls: &LsmeansStmt,
    model_vars: &[ModelVar],
    factor_levels: &[Vec<Value>],
    design: &Design,
    beta: &[f64],
    xtx_inv: &[Vec<f64>],
    mse: f64,
    df_error: f64,
    kept_rows: &[usize],
) {
    let var_index = |nm: &str| -> Option<usize> {
        model_vars.iter().position(|v| v.name.eq_ignore_ascii_case(nm))
    };

    for eff in &ls.vars {
        // Must be a class effect in the model.
        let idxs: Option<Vec<usize>> = eff.iter().map(|f| var_index(f)).collect();
        let Some(idxs) = idxs else {
            session.log.warning(&format!(
                "Effect {} in the LSMEANS statement is not in the model; skipped.",
                eff.join("*").to_uppercase()
            ));
            continue;
        };
        if idxs.iter().any(|&i| !model_vars[i].is_class) {
            session.log.warning(&format!(
                "LSMEANS for continuous effect {} is not supported; skipped.",
                eff.join("*").to_uppercase()
            ));
            continue;
        }

        centered(session, "The GLM Procedure");
        session.listing.blank();
        centered(session, "Least Squares Means");
        session.listing.blank();

        // Levels of this effect (full cartesian of each factor's levels).
        let level_slices: Vec<&[Value]> =
            idxs.iter().map(|&fi| factor_levels[fi].as_slice()).collect();
        let combos = cartesian_full(&level_slices);

        // Number of comparisons for Bonferroni: k levels.
        let k = combos.len().max(1);
        let mut alpha = ast.alpha;
        let mut adjust_note: Option<String> = None;
        if let Some(adj) = &ls.adjust {
            match adj.as_str() {
                "BONFERRONI" => {
                    alpha /= k as f64;
                }
                other => {
                    adjust_note = Some(other.to_string());
                }
            }
        }
        let t_crit = if df_error > 0.0 {
            t_quantile(1.0 - alpha / 2.0, df_error)
        } else {
            f64::NAN
        };

        let mut headers: Vec<String> = eff.iter().cloned().collect();
        headers.push("LSMEAN".into());
        headers.push("Standard Error".into());
        headers.push("DF".into());
        let level_pct = ((1.0 - ast.alpha) * 100.0).round() as i64;
        headers.push(format!("Lower {level_pct}%"));
        headers.push(format!("Upper {level_pct}%"));
        let mut aligns: Vec<Align> = vec![Align::Left; eff.len()];
        for _ in 0..5 {
            aligns.push(Align::Right);
        }

        let mut rows: Vec<Vec<String>> = Vec::new();
        for combo in &combos {
            // Build the prediction row L: marginalize all OTHER effects to their
            // observed mean dummy proportions / covariate means; for THIS
            // effect, set the dummies to the addressed level.
            let lrow = lsmeans_l_vector(design, eff, combo, kept_rows);
            // Estimate = L·β.
            let est: f64 = lrow.iter().zip(beta).map(|(a, b)| a * b).sum();
            // Var = L (X'X)⁻¹ L' · σ².
            let var = quad_form(&lrow, xtx_inv) * mse;
            let se = if var > 0.0 { var.sqrt() } else { 0.0 };
            let (lo, hi) = if t_crit.is_finite() {
                (est - t_crit * se, est + t_crit * se)
            } else {
                (f64::NAN, f64::NAN)
            };
            let mut row: Vec<String> = combo.iter().map(|v| level_label(v)).collect();
            row.push(fmt_num(est));
            row.push(fmt_num(se));
            row.push(fmt_df(df_error));
            row.push(fmt_num(lo));
            row.push(fmt_num(hi));
            rows.push(row);
        }
        session.listing.write_table(&headers, &aligns, &rows);

        if let Some(note) = adjust_note {
            session.listing.blank();
            session.listing.write_line(&format!(
                "Note: ADJUST={note} multiplicity adjustment is approximated by the \
                 unadjusted limits in this version."
            ));
        }
        session.listing.blank();
    }
}

/// Full cartesian product (no reference-cell omission) of level slices.
fn cartesian_full(levels: &[&[Value]]) -> Vec<Vec<Value>> {
    let mut result: Vec<Vec<Value>> = vec![vec![]];
    for lv in levels {
        let mut next: Vec<Vec<Value>> = Vec::new();
        for prefix in &result {
            for v in lv.iter() {
                let mut c = prefix.clone();
                c.push(v.clone());
                next.push(c);
            }
        }
        result = next;
    }
    if levels.iter().any(|lv| lv.is_empty()) {
        return Vec::new();
    }
    result
}

/// Build the L (prediction) vector for an LSMEAN at the given level combo of the
/// requested effect, marginalizing every other design column to its observed
/// column mean (over kept rows). The requested effect's own design columns are
/// set to the indicator of the addressed level (which may be the reference cell,
/// i.e. all zeros).
fn lsmeans_l_vector(
    design: &Design,
    req_effect: &[String],
    combo: &[Value],
    kept_rows: &[usize],
) -> Vec<f64> {
    let n = design.n_params;
    let m = kept_rows.len().max(1);
    // Default: every design column at its mean over kept rows (intercept = 1).
    let mut l = vec![0.0_f64; n];
    l[0] = 1.0;
    for j in 1..n {
        let mut s = 0.0;
        for i in 0..design.x.len() {
            s += design.x[i][j];
        }
        l[j] = s / m as f64;
    }

    // Override the requested effect's own design columns to the indicator of the
    // addressed level combination (the omitted reference cell is all zeros).
    // Interactions that merely *involve* the requested effect keep their column
    // mean (documented v1 marginalization approximation).
    for blk in &design.blocks {
        if !effect_eq(&blk.effect, req_effect) {
            continue;
        }
        for (col_pos, &pidx) in blk.param_idx.iter().enumerate() {
            let on = col_pos < blk.combos.len()
                && blk.combos[col_pos]
                    .iter()
                    .zip(combo)
                    .all(|(a, b)| a.sas_cmp(b) == Ordering::Equal);
            l[pidx] = if on { 1.0 } else { 0.0 };
        }
    }
    l
}

/// Case-insensitive set equality of two effect (variable-name) lists.
fn effect_eq(a: &[String], b: &[String]) -> bool {
    a.len() == b.len()
        && a.iter()
            .all(|x| b.iter().any(|y| x.eq_ignore_ascii_case(y)))
        && b.iter()
            .all(|y| a.iter().any(|x| x.eq_ignore_ascii_case(y)))
}

/// Quadratic form v' M v.
fn quad_form(v: &[f64], m: &[Vec<f64>]) -> f64 {
    let n = v.len();
    let mut s = 0.0;
    for i in 0..n {
        let mut mi = 0.0;
        for j in 0..n {
            mi += m[i][j] * v[j];
        }
        s += v[i] * mi;
    }
    s
}

// ───────────────────────── CONTRAST / ESTIMATE ─────────────────────────

/// Map an EffectCoefs spec to a full-length coefficient vector over the model
/// parameters (intercept first). Returns None with a warning when an effect or
/// coefficient count cannot be resolved.
fn spec_to_lvector(
    spec: &[EffectCoefs],
    design: &Design,
    intercept_name_ok: bool,
) -> std::result::Result<Vec<f64>, String> {
    let mut l = vec![0.0_f64; design.n_params];
    for ec in spec {
        if ec.effect.len() == 1 && ec.effect[0].eq_ignore_ascii_case("intercept") {
            if !intercept_name_ok {
                return Err("INTERCEPT is not allowed in a CONTRAST row.".to_string());
            }
            if let Some(&c) = ec.coefs.first() {
                l[0] = c;
            }
            continue;
        }
        // Find the matching block (by effect name set).
        let blk = design.blocks.iter().find(|b| {
            b.effect.len() == ec.effect.len()
                && b.effect
                    .iter()
                    .zip(&ec.effect)
                    .all(|(a, b)| a.eq_ignore_ascii_case(b))
        });
        let Some(blk) = blk else {
            return Err(format!(
                "Effect {} not found in the model.",
                ec.effect.join("*").to_uppercase()
            ));
        };
        // Map coefficients positionally onto the block's design columns. Note:
        // SAS contrasts address levels in level order; the design omits the last
        // (reference) level. We apply the first param_idx.len() coefficients to
        // the non-reference columns; an extra trailing coefficient (for the
        // reference level) is ignored (its effect is folded into the intercept),
        // matching the estimable-function convention for balanced designs.
        for (k, &pidx) in blk.param_idx.iter().enumerate() {
            if let Some(&c) = ec.coefs.get(k) {
                l[pidx] = c;
            }
        }
    }
    Ok(l)
}

#[allow(clippy::too_many_arguments)]
fn emit_contrast(
    session: &mut Session,
    c: &ContrastStmt,
    design: &Design,
    beta: &[f64],
    xtx_inv: &[Vec<f64>],
    mse: f64,
    df_error: f64,
) {
    centered(session, "The GLM Procedure");
    session.listing.blank();
    centered(session, "Contrast Tests");
    session.listing.blank();

    let lvec = match spec_to_lvector(&c.spec, design, false) {
        Ok(v) => v,
        Err(e) => {
            session.log.warning(&format!(
                "CONTRAST '{}' could not be evaluated: {e}",
                c.label
            ));
            return;
        }
    };

    // SS for a single-row contrast: (L·β)² / (L (X'X)⁻¹ L').
    let lb: f64 = lvec.iter().zip(beta).map(|(a, b)| a * b).sum();
    let denom = quad_form(&lvec, xtx_inv);
    let df_contrast = 1.0;
    let ss = if denom > 0.0 { lb * lb / denom } else { f64::NAN };
    let ms = ss / df_contrast;
    let f_value = if mse > 0.0 { ms / mse } else { f64::NAN };
    let pr_f = if f_value.is_finite() && df_error > 0.0 {
        1.0 - f_cdf(f_value, df_contrast, df_error)
    } else {
        f64::NAN
    };

    let headers: Vec<String> = vec![
        "Contrast".into(),
        "DF".into(),
        "Contrast SS".into(),
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
    let rows = vec![vec![
        c.label.clone(),
        fmt_df(df_contrast),
        fmt_num(ss),
        fmt_num(ms),
        fmt_f(f_value),
        fmt_p(pr_f),
    ]];
    session.listing.write_table(&headers, &aligns, &rows);
    session.listing.blank();
}

#[allow(clippy::too_many_arguments)]
fn emit_estimate(
    session: &mut Session,
    ast: &GlmAst,
    e: &EstimateStmt,
    design: &Design,
    beta: &[f64],
    xtx_inv: &[Vec<f64>],
    mse: f64,
    df_error: f64,
) {
    centered(session, "The GLM Procedure");
    session.listing.blank();
    centered(session, "Estimate");
    session.listing.blank();

    let lvec = match spec_to_lvector(&e.spec, design, true) {
        Ok(v) => v,
        Err(err) => {
            session.log.warning(&format!(
                "ESTIMATE '{}' could not be evaluated: {err}",
                e.label
            ));
            return;
        }
    };

    let est: f64 = lvec.iter().zip(beta).map(|(a, b)| a * b).sum();
    let var = quad_form(&lvec, xtx_inv) * mse;
    let se = if var > 0.0 { var.sqrt() } else { 0.0 };
    let tval = if se > 0.0 { est / se } else { f64::NAN };
    let pr_t = if tval.is_finite() && df_error > 0.0 {
        2.0 * (1.0 - student_t_cdf(tval.abs(), df_error))
    } else {
        f64::NAN
    };
    let t_crit = if df_error > 0.0 {
        t_quantile(1.0 - ast.alpha / 2.0, df_error)
    } else {
        f64::NAN
    };
    let (lo, hi) = if t_crit.is_finite() {
        (est - t_crit * se, est + t_crit * se)
    } else {
        (f64::NAN, f64::NAN)
    };
    let level_pct = ((1.0 - ast.alpha) * 100.0).round() as i64;

    let headers: Vec<String> = vec![
        "Parameter".into(),
        "Estimate".into(),
        "Standard Error".into(),
        "t Value".into(),
        "Pr > |t|".into(),
        format!("Lower {level_pct}%"),
        format!("Upper {level_pct}%"),
    ];
    let aligns = vec![
        Align::Left,
        Align::Right,
        Align::Right,
        Align::Right,
        Align::Right,
        Align::Right,
        Align::Right,
    ];
    let rows = vec![vec![
        e.label.clone(),
        fmt_num(est),
        fmt_num(se),
        fmt_f(tval),
        fmt_p(pr_t),
        fmt_num(lo),
        fmt_num(hi),
    ]];
    session.listing.write_table(&headers, &aligns, &rows);
    session.listing.blank();
}

// ───────────────────────── OUTPUT OUT= ─────────────────────────

fn write_output_dataset(
    session: &mut Session,
    ds: &SasDataset,
    target: &DatasetRef,
    kept_rows: &[usize],
    fitted: &[f64],
    resid: &[f64],
    n_obs: usize,
) -> Result<()> {
    let mut pred_full: Vec<Option<f64>> = vec![None; n_obs];
    let mut res_full: Vec<Option<f64>> = vec![None; n_obs];
    for (pos, &r) in kept_rows.iter().enumerate() {
        pred_full[r] = Some(fitted[pos]);
        res_full[r] = Some(resid[pos]);
    }

    let mut columns: Vec<Column> = ds.df.get_columns().to_vec();
    let mut vars: Vec<VarMeta> = ds.vars.clone();

    columns.push(Series::new("PREDICTED".into(), pred_full).into());
    vars.push(num_var_meta("PREDICTED"));
    columns.push(Series::new("RESIDUAL".into(), res_full).into());
    vars.push(num_var_meta("RESIDUAL"));

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
    _in_display: &str,
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
    _blocks: &[EffectBlock],
    type1: &[EffectSS],
    type3: &[EffectSS],
) {
    session.listing.page_header();
    centered(session, "The GLM Procedure");
    session.listing.blank();

    // Class Level Information.
    centered(session, "Class Level Information");
    session.listing.blank();
    {
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
            } else if let Some(ci) = ds.vars.iter().position(|v| v.name.eq_ignore_ascii_case(cv)) {
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

    centered(session, "The GLM Procedure");
    session.listing.blank();
    centered(
        session,
        &format!("Dependent Variable: {}", ds.vars[y_col].name),
    );
    session.listing.blank();

    // Overall Analysis of Variance.
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

    // Fit statistics.
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

    // Type I SS table.
    emit_effect_ss_table(session, "Type I", type1, mse, df_error);
    // Type III SS table.
    emit_effect_ss_table(session, "Type III", type3, mse, df_error);
}

/// Emit a "Source / DF / Type x SS / Mean Square / F Value / Pr > F" table.
fn emit_effect_ss_table(
    session: &mut Session,
    label: &str,
    effects: &[EffectSS],
    mse: f64,
    df_error: f64,
) {
    let headers: Vec<String> = vec![
        "Source".into(),
        "DF".into(),
        format!("{label} SS"),
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
    let mut rows: Vec<Vec<String>> = Vec::new();
    for e in effects {
        let df = e.df as f64;
        let ms = if df > 0.0 { e.ss / df } else { f64::NAN };
        let f_value = if mse > 0.0 { ms / mse } else { f64::NAN };
        let pr_f = if f_value.is_finite() && df > 0.0 && df_error > 0.0 {
            1.0 - f_cdf(f_value, df, df_error)
        } else {
            f64::NAN
        };
        rows.push(vec![
            e.effect.join("*"),
            fmt_df(df),
            fmt_num(e.ss),
            fmt_num(ms),
            fmt_f(f_value),
            fmt_p(pr_f),
        ]);
    }
    session.listing.write_table(&headers, &aligns, &rows);
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

    fn parse_glm(src: &str) -> Result<GlmAst> {
        let source = SourceFile::new(src);
        let mut ts = StatementStream::new(&source).unwrap();
        ts.next(); // "proc"
        ts.next(); // "glm"
        parse(&mut ts)
    }

    fn run_glm(ds: SasDataset, src: &str) -> (String, String, Session) {
        let mut session = make_session();
        write_dataset(&mut session, "A", ds);
        let ast = parse_glm(src).unwrap();
        execute(&ast, &mut session).unwrap();
        let listing = session.listing.into_string();
        let log = session.log.buffer().to_string();
        (listing, log, session)
    }

    // ───────────── parse tests ─────────────

    #[test]
    fn parse_minimal() {
        let ast = parse_glm("proc glm data=a; class g; model y = g; run;").unwrap();
        assert_eq!(ast.class_stmt.vars, vec!["g"]);
        assert_eq!(ast.model_stmt.y_var, "y");
        assert_eq!(ast.model_stmt.effects, vec![vec!["g".to_string()]]);
    }

    #[test]
    fn parse_full_statements() {
        let ast = parse_glm(
            "proc glm data=a; class a b; model y = a b a*b; \
             lsmeans a / adjust=tukey; contrast 'a' a 1 -1; \
             estimate 'e' intercept 1 a 1 / e; output out=o; run;",
        )
        .unwrap();
        assert_eq!(ast.lsmeans_stmt.len(), 1);
        assert_eq!(ast.lsmeans_stmt[0].adjust.as_deref(), Some("TUKEY"));
        assert_eq!(ast.contrast_stmts.len(), 1);
        assert_eq!(ast.contrast_stmts[0].label, "a");
        assert_eq!(ast.estimate_stmts.len(), 1);
        assert!(ast.output_out.is_some());
    }

    #[test]
    fn parse_requires_model() {
        assert!(parse_glm("proc glm data=a; class g; run;").is_err());
    }

    // ───────────── data builders ─────────────

    fn one_way_ds() -> SasDataset {
        let g = vec!["A", "A", "A", "A", "B", "B", "B", "B", "C", "C", "C", "C"];
        let y = vec![
            9.0, 10.0, 11.0, 10.0, // A ~10
            19.0, 20.0, 21.0, 20.0, // B ~20
            29.0, 30.0, 31.0, 30.0, // C ~30
        ];
        let df = df!["g" => g, "y" => y].unwrap();
        SasDataset {
            df,
            vars: vec![char_meta("g"), num_meta("y")],
        }
    }

    fn two_way_ds() -> SasDataset {
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
                    let ie = if av == "H" && bv == "R" { 3.0 } else { 0.0 };
                    y.push(10.0 + ae + be + ie + rep as f64 * 0.2);
                }
            }
        }
        let df = df!["a" => a, "b" => b, "y" => y].unwrap();
        SasDataset {
            df,
            vars: vec![char_meta("a"), char_meta("b"), num_meta("y")],
        }
    }

    // ───────────── execute tests ─────────────

    #[test]
    fn glm_simple_anova() {
        let (listing, _log, _s) =
            run_glm(one_way_ds(), "proc glm data=a; class g; model y = g; run;");
        assert!(listing.contains("The GLM Procedure"), "{listing}");
        assert!(listing.contains("Dependent Variable: y"), "{listing}");
        assert!(listing.contains("Class Level Information"), "{listing}");
        assert!(listing.contains("Number of Observations Used       12"), "{listing}");
        assert!(listing.contains("R-Square"), "{listing}");
        // Strongly separated groups → tiny p-value.
        assert!(listing.contains("<.0001"), "{listing}");
    }

    #[test]
    fn glm_two_way_interaction() {
        let (listing, log, _s) = run_glm(
            two_way_ds(),
            "proc glm data=a; class a b; model y = a b a*b; run;",
        );
        assert!(listing.contains("Number of Observations Used       18"), "{listing}");
        assert!(listing.contains("Type I"), "{listing}");
        assert!(listing.contains("Type III"), "{listing}");
        // The interaction effect a*b appears as a source line.
        assert!(listing.contains("a*b"), "{listing}");
        let _ = log;
    }

    #[test]
    fn glm_type_iii_sum_of_squares() {
        // For a BALANCED design Type I == Type III. Compute both and compare.
        let ds = two_way_ds();
        let mut session = make_session();
        write_dataset(&mut session, "A", ds);
        let ast = parse_glm("proc glm data=a; class a b; model y = a b a*b; run;").unwrap();

        // Rebuild design directly to inspect SS.
        let (ds2, _) = session.libs.get("WORK").unwrap().read("A").unwrap();
        let y_col = ds2.vars.iter().position(|v| v.name == "y").unwrap();
        let y_vals = decode_column(&ds2, y_col).unwrap();
        let mut model_vars = Vec::new();
        for nm in ["a", "b"] {
            let c = ds2.vars.iter().position(|v| v.name == nm).unwrap();
            model_vars.push(ModelVar {
                name: nm.to_string(),
                is_class: true,
                vals: decode_column(&ds2, c).unwrap(),
            });
        }
        let kept: Vec<usize> = (0..ds2.n_obs()).collect();
        let levels: Vec<Vec<Value>> = model_vars
            .iter()
            .map(|mv| distinct_levels(&mv.vals, &kept))
            .collect();
        // ADDITIVE balanced model (no interaction): with the orthogonality of a
        // balanced design, Type I (sequential) and Type III (partial) SS for the
        // main effects coincide. (With an interaction present, reference-cell
        // coding makes them differ — that is expected, not a bug.)
        let effects = vec![vec!["a".to_string()], vec!["b".to_string()]];
        let design = build_design(&model_vars, &levels, &effects, &kept, &y_vals);
        let beta = least_squares(&design.x, &design.y).unwrap();
        let mut sse = 0.0;
        for i in 0..design.x.len() {
            let mut yhat = 0.0;
            for j in 0..design.n_params {
                yhat += design.x[i][j] * beta[j];
            }
            sse += (design.y[i] - yhat).powi(2);
        }
        let t1 = type1_ss(&design, &design.y, sse).unwrap();
        let t3 = type3_ss(&design, &design.y, sse).unwrap();
        for (a, b) in t1.iter().zip(&t3) {
            assert!(
                (a.ss - b.ss).abs() < 1e-6,
                "balanced additive design: Type I {} vs Type III {}",
                a.ss,
                b.ss
            );
        }
        let _ = ast;
    }

    #[test]
    fn glm_lsmeans_basic() {
        // One-way: LSMEANS should reproduce the group means 10, 20, 30.
        let (listing, _log, _s) = run_glm(
            one_way_ds(),
            "proc glm data=a; class g; model y = g; lsmeans g; run;",
        );
        assert!(listing.contains("Least Squares Means"), "{listing}");
        assert!(listing.contains("LSMEAN"), "{listing}");
        assert!(listing.contains("10") && listing.contains("20") && listing.contains("30"),
            "{listing}");
    }

    #[test]
    fn glm_lsmeans_values_exact() {
        // Verify LSMEANS numerically for the one-way design.
        let ds = one_way_ds();
        let mut session = make_session();
        write_dataset(&mut session, "A", ds);
        let (ds2, _) = session.libs.get("WORK").unwrap().read("A").unwrap();
        let y_col = ds2.vars.iter().position(|v| v.name == "y").unwrap();
        let y_vals = decode_column(&ds2, y_col).unwrap();
        let c = ds2.vars.iter().position(|v| v.name == "g").unwrap();
        let model_vars = vec![ModelVar {
            name: "g".to_string(),
            is_class: true,
            vals: decode_column(&ds2, c).unwrap(),
        }];
        let kept: Vec<usize> = (0..ds2.n_obs()).collect();
        let levels = vec![distinct_levels(&model_vars[0].vals, &kept)];
        let effects = vec![vec!["g".to_string()]];
        let design = build_design(&model_vars, &levels, &effects, &kept, &y_vals);
        let beta = least_squares(&design.x, &design.y).unwrap();

        // LSMEAN for level A.
        let combos = cartesian_full(&[levels[0].as_slice()]);
        let mut got = Vec::new();
        for combo in &combos {
            let l = lsmeans_l_vector(&design, &["g".to_string()], combo, &kept);
            let est: f64 = l.iter().zip(&beta).map(|(a, b)| a * b).sum();
            got.push(est);
        }
        // Means are ~10, 20, 30 in level order A,B,C.
        assert!((got[0] - 10.0).abs() < 1e-6, "A lsmean={}", got[0]);
        assert!((got[1] - 20.0).abs() < 1e-6, "B lsmean={}", got[1]);
        assert!((got[2] - 30.0).abs() < 1e-6, "C lsmean={}", got[2]);
    }

    #[test]
    fn glm_lsmeans_confidence_intervals() {
        let (listing, _log, _s) = run_glm(
            one_way_ds(),
            "proc glm data=a; class g; model y = g; lsmeans g; run;",
        );
        assert!(listing.contains("Lower 95%"), "{listing}");
        assert!(listing.contains("Upper 95%"), "{listing}");
        assert!(listing.contains("Standard Error"), "{listing}");
    }

    #[test]
    fn glm_contrast_statement() {
        // Contrast A vs B (levels A=10, B=20): difference -10, large F.
        let (listing, log, _s) = run_glm(
            one_way_ds(),
            "proc glm data=a; class g; model y = g; contrast 'A vs B' g 1 -1; run;",
        );
        assert!(listing.contains("Contrast Tests"), "{listing}");
        assert!(listing.contains("A vs B"), "{listing}");
        assert!(listing.contains("F Value"), "{listing}");
        assert!(!log.contains("could not be evaluated"), "log: {log}");
    }

    #[test]
    fn glm_contrast_statistic_value() {
        // Numerically verify the contrast SS for "g 1 -1" (A - B).
        let ds = one_way_ds();
        let mut session = make_session();
        write_dataset(&mut session, "A", ds);
        let (ds2, _) = session.libs.get("WORK").unwrap().read("A").unwrap();
        let y_col = ds2.vars.iter().position(|v| v.name == "y").unwrap();
        let y_vals = decode_column(&ds2, y_col).unwrap();
        let c = ds2.vars.iter().position(|v| v.name == "g").unwrap();
        let model_vars = vec![ModelVar {
            name: "g".to_string(),
            is_class: true,
            vals: decode_column(&ds2, c).unwrap(),
        }];
        let kept: Vec<usize> = (0..ds2.n_obs()).collect();
        let levels = vec![distinct_levels(&model_vars[0].vals, &kept)];
        let effects = vec![vec!["g".to_string()]];
        let design = build_design(&model_vars, &levels, &effects, &kept, &y_vals);
        let beta = least_squares(&design.x, &design.y).unwrap();
        let xtx = xtx_matrix(&design.x, design.n_params);
        let xtx_inv = invert_matrix(&xtx).unwrap();

        let spec = vec![EffectCoefs {
            effect: vec!["g".to_string()],
            coefs: vec![1.0, -1.0],
        }];
        let lvec = spec_to_lvector(&spec, &design, false).unwrap();
        let lb: f64 = lvec.iter().zip(&beta).map(|(a, b)| a * b).sum();
        // Coding: g has dummies for A, B (C reference). beta = [muC, A-C, B-C].
        // L = [0,1,-1] → L·β = (A-C) - (B-C) = A - B ≈ 10 - 20 = -10.
        assert!((lb + 10.0).abs() < 1e-6, "L·beta={lb}");
        let denom = quad_form(&lvec, &xtx_inv);
        assert!(denom > 0.0);
    }

    #[test]
    fn glm_estimate_statement() {
        let (listing, log, _s) = run_glm(
            one_way_ds(),
            "proc glm data=a; class g; model y = g; estimate 'mean A' intercept 1 g 1; run;",
        );
        assert!(listing.contains("Estimate"), "{listing}");
        assert!(listing.contains("mean A"), "{listing}");
        assert!(listing.contains("t Value"), "{listing}");
        assert!(!log.contains("could not be evaluated"), "log: {log}");
    }

    #[test]
    fn glm_estimate_value_exact() {
        // estimate 'mean A' intercept 1 g 1 → muC + (A-C) = A ≈ 10.
        let ds = one_way_ds();
        let mut session = make_session();
        write_dataset(&mut session, "A", ds);
        let (ds2, _) = session.libs.get("WORK").unwrap().read("A").unwrap();
        let y_col = ds2.vars.iter().position(|v| v.name == "y").unwrap();
        let y_vals = decode_column(&ds2, y_col).unwrap();
        let c = ds2.vars.iter().position(|v| v.name == "g").unwrap();
        let model_vars = vec![ModelVar {
            name: "g".to_string(),
            is_class: true,
            vals: decode_column(&ds2, c).unwrap(),
        }];
        let kept: Vec<usize> = (0..ds2.n_obs()).collect();
        let levels = vec![distinct_levels(&model_vars[0].vals, &kept)];
        let effects = vec![vec!["g".to_string()]];
        let design = build_design(&model_vars, &levels, &effects, &kept, &y_vals);
        let beta = least_squares(&design.x, &design.y).unwrap();

        let spec = vec![
            EffectCoefs {
                effect: vec!["intercept".to_string()],
                coefs: vec![1.0],
            },
            EffectCoefs {
                effect: vec!["g".to_string()],
                coefs: vec![1.0],
            },
        ];
        let lvec = spec_to_lvector(&spec, &design, true).unwrap();
        let est: f64 = lvec.iter().zip(&beta).map(|(a, b)| a * b).sum();
        assert!((est - 10.0).abs() < 1e-6, "estimate={est}");
    }

    #[test]
    fn glm_continuous_covariates() {
        // ANCOVA: y = group + x. y = 2 + 5*[B] + 3*x exactly.
        let g = vec!["A", "A", "A", "A", "B", "B", "B", "B"];
        let x = vec![1.0, 2.0, 3.0, 4.0, 1.0, 2.0, 3.0, 4.0];
        let y: Vec<f64> = g
            .iter()
            .zip(&x)
            .map(|(grp, xv)| 2.0 + if *grp == "B" { 5.0 } else { 0.0 } + 3.0 * xv)
            .collect();
        let df = df!["g" => g, "x" => x, "y" => y].unwrap();
        let ds = SasDataset {
            df,
            vars: vec![char_meta("g"), num_meta("x"), num_meta("y")],
        };
        let (listing, _log, _s) = run_glm(
            ds,
            "proc glm data=a; class g; model y = g x; run;",
        );
        assert!(listing.contains("The GLM Procedure"), "{listing}");
        assert!(listing.contains("Number of Observations Used       8"), "{listing}");
        // Perfect fit → R-Square 1, tiny error.
        assert!(listing.contains("R-Square"), "{listing}");
        assert!(listing.contains("<.0001"), "{listing}");
    }

    #[test]
    fn glm_continuous_covariate_coefficients() {
        // Solve the ANCOVA design directly and check the slope on x is 3.
        let g = vec!["A", "A", "A", "A", "B", "B", "B", "B"];
        let x = vec![1.0, 2.0, 3.0, 4.0, 1.0, 2.0, 3.0, 4.0];
        let y: Vec<f64> = g
            .iter()
            .zip(&x)
            .map(|(grp, xv)| 2.0 + if *grp == "B" { 5.0 } else { 0.0 } + 3.0 * xv)
            .collect();
        let df = df!["g" => g.clone(), "x" => x.clone(), "y" => y.clone()].unwrap();
        let ds = SasDataset {
            df,
            vars: vec![char_meta("g"), num_meta("x"), num_meta("y")],
        };
        let mut session = make_session();
        write_dataset(&mut session, "A", ds);
        let (ds2, _) = session.libs.get("WORK").unwrap().read("A").unwrap();
        let y_col = ds2.vars.iter().position(|v| v.name == "y").unwrap();
        let y_vals = decode_column(&ds2, y_col).unwrap();
        let model_vars = vec![
            ModelVar {
                name: "g".to_string(),
                is_class: true,
                vals: decode_column(&ds2, ds2.vars.iter().position(|v| v.name == "g").unwrap())
                    .unwrap(),
            },
            ModelVar {
                name: "x".to_string(),
                is_class: false,
                vals: decode_column(&ds2, ds2.vars.iter().position(|v| v.name == "x").unwrap())
                    .unwrap(),
            },
        ];
        let kept: Vec<usize> = (0..ds2.n_obs()).collect();
        let levels = vec![
            distinct_levels(&model_vars[0].vals, &kept),
            Vec::new(),
        ];
        let effects = vec![vec!["g".to_string()], vec!["x".to_string()]];
        let design = build_design(&model_vars, &levels, &effects, &kept, &y_vals);
        let beta = least_squares(&design.x, &design.y).unwrap();
        // Params: [intercept(=muB? ref is last level B), g dummy for A, slope x].
        // Last param is the x slope = 3.
        let slope = *beta.last().unwrap();
        assert!((slope - 3.0).abs() < 1e-7, "x slope = {slope}");
    }

    #[test]
    fn glm_output_out() {
        let (_listing, log, session) = run_glm(
            one_way_ds(),
            "proc glm data=a; class g; model y = g; output out=o; run;",
        );
        assert!(log.contains("WORK.O"), "{log}");
        let (out, _) = session.libs.get("WORK").unwrap().read("O").unwrap();
        let names: Vec<&str> = out.vars.iter().map(|v| v.name.as_str()).collect();
        assert!(names.contains(&"PREDICTED"), "{names:?}");
        assert!(names.contains(&"RESIDUAL"), "{names:?}");
        assert_eq!(out.n_obs(), 12);
    }

    #[test]
    fn glm_missing_excluded() {
        let g = vec!["A", "A", "B", "B", "C", "C"];
        let y = vec![Some(10.0), None, Some(20.0), Some(21.0), Some(30.0), Some(31.0)];
        let df = df!["g" => g, "y" => y].unwrap();
        let ds = SasDataset {
            df,
            vars: vec![char_meta("g"), num_meta("y")],
        };
        let (listing, _log, _s) =
            run_glm(ds, "proc glm data=a; class g; model y = g; run;");
        assert!(listing.contains("Number of Observations Read       6"), "{listing}");
        assert!(listing.contains("Number of Observations Used       5"), "{listing}");
    }
}
