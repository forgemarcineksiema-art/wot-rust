//! The imported-flora catalog (Flora 2.0, FL-4): the shipped `.flora.json` assets embedded
//! hermetically (the same include discipline as map blueprints), their textures decoded and
//! shelf-packed into ONE runtime atlas for the FL-2 group-1 slot, and per-asset UV regions so
//! mesh coordinates remap into the packed page. Deterministic: the same binary always packs
//! the same atlas.

use std::sync::OnceLock;

use world_forge::flora::FloraAsset;

/// The packed atlas page size. 2048² RGBA8 is 16 MiB — one page, no eviction, min-spec safe;
/// the packer asserts the shipped set fits and a grown set fails the build, not the frame.
pub const FLORA_ATLAS_SIZE: u32 = 2048;

/// Where an asset's texture landed in the atlas: `uv_atlas = offset + uv_asset * scale`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FloraRegion {
    pub u_offset: f32,
    pub v_offset: f32,
    pub u_scale: f32,
    pub v_scale: f32,
}

pub struct FloraCatalog {
    pub atlas_rgba: Vec<u8>,
    pub atlas_size: u32,
    entries: Vec<(FloraAsset, FloraRegion)>,
}

impl FloraCatalog {
    pub fn get(&self, name: &str) -> Option<&(FloraAsset, FloraRegion)> {
        self.entries.iter().find(|(asset, _)| asset.name == name)
    }

    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.entries.iter().map(|(asset, _)| asset.name.as_str())
    }
}

/// The shipped set: JSON + PNG pairs embedded at compile time. Append-only, like every
/// catalog; the validation gate re-runs on every build via the tests below.
const SHIPPED: &[(&str, &[u8])] = &[
    (
        include_str!("../../../../assets/flora/stylized-tree.flora.json"),
        include_bytes!("../../../../assets/flora/stylized-tree.flora.png"),
    ),
    (
        include_str!("../../../../assets/flora/stylized-pine.flora.json"),
        include_bytes!("../../../../assets/flora/stylized-pine.flora.png"),
    ),
    (
        include_str!("../../../../assets/flora/stylized-bush.flora.json"),
        include_bytes!("../../../../assets/flora/stylized-bush.flora.png"),
    ),
];

/// The catalog, built once per process: decode, validate, shelf-pack.
pub fn flora_catalog() -> &'static FloraCatalog {
    static CATALOG: OnceLock<FloraCatalog> = OnceLock::new();
    CATALOG.get_or_init(|| build_catalog().expect("shipped flora assets pack"))
}

fn build_catalog() -> Result<FloraCatalog, String> {
    let size = FLORA_ATLAS_SIZE as usize;
    let mut atlas = vec![0u8; size * size * 4];
    let mut entries: Vec<(FloraAsset, Vec<u8>)> = Vec::new();
    for (json, png_bytes) in SHIPPED {
        let asset: FloraAsset =
            serde_json::from_str(json).map_err(|error| format!("flora json: {error}"))?;
        asset.validate().map_err(|reason| format!("{}: {reason}", asset.name))?;
        let decoder = png::Decoder::new(std::io::Cursor::new(*png_bytes));
        let mut reader = decoder.read_info().map_err(|error| format!("png: {error}"))?;
        let mut buffer = vec![0u8; reader.output_buffer_size().unwrap_or_default()];
        let info = reader.next_frame(&mut buffer).map_err(|error| format!("png: {error}"))?;
        if info.color_type != png::ColorType::Rgba || info.bit_depth != png::BitDepth::Eight {
            return Err(format!("{}: texture must be RGBA8", asset.name));
        }
        if info.width != asset.texture_width || info.height != asset.texture_height {
            return Err(format!("{}: texture dimensions disagree with the json", asset.name));
        }
        buffer.truncate((info.width * info.height * 4) as usize);
        entries.push((asset, buffer));
    }
    // Shelf packing, tallest first — deterministic (stable sort by height then name).
    entries
        .sort_by(|a, b| b.0.texture_height.cmp(&a.0.texture_height).then(a.0.name.cmp(&b.0.name)));
    let mut packed = Vec::new();
    let (mut cursor_x, mut cursor_y, mut shelf_height) = (0u32, 0u32, 0u32);
    for (asset, pixels) in entries {
        let (w, h) = (asset.texture_width, asset.texture_height);
        if cursor_x + w > FLORA_ATLAS_SIZE {
            cursor_y += shelf_height;
            cursor_x = 0;
            shelf_height = 0;
        }
        if cursor_y + h > FLORA_ATLAS_SIZE || w > FLORA_ATLAS_SIZE {
            return Err(format!(
                "the shipped flora set no longer fits the {FLORA_ATLAS_SIZE} atlas at {}",
                asset.name
            ));
        }
        for row in 0..h as usize {
            let src = row * w as usize * 4;
            let dst = ((cursor_y as usize + row) * size + cursor_x as usize) * 4;
            atlas[dst..dst + w as usize * 4].copy_from_slice(&pixels[src..src + w as usize * 4]);
        }
        let region = FloraRegion {
            u_offset: cursor_x as f32 / FLORA_ATLAS_SIZE as f32,
            v_offset: cursor_y as f32 / FLORA_ATLAS_SIZE as f32,
            u_scale: w as f32 / FLORA_ATLAS_SIZE as f32,
            v_scale: h as f32 / FLORA_ATLAS_SIZE as f32,
        };
        cursor_x += w;
        shelf_height = shelf_height.max(h);
        packed.push((asset, region));
    }
    // The (0, 0) texel is the FL-2 no-op contract: every procedural vertex samples it, so it
    // must stay OPAQUE WHITE no matter what the packer placed nearby. Reserve it explicitly.
    atlas[0] = 255;
    atlas[1] = 255;
    atlas[2] = 255;
    atlas[3] = 255;
    Ok(FloraCatalog { atlas_rgba: atlas, atlas_size: FLORA_ATLAS_SIZE, entries: packed })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The shipped set decodes, validates, and packs deterministically inside one page —
    /// with every region in bounds and the FL-2 white-texel contract held.
    #[test]
    fn the_shipped_set_packs_deterministically_with_the_white_texel_held() {
        let first = build_catalog().expect("packs");
        let second = build_catalog().expect("packs again");
        assert_eq!(first.atlas_rgba, second.atlas_rgba, "the pack is deterministic");
        assert_eq!(first.entries.len(), 3, "tree, pine, bush ship today");
        for (asset, region) in &first.entries {
            assert!(
                region.u_offset >= 0.0
                    && region.v_offset >= 0.0
                    && region.u_offset + region.u_scale <= 1.0 + 1.0e-6
                    && region.v_offset + region.v_scale <= 1.0 + 1.0e-6,
                "{} region out of bounds: {region:?}",
                asset.name
            );
        }
        assert_eq!(&first.atlas_rgba[0..4], &[255, 255, 255, 255], "the (0,0) no-op texel");
    }

    /// Remapped UVs stay inside their region for every shipped vertex — the mesh can never
    /// sample a neighbour's texture.
    #[test]
    fn remapped_uvs_stay_inside_their_region() {
        let catalog = flora_catalog();
        for name in ["stylized-tree", "stylized-pine", "stylized-bush"] {
            let (asset, region) = catalog.get(name).expect("shipped");
            for uv in &asset.uvs {
                let u = region.u_offset + uv[0] * region.u_scale;
                let v = region.v_offset + uv[1] * region.v_scale;
                assert!(
                    u >= region.u_offset - 1.0e-6
                        && u <= region.u_offset + region.u_scale + 1.0e-6
                        && v >= region.v_offset - 1.0e-6
                        && v <= region.v_offset + region.v_scale + 1.0e-6,
                    "{name}: uv {uv:?} escapes its region"
                );
            }
        }
    }
}
