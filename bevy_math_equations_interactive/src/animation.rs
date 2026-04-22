use bevy::prelude::*;

pub struct AnimationPlugin;

impl Plugin for AnimationPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, tick_animations);
    }
}

/// Smooth position tween from `start` to `end`.
#[derive(Component)]
pub struct Tween {
    pub start: Vec3,
    pub end: Vec3,
    pub elapsed: f32,
    pub duration: f32,
}

impl Tween {
    pub fn new(start: Vec3, end: Vec3, duration: f32) -> Self {
        Self { start, end, elapsed: 0.0, duration }
    }

    pub fn done(&self) -> bool {
        self.elapsed >= self.duration
    }

    pub fn value(&self) -> Vec3 {
        let t = (self.elapsed / self.duration).clamp(0.0, 1.0);
        let t = ease_out_cubic(t);
        self.start.lerp(self.end, t)
    }
}

fn ease_out_cubic(t: f32) -> f32 {
    1.0 - (1.0 - t).powi(3)
}

fn tick_animations(
    time: Res<Time>,
    mut query: Query<(Entity, &mut Tween, &mut Transform)>,
    mut commands: Commands,
) {
    for (entity, mut tween, mut transform) in query.iter_mut() {
        tween.elapsed += time.delta_secs();
        transform.translation = tween.value();
        if tween.done() {
            transform.translation = tween.end;
            commands.entity(entity).remove::<Tween>();
        }
    }
}
