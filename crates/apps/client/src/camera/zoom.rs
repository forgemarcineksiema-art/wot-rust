//! Scroll-wheel zoom ladder shared by both camera modes.
//!
//! Sniper magnification is perceived as `1/FOV`, so the ladder steps are roughly geometric in
//! FOV: each click is a similar *relative* magnification change. A linear FOV slider makes the
//! wide end feel dead and the narrow end jumpy. Scrolling past the ends of a mode hands over to
//! the other mode (boom at minimum -> sniper; sniper at widest -> boom), WoT-style, so the wheel
//! sweeps one continuous zoom axis from far third-person to maximum magnification.

use super::BattleCameraMode;

/// Sniper FOV ladder in degrees, widest first.
pub(crate) const SNIPER_FOV_STEPS_DEGREES: [f32; 5] = [18.0, 12.0, 8.0, 5.0, 3.0];

/// Tolerance for matching the current FOV against a ladder step.
const STEP_EPSILON: f32 = 0.01;

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct ZoomState {
    pub mode: BattleCameraMode,
    pub distance_m: f32,
    pub sniper_fov_degrees: f32,
}

/// One wheel input applied to the zoom axis. Negative `zoom_delta_m` zooms in.
pub(crate) fn apply_zoom(
    state: ZoomState,
    zoom_delta_m: f32,
    min_distance_m: f32,
    max_distance_m: f32,
) -> ZoomState {
    if zoom_delta_m < 0.0 {
        zoom_in(state, zoom_delta_m, min_distance_m)
    } else if zoom_delta_m > 0.0 {
        zoom_out(state, zoom_delta_m, min_distance_m, max_distance_m)
    } else {
        state
    }
}

fn zoom_in(mut state: ZoomState, zoom_delta_m: f32, min_distance_m: f32) -> ZoomState {
    match state.mode {
        BattleCameraMode::Sniper => {
            if let Some(step) = narrower_step(state.sniper_fov_degrees) {
                state.sniper_fov_degrees = step;
            }
        }
        BattleCameraMode::ThirdPerson => {
            if state.distance_m <= min_distance_m + STEP_EPSILON {
                state.mode = BattleCameraMode::Sniper;
                state.sniper_fov_degrees = SNIPER_FOV_STEPS_DEGREES[0];
            } else {
                state.distance_m = (state.distance_m + zoom_delta_m).max(min_distance_m);
            }
        }
    }
    state
}

fn zoom_out(
    mut state: ZoomState,
    zoom_delta_m: f32,
    min_distance_m: f32,
    max_distance_m: f32,
) -> ZoomState {
    match state.mode {
        BattleCameraMode::Sniper => match wider_step(state.sniper_fov_degrees) {
            Some(step) => state.sniper_fov_degrees = step,
            None => {
                state.mode = BattleCameraMode::ThirdPerson;
                state.distance_m = min_distance_m;
            }
        },
        BattleCameraMode::ThirdPerson => {
            state.distance_m = (state.distance_m + zoom_delta_m).min(max_distance_m);
        }
    }
    state
}

/// Next narrower FOV step (zoom in), or `None` at the narrowest step.
fn narrower_step(fov_degrees: f32) -> Option<f32> {
    SNIPER_FOV_STEPS_DEGREES.iter().copied().find(|step| *step < fov_degrees - STEP_EPSILON)
}

/// Next wider FOV step (zoom out), or `None` at or beyond the widest step.
fn wider_step(fov_degrees: f32) -> Option<f32> {
    SNIPER_FOV_STEPS_DEGREES.iter().rev().copied().find(|step| *step > fov_degrees + STEP_EPSILON)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sniper(fov: f32) -> ZoomState {
        ZoomState { mode: BattleCameraMode::Sniper, distance_m: 3.0, sniper_fov_degrees: fov }
    }

    #[test]
    fn sniper_zoom_steps_are_discrete_and_saturate_at_the_narrow_end() {
        let state = apply_zoom(sniper(8.0), -0.8, 3.0, 18.0);
        assert_eq!(state.sniper_fov_degrees, 5.0);
        let state = apply_zoom(state, -0.8, 3.0, 18.0);
        assert_eq!(state.sniper_fov_degrees, 3.0);
        let state = apply_zoom(state, -0.8, 3.0, 18.0);
        assert_eq!(state.sniper_fov_degrees, 3.0, "narrowest step must hold");
    }

    #[test]
    fn sniper_zoom_out_past_the_widest_step_returns_to_the_shortest_boom() {
        let state = apply_zoom(sniper(18.0), 0.8, 3.0, 18.0);
        assert_eq!(state.mode, BattleCameraMode::ThirdPerson);
        assert_eq!(state.distance_m, 3.0);
    }

    #[test]
    fn third_person_zoom_in_at_the_shortest_boom_enters_sniper_at_the_widest_step() {
        let boom = ZoomState {
            mode: BattleCameraMode::ThirdPerson,
            distance_m: 3.0,
            sniper_fov_degrees: 8.0,
        };
        let state = apply_zoom(boom, -0.8, 3.0, 18.0);
        assert_eq!(state.mode, BattleCameraMode::Sniper);
        assert_eq!(state.sniper_fov_degrees, SNIPER_FOV_STEPS_DEGREES[0]);
    }

    #[test]
    fn third_person_zoom_stays_continuous_between_the_distance_clamps() {
        let boom = ZoomState {
            mode: BattleCameraMode::ThirdPerson,
            distance_m: 12.0,
            sniper_fov_degrees: 8.0,
        };
        let state = apply_zoom(boom, -0.8, 3.0, 18.0);
        assert_eq!(state.mode, BattleCameraMode::ThirdPerson);
        assert!((state.distance_m - 11.2).abs() < 1.0e-5);
        let state = apply_zoom(state, 100.0, 3.0, 18.0);
        assert_eq!(state.distance_m, 18.0, "zoom out clamps at the longest boom");
    }

    #[test]
    fn off_ladder_fov_snaps_to_the_nearest_step_in_the_scroll_direction() {
        let state = apply_zoom(sniper(10.0), -0.8, 3.0, 18.0);
        assert_eq!(state.sniper_fov_degrees, 8.0);
        let state = apply_zoom(sniper(10.0), 0.8, 3.0, 18.0);
        assert_eq!(state.sniper_fov_degrees, 12.0);
    }
}
