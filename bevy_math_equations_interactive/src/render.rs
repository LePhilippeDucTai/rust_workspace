use crate::ast::Side;
use crate::layout::*;
use bevy::prelude::*;

pub struct RenderPlugin;

impl Plugin for RenderPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<MathFont>()
            .add_systems(Startup, load_font)
            .add_systems(Update, (render_equation_when_dirty, render_input_field));
    }
}

// ─── Resources ──────────────────────────────────────────────────────────────

#[derive(Resource, Default)]
pub struct MathFont {
    pub handle: Handle<Font>,
    pub loaded: bool,
}

#[derive(Resource, Default)]
pub struct EquationDirty(pub bool);

// ─── Components ─────────────────────────────────────────────────────────────

/// Marks entities that are part of the equation display (not UI)
#[derive(Component)]
pub struct EquationEntity;

/// Marks a draggable term entity
#[derive(Component)]
pub struct DraggableTerm {
    pub side: Side,
    pub index: usize,
}

/// Marks a drop-indicator entity with its insert position
#[derive(Component)]
pub struct DropIndicator {
    pub side: Side,
    pub insert_before: usize,
}

/// Highlight overlay for hover/drag states
#[derive(Component)]
pub struct TermHighlight;

/// Marks the input field background
#[derive(Component)]
pub struct InputFieldBg;

/// Marks the input field text display
#[derive(Component)]
pub struct InputFieldText;

// ─── Startup ────────────────────────────────────────────────────────────────

fn load_font(asset_server: Res<AssetServer>, mut math_font: ResMut<MathFont>) {
    // Try to load a math font from assets/fonts/. Any TTF/OTF works.
    // For best results, place "math.ttf" (e.g. STIX Two Math or FiraSans) in assets/fonts/.
    math_font.handle = asset_server.load("fonts/math.ttf");
}

// ─── Equation rendering ─────────────────────────────────────────────────────

use crate::interaction::{AppState, DragState, EquationRes};

pub fn render_equation_when_dirty(
    mut commands: Commands,
    mut dirty: ResMut<EquationDirty>,
    eq_res: Res<EquationRes>,
    drag_state: Res<DragState>,
    font: Res<MathFont>,
    old_entities: Query<Entity, With<EquationEntity>>,
    app_state: Res<State<AppState>>,
) {
    let in_display = *app_state.get() != AppState::Input;
    if !dirty.0 || !in_display {
        return;
    }
    dirty.0 = false;

    // Despawn old
    for e in old_entities.iter() {
        commands.entity(e).despawn_recursive();
    }

    let Some(ref eq) = eq_res.equation else {
        return;
    };

    let layout = layout_equation(eq);

    // Spawn text atoms
    let mut texts: Vec<TextAtom> = Vec::new();
    layout.lhs.collect_texts(&mut texts);
    layout.eq_sign.collect_texts(&mut texts);
    layout.rhs.collect_texts(&mut texts);

    for atom in &texts {
        let is_draggable = atom.term_ref.is_some();
        let color = if let Some(tr) = atom.term_ref {
            if drag_state.dragging == Some(tr) {
                Color::srgba(1.0, 1.0, 1.0, 0.35)
            } else {
                Color::WHITE
            }
        } else {
            Color::WHITE
        };

        let mut ent = commands.spawn((
            Text2d::new(atom.text.clone()),
            TextFont {
                font: font.handle.clone(),
                font_size: atom.font_size,
                ..default()
            },
            TextColor(color),
            Transform::from_xyz(atom.pos.x, atom.pos.y, 1.0),
            EquationEntity,
        ));
        if is_draggable {
            if let Some((side, index)) = atom.term_ref {
                ent.insert(DraggableTerm { side, index });
            }
        }
    }

    // Spawn fraction bars and radical overlines
    let mut lines: Vec<LineAtom> = Vec::new();
    layout.lhs.collect_lines(&mut lines);
    layout.rhs.collect_lines(&mut lines);

    for line in &lines {
        let color = Color::WHITE;
        commands.spawn((
            Sprite {
                color,
                custom_size: Some(Vec2::new(line.width, line.thickness)),
                ..default()
            },
            Transform::from_xyz(line.x + line.width * 0.5, line.y, 1.0),
            EquationEntity,
        ));
    }

    // Spawn invisible hit areas for draggable terms
    spawn_hit_areas(&mut commands, &layout.lhs, &drag_state);
    spawn_hit_areas(&mut commands, &layout.rhs, &drag_state);

    // Spawn drop indicators (initially invisible, shown during drag)
    for ind in &layout.drop_indicators {
        let visible = drag_state.dragging.is_some();
        if let Some((side, idx)) = ind.insert_ref {
            let color = if visible {
                Color::srgba(0.4, 0.7, 1.0, 0.6)
            } else {
                Color::srgba(0.0, 0.0, 0.0, 0.0)
            };
            commands.spawn((
                Sprite {
                    color,
                    custom_size: Some(Vec2::new(2.0, ind.height)),
                    ..default()
                },
                Transform::from_xyz(ind.x, ind.y - ind.height * 0.5 + ind.height * 0.5, 2.0),
                EquationEntity,
                DropIndicator {
                    side,
                    insert_before: idx,
                },
            ));
        }
    }
}

fn spawn_hit_areas(commands: &mut Commands, node: &LayoutNode, drag_state: &DragState) {
    // Les nœuds DropIndicator portent aussi un term_ref (pour mémoriser leur
    // position d'insertion), mais ils ne doivent PAS donner lieu à une zone
    // cliquable de type « terme draggable » — sinon un clic sur un indicateur
    // de dépôt serait interprété comme un clic sur un terme inexistant.
    if matches!(node.kind, crate::layout::NodeKind::DropIndicator) {
        return;
    }

    if let Some((side, index)) = node.term_ref {
        let (min, max) = node.world_rect();
        let w = max.x - min.x;
        let h = max.y - min.y;
        let cx = (min.x + max.x) * 0.5;
        let cy = (min.y + max.y) * 0.5;

        let is_dragging = drag_state.dragging == Some((side, index));
        let alpha = if is_dragging { 0.18 } else { 0.0 };

        commands.spawn((
            Sprite {
                color: Color::srgba(1.0, 1.0, 1.0, alpha),
                custom_size: Some(Vec2::new(w, h)),
                ..default()
            },
            Transform::from_xyz(cx, cy, 0.5),
            EquationEntity,
            DraggableTerm { side, index },
        ));
        return;
    }

    // Recurse into children
    match &node.kind {
        crate::layout::NodeKind::HList { children } => {
            for c in children {
                spawn_hit_areas(commands, c, drag_state);
            }
        }
        crate::layout::NodeKind::Fraction {
            numerator,
            denominator,
            ..
        } => {
            spawn_hit_areas(commands, numerator, drag_state);
            spawn_hit_areas(commands, denominator, drag_state);
        }
        crate::layout::NodeKind::Radical { radicand, index } => {
            spawn_hit_areas(commands, radicand, drag_state);
            if let Some(i) = index {
                spawn_hit_areas(commands, i, drag_state);
            }
        }
        crate::layout::NodeKind::Script { base, sup } => {
            spawn_hit_areas(commands, base, drag_state);
            if let Some(s) = sup {
                spawn_hit_areas(commands, s, drag_state);
            }
        }
        crate::layout::NodeKind::Parenthesised { inner } => {
            spawn_hit_areas(commands, inner, drag_state);
        }
        _ => {}
    }
}

// ─── Input field ─────────────────────────────────────────────────────────────

pub fn render_input_field(
    mut commands: Commands,
    eq_res: Res<EquationRes>,
    font: Res<MathFont>,
    old_bg: Query<Entity, With<InputFieldBg>>,
    old_txt: Query<Entity, With<InputFieldText>>,
    time: Res<Time>,
    app_state: Res<State<AppState>>,
) {
    if !eq_res.is_changed() && !app_state.is_changed() {
        return;
    }

    for e in old_bg.iter() {
        commands.entity(e).despawn();
    }
    for e in old_txt.iter() {
        commands.entity(e).despawn();
    }

    let in_input = *app_state.get() == AppState::Input;

    // Input box background at the bottom of the screen
    let box_w = 800.0_f32;
    let box_h = 56.0_f32;
    let box_y = -320.0_f32;

    let bg_color = if in_input {
        Color::srgba(0.08, 0.08, 0.12, 0.95)
    } else {
        Color::srgba(0.04, 0.04, 0.06, 0.85)
    };
    let border_color = if in_input {
        Color::srgba(0.7, 0.7, 1.0, 0.9)
    } else {
        Color::srgba(0.3, 0.3, 0.5, 0.6)
    };

    // Background
    commands.spawn((
        Sprite {
            color: bg_color,
            custom_size: Some(Vec2::new(box_w, box_h)),
            ..default()
        },
        Transform::from_xyz(0.0, box_y, 10.0),
        InputFieldBg,
    ));
    // Border (4 thin sprites)
    let bw = 1.5_f32;
    for (dx, dy, w, h) in [
        (0.0, box_h * 0.5, box_w, bw),
        (0.0, -box_h * 0.5, box_w, bw),
        (-box_w * 0.5, 0.0, bw, box_h),
        (box_w * 0.5, 0.0, bw, box_h),
    ] {
        commands.spawn((
            Sprite {
                color: border_color,
                custom_size: Some(Vec2::new(w, h)),
                ..default()
            },
            Transform::from_xyz(dx, box_y + dy, 10.1),
            InputFieldBg,
        ));
    }

    // Label
    let label = if in_input {
        "Équation :"
    } else {
        "Appuyez sur E pour éditer"
    };
    commands.spawn((
        Text2d::new(label),
        TextFont {
            font: font.handle.clone(),
            font_size: 14.0,
            ..default()
        },
        TextColor(Color::srgba(0.6, 0.6, 0.8, 0.8)),
        Transform::from_xyz(-box_w * 0.5 + 10.0, box_y + box_h * 0.5 + 10.0, 10.0),
        InputFieldText,
    ));

    // Content
    let cursor = if in_input { "█" } else { "" };
    let display = format!("{}{}", eq_res.input_text, cursor);
    commands.spawn((
        Text2d::new(display),
        TextFont {
            font: font.handle.clone(),
            font_size: 22.0,
            ..default()
        },
        TextColor(Color::WHITE),
        Transform::from_xyz(-box_w * 0.5 + 20.0, box_y, 10.2),
        InputFieldText,
    ));

    // Help text when in display mode
    if !in_input {
        if let Some(ref eq) = eq_res.equation {
            let _ = eq; // equation is displayed
            commands.spawn((
                Text2d::new("Glissez un terme pour le déplacer de l'autre côté"),
                TextFont {
                    font: font.handle.clone(),
                    font_size: 13.0,
                    ..default()
                },
                TextColor(Color::srgba(0.5, 0.5, 0.7, 0.7)),
                Transform::from_xyz(0.0, box_y - box_h * 0.5 - 16.0, 10.0),
                InputFieldText,
            ));
        }
    }
}
