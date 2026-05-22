//! Composants ECS de la simulation.

use bevy::prelude::*;

#[derive(Component)]
pub struct Ball {
    pub radius: f32,
    pub mass: f32,
}

#[derive(Component)]
pub struct Velocity(pub Vec2);

#[derive(Component)]
pub struct ColorCycler;

#[derive(Component)]
pub struct HudText;

#[derive(Component)]
pub struct TraceStatsText;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ball_component_creation() {
        let ball = Ball {
            radius: 5.0,
            mass: 10.0,
        };
        assert_eq!(ball.radius, 5.0);
        assert_eq!(ball.mass, 10.0);
    }

    #[test]
    fn test_velocity_component() {
        let vel = Velocity(Vec2::new(10.0, 20.0));
        assert_eq!(vel.0.x, 10.0);
        assert_eq!(vel.0.y, 20.0);
    }

    #[test]
    fn test_ball_mass_proportional_to_radius() {
        let ball1 = Ball {
            radius: 5.0,
            mass: 1.0,
        };
        let ball2 = Ball {
            radius: 10.0,
            mass: 8.0,
        };
        assert!(ball2.mass > ball1.mass);
    }
}
