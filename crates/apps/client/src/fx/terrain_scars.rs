//! Scars on the battlefield itself: every shell the ground swallows leaves a churned crater —
//! a dark core inside a soft ring of turned earth, with ejecta rays thrown outward — stamped
//! flat onto the terrain through the FX pass. The marks live in world space in a budgeted pool
//! (a long barrage recycles the oldest crater, never grows unbounded) and fade back into the
//! field over a couple of minutes, so a fought-over slope *looks* fought over.

use glam::Vec3;
use renderer_api::FxVertex;
use terrain::HeightMap;

use super::decals::{Plate, premul, push_stamp};

/// How many ground marks the battle holds at once; past the budget the oldest is recycled.
pub(crate) const MAX_TERRAIN_SCARS: usize = 128;
/// A crater's full lifetime, and the tail of it spent fading back into the field.
const LIFETIME_S: f32 = 150.0;
const FADE_S: f32 = 30.0;
/// Lift off the ground along its normal so the stamp never z-fights the terrain triangles.
const GROUND_LIFT_M: f32 = 0.05;
/// Central-difference step for the terrain normal, roughly one heightmap cell.
const NORMAL_STEP_M: f32 = 0.75;
/// Ejecta rays thrown out of one crater.
const EJECTA_RAYS: usize = 6;

#[derive(Debug, Default)]
pub(crate) struct TerrainScars {
    scars: Vec<TerrainScar>,
}

#[derive(Debug, Clone, Copy)]
struct TerrainScar {
    center: Vec3,
    normal: Vec3,
    radius_m: f32,
    age_s: f32,
}

impl TerrainScars {
    /// Record one shell death on the ground. The mark snaps to the sampled terrain height and
    /// leans with the local slope — the replicated impact position may sit slightly above the
    /// surface (shell radius, tick quantization), the scar must not float with it.
    pub fn record(&mut self, impact: Vec3, heightmap: &HeightMap) {
        let ground_y = heightmap.sample_height(impact.x, impact.z).unwrap_or(impact.y);
        let center = Vec3::new(impact.x, ground_y, impact.z);
        let mut seed = seed_from(center);
        let scar = TerrainScar {
            center,
            normal: terrain_normal(heightmap, impact.x, impact.z, ground_y),
            radius_m: 0.5 + game_core::math::next_hash_unit(&mut seed) * 0.3,
            age_s: 0.0,
        };
        if self.scars.len() < MAX_TERRAIN_SCARS {
            self.scars.push(scar);
            return;
        }
        if let Some(oldest) = self.scars.iter_mut().max_by(|a, b| a.age_s.total_cmp(&b.age_s)) {
            *oldest = scar;
        }
    }

    /// Age every mark one presented frame and drop the fully faded.
    pub fn tick(&mut self, dt: f32) {
        let dt = dt.clamp(0.0, 0.1).max(0.0);
        self.scars.retain_mut(|scar| {
            scar.age_s += dt;
            scar.age_s < LIFETIME_S
        });
    }

    /// Append every live crater to this frame's FX batch. Called before the particle pool so the
    /// ground marks draw first — they are literally the farthest surface layer, and smoke must
    /// composite over them, not under.
    pub fn append_quads(&self, vertices: &mut Vec<FxVertex>) {
        for scar in &self.scars {
            let opacity = scar.opacity();
            if opacity <= 0.0 {
                continue;
            }
            push_crater(vertices, scar, opacity);
        }
    }

    #[cfg(test)]
    pub fn live_scars(&self) -> usize {
        self.scars.len()
    }
}

impl TerrainScar {
    /// Full strength for most of the lifetime, then a linear fade back into the field.
    fn opacity(&self) -> f32 {
        ((LIFETIME_S - self.age_s) / FADE_S).clamp(0.0, 1.0)
    }
}

/// One crater, three layers in the decal language: a wide SOFT ring of turned earth, the
/// hard-edged dark core where the shell dug in, and a fan of ejecta rays — hashed from the
/// impact point, so every crater looks individual yet renders identically every frame.
fn push_crater(vertices: &mut Vec<FxVertex>, scar: &TerrainScar, opacity: f32) {
    let r = scar.radius_m;
    let normal = scar.normal;
    let mut u = normal.cross(Vec3::X);
    if u.length_squared() < 1.0e-6 {
        u = normal.cross(Vec3::Z);
    }
    let u = u.normalize_or_zero();
    let v = normal.cross(u);
    // A hashed in-plane twist so a line of craters does not share one orientation.
    let mut seed = seed_from(scar.center);
    let twist = game_core::math::next_hash_unit(&mut seed) * std::f32::consts::TAU;
    let (sin, cos) = twist.sin_cos();
    let plate = Plate {
        center: scar.center + normal * GROUND_LIFT_M,
        u: u * cos + v * sin,
        v: v * cos - u * sin,
    };

    // Turned EARTH, not a hole in the world: the old near-black core (0.04 @ 0.85) read as
    // a cartoon void on pale grass. Browner, softer, and the core no longer saturates.
    push_stamp(
        vertices,
        plate,
        r * 2.3,
        r * 2.3,
        0.9,
        premul([0.16, 0.125, 0.085], 0.45 * opacity),
    );
    push_stamp(vertices, plate, r, r, 2.2, premul([0.11, 0.085, 0.058], 0.6 * opacity));
    for ray in 0..EJECTA_RAYS {
        let angle = ray as f32 / EJECTA_RAYS as f32 * std::f32::consts::TAU
            + game_core::math::next_hash_unit(&mut seed);
        let length = r * (1.1 + game_core::math::next_hash_unit(&mut seed) * 0.8);
        let direction = plate.u * angle.cos() + plate.v * angle.sin();
        let streak = Plate {
            center: plate.center + direction * (r * 0.8 + length * 0.5),
            u: direction,
            v: normal.cross(direction).normalize_or_zero(),
        };
        push_stamp(
            vertices,
            streak,
            length * 0.5,
            r * 0.16,
            1.7,
            premul([0.16, 0.13, 0.088], 0.4 * opacity),
        );
    }
}

/// Terrain normal from central height differences — the same bilinear field the tracks ride on,
/// so the crater leans exactly with the slope it marks. Off-map samples fall back to the center
/// height, degrading toward a flat mark at the world edge.
fn terrain_normal(heightmap: &HeightMap, x: f32, z: f32, center_y: f32) -> Vec3 {
    let s = NORMAL_STEP_M;
    let at = |px: f32, pz: f32| heightmap.sample_height(px, pz).unwrap_or(center_y);
    Vec3::new(at(x - s, z) - at(x + s, z), 2.0 * s, at(x, z - s) - at(x, z + s))
        .normalize_or(Vec3::Y)
}

/// Deterministic per-scar seed folded from the impact position bits.
fn seed_from(position: Vec3) -> u64 {
    position.to_array().iter().fold(0x9E37_79B9_7F4A_7C15_u64, |acc, c| {
        acc.wrapping_mul(31).wrapping_add(u64::from(c.to_bits()))
    })
}

/// One splitmix64 step mapped to `[0, 1)` — the FX family's shared generator.
#[cfg(test)]
mod tests {
    use super::*;

    const QUADS_PER_CRATER: usize = 2 + EJECTA_RAYS;

    #[test]
    fn a_scar_stamps_onto_the_sampled_ground_not_the_shell_height() {
        let map = HeightMap::flat(65, 65, 1.0, 3.0).expect("valid map");
        let mut scars = TerrainScars::default();
        // The replicated impact floats half a meter over the surface; the mark must not.
        scars.record(Vec3::new(10.0, 7.5, 12.0), &map);

        let mut vertices = Vec::new();
        scars.append_quads(&mut vertices);
        assert_eq!(vertices.len(), QUADS_PER_CRATER * 6);
        for vertex in &vertices {
            assert!(
                (vertex.position[1] - (3.0 + GROUND_LIFT_M)).abs() < 1.0e-3,
                "flat ground keeps every stamp at ground + lift, got y {}",
                vertex.position[1]
            );
        }
    }

    #[test]
    fn the_crater_leans_with_the_slope() {
        // Height rises 1:1 along x, so the true normal is (-1, 1, 0) normalized.
        let samples: Vec<f32> = (0..65 * 65).map(|index| (index % 65) as f32).collect();
        let map = HeightMap::new(65, 65, 1.0, samples).expect("valid map");
        let mut scars = TerrainScars::default();
        scars.record(Vec3::new(30.0, 31.0, 30.0), &map);

        let normal = Vec3::new(-1.0, 1.0, 0.0).normalize();
        let plane_point = Vec3::new(30.0, 30.0, 30.0) + normal * GROUND_LIFT_M;
        let mut vertices = Vec::new();
        scars.append_quads(&mut vertices);
        assert!(!vertices.is_empty());
        for vertex in &vertices {
            let off_plane = (Vec3::from_array(vertex.position) - plane_point).dot(normal);
            assert!(
                off_plane.abs() < 1.0e-2,
                "every stamp lies in the slope's plane, got {off_plane}"
            );
        }
    }

    #[test]
    fn the_pool_is_budgeted_and_recycles_the_oldest_crater() {
        let map = HeightMap::flat(65, 65, 1.0, 0.0).expect("valid map");
        let mut scars = TerrainScars::default();
        scars.record(Vec3::new(1.0, 0.0, 1.0), &map);
        scars.tick(0.1); // the first crater is now the oldest
        for index in 0..MAX_TERRAIN_SCARS {
            scars.record(Vec3::new(2.0 + index as f32 * 0.3, 0.0, 5.0), &map);
        }

        assert_eq!(scars.live_scars(), MAX_TERRAIN_SCARS, "pool never exceeds the budget");
        assert!(
            scars.scars.iter().all(|scar| scar.age_s < 0.1),
            "the aged crater is the one recycled"
        );
    }

    #[test]
    fn a_crater_fades_out_and_dies() {
        let map = HeightMap::flat(65, 65, 1.0, 0.0).expect("valid map");
        let mut scars = TerrainScars::default();
        scars.record(Vec3::new(10.0, 0.0, 10.0), &map);

        let fresh_alpha = {
            let mut vertices = Vec::new();
            scars.append_quads(&mut vertices);
            vertices[0].color[3]
        };

        // Age deep into the fade window (tick clamps each step like the particle pool does).
        let mid_fade = LIFETIME_S - FADE_S * 0.5;
        for _ in 0..(mid_fade * 10.0) as u32 {
            scars.tick(0.1);
        }
        let mut vertices = Vec::new();
        scars.append_quads(&mut vertices);
        let faded_alpha = vertices[0].color[3];
        assert!(
            faded_alpha > 0.0 && faded_alpha < fresh_alpha,
            "mid-fade the mark is thinner but alive: {faded_alpha} vs {fresh_alpha}"
        );

        for _ in 0..(FADE_S * 10.0) as u32 {
            scars.tick(0.1);
        }
        assert_eq!(scars.live_scars(), 0, "a fully faded crater leaves the pool");
    }
}
