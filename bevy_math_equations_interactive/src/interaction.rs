use bevy::prelude::*;
use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::input::ButtonState;
use crate::ast::{Equation, Side};
use crate::parser::parse;
use crate::transform::move_term;
use crate::layout::{layout_equation, EquationLayout};
use crate::render::{EquationDirty, MathFont, DraggableTerm, DropIndicator};
use crate::animation::Tween;

pub struct InteractionPlugin;

impl Plugin for InteractionPlugin {
    fn build(&self, app: &mut App) {
        app
            .init_state::<AppState>()
            .init_resource::<EquationRes>()
            .init_resource::<DragState>()
            .init_resource::<EquationDirty>()
            .add_systems(Update, (
                handle_keyboard,
                handle_mouse.after(handle_keyboard),
                update_drag_ghost.after(handle_mouse),
                update_hover_highlight.after(handle_mouse),
                update_drop_indicators.after(handle_mouse),
            ));
    }
}

// ─── App state ──────────────────────────────────────────────────────────────

#[derive(States, Debug, Clone, PartialEq, Eq, Hash, Default)]
pub enum AppState {
    #[default]
    Input,
    Display,
}

// ─── Resources ──────────────────────────────────────────────────────────────

#[derive(Resource, Default)]
pub struct EquationRes {
    pub equation: Option<Equation>,
    pub input_text: String,
    pub parse_error: Option<String>,
}

#[derive(Resource, Default)]
pub struct DragState {
    /// Which term is being dragged (side, index)
    pub dragging: Option<(Side, usize)>,
    /// Cursor world position
    pub cursor_world: Vec2,
    /// Offset from entity center to cursor at drag start
    pub drag_offset: Vec2,
    /// Nearest drop target found this frame: (side, insert_before)
    pub hover_drop: Option<(Side, usize)>,
}

// ─── Keyboard input ─────────────────────────────────────────────────────────

fn handle_keyboard(
    mut key_events: EventReader<KeyboardInput>,
    mut eq_res: ResMut<EquationRes>,
    mut next_state: ResMut<NextState<AppState>>,
    mut dirty: ResMut<EquationDirty>,
    app_state: Res<State<AppState>>,
) {
    for ev in key_events.read() {
        if ev.state != ButtonState::Pressed { continue; }

        match *app_state.get() {
            AppState::Input => match &ev.logical_key {
                Key::Enter => {
                    let text = eq_res.input_text.trim().to_string();
                    if text.is_empty() { continue; }
                    match parse(&text) {
                        Ok(eq) => {
                            eq_res.equation = Some(eq);
                            eq_res.parse_error = None;
                            dirty.0 = true;
                            next_state.set(AppState::Display);
                        }
                        Err(e) => {
                            eq_res.parse_error = Some(e);
                        }
                    }
                }
                Key::Backspace => { eq_res.input_text.pop(); }
                Key::Space => { eq_res.input_text.push(' '); }
                Key::Character(c) => {
                    for ch in c.chars() {
                        if !ch.is_control() {
                            eq_res.input_text.push(ch);
                        }
                    }
                }
                Key::Escape => {
                    eq_res.input_text.clear();
                    eq_res.parse_error = None;
                }
                _ => {}
            },

            AppState::Display => match &ev.logical_key {
                Key::Character(c) if c.as_str() == "e" || c.as_str() == "E" => {
                    next_state.set(AppState::Input);
                }
                _ => {}
            },
        }
    }
}

// ─── Mouse drag & drop ───────────────────────────────────────────────────────

#[derive(Component)]
pub struct DragGhost;

fn handle_mouse(
    mut commands: Commands,
    mut drag_state: ResMut<DragState>,
    mut eq_res: ResMut<EquationRes>,
    mut dirty: ResMut<EquationDirty>,
    mouse: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window>,
    camera_q: Query<(&Camera, &GlobalTransform), With<Camera2d>>,
    draggable_q: Query<(&DraggableTerm, &Sprite, &Transform)>,
    drop_q: Query<(&DropIndicator, &Transform)>,
    ghost_q: Query<Entity, With<DragGhost>>,
    app_state: Res<State<AppState>>,
    font: Res<MathFont>,
) {
    if *app_state.get() != AppState::Display { return; }

    let Ok(window) = windows.get_single() else { return };
    let Ok((camera, cam_transform)) = camera_q.get_single() else { return };

    if let Some(cursor_px) = window.cursor_position() {
        if let Ok(world) = camera.viewport_to_world_2d(cam_transform, cursor_px) {
            drag_state.cursor_world = world;
        }
    }

    let cursor = drag_state.cursor_world;

    // ── Drag start ─────────────────────────────────────────────────────────
    if mouse.just_pressed(MouseButton::Left) && drag_state.dragging.is_none() {
        // Find which draggable term was clicked
        for (dt, sprite, transform) in draggable_q.iter() {
            let size = sprite.custom_size.unwrap_or(Vec2::ONE);
            let center = transform.translation.truncate();
            let half = size * 0.5;
            if cursor.x >= center.x - half.x && cursor.x <= center.x + half.x
                && cursor.y >= center.y - half.y && cursor.y <= center.y + half.y
            {
                drag_state.dragging = Some((dt.side, dt.index));
                drag_state.drag_offset = center - cursor;
                // Mark dirty to update highlights
                dirty.0 = true;
                break;
            }
        }
    }

    // ── Drag move: update hover_drop ────────────────────────────────────────
    if drag_state.dragging.is_some() {
        let mut best: Option<(Side, usize)> = None;
        let mut best_dist = f32::MAX;
        for (di, dt) in drop_q.iter() {
            let pos = dt.translation.truncate();
            let dist = (pos.x - cursor.x).abs();
            if dist < best_dist {
                best_dist = dist;
                best = Some((di.side, di.insert_before));
            }
        }
        drag_state.hover_drop = best;
    }

    // ── Drag end (release) ─────────────────────────────────────────────────
    if mouse.just_released(MouseButton::Left) {
        if let Some((from_side, from_idx)) = drag_state.dragging.take() {
            if let Some((to_side, to_pos)) = drag_state.hover_drop.take() {
                if let Some(ref eq) = eq_res.equation.clone() {
                    match move_term(eq, from_side, from_idx, to_side, to_pos) {
                        Ok(new_eq) => {
                            eq_res.equation = Some(new_eq);
                            dirty.0 = true;
                        }
                        Err(e) => {
                            eq_res.parse_error = Some(e);
                        }
                    }
                }
            }
            drag_state.dragging = None;
            drag_state.hover_drop = None;
            dirty.0 = true;
        }
        // Remove ghost
        for e in ghost_q.iter() { commands.entity(e).despawn(); }
    }
}

fn update_drag_ghost(
    mut commands: Commands,
    drag_state: Res<DragState>,
    eq_res: Res<EquationRes>,
    font: Res<MathFont>,
    ghost_q: Query<Entity, With<DragGhost>>,
) {
    if drag_state.dragging.is_none() {
        for e in ghost_q.iter() { commands.entity(e).despawn(); }
        return;
    }
    let (side, idx) = drag_state.dragging.unwrap();
    let Some(ref eq) = eq_res.equation else { return };

    // Despawn old ghost
    for e in ghost_q.iter() { commands.entity(e).despawn(); }

    let cursor = drag_state.cursor_world + drag_state.drag_offset;

    // Build a tiny layout for just this term
    use crate::ast::draggable_terms;
    use crate::layout::{layout_expr, FONT_SIZE};

    let expr = eq.side(side);
    if let Some((_, terms)) = draggable_terms(expr) {
        if let Some(term) = terms.get(idx) {
            let mut node = layout_expr(term, FONT_SIZE * 0.95);
            // center on cursor
            let cx = cursor.x - node.width * 0.5;
            let cy = cursor.y;
            node.place(bevy::prelude::Vec2::new(cx, cy));

            let mut texts = Vec::new();
            node.collect_texts(&mut texts);

            for atom in texts {
                commands.spawn((
                    Text2d::new(atom.text),
                    TextFont { font: font.handle.clone(), font_size: atom.font_size, ..default() },
                    TextColor(Color::srgba(0.6, 0.85, 1.0, 0.92)),
                    Transform::from_xyz(atom.pos.x, atom.pos.y, 20.0),
                    DragGhost,
                ));
            }

            let mut lines = Vec::new();
            node.collect_lines(&mut lines);
            for line in lines {
                commands.spawn((
                    Sprite {
                        color: Color::srgba(0.6, 0.85, 1.0, 0.92),
                        custom_size: Some(Vec2::new(line.width, line.thickness)),
                        ..default()
                    },
                    Transform::from_xyz(line.x + line.width * 0.5, line.y, 20.0),
                    DragGhost,
                ));
            }
        }
    }
}

fn update_hover_highlight(
    drag_state: Res<DragState>,
    mut draggable_q: Query<(&DraggableTerm, &mut Sprite)>,
) {
    let dragging = drag_state.dragging;
    let cursor = drag_state.cursor_world;

    for (dt, mut sprite) in draggable_q.iter_mut() {
        let tr = (dt.side, dt.index);
        let is_dragging = dragging == Some(tr);

        if !is_dragging && dragging.is_none() {
            // Hover detection
            let size = sprite.custom_size.unwrap_or(Vec2::ZERO);
            // We can't easily get world pos here without Transform; use a simple alpha
            sprite.color = Color::srgba(1.0, 1.0, 1.0, 0.0);
        } else if is_dragging {
            sprite.color = Color::srgba(0.6, 0.85, 1.0, 0.22);
        } else {
            sprite.color = Color::srgba(1.0, 1.0, 1.0, 0.0);
        }
    }
}

fn update_drop_indicators(
    drag_state: Res<DragState>,
    mut indicator_q: Query<(&DropIndicator, &mut Sprite)>,
) {
    let is_dragging = drag_state.dragging.is_some();
    let hover = drag_state.hover_drop;

    for (di, mut sprite) in indicator_q.iter_mut() {
        if !is_dragging {
            sprite.color = Color::srgba(0.0, 0.0, 0.0, 0.0);
            continue;
        }
        let key = (di.side, di.insert_before);
        if hover == Some(key) {
            sprite.color = Color::srgba(0.4, 0.8, 1.0, 0.95);
        } else {
            sprite.color = Color::srgba(0.25, 0.45, 0.7, 0.4);
        }
    }
}
