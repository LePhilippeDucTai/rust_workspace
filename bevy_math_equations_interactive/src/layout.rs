use crate::ast::*;
use bevy::prelude::Vec2;

pub const FONT_SIZE: f32 = 42.0;
pub const SMALL_FONT: f32 = 28.0;  // superscripts, root index
pub const TINY_FONT: f32 = 20.0;
pub const EQ_SIGN_GAP: f32 = 28.0;
pub const TERM_GAP: f32 = 12.0;
pub const OP_GAP: f32 = 8.0;
pub const FRAC_BAR_PADDING: f32 = 6.0; // vertical space around fraction bar

/// Approximate character width for a given font size
pub fn char_width(ch: char, size: f32) -> f32 {
    let ratio = match ch {
        'i' | 'l' | '1' | '.' | ',' | ':' | ';' | '!' | '|' | '\'' => 0.28,
        'f' | 'j' | 'r' | 't' => 0.38,
        'm' | 'w' | 'M' | 'W' => 0.72,
        '√' => 0.6,
        _ => 0.52,
    };
    size * ratio
}

pub fn text_width(text: &str, size: f32) -> f32 {
    text.chars().map(|c| char_width(c, size)).sum::<f32>()
}

// ─── Layout node ────────────────────────────────────────────────────────────

#[derive(Clone, Debug)]
pub struct LayoutNode {
    /// Width of the bounding box
    pub width: f32,
    /// Height above the baseline
    pub height: f32,
    /// Depth below the baseline
    pub depth: f32,
    pub kind: NodeKind,
    /// Set during placement to absolute world position of the baseline-left corner
    pub origin: Vec2,
    /// Which draggable term this node belongs to (side, index)
    pub term_ref: Option<(Side, usize)>,
}

#[derive(Clone, Debug)]
pub enum NodeKind {
    /// Plain text glyph(s)
    Text {
        content: String,
        font_size: f32,
    },
    /// Horizontal list of nodes, aligned on baseline
    HList {
        children: Vec<LayoutNode>,
    },
    /// Fraction: numerator over denominator
    Fraction {
        numerator: Box<LayoutNode>,
        denominator: Box<LayoutNode>,
        bar_thickness: f32,
    },
    /// Square root or n-th root
    Radical {
        index: Option<Box<LayoutNode>>,  // None for sqrt
        radicand: Box<LayoutNode>,
    },
    /// Base with optional superscript / subscript
    Script {
        base: Box<LayoutNode>,
        sup: Option<Box<LayoutNode>>,
    },
    /// Parenthesised group with scaling brackets
    Parenthesised {
        inner: Box<LayoutNode>,
    },
    /// Vertical drop-insertion indicator (rendered as a thin line)
    DropIndicator,
}

impl LayoutNode {
    pub fn total_height(&self) -> f32 {
        self.height + self.depth
    }

    /// Bounding rect in world space (after placement)
    pub fn world_rect(&self) -> (Vec2, Vec2) {
        let min = Vec2::new(self.origin.x, self.origin.y - self.depth);
        let max = Vec2::new(self.origin.x + self.width, self.origin.y + self.height);
        (min, max)
    }

    /// Set origin recursively (HList distributes to children)
    pub fn place(&mut self, origin: Vec2) {
        self.origin = origin;
        match &mut self.kind {
            NodeKind::HList { children } => {
                let mut x = origin.x;
                for child in children.iter_mut() {
                    child.place(Vec2::new(x, origin.y));
                    x += child.width;
                }
            }
            NodeKind::Fraction { numerator, denominator, bar_thickness } => {
                let bar_y = origin.y; // fraction bar sits on the math axis
                let bar_half = *bar_thickness * 0.5;
                let num_y = bar_y + bar_half + FRAC_BAR_PADDING + numerator.depth;
                let den_y = bar_y - bar_half - FRAC_BAR_PADDING - denominator.height;
                let cx = origin.x + self.width * 0.5;
                numerator.place(Vec2::new(cx - numerator.width * 0.5, num_y));
                denominator.place(Vec2::new(cx - denominator.width * 0.5, den_y));
            }
            NodeKind::Radical { index, radicand } => {
                let rad_x = origin.x + radical_prefix_width(radicand.total_height());
                radicand.place(Vec2::new(rad_x, origin.y));
                if let Some(idx) = index {
                    // Place index at top-left of the radical sign, slightly raised
                    idx.place(Vec2::new(origin.x, origin.y + radicand.height - idx.depth * 0.5));
                }
            }
            NodeKind::Script { base, sup } => {
                base.place(origin);
                if let Some(s) = sup {
                    s.place(Vec2::new(
                        origin.x + base.width,
                        origin.y + base.height * 0.6,
                    ));
                }
            }
            NodeKind::Parenthesised { inner } => {
                let paren_w = paren_width(inner.total_height());
                inner.place(Vec2::new(origin.x + paren_w, origin.y));
            }
            NodeKind::Text { .. } | NodeKind::DropIndicator => {}
        }
    }

    /// Collect all world-space text atoms for rendering
    pub fn collect_texts(&self, out: &mut Vec<TextAtom>) {
        match &self.kind {
            NodeKind::Text { content, font_size } => {
                out.push(TextAtom {
                    text: content.clone(),
                    font_size: *font_size,
                    pos: self.origin,
                    term_ref: self.term_ref,
                });
            }
            NodeKind::HList { children } => {
                for c in children { c.collect_texts(out); }
            }
            NodeKind::Fraction { numerator, denominator, .. } => {
                numerator.collect_texts(out);
                denominator.collect_texts(out);
            }
            NodeKind::Radical { index, radicand } => {
                if let Some(idx) = index { idx.collect_texts(out); }
                radicand.collect_texts(out);
            }
            NodeKind::Script { base, sup } => {
                base.collect_texts(out);
                if let Some(s) = sup { s.collect_texts(out); }
            }
            NodeKind::Parenthesised { inner } => {
                inner.collect_texts(out);
            }
            NodeKind::DropIndicator => {}
        }
    }

    /// Collect fraction bars (line segments) for rendering
    pub fn collect_lines(&self, out: &mut Vec<LineAtom>) {
        match &self.kind {
            NodeKind::Fraction { numerator: _, denominator: _, bar_thickness } => {
                let bar_y = self.origin.y;
                out.push(LineAtom {
                    x: self.origin.x + 4.0,
                    y: bar_y,
                    width: self.width - 8.0,
                    thickness: *bar_thickness,
                    term_ref: self.term_ref,
                });
            }
            NodeKind::Radical { radicand, index: _ } => {
                // Overline above radicand
                let rad_x = self.origin.x + radical_prefix_width(radicand.total_height());
                let top_y = self.origin.y + radicand.height + 4.0;
                out.push(LineAtom {
                    x: rad_x,
                    y: top_y,
                    width: radicand.width + 4.0,
                    thickness: 2.0,
                    term_ref: self.term_ref,
                });
            }
            NodeKind::HList { children } => {
                for c in children { c.collect_lines(out); }
            }
            NodeKind::Script { base, sup } => {
                base.collect_lines(out);
                if let Some(s) = sup { s.collect_lines(out); }
            }
            NodeKind::Parenthesised { inner } => {
                inner.collect_lines(out);
            }
            _ => {}
        }
    }

    /// Collect drop-indicator positions
    pub fn collect_drop_indicators(&self, out: &mut Vec<DropIndicatorAtom>) {
        match &self.kind {
            NodeKind::DropIndicator => {
                out.push(DropIndicatorAtom {
                    x: self.origin.x,
                    y: self.origin.y,
                    height: self.total_height(),
                    insert_ref: self.term_ref,
                });
            }
            NodeKind::HList { children } => {
                for c in children { c.collect_drop_indicators(out); }
            }
            _ => {}
        }
    }

    /// Find the term_ref at world position (for click detection)
    pub fn term_at(&self, pt: Vec2) -> Option<(Side, usize)> {
        let (min, max) = self.world_rect();
        if pt.x < min.x || pt.x > max.x || pt.y < min.y || pt.y > max.y {
            return None;
        }
        if let Some(tr) = self.term_ref {
            return Some(tr);
        }
        match &self.kind {
            NodeKind::HList { children } => {
                for c in children {
                    if let Some(tr) = c.term_at(pt) { return Some(tr); }
                }
                None
            }
            NodeKind::Fraction { numerator, denominator, .. } => {
                numerator.term_at(pt).or_else(|| denominator.term_at(pt))
            }
            NodeKind::Radical { radicand, index } => {
                radicand.term_at(pt).or_else(|| index.as_ref().and_then(|i| i.term_at(pt)))
            }
            NodeKind::Script { base, sup } => {
                base.term_at(pt).or_else(|| sup.as_ref().and_then(|s| s.term_at(pt)))
            }
            NodeKind::Parenthesised { inner } => inner.term_at(pt),
            _ => None,
        }
    }
}

// ─── Helper atoms collected for rendering ──────────────────────────────────

#[derive(Clone, Debug)]
pub struct TextAtom {
    pub text: String,
    pub font_size: f32,
    pub pos: Vec2,
    pub term_ref: Option<(Side, usize)>,
}

#[derive(Clone, Debug)]
pub struct LineAtom {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub thickness: f32,
    pub term_ref: Option<(Side, usize)>,
}

#[derive(Clone, Debug)]
pub struct DropIndicatorAtom {
    pub x: f32,
    pub y: f32,
    pub height: f32,
    pub insert_ref: Option<(Side, usize)>, // (side, insert_before_idx)
}

// ─── Layout computation ─────────────────────────────────────────────────────

fn radical_prefix_width(content_height: f32) -> f32 {
    // √ symbol width scales slightly with content height
    content_height * 0.7 + 8.0
}

fn paren_width(content_height: f32) -> f32 {
    (content_height * 0.18 + 6.0).max(10.0)
}

fn make_text(content: impl Into<String>, font_size: f32) -> LayoutNode {
    let content = content.into();
    let w = text_width(&content, font_size);
    let h = font_size * 0.72;
    let d = font_size * 0.22;
    LayoutNode {
        width: w,
        height: h,
        depth: d,
        kind: NodeKind::Text { content, font_size },
        origin: Vec2::ZERO,
        term_ref: None,
    }
}

fn hlist(children: Vec<LayoutNode>) -> LayoutNode {
    let width: f32 = children.iter().map(|c| c.width).sum();
    let height = children.iter().map(|c| c.height).fold(0.0_f32, f32::max);
    let depth = children.iter().map(|c| c.depth).fold(0.0_f32, f32::max);
    LayoutNode {
        width,
        height,
        depth,
        kind: NodeKind::HList { children },
        origin: Vec2::ZERO,
        term_ref: None,
    }
}

fn make_fraction(num: LayoutNode, den: LayoutNode) -> LayoutNode {
    let bar_thickness = 2.0;
    let w = num.width.max(den.width) + 16.0;
    // Math axis offset: center of bar is at y=0, height above = bar + padding + num_total
    let height = bar_thickness * 0.5 + FRAC_BAR_PADDING + num.height + num.depth;
    let depth = bar_thickness * 0.5 + FRAC_BAR_PADDING + den.height + den.depth;
    LayoutNode {
        width: w,
        height,
        depth,
        kind: NodeKind::Fraction {
            numerator: Box::new(num),
            denominator: Box::new(den),
            bar_thickness,
        },
        origin: Vec2::ZERO,
        term_ref: None,
    }
}

fn make_radical(index: Option<LayoutNode>, radicand: LayoutNode) -> LayoutNode {
    let rh = radicand.total_height();
    let prefix_w = radical_prefix_width(rh);
    let overline_extra = 6.0;
    let width = prefix_w + radicand.width + overline_extra;
    let height = radicand.height + 8.0; // space for overline
    let depth = radicand.depth;
    LayoutNode {
        width,
        height,
        depth,
        kind: NodeKind::Radical {
            index: index.map(Box::new),
            radicand: Box::new(radicand),
        },
        origin: Vec2::ZERO,
        term_ref: None,
    }
}

fn make_script(base: LayoutNode, sup: LayoutNode) -> LayoutNode {
    let width = base.width + sup.width;
    let height = (base.height + sup.height * 0.7).max(base.height + sup.height * 0.6);
    let depth = base.depth;
    LayoutNode {
        width,
        height,
        depth,
        kind: NodeKind::Script { base: Box::new(base), sup: Some(Box::new(sup)) },
        origin: Vec2::ZERO,
        term_ref: None,
    }
}

fn make_paren(inner: LayoutNode) -> LayoutNode {
    let h = inner.total_height();
    let pw = paren_width(h);
    let width = pw + inner.width + pw;
    LayoutNode {
        width,
        height: inner.height + 4.0,
        depth: inner.depth + 4.0,
        kind: NodeKind::Parenthesised { inner: Box::new(inner) },
        origin: Vec2::ZERO,
        term_ref: None,
    }
}

/// Whether an expression needs parentheses when used as a child of Mul/Div/Pow
fn needs_parens_in_product(expr: &Expr) -> bool {
    matches!(expr, Expr::Add(_) | Expr::Neg(_))
}

fn needs_parens_in_power(expr: &Expr) -> bool {
    matches!(expr, Expr::Add(_) | Expr::Mul(_) | Expr::Neg(_) | Expr::Div(_, _))
}

pub fn layout_expr(expr: &Expr, size: f32) -> LayoutNode {
    match expr {
        Expr::Num(n) => {
            let s = if n.fract() == 0.0 && n.abs() < 1e10 {
                format!("{}", *n as i64)
            } else {
                format!("{}", n)
            };
            make_text(s, size)
        }
        Expr::Var(name) => make_text(name.clone(), size),

        Expr::Neg(inner) => {
            let minus = make_text("−", size);
            let inner_node = layout_expr(inner, size);
            let need_p = matches!(inner.as_ref(), Expr::Add(_));
            let inner_node = if need_p { make_paren(inner_node) } else { inner_node };
            hlist(vec![minus, inner_node])
        }

        Expr::Add(terms) => {
            let mut children: Vec<LayoutNode> = Vec::new();
            for (i, term) in terms.iter().enumerate() {
                if i > 0 {
                    let is_neg = matches!(term, Expr::Neg(_));
                    if !is_neg {
                        children.push(make_text(" + ", size));
                    } else {
                        // Neg already renders with − sign; add space
                        children.push(make_text(" ", size));
                    }
                }
                children.push(layout_expr(term, size));
            }
            hlist(children)
        }

        Expr::Mul(factors) => {
            let mut children: Vec<LayoutNode> = Vec::new();
            for (i, factor) in factors.iter().enumerate() {
                if i > 0 {
                    children.push(make_text(" · ", size));
                }
                let node = layout_expr(factor, size);
                if needs_parens_in_product(factor) {
                    children.push(make_paren(node));
                } else {
                    children.push(node);
                }
            }
            hlist(children)
        }

        Expr::Div(num, den) => {
            let n = layout_expr(num, size * 0.82);
            let d = layout_expr(den, size * 0.82);
            make_fraction(n, d)
        }

        Expr::Pow(base, exp) => {
            let b = layout_expr(base, size);
            let e = layout_expr(exp, SMALL_FONT.min(size * 0.65));
            let b = if needs_parens_in_power(base) { make_paren(b) } else { b };
            make_script(b, e)
        }

        Expr::Sqrt(inner) => {
            let inner_node = layout_expr(inner, size * 0.9);
            make_radical(None, inner_node)
        }

        Expr::Root(n, inner) => {
            let n_node = layout_expr(n, TINY_FONT.min(size * 0.48));
            let inner_node = layout_expr(inner, size * 0.9);
            make_radical(Some(n_node), inner_node)
        }
    }
}

// ─── Full equation layout ───────────────────────────────────────────────────

pub struct EquationLayout {
    pub lhs: LayoutNode,
    pub rhs: LayoutNode,
    pub eq_sign: LayoutNode,
    /// World-space baseline y
    pub baseline_y: f32,
    /// Bounding box: (min, max)
    pub bbox: (Vec2, Vec2),
    /// Drop indicator atoms
    pub drop_indicators: Vec<DropIndicatorAtom>,
}

pub fn layout_equation(eq: &Equation) -> EquationLayout {
    let lhs_raw = layout_side(eq, Side::Lhs);
    let rhs_raw = layout_side(eq, Side::Rhs);
    let eq_sign = make_text(" = ", FONT_SIZE);

    let total_w = lhs_raw.width + eq_sign.width + rhs_raw.width;
    let max_h = lhs_raw.height.max(rhs_raw.height).max(eq_sign.height);
    let max_d = lhs_raw.depth.max(rhs_raw.depth).max(eq_sign.depth);

    let start_x = -total_w * 0.5;
    let baseline_y = 0.0;

    let mut lhs = lhs_raw;
    let mut eq_s = eq_sign;
    let mut rhs = rhs_raw;

    lhs.place(Vec2::new(start_x, baseline_y));
    eq_s.place(Vec2::new(start_x + lhs.width, baseline_y));
    rhs.place(Vec2::new(start_x + lhs.width + eq_s.width, baseline_y));

    let mut drop_indicators = Vec::new();
    lhs.collect_drop_indicators(&mut drop_indicators);
    rhs.collect_drop_indicators(&mut drop_indicators);

    let pad = 20.0;
    let bbox = (
        Vec2::new(start_x - pad, baseline_y - max_d - pad),
        Vec2::new(start_x + total_w + pad, baseline_y + max_h + pad),
    );

    EquationLayout { lhs, rhs, eq_sign: eq_s, baseline_y, bbox, drop_indicators }
}

/// Layout one side of the equation, inserting drop-indicator nodes between draggable terms
fn layout_side(eq: &Equation, side: Side) -> LayoutNode {
    let expr = eq.side(side);

    match draggable_terms(expr) {
        Some((TermGroup::Additive, terms)) => {
            let mut children: Vec<LayoutNode> = Vec::new();
            // Drop indicator before first term
            children.push(drop_indicator(side, 0));

            for (i, term) in terms.iter().enumerate() {
                if i > 0 {
                    let is_neg = matches!(term, Expr::Neg(_));
                    let op_str = if is_neg { " " } else { " + " };
                    children.push(make_text(op_str, FONT_SIZE));
                }

                let mut node = layout_expr(term, FONT_SIZE);
                node.term_ref = Some((side, i));
                // Pad for hit target
                node = with_horizontal_pad(node, TERM_GAP * 0.5);
                children.push(node);

                // Drop indicator after each term
                children.push(drop_indicator(side, i + 1));
            }
            hlist(children)
        }

        Some((TermGroup::Multiplicative, factors)) => {
            let mut children: Vec<LayoutNode> = Vec::new();
            children.push(drop_indicator(side, 0));

            for (i, factor) in factors.iter().enumerate() {
                if i > 0 {
                    children.push(make_text(" · ", FONT_SIZE));
                }

                let inner = layout_expr(factor, FONT_SIZE);
                let inner = if needs_parens_in_product(factor) { make_paren(inner) } else { inner };
                let mut node = inner;
                node.term_ref = Some((side, i));
                node = with_horizontal_pad(node, TERM_GAP * 0.5);
                children.push(node);

                children.push(drop_indicator(side, i + 1));
            }
            hlist(children)
        }

        None => {
            // Côté à un seul terme (ex: `= 7`, `= y`, `= x^2`).
            // On l'expose quand même comme un unique terme additif draggable
            // et on ajoute des indicateurs de dépôt avant et après, sinon
            // l'utilisateur ne peut ni le glisser, ni déposer un autre terme
            // sur ce côté.
            let mut children: Vec<LayoutNode> = Vec::new();
            children.push(drop_indicator(side, 0));

            let mut node = layout_expr(expr, FONT_SIZE);
            node.term_ref = Some((side, 0));
            node = with_horizontal_pad(node, TERM_GAP * 0.5);
            children.push(node);

            children.push(drop_indicator(side, 1));
            hlist(children)
        }
    }
}

fn drop_indicator(side: Side, insert_idx: usize) -> LayoutNode {
    LayoutNode {
        width: 6.0,
        height: FONT_SIZE * 0.9,
        depth: FONT_SIZE * 0.3,
        kind: NodeKind::DropIndicator,
        origin: Vec2::ZERO,
        term_ref: Some((side, insert_idx)),
    }
}

fn with_horizontal_pad(mut node: LayoutNode, pad: f32) -> LayoutNode {
    // Wrap in an HList with invisible spacers so the bounding box is padded
    let left = LayoutNode {
        width: pad,
        height: node.height,
        depth: node.depth,
        kind: NodeKind::Text { content: String::new(), font_size: FONT_SIZE },
        origin: Vec2::ZERO,
        term_ref: None,
    };
    let right = LayoutNode {
        width: pad,
        height: node.height,
        depth: node.depth,
        kind: NodeKind::Text { content: String::new(), font_size: FONT_SIZE },
        origin: Vec2::ZERO,
        term_ref: None,
    };
    // Carry the term_ref on the wrapper
    let tr = node.term_ref;
    node.term_ref = None;
    let mut wrapper = hlist(vec![left, node, right]);
    wrapper.term_ref = tr;
    wrapper
}
