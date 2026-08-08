//! Renderer-neutral, deterministic default vehicle material pixels.
//!
//! A clean runtime checkout has no `target/forge` tree, but it should still render the same five
//! physical material roles as a baked Forge artifact. Keep the source pixels here, next to the
//! Forge synthesis rules, and cache them once per process. The client can then upload one shared
//! texture-array material for every vehicle that has no artifact-specific override.

use std::sync::OnceLock;

use super::material_synthesis::{MapKind, Profile, pixel, profile};
use super::texture_maps::MaterialFamily;

/// Edge of every generated default map.
pub const DEFAULT_MATERIAL_MAP_SIZE: u32 = 256;

/// One tightly packed RGBA8 map, independent of renderer and image-codec types.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DefaultMaterialMap {
    width: u32,
    height: u32,
    rgba: Vec<u8>,
}

impl DefaultMaterialMap {
    pub fn width(&self) -> u32 {
        self.width
    }

    pub fn height(&self) -> u32 {
        self.height
    }

    pub fn rgba(&self) -> &[u8] {
        &self.rgba
    }
}

/// The four PBR-lite maps for one material-role layer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DefaultMaterialFamily {
    family: MaterialFamily,
    albedo: DefaultMaterialMap,
    normal: DefaultMaterialMap,
    ao_roughness_metalness: DefaultMaterialMap,
    cavity: DefaultMaterialMap,
}

impl DefaultMaterialFamily {
    pub fn family(&self) -> MaterialFamily {
        self.family
    }

    pub fn albedo(&self) -> &DefaultMaterialMap {
        &self.albedo
    }

    pub fn normal(&self) -> &DefaultMaterialMap {
        &self.normal
    }

    pub fn ao_roughness_metalness(&self) -> &DefaultMaterialMap {
        &self.ao_roughness_metalness
    }

    pub fn cavity(&self) -> &DefaultMaterialMap {
        &self.cavity
    }

    pub(super) fn map(&self, kind: MapKind) -> &DefaultMaterialMap {
        match kind {
            MapKind::Albedo => self.albedo(),
            MapKind::Normal => self.normal(),
            MapKind::AoRoughnessMetalness => self.ao_roughness_metalness(),
            MapKind::Cavity => self.cavity(),
        }
    }
}

static DEFAULT_MATERIAL_FAMILIES: OnceLock<[DefaultMaterialFamily; 5]> = OnceLock::new();

/// The five material families in renderer `material_id` order, generated only once per process.
pub fn default_material_families() -> &'static [DefaultMaterialFamily; 5] {
    DEFAULT_MATERIAL_FAMILIES.get_or_init(|| MaterialFamily::ALL.map(bake_family))
}

pub(super) fn default_material_family(family: MaterialFamily) -> &'static DefaultMaterialFamily {
    &default_material_families()[family.layer() as usize]
}

fn bake_family(family: MaterialFamily) -> DefaultMaterialFamily {
    let profile = profile(family);
    DefaultMaterialFamily {
        family,
        albedo: bake_map(&profile, MapKind::Albedo),
        normal: bake_map(&profile, MapKind::Normal),
        ao_roughness_metalness: bake_map(&profile, MapKind::AoRoughnessMetalness),
        cavity: bake_map(&profile, MapKind::Cavity),
    }
}

fn bake_map(profile: &Profile, kind: MapKind) -> DefaultMaterialMap {
    let mut rgba =
        Vec::with_capacity((DEFAULT_MATERIAL_MAP_SIZE * DEFAULT_MATERIAL_MAP_SIZE * 4) as usize);
    for y in 0..DEFAULT_MATERIAL_MAP_SIZE {
        for x in 0..DEFAULT_MATERIAL_MAP_SIZE {
            rgba.extend_from_slice(&pixel(profile, kind, x, y));
        }
    }
    DefaultMaterialMap { width: DEFAULT_MATERIAL_MAP_SIZE, height: DEFAULT_MATERIAL_MAP_SIZE, rgba }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_pixels_are_cached_and_cover_all_five_roles() {
        let first = default_material_families();
        let second = default_material_families();

        assert!(std::ptr::eq(first, second), "the process must reuse one pixel source");
        assert_eq!(first.len(), MaterialFamily::ALL.len());
        for (layer, family) in first.iter().enumerate() {
            assert_eq!(family.family().layer() as usize, layer);
            for map in
                [family.albedo(), family.normal(), family.ao_roughness_metalness(), family.cavity()]
            {
                assert_eq!(map.width(), DEFAULT_MATERIAL_MAP_SIZE);
                assert_eq!(map.height(), DEFAULT_MATERIAL_MAP_SIZE);
                assert_eq!(
                    map.rgba().len(),
                    (DEFAULT_MATERIAL_MAP_SIZE * DEFAULT_MATERIAL_MAP_SIZE * 4) as usize
                );
            }
        }
    }

    #[test]
    fn roles_and_map_semantics_are_not_flat_duplicates() {
        let families = default_material_families();
        let rolled = &families[MaterialFamily::RolledArmor.layer() as usize];
        let cast = &families[MaterialFamily::CastArmor.layer() as usize];
        let rubber = &families[MaterialFamily::Rubber.layer() as usize];

        assert_ne!(rolled.albedo().rgba(), cast.albedo().rgba());
        assert_ne!(rolled.normal().rgba(), cast.normal().rgba());
        assert_ne!(rolled.ao_roughness_metalness().rgba(), rubber.ao_roughness_metalness().rgba());
        assert_ne!(rolled.albedo().rgba(), rolled.normal().rgba());
        assert_ne!(rolled.normal().rgba(), rolled.cavity().rgba());
    }

    #[test]
    fn material_values_stay_readable_neutral_and_physically_bounded() {
        for family in default_material_families() {
            let albedo_pixels: Vec<&[u8]> = family.albedo().rgba().chunks_exact(4).collect();
            let darkest = albedo_pixels
                .iter()
                .flat_map(|pixel| &pixel[..3])
                .copied()
                .min()
                .expect("albedo pixels");
            let strongest_dominant = albedo_pixels
                .iter()
                .map(|pixel| {
                    let (lo, hi) = pixel[..3].iter().fold((u8::MAX, u8::MIN), |(lo, hi), value| {
                        (lo.min(*value), hi.max(*value))
                    });
                    hi - lo
                })
                .max()
                .expect("albedo chroma");
            let mean = albedo_pixels
                .iter()
                .flat_map(|pixel| &pixel[..3])
                .map(|value| u64::from(*value))
                .sum::<u64>()
                / (albedo_pixels.len() * 3) as u64;

            assert!(darkest >= 205, "{:?} multiplier must not crush diffuse", family.family());
            assert!(mean >= 220, "{:?} multiplier mean is too dark: {mean}", family.family());
            assert!(
                strongest_dominant <= 10,
                "{:?} albedo has an implausible RGB dominant",
                family.family()
            );
            // The invariant is that this stays a VALID tangent-space normal: +Z dominant, tilt
            // inside a plausible angle for a surface treatment. It is not a licence to decide how
            // much surface character a material may have.
            //
            // It used to be `(122..=134)` on X and Y — a +-6/255 window, about 2.7 degrees of
            // tilt. That is not a physical bound, it is a look decision wearing a test's clothes,
            // and it was the thing holding the fleet flat: raising the synthesis amplitudes to
            // where a casting reads as a casting fails it immediately. Widened to +-48 (~20.6
            // degrees), which is still unmistakably a gentle detail normal and no longer decides
            // the art direction. `tests/material_floor.rs` holds the other end.
            assert!(
                family.normal().rgba().chunks_exact(4).all(|p| {
                    (80..=176).contains(&p[0])
                        && (80..=176).contains(&p[1])
                        && p[2] >= p[0].max(p[1])
                }),
                "{:?} normal must remain a valid +Z-dominant tangent-space perturbation",
                family.family()
            );
            // Same correction as the normal bound above: what this must protect is that a crevice
            // never crushes to black, so indirect light still reaches it. `>= 210` allowed a
            // crevice to darken by at most 18% — not a physical statement, a look decision, and
            // the one that kept worn track steel looking freshly cast. 140/255 keeps the honest
            // half of it: no cavity map may take a surface below ~55% brightness.
            assert!(
                family.cavity().rgba().chunks_exact(4).all(|p| p[0] >= 140),
                "{:?} cavity must retain readable indirect light",
                family.family()
            );
        }
    }
}
