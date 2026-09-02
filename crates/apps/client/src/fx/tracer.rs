//! Shell tracers: the in-flight shell drawn as what the eye actually sees at 900 m/s — a bright
//! streak with its head at the shell's replicated position and its tail trailing down the
//! flight path — instead of a floating marker. The streak itself is stateless per frame:
//! geometry derives purely from the interpolated `ShellSnapshot`s. The PATH the shell leaves
//! behind is not (Inny Poziom A8): [`ShellTrails`] remembers where each shell has been for
//! under a second and draws it as a dimming line, so the eye can read the arc after the round
//! has gone — the shot that fell short, the one that cleared the crest — instead of a 20 m
//! streak that shows 5 % of a 400 m flight at any instant.

use game_core::{ShellId, ShellType};
use glam::Vec3;
use net::ShellSnapshot;
use renderer_api::FxVertex;

use super::billboard::push_stretched;

/// Seconds of flight the tracer streak spans. At a 900 m/s AP shell this is ~20 m of glow —
/// readable across Prokhorovka without turning into a laser beam.
const TRAIL_SECONDS: f32 = 0.022;
const MIN_LENGTH_M: f32 = 3.0;
const MAX_LENGTH_M: f32 = 22.0;

/// How long the eye keeps a shell's path after the shell has passed a point. Long enough to
/// read a 400 m arc back from its landing, short enough that a firefight never carpets the
/// field in old lines.
pub(crate) const TRAIL_MEMORY_S: f32 = 0.9;
/// Minimum flight between two remembered samples — a frame at 900 m/s is ~15 m, so the memory
/// is a polyline of frame samples; the floor keeps a slow HE lob from spending a sample per
/// centimetre.
const TRAIL_SAMPLE_SPACING_M: f32 = 2.0;
/// Samples one shell may keep: at the spacing floor that is ~200 m of the slowest lob, and
/// far more than a second of the fastest round.
const TRAIL_MAX_SAMPLES: usize = 96;
/// The remembered line's brightness against the live streak's glow, before fading.
const TRAIL_GLOW: f32 = 0.42;
const TRAIL_WIDTH_M: f32 = 0.10;

#[derive(Debug, Clone, Copy)]
struct TrailSample {
    position: Vec3,
    age_s: f32,
}

#[derive(Debug, Clone)]
struct ShellTrail {
    id: ShellId,
    shell_type: ShellType,
    caliber_mm: f32,
    samples: Vec<TrailSample>,
}

/// The paths of every shell the viewer has seen fly, remembered for [`TRAIL_MEMORY_S`] and
/// keyed by `shell_id` (never by owner — a shell's owner is intel, its path is a world event).
#[derive(Debug, Default)]
pub(crate) struct ShellTrails {
    trails: Vec<ShellTrail>,
}

impl ShellTrails {
    /// Age every remembered sample by one presented frame, then note where each live shell is
    /// now. A shell that has vanished (landed, expired) keeps its path until the path has faded.
    pub(crate) fn record(&mut self, shells: &[ShellSnapshot], dt_s: f32) {
        let dt_s = dt_s.clamp(0.0, 0.1);
        for trail in &mut self.trails {
            for sample in &mut trail.samples {
                sample.age_s += dt_s;
            }
        }
        for shell in shells {
            let head = Vec3::from_array(shell.position);
            match self.trails.iter_mut().find(|trail| trail.id == shell.shell_id) {
                Some(trail) => {
                    let moved = trail
                        .samples
                        .last()
                        .is_none_or(|last| last.position.distance(head) >= TRAIL_SAMPLE_SPACING_M);
                    if moved {
                        trail.samples.push(TrailSample { position: head, age_s: 0.0 });
                        if trail.samples.len() > TRAIL_MAX_SAMPLES {
                            trail.samples.remove(0);
                        }
                    }
                }
                None => self.trails.push(ShellTrail {
                    id: shell.shell_id,
                    shell_type: shell.shell_type,
                    caliber_mm: shell.caliber_mm,
                    samples: vec![TrailSample { position: head, age_s: 0.0 }],
                }),
            }
        }
        for trail in &mut self.trails {
            trail.samples.retain(|sample| sample.age_s < TRAIL_MEMORY_S);
        }
        // A path is forgotten only once every sample of it has faded; a fresh shell holds ONE
        // sample until it has flown a spacing's worth, and that one sample is its anchor.
        self.trails.retain(|trail| !trail.samples.is_empty());
    }

    /// The remembered paths as dimming lines: each segment carries the shell's own tracer glow
    /// at [`TRAIL_GLOW`], fading with the square of its age so the line dies from the far end
    /// forward — the way a tracer's smoke thins.
    pub(crate) fn append(&self, vertices: &mut Vec<FxVertex>, eye: Vec3) {
        for trail in &self.trails {
            let look = tracer_look_for(trail.shell_type, trail.caliber_mm);
            for pair in trail.samples.windows(2) {
                let (from, to) = (pair[0], pair[1]);
                let axis_half = (to.position - from.position) * 0.5;
                if axis_half.length_squared() < 1.0e-6 {
                    continue;
                }
                let fade = (1.0 - to.age_s / TRAIL_MEMORY_S).clamp(0.0, 1.0);
                let strength = TRAIL_GLOW * fade * fade;
                let color = [
                    look.glow[0] * strength,
                    look.glow[1] * strength,
                    look.glow[2] * strength,
                    0.0,
                ];
                push_stretched(
                    vertices,
                    from.position + axis_half,
                    axis_half,
                    TRAIL_WIDTH_M * look.width_scale,
                    color,
                    eye,
                );
            }
        }
    }

    #[cfg(test)]
    fn remembered_shells(&self) -> usize {
        self.trails.len()
    }
}

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
    tracer_look_for(shell.shell_type, shell.caliber_mm)
}

fn tracer_look_for(shell_type: ShellType, caliber_mm: f32) -> TracerLook {
    let caliber_scale = (caliber_mm / 100.0).sqrt().clamp(0.72, 1.45);
    let (core, glow, head, type_width, trail_scale) = match shell_type {
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
            owner: Some(TankId(1)),
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

#[cfg(test)]
mod trail_tests {
    use game_core::TankId;

    use super::*;

    fn shell(id: u64, position: [f32; 3]) -> ShellSnapshot {
        ShellSnapshot {
            shell_id: ShellId(id),
            owner: Some(TankId(1)),
            position,
            velocity_mps: [0.0, 0.0, 900.0],
            caliber_mm: 100.0,
            ..Default::default()
        }
    }

    fn span_z(vertices: &[FxVertex]) -> (f32, f32) {
        vertices.iter().fold((f32::MAX, f32::MIN), |(lo, hi), v| {
            (lo.min(v.position[2]), hi.max(v.position[2]))
        })
    }

    /// Inny Poziom A8: the path stays in the eye after the shell has passed — from where it was
    /// first seen to where it was last seen — and dies within the memory, not before.
    #[test]
    fn a_shells_path_stays_in_the_eye_after_it_has_passed() {
        let mut trails = ShellTrails::default();
        let dt = 1.0 / 60.0;
        for frame in 0..5 {
            trails.record(&[shell(7, [0.0, 2.0, 15.0 * frame as f32])], dt);
        }
        // The shell lands; the snapshot stops carrying it.
        trails.record(&[], dt);
        let mut vertices = Vec::new();
        trails.append(&mut vertices, Vec3::new(30.0, 5.0, 30.0));
        assert!(!vertices.is_empty(), "the path outlives the shell");
        let (lo, hi) = span_z(&vertices);
        assert!(lo <= 0.5 && hi >= 59.5, "the line spans the flight: {lo}..{hi}");

        // Half the memory later it is still there, dimmer; past the memory it is gone.
        for _ in 0..25 {
            trails.record(&[], dt);
        }
        let mut dimmer = Vec::new();
        trails.append(&mut dimmer, Vec3::new(30.0, 5.0, 30.0));
        assert!(!dimmer.is_empty(), "half a memory in, the path still reads");
        let brightest = |v: &[FxVertex]| v.iter().map(|x| x.color[0]).fold(0.0_f32, f32::max);
        assert!(brightest(&dimmer) < brightest(&vertices), "the line dims with age");
        for _ in 0..40 {
            trails.record(&[], dt);
        }
        let mut gone = Vec::new();
        trails.append(&mut gone, Vec3::new(30.0, 5.0, 30.0));
        assert!(gone.is_empty(), "past the memory the path is forgotten");
        assert_eq!(trails.remembered_shells(), 0);
    }

    /// Paths are kept per shell id: two rounds in the air are two lines, and a round that has
    /// not moved a sample's worth leaves no line at all.
    #[test]
    fn paths_are_kept_per_shell_and_a_still_shell_leaves_none() {
        let mut trails = ShellTrails::default();
        let dt = 1.0 / 60.0;
        for frame in 0..4 {
            let z = 15.0 * frame as f32;
            trails.record(&[shell(1, [0.0, 2.0, z]), shell(2, [40.0, 2.0, z])], dt);
        }
        assert_eq!(trails.remembered_shells(), 2);
        let mut vertices = Vec::new();
        trails.append(&mut vertices, Vec3::new(20.0, 5.0, -20.0));
        let on_left = vertices.iter().filter(|v| v.position[0] < 20.0).count();
        let on_right = vertices.iter().filter(|v| v.position[0] > 20.0).count();
        assert!(on_left > 0 && on_right > 0, "both rounds draw their own line");

        let mut still = ShellTrails::default();
        for _ in 0..200 {
            still.record(&[shell(3, [5.0, 2.0, 5.0])], dt);
        }
        let mut none = Vec::new();
        still.append(&mut none, Vec3::ONE);
        assert!(none.is_empty(), "a shell that has not flown a sample's worth draws nothing");
    }
}
