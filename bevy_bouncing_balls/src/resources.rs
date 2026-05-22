//! Ressources globales et run conditions.

use bevy::prelude::*;

#[derive(Resource)]
pub struct SimSettings {
    pub gravity_enabled: bool,
    pub collisions_enabled: bool,
    pub paused: bool,
}

impl Default for SimSettings {
    fn default() -> Self {
        Self {
            gravity_enabled: false,
            collisions_enabled: true,
            paused: false,
        }
    }
}

pub fn not_paused(settings: Res<SimSettings>) -> bool {
    !settings.paused
}

#[derive(Resource, Default)]
pub struct TraceData {
    pub active: bool,
    pub target: Option<Entity>,
    pub positions: Vec<Vec2>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sim_settings_default() {
        let settings = SimSettings::default();
        assert!(!settings.gravity_enabled);
        assert!(settings.collisions_enabled);
        assert!(!settings.paused);
    }

    #[test]
    fn test_sim_settings_toggle() {
        let mut settings = SimSettings::default();
        settings.gravity_enabled = true;
        settings.paused = true;

        assert!(settings.gravity_enabled);
        assert!(settings.paused);
    }

    #[test]
    fn test_not_paused_run_condition() {
        let settings = SimSettings {
            paused: false,
            ..Default::default()
        };
        assert!(not_paused(Res::new(settings)));

        let settings_paused = SimSettings {
            paused: true,
            ..Default::default()
        };
        assert!(!not_paused(Res::new(settings_paused)));
    }

    #[test]
    fn test_trace_data_default() {
        let trace = TraceData::default();
        assert!(!trace.active);
        assert_eq!(trace.target, None);
        assert!(trace.positions.is_empty());
    }

    #[test]
    fn test_trace_data_modification() {
        let mut trace = TraceData::default();
        trace.active = true;
        trace.positions.push(Vec2::new(10.0, 20.0));
        trace.positions.push(Vec2::new(30.0, 40.0));

        assert!(trace.active);
        assert_eq!(trace.positions.len(), 2);
        assert_eq!(trace.positions[0], Vec2::new(10.0, 20.0));
    }
}
