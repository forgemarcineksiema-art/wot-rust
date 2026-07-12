//! Shell tracers: the in-flight shell drawn as what the eye actually sees at 900 m/s — a bright
//! streak with its head at the shell's replicated position and its tail trailing down the
//! flight path — instead of a floating marker. Stateless per frame: geometry derives purely
//! from the interpolated `ShellSnapshot`s, so it needs no per-shell identity or history.

use game_core::ShellType;
use glam::Vec3;
use net::ShellSnapshot;
use renderer_api::FxVertex;

use super::billboard::push_stretched;

/// Seconds of flight the tracer streak spans. At a 900 m/s AP shell this is ~20 m of glow —
/// readable across Prokhorovka without turning into a laser beam.
const TRAIL_SECONDS: f32 = 0.022;
const MIN_LENGTH_M: f32 = 3.0;
const MAX_LENGTH_M: f32 = 22.0;

/// All colors premultiplied with `alpha = 0`: tracers are pure additive glow, so overlapping
/// streaks brighten instead of occluding, and draw order cannot matter.
struct TracerLook {
    core: [f32; 4],
    glow: [f32; 4],
    head: [f32; 4],
    width_scale: f32,
    trail_scale: f32,
}

fn tracer_look(shell: &ShellSnapshot) -> TracerLook {
    let caliber_scale = (shell.caliber_mm / 100.0).sqrt().clamp(0.72, 1.45);
    let (core, glow, head, type_width, trail_scale) = match shell.shell_type {
        ShellType::ArmorPiercing => {
            ([1.0, 0.93, 0.72, 0.0], [0.55, 0.30, 0.10, 0.0], [1.0, 0.98, 0.90, 0.0], 1.0, 1.0)
        }
        ShellType::Apcr => {
            ([0.88, 0.96, 1.0, 0.0], [0.18, 0.42, 0.72, 0.0], [0.96, 1.0, 1.0, 0.0], 0.82, 1.12)
        }
        ShellType::Heat => {
            ([1.0, 0.82, 0.42, 0.0], [0.72, 0.18, 0.03, 0.0], [1.0, 0.92, 0.64, 0.0], 1.12, 0.82)
        }
        ShellType::HighExplosive => {
            ([1.0, 0.62, 0.24, 0.0], [0.68, 0.08, 0.02, 0.0], [1.0, 0.78, 0.38, 0.0], 1.30, 0.72)
        }
    };
    TracerLook { core, glow, head, width_scale: caliber_scale * type_width, trail_scale }
}

/// Append every live shell's tracer to this frame's FX batch: a thin white-hot core inside a
/// wider amber glow, plus a short bright head right at the shell.
pub(crate) fn append_shell_tracers(
    vertices: &mut Vec<FxVertex>,
    shells: &[ShellSnapshot],
    eye: Vec3,
) {
    for shell in shells {
        let velocity = Vec3::from_array(shell.velocity_mps);
        let speed = velocity.length();
        if speed < 1.0 {
            continue;
        }
        let direction = velocity / speed;
        let look = tracer_look(shell);
        let length = (speed * TRAIL_SECONDS * look.trail_scale).clamp(MIN_LENGTH_M, MAX_LENGTH_M);
        let head = Vec3::from_array(shell.position);
        // The streak's LEADING edge sits on the shell: center it half a length behind the head.
        let center = head - direction * (length * 0.5);
        let axis_half = direction * (length * 0.5);

        push_stretched(vertices, center, axis_half, 0.30 * look.width_scale, look.glow, eye);
        push_stretched(vertices, center, axis_half, 0.09 * look.width_scale, look.core, eye);
        push_stretched(
            vertices,
            head - direction * 0.6,
            direction * 0.6,
            0.22 * look.width_scale,
            look.head,
            eye,
        );
    }
}

#[cfg(test)]
mod tests {
    use game_core::TankId;

    use super::*;

    fn shell(position: [f32; 3], velocity_mps: [f32; 3]) -> ShellSnapshot {
        ShellSnapshot {
            owner: TankId(1),
            position,
            velocity_mps,
            caliber_mm: 100.0,
            ..Default::default()
        }
    }

    /// Longest reach of any vertex along `direction` from the origin.
    fn max_along(vertices: &[FxVertex], direction: Vec3) -> f32 {
        vertices
            .iter()
            .map(|vertex| Vec3::from_array(vertex.position).dot(direction))
            .fold(f32::MIN, f32::max)
    }

    #[test]
    fn the_tracer_head_rides_the_shell_and_the_tail_trails_the_flight_path() {
        let mut vertices = Vec::new();
        let position = Vec3::new(100.0, 10.0, 50.0);
        append_shell_tracers(
            &mut vertices,
            &[shell(position.to_array(), [0.0, 0.0, 900.0])],
            Vec3::new(80.0, 12.0, 0.0),
        );

        assert_eq!(vertices.len(), 18, "glow + core + head quads");
        let direction = Vec3::Z;
        let forward_reach = max_along(&vertices, direction) - position.dot(direction);
        assert!(forward_reach.abs() < 0.01, "nothing leads the shell, got {forward_reach}");
        let tail_reach = position.dot(direction) + max_along(&vertices, -direction);
        assert!(
            (tail_reach - (900.0 * TRAIL_SECONDS)).abs() < 0.5,
            "the tail spans the flight-time streak, got {tail_reach}"
        );
    }

    #[test]
    fn slow_and_degenerate_shells_stay_bounded_and_finite() {
        let mut vertices = Vec::new();
        // A slow (mortar-like) shell clamps to the minimum readable streak.
        append_shell_tracers(&mut vertices, &[shell([0.0; 3], [0.0, -30.0, 0.0])], Vec3::ONE);
        let vertical = max_along(&vertices, Vec3::Y) + max_along(&vertices, -Vec3::Y);
        assert!(vertical.abs() <= MIN_LENGTH_M + 1.0, "short streak clamps, got {vertical}");
        for vertex in &vertices {
            assert!(vertex.position.iter().all(|component| component.is_finite()));
        }

        // A stationary shell (corrupt snapshot) is skipped rather than dividing by zero.
        let mut none = Vec::new();
        append_shell_tracers(&mut none, &[shell([0.0; 3], [0.0; 3])], Vec3::ONE);
        assert!(none.is_empty());
    }

    #[test]
    fn tracers_are_pure_additive_glow() {
        let mut vertices = Vec::new();
        append_shell_tracers(&mut vertices, &[shell([0.0; 3], [900.0, 0.0, 0.0])], Vec3::ONE);
        assert!(
            vertices.iter().all(|vertex| vertex.color[3] == 0.0),
            "alpha 0 everywhere: overlapping tracers brighten, never occlude"
        );
    }
}
