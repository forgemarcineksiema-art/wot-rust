//! Imported flora (Flora 2.0, FL-3): the DATA side of the CC0 import pipeline. An asset is a
//! textured mesh (positions/normals/UVs into its own texture) plus its PROVENANCE — a license
//! record naming the author and source. Validation is the gate: an asset without a CC0
//! license, over the triangle budget, off its ground plane or with out-of-range UVs never
//! becomes a file. The tool (`tools import-flora`) writes the pair
//! `<name>.flora.json` + `<name>.flora.png`; consumers pack the textures into the runtime
//! foliage atlas (FL-2) and remap the UVs per region.

use serde::{Deserialize, Serialize};

/// The one license the pipeline accepts. CC-BY and friends are deliberately refused:
/// attribution management in a commercial build is a liability the project does not take on
/// (urban-map doctrine decision 10).
pub const ACCEPTED_SPDX: &str = "CC0-1.0";

/// Per-asset triangle ceiling: imported foliage competes with the baked procedural trees
/// (LOD1 budget) and lives many-per-map — a downloaded 40k-tri showpiece is a decimation
/// task, not an asset.
pub const FLORA_MAX_TRIS: usize = 1500;

/// The provenance record — part of the asset, not a side note.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FloraLicense {
    /// SPDX identifier; must equal [`ACCEPTED_SPDX`].
    pub spdx: String,
    pub author: String,
    pub source_url: String,
}

/// The imported, normalized asset: grounded at y = 0, recentred in XZ, UVs in [0, 1] over its
/// OWN texture (the atlas packer remaps them at load).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FloraAsset {
    /// Asset name (kebab-case file stem).
    pub name: String,
    pub license: FloraLicense,
    /// The sibling texture file (relative, e.g. `birch-a.flora.png`), tight RGBA8.
    pub texture_file: String,
    pub texture_width: u32,
    pub texture_height: u32,
    pub positions: Vec<[f32; 3]>,
    pub normals: Vec<[f32; 3]>,
    pub uvs: Vec<[f32; 2]>,
    /// Per-vertex tint baked from the source materials' base_color_factor: these packs paint
    /// with factor × shared gradient texture, and the runtime reconstructs exactly that
    /// (vertex color × sampled texel).
    pub colors: Vec<[f32; 3]>,
    pub indices: Vec<u32>,
    /// Canopy top over the ground plane, metres.
    pub height_m: f32,
}

impl FloraAsset {
    pub fn triangle_count(&self) -> usize {
        self.indices.len() / 3
    }

    /// THE import gate: every reason a downloaded model is refused, spelled out. The tool
    /// runs this before writing; consumers may re-run it on load (a hand-edited file gets
    /// the same scrutiny as a fresh import).
    pub fn validate(&self) -> Result<(), String> {
        if self.license.spdx != ACCEPTED_SPDX {
            return Err(format!(
                "license {} refused: only {ACCEPTED_SPDX} enters the pipeline (attribution \
                 management is a liability, not a feature)",
                self.license.spdx
            ));
        }
        if self.license.author.trim().is_empty() || self.license.source_url.trim().is_empty() {
            return Err("provenance incomplete: author and source_url are required".to_string());
        }
        if self.name.trim().is_empty() {
            return Err("the asset needs a name".to_string());
        }
        let vertex_count = self.positions.len();
        if vertex_count == 0 || self.indices.len() < 3 || !self.indices.len().is_multiple_of(3) {
            return Err("the mesh must carry whole triangles".to_string());
        }
        if self.normals.len() != vertex_count
            || self.uvs.len() != vertex_count
            || self.colors.len() != vertex_count
        {
            return Err(format!(
                "attribute streams disagree: {} positions, {} normals, {} uvs, {} colors",
                vertex_count,
                self.normals.len(),
                self.uvs.len(),
                self.colors.len()
            ));
        }
        for color in &self.colors {
            if color.iter().any(|channel| !(0.0..=1.0).contains(channel)) {
                return Err(format!("vertex tint {color:?} outside [0, 1]"));
            }
        }
        if self.triangle_count() > FLORA_MAX_TRIS {
            return Err(format!(
                "{} triangles over the {FLORA_MAX_TRIS} budget: decimate the source, do not \
                 raise the ceiling",
                self.triangle_count()
            ));
        }
        if let Some(bad) = self.indices.iter().find(|&&index| index as usize >= vertex_count) {
            return Err(format!("index {bad} out of range ({vertex_count} vertices)"));
        }
        for uv in &self.uvs {
            if !(0.0..=1.0).contains(&uv[0]) || !(0.0..=1.0).contains(&uv[1]) {
                return Err(format!(
                    "uv {uv:?} outside [0, 1]: the atlas packer remaps whole regions, \
                     not wrapped coordinates"
                ));
            }
        }
        let min_y = self.positions.iter().map(|p| p[1]).fold(f32::INFINITY, f32::min);
        let max_y = self.positions.iter().map(|p| p[1]).fold(f32::NEG_INFINITY, f32::max);
        if min_y < -0.05 {
            return Err(format!("the asset floats below its ground plane (min y {min_y:.3})"));
        }
        if !(0.3..=25.0).contains(&self.height_m) || (max_y - self.height_m).abs() > 0.1 {
            return Err(format!(
                "height {:.2} m disagrees with the mesh top {max_y:.2} m or leaves the sane \
                 0.3-25 m flora range",
                self.height_m
            ));
        }
        if self.texture_width == 0 || self.texture_height == 0 {
            return Err("the texture must have real dimensions".to_string());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sane() -> FloraAsset {
        FloraAsset {
            name: "test-bush".to_string(),
            license: FloraLicense {
                spdx: ACCEPTED_SPDX.to_string(),
                author: "Someone".to_string(),
                source_url: "https://example.org/bush".to_string(),
            },
            texture_file: "test-bush.flora.png".to_string(),
            texture_width: 64,
            texture_height: 64,
            positions: vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.5, 1.2, 0.0]],
            normals: vec![[0.0, 0.0, 1.0]; 3],
            uvs: vec![[0.0, 1.0], [1.0, 1.0], [0.5, 0.0]],
            colors: vec![[0.4, 0.7, 0.3]; 3],
            indices: vec![0, 1, 2],
            height_m: 1.2,
        }
    }

    /// The license gate is the point of the pipeline: CC0 passes, everything else is refused
    /// BY NAME, and missing provenance is as fatal as a wrong license.
    #[test]
    fn only_cc0_with_full_provenance_enters() {
        assert!(sane().validate().is_ok());
        let mut wrong = sane();
        wrong.license.spdx = "CC-BY-4.0".to_string();
        assert!(wrong.validate().unwrap_err().contains("refused"));
        let mut anonymous = sane();
        anonymous.license.author.clear();
        assert!(anonymous.validate().unwrap_err().contains("provenance"));
    }

    /// The budget and geometry gates: over-budget, floating, wrapped-UV or torn meshes never
    /// become files.
    #[test]
    fn the_geometry_gates_bite() {
        let mut fat = sane();
        fat.indices = (0..(FLORA_MAX_TRIS as u32 + 1) * 3).map(|i| i % 3).collect();
        assert!(fat.validate().unwrap_err().contains("budget"));
        let mut floating = sane();
        for p in &mut floating.positions {
            p[1] -= 0.5;
        }
        assert!(floating.validate().unwrap_err().contains("ground plane"));
        let mut wrapped = sane();
        wrapped.uvs[0] = [1.5, 0.0];
        assert!(wrapped.validate().unwrap_err().contains("outside"));
        let mut lying = sane();
        lying.height_m = 5.0;
        assert!(lying.validate().unwrap_err().contains("disagrees"));
    }

    /// The asset round-trips losslessly through serde JSON — the same determinism discipline
    /// every other asset format keeps.
    #[test]
    fn the_asset_round_trips_losslessly() {
        let asset = sane();
        let json = serde_json::to_string(&asset).expect("serialize");
        let back: FloraAsset = serde_json::from_str(&json).expect("parse");
        assert_eq!(asset, back);
        let json_again = serde_json::to_string(&back).expect("serialize again");
        assert_eq!(json, json_again, "canonical form is stable");
    }
}
