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

/// Per-role synthesis parameters. These encode the plan's material character: cast armour gets a
/// low-frequency undulation with muted cavity, rolled armour a finer plate grain, steel a smoother
/// (lower-roughness) surface, track metal a dark high-cavity finish, rubber very dark and rough.
pub(super) struct Profile {
    albedo: [u8; 3],
    fine_grain: u8,
    undulation: u8,
    roughness: u8,
    metalness: u8,
    cavity_base: u8,
    cavity_amp: u8,
    normal_jitter: u8,
}

pub(super) fn profile(family: MaterialFamily) -> Profile {
    match family {
        MaterialFamily::RolledArmor => Profile {
            albedo: [108, 114, 119],
            fine_grain: 7,
            undulation: 3,
            roughness: 140,
            metalness: 230,
            cavity_base: 236,
            cavity_amp: 8,
            normal_jitter: 4,
        },
        MaterialFamily::CastArmor => Profile {
            albedo: [116, 119, 122],
            fine_grain: 4,
            undulation: 14,
            roughness: 184,
            metalness: 228,
            cavity_base: 246,
            cavity_amp: 3,
            normal_jitter: 7,
        },
        MaterialFamily::BarrelSteel => Profile {
            albedo: [40, 42, 45],
            fine_grain: 3,
            undulation: 1,
            roughness: 78,
            metalness: 255,
            cavity_base: 248,
            cavity_amp: 3,
            normal_jitter: 2,
        },
        MaterialFamily::TrackMetal => Profile {
            albedo: [30, 30, 33],
            fine_grain: 9,
            undulation: 6,
            roughness: 216,
            metalness: 200,
            cavity_base: 196,
            cavity_amp: 34,
            normal_jitter: 9,
        },
        MaterialFamily::Rubber => Profile {
            albedo: [15, 15, 17],
            fine_grain: 5,
            undulation: 2,
            roughness: 244,
            metalness: 0,
            cavity_base: 232,
            cavity_amp: 9,
            normal_jitter: 3,
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
            [ao, p.roughness, p.metalness, 255]
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
