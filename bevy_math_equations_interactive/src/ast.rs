/// Mathematical expression tree
#[derive(Clone, Debug, PartialEq)]
pub enum Expr {
    Num(f64),
    Var(String),
    Neg(Box<Expr>),
    Add(Vec<Expr>),               // subtraction = Neg child
    Mul(Vec<Expr>),               // all factors in numerator
    Div(Box<Expr>, Box<Expr>),    // fraction: num / den
    Pow(Box<Expr>, Box<Expr>),    // base ^ exponent
    Sqrt(Box<Expr>),              // √x
    Root(Box<Expr>, Box<Expr>),   // ⁿ√x : Root(n, x)
}

#[derive(Clone, Debug)]
pub struct Equation {
    pub lhs: Expr,
    pub rhs: Expr,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Side {
    Lhs,
    Rhs,
}

/// What algebraic operation the draggable terms are part of
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TermGroup {
    Additive,
    Multiplicative,
}

impl Equation {
    pub fn side(&self, side: Side) -> &Expr {
        match side {
            Side::Lhs => &self.lhs,
            Side::Rhs => &self.rhs,
        }
    }

    pub fn side_mut(&mut self, side: Side) -> &mut Expr {
        match side {
            Side::Lhs => &mut self.lhs,
            Side::Rhs => &mut self.rhs,
        }
    }
}

/// Returns the draggable terms of an expression side:
/// (group kind, list of terms)
pub fn draggable_terms(expr: &Expr) -> Option<(TermGroup, &Vec<Expr>)> {
    match expr {
        Expr::Add(terms) if terms.len() > 0 => Some((TermGroup::Additive, terms)),
        Expr::Mul(factors) if factors.len() > 0 => Some((TermGroup::Multiplicative, factors)),
        _ => None,
    }
}

/// Simplify trivial wrappers: Add([x]) → x, Mul([x]) → x
pub fn simplify_trivial(expr: Expr) -> Expr {
    match expr {
        Expr::Add(mut terms) => {
            if terms.len() == 1 {
                simplify_trivial(terms.remove(0))
            } else {
                Expr::Add(terms.into_iter().map(simplify_trivial).collect())
            }
        }
        Expr::Mul(mut factors) => {
            if factors.len() == 1 {
                simplify_trivial(factors.remove(0))
            } else {
                Expr::Mul(factors.into_iter().map(simplify_trivial).collect())
            }
        }
        Expr::Neg(inner) => {
            let inner = simplify_trivial(*inner);
            // double negation
            if let Expr::Neg(x) = inner {
                *x
            } else {
                Expr::Neg(Box::new(inner))
            }
        }
        other => other,
    }
}

/// Negate an expression (for additive term movement across =)
pub fn negate(expr: Expr) -> Expr {
    match expr {
        Expr::Neg(inner) => *inner,
        other => Expr::Neg(Box::new(other)),
    }
}
