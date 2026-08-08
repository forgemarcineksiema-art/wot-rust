//! Deterministic per-role pixel synthesis for the material families. Split from `texture_maps` so
//! each file stays within the review budget: this module owns the map kinds, the per-role parameter
//! profiles and the value-noise helpers; `texture_maps` owns the family enum, manifest and bake loop.

use super::texture_maps::MaterialFamily;

/// The four PBR-lite maps every family carries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum MapKind {
    Albedo,
    Normal,
    AoRoughnessMetalness,
    Cavity,
}

impl MapKind {
    pub(super) const ALL: [MapKind; 4] =
        [MapKind::Albedo, MapKind::Normal, MapKind::AoRoughnessMetalness, MapKind::Cavity];

    pub(super) fn semantic(self) -> &'static str {
        match self {
            MapKind::Albedo => "albedo",
            MapKind::Normal => "normal",
            MapKind::AoRoughnessMetalness => "ao_roughness_metalness",
            MapKind::Cavity => "cavity",
        }
    }

    pub(super) fn channels(self) -> &'static str {
        match self {
            MapKind::Albedo => "rgba",
            MapKind::Normal => "xyz",
            MapKind::AoRoughnessMetalness => "ao,roughness,metalness,unused",
            MapKind::Cavity => "cavity,cavity,cavity,unused",
        }
    }
}

/// Per-role synthesis parameters. Albedo is a near-neutral detail multiplier, not a second absolute
/// base colour: the shader already owns each role's physical base albedo. Keeping ALBEDO near white
/// prevents accidental albedo-squared black surfaces — that reasoning is sound and unchanged.
///
/// What was wrong was applying it to the SURFACE-CHARACTER amplitudes as well. Until 2026-08-08
/// `normal_jitter` ran at 3/255 on rolled armour and 5/255 on cast: a **1.2% normal perturbation**,
/// with the two armour families separated by 2/255. The comment here claimed grain, normals,
/// roughness and cavity "still distinguish cast armour, plate, painted barrel steel, tracks and
/// rubber". At those amplitudes they did not distinguish anything, and that — not any missing
/// capability — is why the whole fleet read as one flat plastic tone. Normal mapping was wired
/// end to end the entire time; it was turned down to nothing.
///
/// Grain, undulation, normal jitter and cavity are now x4 their old values, chosen off a rendered
/// ladder (x1 / x3 / x6 / x10 on `t54_studio`) rather than from taste: x1 is invisible, x10 turns
/// the cast turret into pumice, and the useful band is x3–x6. Measured at x4 the turret casting
/// reads as a casting and the plate reads as painted steel.
///
/// `vehicle_forge/tests/material_floor.rs` is the lock. It exists because this failure mode is
/// silent: a material tuned to invisibility renders, passes every test, and looks for all the
/// world like a modelling problem rather than a knob turned down.
pub(super) struct Profile {
    albedo: [u8; 3],
    fine_grain: u8,
    undulation: u8,
    roughness: u8,
    /// How far the finish wanders across the surface, +-this many levels. Zero is a uniform
    /// gloss, which is what every family shipped with until 2026-08-08: measured span 0 in all
    /// of them, so a pitted casting and a polished track face answered the sun identically no
    /// matter how much normal or cavity detail they carried.
    roughness_amp: u8,
    metalness: u8,
    cavity_base: u8,
    cavity_amp: u8,
    normal_jitter: u8,
}

pub(super) fn profile(family: MaterialFamily) -> Profile {
    match family {
        MaterialFamily::RolledArmor => Profile {
            albedo: [244, 245, 246],
            fine_grain: 12,
            undulation: 8,
            roughness: 145,
            roughness_amp: 14,
            metalness: 10,
            cavity_base: 246,
            cavity_amp: 24,
            normal_jitter: 12,
        },
        MaterialFamily::CastArmor => Profile {
            albedo: [240, 242, 243],
            fine_grain: 8,
            undulation: 28,
            roughness: 170,
            roughness_amp: 22,
            metalness: 8,
            cavity_base: 248,
            cavity_amp: 12,
            normal_jitter: 20,
        },
        MaterialFamily::BarrelSteel => Profile {
            albedo: [232, 234, 236],
            fine_grain: 8,
            undulation: 4,
            roughness: 175,
            roughness_amp: 12,
            metalness: 12,
            cavity_base: 248,
            cavity_amp: 12,
            normal_jitter: 8,
        },
        MaterialFamily::TrackMetal => Profile {
            albedo: [224, 222, 218],
            fine_grain: 20,
            undulation: 12,
            roughness: 200,
            roughness_amp: 44,
            metalness: 128,
            cavity_base: 230,
            cavity_amp: 64,
            normal_jitter: 24,
        },
        MaterialFamily::Rubber => Profile {
            albedo: [228, 228, 230],
            fine_grain: 12,
            undulation: 4,
            roughness: 225,
            roughness_amp: 10,
            metalness: 0,
            cavity_base: 238,
            cavity_amp: 32,
            normal_jitter: 8,
        },
        // The three below are authored at CALIBRATED amplitudes from the start — roughly what the
        // x4 pass gives the five above — because they have no legacy look to preserve. If the two
        // land in either order the fleet ends up consistent; before that, canvas/glass/timber are
        // simply the only surfaces on the vehicle carrying their real character.
        //
        // Proofed duck: a woven weave is high-frequency and near-isotropic, the cloth drapes
        // rather than undulating, and it has no specular lobe worth speaking of.
        MaterialFamily::Canvas => Profile {
            albedo: [236, 235, 232],
            fine_grain: 18,
            undulation: 8,
            roughness: 248,
            roughness_amp: 12,
            metalness: 0,
            cavity_base: 232,
            cavity_amp: 26,
            normal_jitter: 18,
        },
        // Glass is the ONE surface on a tank that is supposed to be smooth, and it still has to
        // clear the material floor — which is the right answer rather than the first exemption in
        // a rule whose whole point is that it has none. A headlight lens is PRESSED, not polished
        // flat: moulded glass ripples, and that ripple is what walks a highlight across the lens
        // instead of parking it. `roughness: 28` is what makes this read as glass; the jitter only
        // has to be enough to be seen (measured mean tilt 2.5/255 against a floor of 1.8).
        MaterialFamily::Glass => Profile {
            albedo: [246, 248, 250],
            fine_grain: 2,
            undulation: 4,
            roughness: 28,
            roughness_amp: 6,
            metalness: 24,
            cavity_base: 250,
            cavity_amp: 12,
            normal_jitter: 10,
        },
        // Sawn timber: a fibrous surface, matte, its grain carried by the NORMAL rather than by a
        // brown albedo. The warmth belongs to `material_params`, which is what this table means by
        // "a near-neutral detail multiplier, not a second absolute base colour" — a lesson learned
        // by writing [230, 224, 214] here and watching `must not crush diffuse` catch it.
        MaterialFamily::Timber => Profile {
            albedo: [238, 236, 232],
            fine_grain: 12,
            undulation: 12,
            roughness: 236,
            roughness_amp: 26,
            metalness: 0,
            cavity_base: 226,
            cavity_amp: 30,
            normal_jitter: 22,
        },
    }
}

pub(super) fn pixel(p: &Profile, kind: MapKind, x: u32, y: u32) -> [u8; 4] {
    match kind {
        MapKind::Albedo => {
            let v = grain(x, y, p.fine_grain) as i32 + undulation(x, y, p.undulation);
            [shift(p.albedo[0], v), shift(p.albedo[1], v), shift(p.albedo[2], v), 255]
        }
        MapKind::Normal => {
            let jx = grain(x, y, p.normal_jitter) as i32 - p.normal_jitter as i32 / 2;
            let jy = grain(y, x, p.normal_jitter) as i32 - p.normal_jitter as i32 / 2;
            [shift(128, jx), shift(128, jy), 255, 255]
        }
        MapKind::AoRoughnessMetalness => {
            let ao = shift(238, -(grain(x, y, 4) as i32));
            // Broad patches of differently-worn finish, not fine noise: a track's contact faces
            // polish while its webs stay rough, and paint weathers in areas rather than pixels.
            // Offset from the albedo lattice so the two do not wander in lockstep, which reads as
            // one printed pattern rather than two independent properties of the surface.
            let rough = shift(p.roughness, undulation(x + 97, y + 61, p.roughness_amp));
            [ao, rough, p.metalness, 255]
        }
        MapKind::Cavity => {
            let c = shift(p.cavity_base, -(grain(x, y, p.cavity_amp) as i32));
            [c, c, c, 255]
        }
    }
}

/// Saturating signed offset of a base channel value.
fn shift(base: u8, delta: i32) -> u8 {
    (base as i32 + delta).clamp(0, 255) as u8
}

/// Fine high-frequency grain in `0..=amplitude`, deterministic in `(x, y)`.
fn grain(x: u32, y: u32, amplitude: u8) -> u8 {
    if amplitude == 0 {
        return 0;
    }
    let mut value = x.wrapping_mul(1_103_515_245) ^ y.wrapping_mul(12_345);
    value ^= value >> 16;
    (value % (u32::from(amplitude) + 1)) as u8
}

/// Low-frequency smooth undulation in `-amplitude..=amplitude` (bilinear value noise on a coarse
/// lattice) — the slow waviness of a sand-cast wall, distinct from the fine grain.
fn undulation(x: u32, y: u32, amplitude: u8) -> i32 {
    if amplitude == 0 {
        return 0;
    }
    const CELL: f32 = 48.0;
    let (gx, gy) = (x as f32 / CELL, y as f32 / CELL);
    let (x0, y0) = (gx.floor() as i32, gy.floor() as i32);
    let (fx, fy) = (gx - x0 as f32, gy - y0 as f32);
    let (sx, sy) = (smooth(fx), smooth(fy));
    let mix = |a: f32, b: f32, t: f32| a + (b - a) * t;
    let n0 = mix(lattice(x0, y0), lattice(x0 + 1, y0), sx);
    let n1 = mix(lattice(x0, y0 + 1), lattice(x0 + 1, y0 + 1), sx);
    (mix(n0, n1, sy) * amplitude as f32).round() as i32
}

fn lattice(x: i32, y: i32) -> f32 {
    let mut h = (x as u32).wrapping_mul(0x27d4_eb2d) ^ (y as u32).wrapping_mul(0x1656_67b1);
    h ^= h >> 15;
    h = h.wrapping_mul(0x2c1b_3c6d);
    h ^= h >> 12;
    (h & 0xffff) as f32 / 32_767.5 - 1.0
}

fn smooth(t: f32) -> f32 {
    t * t * (3.0 - 2.0 * t)
}
