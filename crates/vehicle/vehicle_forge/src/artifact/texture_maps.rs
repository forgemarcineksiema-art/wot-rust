//! Deterministic role-aware material families baked into the artifact.
//!
//! One generic grain texture cannot make a cast turret, a welded hull, a steel barrel, tracks and
//! rubber read as different physical surfaces. So the Forge bakes a *family* per material role — each
//! with its own albedo, tangent-space normal, packed AO/roughness/metalness and cavity — synthesised
//! procedurally (no external assets) and reproducibly. The renderer stacks the families into texture
//! arrays and selects the layer by `material_id`, so a vehicle stays one mesh/material binding.

use std::io;

use serde::{Deserialize, Serialize};

use super::default_materials::default_material_family;
use super::material_synthesis::MapKind;

/// A material role family. The discriminant **is** the renderer's `material_id` / texture-array
/// layer, so order here is load-bearing and must match `material_role_id` in the client.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MaterialFamily {
    RolledArmor,
    CastArmor,
    BarrelSteel,
    TrackMetal,
    Rubber,
}

impl MaterialFamily {
    /// Every family in `material_id` order (layer 0..=4).
    pub const ALL: [MaterialFamily; 5] = [
        MaterialFamily::RolledArmor,
        MaterialFamily::CastArmor,
        MaterialFamily::BarrelSteel,
        MaterialFamily::TrackMetal,
        MaterialFamily::Rubber,
    ];

    /// Stable lowercase slug used in baked filenames and the manifest role field.
    pub fn slug(self) -> &'static str {
        match self {
            MaterialFamily::RolledArmor => "rolled_armor",
            MaterialFamily::CastArmor => "cast_armor",
            MaterialFamily::BarrelSteel => "barrel_steel",
            MaterialFamily::TrackMetal => "track_metal",
            MaterialFamily::Rubber => "rubber",
        }
    }

    /// The renderer layer index this family occupies.
    pub fn layer(self) -> u32 {
        Self::ALL.iter().position(|f| *f == self).unwrap() as u32
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ForgeTextureManifest {
    file: String,
    /// The material role this map belongs to (e.g. `cast_armor`).
    role: String,
    semantic: String,
    channels: String,
    bytes: usize,
}

impl ForgeTextureManifest {
    pub fn file(&self) -> &str {
        &self.file
    }

    pub fn role(&self) -> &str {
        &self.role
    }

    pub fn semantic(&self) -> &str {
        &self.semantic
    }

    pub fn channels(&self) -> &str {
        &self.channels
    }

    pub fn bytes(&self) -> usize {
        self.bytes
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BakedTextureMap {
    manifest: ForgeTextureManifest,
    bytes: Vec<u8>,
}

impl BakedTextureMap {
    pub(super) fn from_loaded(manifest: ForgeTextureManifest, bytes: Vec<u8>) -> Self {
        Self { manifest, bytes }
    }

    pub fn manifest(&self) -> &ForgeTextureManifest {
        &self.manifest
    }

    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
}

/// Bake the full role-aware set: every family × every map (20 PNGs), in `material_id` layer order.
pub fn bake_default_set() -> Result<Vec<BakedTextureMap>, io::Error> {
    let mut maps = Vec::with_capacity(MaterialFamily::ALL.len() * MapKind::ALL.len());
    for family in MaterialFamily::ALL {
        for kind in MapKind::ALL {
            maps.push(bake_family_map(family, kind)?);
        }
    }
    Ok(maps)
}

fn bake_family_map(family: MaterialFamily, kind: MapKind) -> Result<BakedTextureMap, io::Error> {
    let map = default_material_family(family).map(kind);
    let bytes = encode_png(map.width(), map.height(), map.rgba())?;
    let manifest = ForgeTextureManifest {
        file: format!("{}_{}.png", family.slug(), kind.semantic()),
        role: family.slug().to_string(),
        semantic: kind.semantic().to_string(),
        channels: kind.channels().to_string(),
        bytes: bytes.len(),
    };
    Ok(BakedTextureMap { manifest, bytes })
}

fn encode_png(width: u32, height: u32, rgba: &[u8]) -> Result<Vec<u8>, io::Error> {
    let mut bytes = Vec::new();
    let mut encoder = png::Encoder::new(&mut bytes, width, height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.write_header()?.write_image_data(rgba)?;
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mean_luma(map: &BakedTextureMap) -> f32 {
        let decoder = png::Decoder::new(std::io::Cursor::new(map.bytes()));
        let mut reader = decoder.read_info().expect("png header");
        let mut buf = vec![0; reader.output_buffer_size().expect("buffer size")];
        let info = reader.next_frame(&mut buf).expect("png frame");
        let px = &buf[..info.buffer_size()];
        let sum: f64 = px
            .chunks_exact(4)
            .map(|p| 0.299 * p[0] as f64 + 0.587 * p[1] as f64 + 0.114 * p[2] as f64)
            .sum();
        (sum / (px.len() / 4) as f64) as f32
    }

    fn map_of(maps: &[BakedTextureMap], file: &str) -> BakedTextureMap {
        maps.iter().find(|m| m.manifest().file() == file).expect("map exists").clone()
    }

    #[test]
    fn the_family_set_is_five_roles_times_four_maps() {
        let maps = bake_default_set().expect("maps bake");
        assert_eq!(maps.len(), 20);
        for family in MaterialFamily::ALL {
            for semantic in ["albedo", "normal", "ao_roughness_metalness", "cavity"] {
                let file = format!("{}_{}.png", family.slug(), semantic);
                let map = map_of(&maps, &file);
                assert_eq!(map.manifest().role(), family.slug(), "{file} tagged with its role");
            }
        }
    }

    #[test]
    fn every_map_is_a_clean_square_png_at_least_256px() {
        for map in bake_default_set().expect("maps bake") {
            let decoder = png::Decoder::new(std::io::Cursor::new(map.bytes()));
            let reader = decoder.read_info().expect("png header");
            assert!(reader.info().width >= 256 && reader.info().width == reader.info().height);
        }
    }

    #[test]
    fn roles_read_as_distinct_surfaces() {
        let maps = bake_default_set().expect("maps bake");
        let albedo =
            |f: MaterialFamily| mean_luma(&map_of(&maps, &format!("{}_albedo.png", f.slug())));
        // Albedo is a near-white detail multiplier; role identity stays subtle here and is carried
        // more strongly by the normal, roughness and cavity maps.
        assert!(albedo(MaterialFamily::RolledArmor) > albedo(MaterialFamily::TrackMetal));
        assert!(albedo(MaterialFamily::TrackMetal) > 210.0, "track multiplier stays readable");
        assert!(
            albedo(MaterialFamily::RolledArmor) - albedo(MaterialFamily::TrackMetal) < 35.0,
            "role multipliers must remain subtle"
        );
    }

    #[test]
    fn the_bake_is_deterministic() {
        let a = bake_default_set().expect("a");
        let b = bake_default_set().expect("b");
        assert_eq!(a, b, "material families must bake byte-identically every run");
    }

    #[test]
    fn the_family_layer_order_matches_material_id() {
        assert_eq!(MaterialFamily::RolledArmor.layer(), 0);
        assert_eq!(MaterialFamily::CastArmor.layer(), 1);
        assert_eq!(MaterialFamily::BarrelSteel.layer(), 2);
        assert_eq!(MaterialFamily::TrackMetal.layer(), 3);
        assert_eq!(MaterialFamily::Rubber.layer(), 4);
    }
}
