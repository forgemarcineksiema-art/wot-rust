//! Terrain Material 2.0 (`docs/art-direction-policy.md` rule 2/5): the ground's albedo stops
//! being one baked vertex colour and becomes four blended material layers — lush grass, dry
//! straw, worn dirt, broken rock/chalk — weighted per-pixel by a splat map baked from map data
//! (height, slope, roads, water margins) and lit through a macro normal map sampled finer than
//! the 5 m render grid. All of it is DATA: each map owns a `TerrainMaterialSet` (palette bound
//! to the policy ground swatches) and the baked `TerrainGroundMaps`; no layer colour hides in
//! a shader.

/// One ground layer: a policy-conformant albedo (saturation <= 0.45 — locked), how strongly
/// the two detail octaves modulate it, and its specular lane.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TerrainLayer {
    /// Linear albedo, inside the art-direction ground-saturation window.
    pub albedo: [f32; 3],
    /// Amplitude of the shader's two-octave detail modulation (1.0 = the scene shader's
    /// historical 0.92..1.08 swing; 0 = flat paint).
    pub detail: f32,
    /// Specular lane for this layer (see `SceneVertex::gloss`).
    pub gloss: f32,
}

/// The four layers a splat texel weights, in channel order R, G, B, A.
pub const TERRAIN_LAYERS: usize = 4;

/// A map's ground material set: the four layers plus how strongly the baked macro normal bends
/// the 5 m vertex normal. Map-owned data (Prokhorovka's chalk steppe vs Bystra's river valley),
/// envelope-locked like the lighting profiles.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TerrainMaterialSet {
    /// R = lush grass, G = dry straw, B = worn dirt, A = broken rock/chalk.
    pub layers: [TerrainLayer; TERRAIN_LAYERS],
    /// 0..1: how far ground lighting leans from the vertex normal toward the baked macro
    /// normal. The macro map carries ~1 m relief the 5 m grid cannot; raking light reads it.
    pub macro_normal_strength: f32,
    /// 0..1: the field-patch macro band (Ziemia 2.0). Worked land is FIELDS, not one lawn —
    /// a low-frequency patch pattern gives every ~50–100 m plot its own tone (lusher, drier,
    /// lighter, darker) with soft headland borders. Only the vegetation layers move; dirt
    /// roads and rock cut through the pattern unchanged. 0 disables the band.
    pub field_patch_strength: f32,
}

impl TerrainMaterialSet {
    /// The Prokhorovka steppe: grey-green grass drifting to sun-dried straw, dirt roads, and
    /// the chalk that breaks through Kursk crests. Albedos are the policy swatches.
    pub fn prokhorovka() -> Self {
        Self {
            layers: [
                TerrainLayer { albedo: [0.28, 0.33, 0.20], detail: 1.0, gloss: 0.03 },
                TerrainLayer { albedo: [0.45, 0.40, 0.26], detail: 1.0, gloss: 0.03 },
                TerrainLayer { albedo: [0.36, 0.30, 0.22], detail: 0.8, gloss: 0.05 },
                TerrainLayer { albedo: [0.52, 0.51, 0.47], detail: 0.9, gloss: 0.12 },
            ],
            macro_normal_strength: 0.65,
            // The open steppe is a quilt of worked plots — the band runs at full strength.
            field_patch_strength: 1.0,
        }
    }

    /// The Bystra valley: damper greens, darker river-worn earth, and grey field stone in
    /// place of chalk.
    pub fn bystra() -> Self {
        Self {
            layers: [
                TerrainLayer { albedo: [0.26, 0.33, 0.21], detail: 1.0, gloss: 0.03 },
                TerrainLayer { albedo: [0.41, 0.38, 0.26], detail: 1.0, gloss: 0.03 },
                TerrainLayer { albedo: [0.30, 0.26, 0.20], detail: 0.8, gloss: 0.06 },
                TerrainLayer { albedo: [0.42, 0.40, 0.36], detail: 0.9, gloss: 0.14 },
            ],
            macro_normal_strength: 0.65,
            // Valley meadows patch more softly than the steppe's worked plots.
            field_patch_strength: 0.75,
        }
    }
}

/// The baked per-map ground textures the terrain pipeline samples: a splat map (four layer
/// weights, RGBA8, normalized per texel) and a macro normal map (world-space normal packed
/// `0.5 * n + 0.5` into RGB), both square and spanning the map's ground extent exactly.
#[derive(Debug, Clone, PartialEq)]
pub struct TerrainGroundMaps {
    /// Texture edge in texels (square).
    pub size: u32,
    /// RGBA8 layer weights, row-major, `size * size * 4` bytes.
    pub splat: Vec<u8>,
    /// RGBA8 packed world-space normals, row-major, `size * size * 4` bytes.
    pub macro_normal: Vec<u8>,
    /// World-space ground extent the textures span: UV = world.xz / extent.
    pub extent_m: [f32; 2],
}

impl TerrainGroundMaps {
    /// Sanity used by tests and the upload path: buffers match `size`, extent is positive.
    pub fn is_well_formed(&self) -> bool {
        let texels = self.size as usize * self.size as usize * 4;
        self.size > 0
            && self.splat.len() == texels
            && self.macro_normal.len() == texels
            && self.extent_m[0] > 0.0
            && self.extent_m[1] > 0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Ziemia 2.0's contract: every map runs the field-patch band (worked land is a quilt,
    /// not a lawn), the steppe patches harder than the valley meadows, and the maximum
    /// lightness drift the shader applies (±17% at strength 1) cannot push any vegetation
    /// albedo out of the art-direction ground window (saturation ≤ 0.45, lightness sane).
    #[test]
    fn the_field_quilt_is_on_everywhere_and_stays_inside_the_ground_window() {
        let steppe = TerrainMaterialSet::prokhorovka();
        let valley = TerrainMaterialSet::bystra();
        assert_eq!(steppe.field_patch_strength, 1.0);
        assert_eq!(valley.field_patch_strength, 0.75);
        assert!(steppe.field_patch_strength > valley.field_patch_strength);

        for set in [steppe, valley] {
            for layer in &set.layers[..2] {
                // The shader's worst-case plot lightening: field_light = 1 + 0.5 * 0.34.
                let lifted = layer.albedo.map(|c| c * 1.17);
                let max = lifted.iter().copied().fold(0.0f32, f32::max);
                let min = lifted.iter().copied().fold(1.0f32, f32::min);
                assert!(max <= 0.62, "a lightened plot must stay soil, not neon: {lifted:?}");
                let saturation = if max > 0.0 { (max - min) / max } else { 0.0 };
                assert!(
                    saturation <= 0.45 + 1.0e-3,
                    "vegetation saturation must hold the policy window, got {saturation}"
                );
            }
        }
    }
}
