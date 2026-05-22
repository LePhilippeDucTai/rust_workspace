use std::collections::VecDeque;
use bevy::prelude::*;

#[derive(Component)]
pub struct Body {
    pub mass: f32,
    pub base_color: Color,
}

#[derive(Component, Default)]
pub struct Trail {
    pub points: VecDeque<Vec3>,
}

#[derive(Component)]
pub struct OrbitCamera {
    pub yaw: f32,
    pub pitch: f32,
    pub distance: f32,
    pub target: Vec3,
}

#[derive(Component)]
pub struct HudText;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_body_component() {
        let body = Body {
            mass: 100.0,
            base_color: Color::srgb(1.0, 0.0, 0.0),
        };
        assert_eq!(body.mass, 100.0);
    }

    #[test]
    fn test_trail_component_default() {
        let trail = Trail::default();
        assert!(trail.points.is_empty());
    }

    #[test]
    fn test_trail_component_add_point() {
        let mut trail = Trail::default();
        trail.points.push_back(Vec3::new(1.0, 2.0, 3.0));
        trail.points.push_back(Vec3::new(4.0, 5.0, 6.0));

        assert_eq!(trail.points.len(), 2);
        assert_eq!(trail.points[0], Vec3::new(1.0, 2.0, 3.0));
    }

    #[test]
    fn test_orbit_camera_component() {
        let camera = OrbitCamera {
            yaw: 1.57,
            pitch: 0.785,
            distance: 50.0,
            target: Vec3::new(0.0, 0.0, 0.0),
        };
        assert!(camera.yaw > 0.0);
        assert!(camera.pitch > 0.0);
        assert!(camera.distance > 0.0);
    }
}
