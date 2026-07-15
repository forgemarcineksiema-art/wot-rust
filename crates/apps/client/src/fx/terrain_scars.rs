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

#[derive(Debug, Default)]
pub struct TerrainScars {
    scars: Vec<TerrainScar>,
}

#[derive(Debug, Clone)]
struct TerrainScar {
    center: Vec3,
    normal: Vec3,
    radius_m: f32,
    age_s: f32,
    /// What died here (v30): a kinetic bolt PLOUGHS, a charge CRATERS — different marks.
    shell_type: game_core::ShellType,
    /// Ground-plane flight direction at death, ZERO on old snapshots (degrades to radial).
    direction_xz: Vec3,
    /// The mark's geometry, built ONCE at record time against the ground of that moment —
    /// DRAPED over the deformed surface (P4c), colors at full strength. `append_quads` only
    /// scales by the current fade; a flat stamp would sink into the very bowl it marks.
    stamps: Vec<FxVertex>,
}

impl TerrainScars {
    /// Record one shell death on the ground. The mark snaps to the sampled terrain height and
    /// leans with the local slope — the replicated impact position may sit slightly above the
    /// surface (shell radius, tick quantization), the scar must not float with it.
    pub fn record(&mut self, impact: &game_core::ShellImpact, heightmap: &HeightMap) {
        let position = impact.position;
        let ground_y = heightmap.sample_height(position.x, position.z).unwrap_or(position.y);
        let center = Vec3::new(position.x, ground_y, position.z);
        let mut seed = seed_from(center);
        // The mark's size is the projectile's size (v30): a 122 mm hole is not a 76 mm one.
        // Old snapshots (caliber 0) fall back to the historical medium-calibre look.
        let caliber_m = if impact.caliber_mm > 1.0 { impact.caliber_mm * 0.001 } else { 0.09 };
        // A high-explosive mark is sized by the SAME derivation the sim's crater ledger uses,
        // so the scorch fills exactly the bowl physics dug; kinetic furrows keep the
        // calibre-scale gouge (they move no earth).
        let radius_m = if impact.shell_type == game_core::ShellType::HighExplosive {
            terrain::he_crater_radius_m(impact.caliber_mm)
        } else {
            caliber_m * (4.5 + game_core::math::next_hash_unit(&mut seed) * 1.5)
        };
        let mut scar = TerrainScar {
            center,
            normal: terrain_normal(heightmap, position.x, position.z, ground_y),
            radius_m,
            age_s: 0.0,
            shell_type: impact.shell_type,
            direction_xz: Vec3::new(impact.direction.x, 0.0, impact.direction.z)
                .normalize_or_zero(),
            stamps: Vec::new(),
        };
        scar.build_stamps(heightmap);
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
            // Premultiplied colors: one uniform scale of all four lanes IS the fade.
            vertices.extend(scar.stamps.iter().map(|vertex| {
                let mut faded = *vertex;
                for lane in &mut faded.color {
                    *lane *= opacity;
                }
                faded
            }));
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

    /// Build the mark once, against the ground as it stands at record time.
    fn build_stamps(&mut self, heightmap: &HeightMap) {
        let kinetic = matches!(
            self.shell_type,
            game_core::ShellType::ArmorPiercing | game_core::ShellType::Apcr
        );
        let mut stamps = Vec::new();
        if kinetic && self.direction_xz.length_squared() > 0.5 {
            push_furrow(&mut stamps, self, 1.0);
        } else {
            push_crater(&mut stamps, self, 1.0, heightmap);
        }
        self.stamps = stamps;
    }
}

/// The kinetic mark (Fizyczny Świat P2): a full-calibre bolt arriving at 800+ m/s does NOT
/// crater — it PLOUGHS. One elongated gouge along the flight track (dark turned soil), a
/// narrow lighter border of disturbed earth, and the spoil thrown FORWARD along the track —
/// never radially. Photo reference: an AP shot into a field leaves a metre-scale furrow
/// pointing back at the gun.
fn push_furrow(vertices: &mut Vec<FxVertex>, scar: &TerrainScar, opacity: f32) {
    let along = scar.direction_xz;
    let across = scar.normal.cross(along).normalize_or_zero();
    let mut seed = seed_from(scar.center);
    // Furrow length: the shallower the arrival, the longer the plough. The scar's radius is
    // calibre-derived; the gouge runs several of those, jittered per impact.
    let length = scar.radius_m * (2.6 + game_core::math::next_hash_unit(&mut seed) * 1.4);
    let width = scar.radius_m * 0.55;
    // The gouge is centred FORWARD of the strike point — the shell ploughed onward.
    let plate = Plate {
        center: scar.center + scar.normal * GROUND_LIFT_M + along * (length * 0.35),
        u: along,
        v: across,
    };
    // Dark turned soil first read: the gouge dominates the mark. The disturbed border is a
    // NARROW, muted fringe — the first cut ran it 1.9x as wide at 0.35 alpha, and under a warm
    // evening/overcast grade that pale sheet read as an orange sticker floating on the grass
    // (caught on a live battle screenshot). Turned earth is dark in ANY light.
    push_stamp(
        vertices,
        plate,
        length,
        width * 0.75,
        2.4,
        premul([0.085, 0.066, 0.045], 0.78 * opacity),
    );
    push_stamp(
        vertices,
        plate,
        length * 1.1,
        width * 1.25,
        1.4,
        premul([0.12, 0.098, 0.066], 0.30 * opacity),
    );
    // Spoil thrown forward: two or three clods DOWNRANGE of the gouge, spreading slightly.
    let clods = 2 + (game_core::math::next_hash_unit(&mut seed) * 1.99) as usize;
    for _ in 0..clods {
        let forward = length * (0.9 + game_core::math::next_hash_unit(&mut seed) * 0.9);
        let side = (game_core::math::next_hash_unit(&mut seed) - 0.5) * width * 2.2;
        let clod = Plate {
            center: scar.center + scar.normal * GROUND_LIFT_M + along * forward + across * side,
            u: along,
            v: across,
        };
        let size = scar.radius_m * (0.25 + game_core::math::next_hash_unit(&mut seed) * 0.3);
        push_stamp(
            vertices,
            clod,
            size * 1.6,
            size,
            1.6,
            premul([0.13, 0.105, 0.07], 0.42 * opacity),
        );
    }
}

/// The blast mark (Fizyczny Świat P3/P4c): high explosive CRATERS. Photo reference — a raised
/// rim of fresh-turned, LIGHTER earth around a scorched dark bowl, with clods thrown metres
/// out, biased DOWNRANGE at the shallow arrival angles of tank fire. Sized by the SAME
/// derivation as the replicated deformation and DRAPED over it, so the scorch lines the bowl
/// physics dug instead of hovering over it.
fn push_crater(
    vertices: &mut Vec<FxVertex>,
    scar: &TerrainScar,
    opacity: f32,
    heightmap: &HeightMap,
) {
    // The scorch fills exactly the bowl the ledger dug; the spoil ring runs out to the
    // deformation's influence edge. Both are DRAPED over the deformed surface — a flat quad
    // would sink into the very bowl it marks and surface only at the deepest point.
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
    let plate = Plate { center: scar.center, u: u * cos + v * sin, v: v * cos - u * sin };

    // The rim: fresh subsoil thrown up and out — LIGHTER than the field it landed on. The
    // bowl rides a hair higher than the rim where they overlap, so the two draped sheets
    // never z-fight on the steep wall.
    push_draped_stamp(
        vertices,
        heightmap,
        plate,
        r * 1.5,
        1.0,
        premul([0.32, 0.27, 0.185], 0.55 * opacity),
        GROUND_LIFT_M,
    );
    // The bowl: scorched, dark — but earth-dark, never a void.
    push_draped_stamp(
        vertices,
        heightmap,
        plate,
        r,
        2.0,
        premul([0.085, 0.07, 0.05], 0.7 * opacity),
        GROUND_LIFT_M * 1.6,
    );

    // Clods: lumps of soil metres out, biased DOWNRANGE when the wire knows the track
    // (shallow tank-gun arrivals shovel most of the spoil forward); uniform on old snapshots.
    let has_track = scar.direction_xz.length_squared() > 0.5;
    let clods = 4 + (game_core::math::next_hash_unit(&mut seed) * 2.99) as usize;
    for _ in 0..clods {
        let spread = (game_core::math::next_hash_unit(&mut seed) - 0.5) * std::f32::consts::TAU;
        let angle = if has_track {
            // ~±70° cone around the flight direction for 3 of 4 clods.
            if game_core::math::next_hash_unit(&mut seed) < 0.75 { spread * 0.4 } else { spread }
        } else {
            spread
        };
        let base_dir = if has_track { scar.direction_xz } else { plate.u };
        let across = normal.cross(base_dir).normalize_or_zero();
        let (sa, ca) = angle.sin_cos();
        let dir = (base_dir * ca + across * sa).normalize_or_zero();
        let dist = r * (1.2 + game_core::math::next_hash_unit(&mut seed) * 1.6);
        let size = r * (0.12 + game_core::math::next_hash_unit(&mut seed) * 0.14);
        let lift = scar.normal * GROUND_LIFT_M;
        let clod = Plate {
            center: ground_point(heightmap, plate.center + dir * dist) + lift,
            u: dir,
            v: across,
        };
        push_stamp(
            vertices,
            clod,
            size * 1.5,
            size,
            1.6,
            premul([0.24, 0.20, 0.135], 0.45 * opacity),
        );
    }
}

/// A stamp draped over the ground: the same [-1,1]^2 falloff domain `push_stamp` draws, but
/// subdivided, each vertex dropped onto the sampled surface (plus the anti-z-fight lift along
/// the vertical, scaled so the offset measured along the slope normal stays constant).
fn push_draped_stamp(
    vertices: &mut Vec<FxVertex>,
    heightmap: &HeightMap,
    plate: Plate,
    half_m: f32,
    sharpness: f32,
    color: [f32; 4],
    lift_m: f32,
) {
    const DRAPE_SUBDIVISIONS: usize = 5;
    let n = DRAPE_SUBDIVISIONS;
    // Vertical lift sized so the offset measured along the slope normal stays `lift_m`.
    let lift = lift_m / plate.u.cross(plate.v).y.abs().max(0.35);
    let corner = |iu: usize, iv: usize| -> ([f32; 3], [f32; 2]) {
        let uu = -1.0 + 2.0 * iu as f32 / n as f32;
        let vv = -1.0 + 2.0 * iv as f32 / n as f32;
        let flat = plate.center + plate.u * (half_m * uu) + plate.v * (half_m * vv);
        let grounded = ground_point(heightmap, flat);
        ([grounded.x, grounded.y + lift, grounded.z], [uu, vv])
    };
    for iv in 0..n {
        for iu in 0..n {
            let quad = [
                (iu, iv),
                (iu + 1, iv),
                (iu + 1, iv + 1),
                (iu, iv),
                (iu + 1, iv + 1),
                (iu, iv + 1),
            ];
            for (cu, cv) in quad {
                let (position, uv) = corner(cu, cv);
                vertices.push(FxVertex::sharp(position, uv, sharpness, color));
            }
        }
    }
}

/// Project a point onto the sampled ground (deformed truth); off-map keeps its own height.
fn ground_point(heightmap: &HeightMap, point: Vec3) -> Vec3 {
    let y = heightmap.sample_height(point.x, point.z).unwrap_or(point.y);
    Vec3::new(point.x, y, point.z)
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

    /// An HE impact at a position — the radial-crater branch (kinetic marks get furrows).
    fn he_impact(position: Vec3) -> game_core::ShellImpact {
        game_core::ShellImpact {
            owner: game_core::TankId(1),
            position,
            surface: game_core::ImpactSurface::Terrain,
            shell_type: game_core::ShellType::HighExplosive,
            caliber_mm: 100.0,
            ..Default::default()
        }
    }

    /// A kinetic impact flying along +Z at the given calibre.
    fn ap_impact(position: Vec3, caliber_mm: f32) -> game_core::ShellImpact {
        game_core::ShellImpact {
            owner: game_core::TankId(1),
            position,
            surface: game_core::ImpactSurface::Terrain,
            shell_type: game_core::ShellType::ArmorPiercing,
            direction: Vec3::new(0.0, -0.3, 0.95).normalize(),
            caliber_mm,
        }
    }

    /// Draped rim (25 cells) + draped bowl (25 cells) + 4..=6 hashed flat clods (P3/P4c):
    /// the clod count varies per impact, so tests assert the RANGE, not one number.
    const DRAPE_CELLS: usize = 5 * 5;
    const MIN_QUADS_PER_CRATER: usize = 2 * DRAPE_CELLS + 4;
    const MAX_QUADS_PER_CRATER: usize = 2 * DRAPE_CELLS + 6;

    #[test]
    fn a_scar_stamps_onto_the_sampled_ground_not_the_shell_height() {
        let map = HeightMap::flat(65, 65, 1.0, 3.0).expect("valid map");
        let mut scars = TerrainScars::default();
        // The replicated impact floats half a meter over the surface; the mark must not.
        scars.record(&he_impact(Vec3::new(10.0, 7.5, 12.0)), &map);

        let mut vertices = Vec::new();
        scars.append_quads(&mut vertices);
        let quads = vertices.len() / 6;
        assert!(
            (MIN_QUADS_PER_CRATER..=MAX_QUADS_PER_CRATER).contains(&quads),
            "rim + bowl + clods, got {quads} quads"
        );
        for vertex in &vertices {
            let above = vertex.position[1] - 3.0;
            assert!(
                (GROUND_LIFT_M - 1.0e-3..=GROUND_LIFT_M * 1.6 + 1.0e-3).contains(&above),
                "flat ground keeps every stamp in the anti-z-fight lift band, got y {}",
                vertex.position[1]
            );
        }
    }

    /// P3's contract: the HE crater reads like the photographs — a LIGHTER fresh-earth rim
    /// around a darker scorched bowl, and the clods fly with a downrange bias when the wire
    /// knows the track.
    #[test]
    fn the_he_crater_has_a_light_rim_a_dark_bowl_and_downrange_clods() {
        let map = HeightMap::flat(65, 65, 1.0, 0.0).expect("valid map");
        let origin = Vec3::new(30.0, 0.0, 30.0);
        let mut impact = he_impact(origin);
        impact.direction = Vec3::new(0.0, -0.4, 0.92).normalize();
        let mut scars = TerrainScars::default();
        scars.record(&impact, &map);
        let mut vertices = Vec::new();
        scars.append_quads(&mut vertices);

        // Rim cells first, bowl cells second: the rim must be visibly lighter.
        let luma = |v: &renderer_api::FxVertex| v.color[0] + v.color[1] + v.color[2];
        assert!(
            luma(&vertices[0]) > luma(&vertices[DRAPE_CELLS * 6]) * 1.5,
            "fresh-earth rim outshines the scorched bowl"
        );

        // Clods (after both draped stamps): the field's centroid sits downrange (+Z).
        let clods = &vertices[2 * DRAPE_CELLS * 6..];
        let centroid_z = clods.iter().map(|v| v.position[2]).sum::<f32>() / clods.len() as f32;
        assert!(
            centroid_z > origin.z,
            "the spoil centroid flies downrange, got z {centroid_z} vs strike {}",
            origin.z
        );
    }

    #[test]
    fn the_crater_leans_with_the_slope() {
        // Height rises 1:1 along x, so the true normal is (-1, 1, 0) normalized.
        let samples: Vec<f32> = (0..65 * 65).map(|index| (index % 65) as f32).collect();
        let map = HeightMap::new(65, 65, 1.0, samples).expect("valid map");
        let mut scars = TerrainScars::default();
        scars.record(&he_impact(Vec3::new(30.0, 31.0, 30.0)), &map);

        let normal = Vec3::new(-1.0, 1.0, 0.0).normalize();
        let plane_point = Vec3::new(30.0, 30.0, 30.0) + normal * GROUND_LIFT_M;
        let mut vertices = Vec::new();
        scars.append_quads(&mut vertices);
        assert!(!vertices.is_empty());
        for vertex in &vertices {
            let off_plane = (Vec3::from_array(vertex.position) - plane_point).dot(normal);
            assert!(
                (-1.0e-2..=GROUND_LIFT_M * 0.6 + 1.0e-2).contains(&off_plane),
                "every stamp hugs the slope's plane inside the lift band, got {off_plane}"
            );
        }
    }

    /// P2's contract: a kinetic bolt PLOUGHS — every furrow stamp lies along the flight
    /// track, the spoil flies only DOWNRANGE, the mark scales with calibre, and an old
    /// snapshot without a direction degrades to the radial mark instead of guessing.
    #[test]
    fn a_kinetic_round_ploughs_a_furrow_along_its_track() {
        let map = HeightMap::flat(65, 65, 1.0, 0.0).expect("valid map");
        let origin = Vec3::new(30.0, 0.0, 30.0);

        let mut scars = TerrainScars::default();
        scars.record(&ap_impact(origin, 100.0), &map);
        let mut vertices = Vec::new();
        scars.append_quads(&mut vertices);
        assert!(!vertices.is_empty());
        // Every stamp's centroid sits downrange (+Z) or on the strike point — never behind.
        let count = vertices.len() / 6;
        for stamp in 0..count {
            let quad = &vertices[stamp * 6..stamp * 6 + 6];
            let centroid_z = quad.iter().map(|v| v.position[2]).sum::<f32>() / quad.len() as f32;
            assert!(
                centroid_z >= origin.z - 0.3,
                "spoil flies FORWARD along the track, got centroid z {centroid_z}"
            );
        }
        // The gouge is elongated along +Z: the batch spans far more track than width.
        let (mut min_x, mut max_x, mut min_z, mut max_z) = (f32::MAX, f32::MIN, f32::MAX, f32::MIN);
        for vertex in &vertices {
            min_x = min_x.min(vertex.position[0]);
            max_x = max_x.max(vertex.position[0]);
            min_z = min_z.min(vertex.position[2]);
            max_z = max_z.max(vertex.position[2]);
        }
        assert!(
            (max_z - min_z) > (max_x - min_x) * 1.3,
            "the furrow runs along the track: dz {} vs dx {}",
            max_z - min_z,
            max_x - min_x
        );

        // Calibre scales the mark monotonically.
        let span = |caliber: f32| {
            let mut scars = TerrainScars::default();
            scars.record(&ap_impact(origin, caliber), &map);
            let mut v = Vec::new();
            scars.append_quads(&mut v);
            v.iter().map(|x| x.position[2]).fold(f32::MIN, f32::max)
                - v.iter().map(|x| x.position[2]).fold(f32::MAX, f32::min)
        };
        assert!(span(122.0) > span(76.0), "a 122 mm furrow outsizes a 76 mm one");

        // No direction on the wire (old snapshot) = the radial mark, not a guessed furrow.
        let mut legacy = ap_impact(origin, 100.0);
        legacy.direction = Vec3::ZERO;
        let mut scars = TerrainScars::default();
        scars.record(&legacy, &map);
        let mut v = Vec::new();
        scars.append_quads(&mut v);
        assert!(!v.is_empty(), "a directionless kinetic impact still marks the ground");
    }

    #[test]
    fn the_pool_is_budgeted_and_recycles_the_oldest_crater() {
        let map = HeightMap::flat(65, 65, 1.0, 0.0).expect("valid map");
        let mut scars = TerrainScars::default();
        scars.record(&he_impact(Vec3::new(1.0, 0.0, 1.0)), &map);
        scars.tick(0.1); // the first crater is now the oldest
        for index in 0..MAX_TERRAIN_SCARS {
            scars.record(&he_impact(Vec3::new(2.0 + index as f32 * 0.3, 0.0, 5.0)), &map);
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
        scars.record(&he_impact(Vec3::new(10.0, 0.0, 10.0)), &map);

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
