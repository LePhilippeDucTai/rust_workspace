use crate::ast::*;

/// Move the term at `from_idx` from `from_side` to position `to_pos` in `to_side`.
///
/// If same side: reorder only (no algebraic change).
/// If different sides and additive: term's sign is flipped.
/// If different sides and multiplicative: the entire other side gets divided by that factor.
pub fn move_term(
    eq: &Equation,
    from_side: Side,
    from_idx: usize,
    to_side: Side,
    to_pos: usize,
) -> Result<Equation, String> {
    let source = eq.side(from_side);

    // Un côté peut contenir plusieurs termes (Add/Mul) ou un terme unique
    // (ex: `Num(7)`, `Var("y")`, `Pow(x, 2)`). Dans ce dernier cas, on le
    // considère comme un unique terme additif d'index 0.
    let (group, terms): (TermGroup, Vec<Expr>) = match draggable_terms(source) {
        Some((g, t)) => (g, t.clone()),
        None => (TermGroup::Additive, vec![source.clone()]),
    };

    if from_idx >= terms.len() {
        return Err(format!("Term index {} out of range ({})", from_idx, terms.len()));
    }

    if from_side == to_side {
        return reorder(eq, from_side, group, from_idx, to_pos);
    }

    let term = terms[from_idx].clone();

    match group {
        TermGroup::Additive => move_additive(eq, from_side, from_idx, to_side, to_pos, term),
        TermGroup::Multiplicative => move_multiplicative(eq, from_side, from_idx, to_side, to_pos, term),
    }
}

fn reorder(
    eq: &Equation,
    side: Side,
    group: TermGroup,
    from_idx: usize,
    to_pos: usize,
) -> Result<Equation, String> {
    let terms = match eq.side(side) {
        Expr::Add(t) => t.clone(),
        Expr::Mul(t) => t.clone(),
        // Côté à un seul terme: « réordonner » est un no-op, mais il ne
        // faut pas le traiter comme une erreur (sinon un glisser-déposer
        // sur le même côté générerait un message d'erreur).
        _ => return Ok(eq.clone()),
    };

    let mut new_terms = terms;
    let term = new_terms.remove(from_idx);
    let insert_at = to_pos.min(new_terms.len());
    new_terms.insert(insert_at, term);

    let new_expr = match group {
        TermGroup::Additive => simplify_trivial(Expr::Add(new_terms)),
        TermGroup::Multiplicative => simplify_trivial(Expr::Mul(new_terms)),
    };

    let mut new_eq = eq.clone();
    *new_eq.side_mut(side) = new_expr;
    Ok(new_eq)
}

fn move_additive(
    eq: &Equation,
    from_side: Side,
    from_idx: usize,
    to_side: Side,
    to_pos: usize,
    term: Expr,
) -> Result<Equation, String> {
    let src_terms = match eq.side(from_side) {
        Expr::Add(t) => t.clone(),
        other => vec![other.clone()],
    };

    // Remove term from source, leave zero if empty
    let mut new_src = src_terms;
    new_src.remove(from_idx);
    let new_src_expr = if new_src.is_empty() {
        Expr::Num(0.0)
    } else {
        simplify_trivial(Expr::Add(new_src))
    };

    // Negate the term for the destination
    let moved = negate(term);

    // Insert into destination
    let dst_terms = match eq.side(to_side) {
        Expr::Add(t) => t.clone(),
        Expr::Num(n) if *n == 0.0 => vec![],
        other => vec![other.clone()],
    };
    let mut new_dst = dst_terms;
    let insert_at = to_pos.min(new_dst.len());
    new_dst.insert(insert_at, moved);
    let new_dst_expr = simplify_trivial(Expr::Add(new_dst));

    let mut new_eq = eq.clone();
    *new_eq.side_mut(from_side) = new_src_expr;
    *new_eq.side_mut(to_side) = new_dst_expr;
    Ok(new_eq)
}

fn move_multiplicative(
    eq: &Equation,
    from_side: Side,
    from_idx: usize,
    to_side: Side,
    to_pos: usize,
    term: Expr,
) -> Result<Equation, String> {
    let src_factors = match eq.side(from_side) {
        Expr::Mul(f) => f.clone(),
        other => vec![other.clone()],
    };

    let mut new_src = src_factors;
    new_src.remove(from_idx);
    let new_src_expr = if new_src.is_empty() {
        Expr::Num(1.0)
    } else {
        simplify_trivial(Expr::Mul(new_src))
    };

    // The destination side gets divided by the term
    let dst_expr = eq.side(to_side).clone();
    let new_dst_expr = divide_by(dst_expr, term, to_pos);

    let mut new_eq = eq.clone();
    *new_eq.side_mut(from_side) = new_src_expr;
    *new_eq.side_mut(to_side) = new_dst_expr;
    Ok(new_eq)
}

/// Wrap `expr` in a division by `divisor`, simplifying nested fractions.
/// `_position` is unused for now (division always wraps the whole expression).
fn divide_by(expr: Expr, divisor: Expr, _position: usize) -> Expr {
    match expr {
        // (a/b) / d  =>  a / (b*d)
        Expr::Div(num, den) => {
            let new_den = match *den {
                Expr::Mul(mut factors) => {
                    factors.push(divisor);
                    Expr::Mul(factors)
                }
                other => Expr::Mul(vec![other, divisor]),
            };
            Expr::Div(num, Box::new(simplify_trivial(new_den)))
        }
        // a / d
        other => Expr::Div(Box::new(other), Box::new(divisor)),
    }
}
