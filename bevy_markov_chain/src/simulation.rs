//! Système de mise à jour des agents : transitions markoviennes et
//! interpolation visuelle entre nœuds.

use bevy::prelude::*;

use crate::components::Agent;
use crate::config::{AGENT_RADIUS, STATE_COLORS};
use crate::resources::{Chain, EmpiricalDistribution, NodePositions, SimSettings};

pub struct SimulationPlugin;

impl Plugin for SimulationPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, (advance_agents, update_agent_positions).chain())
            .add_systems(Update, recompute_empirical);
    }
}

/// À chaque frame, fait progresser la transition des agents. Quand un agent
/// atteint son nœud cible (progress >= 1), on échantillonne son prochain
/// état selon la matrice de transition.
fn advance_agents(
    time: Res<Time>,
    chain: Res<Chain>,
    mut settings: ResMut<SimSettings>,
    mut agents: Query<&mut Agent>,
) {
    if settings.paused {
        return;
    }

    let dt = time.delta_secs();
    let speed = settings.steps_per_second;
    let mut rng = rand::thread_rng();
    let mut transitions_this_frame: u64 = 0;

    for mut agent in agents.iter_mut() {
        agent.progress += dt * speed;
        while agent.progress >= 1.0 {
            agent.progress -= 1.0;
            let next = chain.0.sample_next(agent.current_state, &mut rng);
            agent.previous_state = agent.current_state;
            agent.current_state = next;
            transitions_this_frame += 1;
        }
    }

    settings.elapsed_steps += transitions_this_frame;
}

/// Place visuellement chaque agent : interpolation linéaire entre le nœud
/// précédent et le nœud courant, modulée par `progress`.
fn update_agent_positions(
    positions: Res<NodePositions>,
    mut agents: Query<(&Agent, &mut Transform)>,
) {
    for (agent, mut tf) in agents.iter_mut() {
        let from = positions.0[agent.previous_state];
        let to = positions.0[agent.current_state];
        let p = agent.progress.clamp(0.0, 1.0);
        let pos = from.lerp(to, p);
        tf.translation.x = pos.x;
        tf.translation.y = pos.y;
        tf.translation.z = 2.0;
    }
}

/// Recalcule la distribution empirique (proportion d'agents par état).
fn recompute_empirical(
    chain: Res<Chain>,
    agents: Query<&Agent>,
    mut empirical: ResMut<EmpiricalDistribution>,
) {
    let n = chain.0.num_states();
    let mut counts = vec![0.0_f32; n];
    let mut total = 0.0_f32;
    for agent in agents.iter() {
        counts[agent.current_state] += 1.0;
        total += 1.0;
    }
    empirical.counts = counts;
    empirical.total = total;
}

/// Helper : matériau coloré pour l'état `state`.
pub fn state_color(state: usize) -> Color {
    let (r, g, b) = STATE_COLORS[state % STATE_COLORS.len()];
    Color::srgb(r, g, b)
}

/// Crée un agent au démarrage (utilisé aussi pour ajouter à chaud).
pub fn spawn_agent(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<ColorMaterial>,
    positions: &NodePositions,
    initial_state: usize,
) {
    let pos = positions.0[initial_state];
    let color = state_color(initial_state);
    commands.spawn((
        Mesh2d(meshes.add(Circle::new(AGENT_RADIUS))),
        MeshMaterial2d(materials.add(color)),
        Transform::from_xyz(pos.x, pos.y, 2.0),
        Agent {
            current_state: initial_state,
            previous_state: initial_state,
            progress: 0.0,
        },
    ));
}

/// Met à jour la couleur des agents pour refléter leur état courant.
pub fn agent_color_system(
    agents: Query<(&Agent, &MeshMaterial2d<ColorMaterial>)>,
    mut materials: ResMut<Assets<ColorMaterial>>,
) {
    for (agent, handle) in agents.iter() {
        if let Some(mat) = materials.get_mut(&handle.0) {
            // Interpole la couleur entre l'état précédent et le courant
            // pendant la transition pour un rendu plus fluide.
            let c_prev = state_color(agent.previous_state).to_srgba();
            let c_curr = state_color(agent.current_state).to_srgba();
            let t = agent.progress.clamp(0.0, 1.0);
            mat.color = Color::srgba(
                c_prev.red * (1.0 - t) + c_curr.red * t,
                c_prev.green * (1.0 - t) + c_curr.green * t,
                c_prev.blue * (1.0 - t) + c_curr.blue * t,
                1.0,
            );
        }
    }
}
