//! Imported-flora catalog: embedded CC0 assets, deterministic regions and a complete,
//! alpha-coverage-preserving runtime atlas mip chain.

mod mips;
#[cfg(test)]
mod tests;

use std::sync::OnceLock;

use renderer_api::{Rgba8MipChain, Rgba8MipLevel};
use world_forge::flora::FloraAsset;

/// One 2048² RGBA8 page plus mips is ~21 MiB and remains min-spec safe.
pub const FLORA_ATLAS_SIZE: u32 = 2048;
/// Stop at 16x16: every shipped region still owns distinct content and gutter texels there.
///
/// One rung deeper (8x8) was safe while two regions sat DIAGONALLY across the page. A third
/// asset necessarily makes two of them side by side, and at 8x8 a 512-texel region shrinks to
/// two texels — its one-texel gutter then reaches into the neighbour and a distant card samples
/// its sibling's leaves. Clamping one level shallower keeps the isolation the packer promises;
/// nothing loses detail, because a tree that far away is drawing the impostor, whose region is
/// four times wider and never reaches this tail.
pub const FLORA_MAX_SAMPLED_MIP: u32 = 7;

const SLOT_SIZE: u32 = FLORA_ATLAS_SIZE / 2;
const PACK_SLOTS: [(u32, u32); 3] = [(1, 0), (0, 1), (1, 1)];

/// `uv_atlas = offset + uv_asset * scale`; endpoints address texel centers, not neighbours.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FloraRegion {
    pub u_offset: f32,
    pub v_offset: f32,
    pub u_scale: f32,
    pub v_scale: f32,
}

pub struct FloraCatalog {
    pub atlas_mips: Rgba8MipChain,
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

// Hero-flora program (2026-08-05): the stylized download pack is retired; `dab-hero` — an
// oak distilled from a Blender master with photoscan CC0 textures (ambientCG Bark012 +
// LeafSet016) — is the first of the new family. Freed slots host its future siblings.
// The rungs of one tree, not three species: the near mesh, its sparse mid distillation and the
// two-quad impostor all come from the same Blender master, and `tree_lod` swaps between them by
// distance. They share the atlas because they share the art.
const SHIPPED: &[(&str, &[u8])] = &[
    (
        include_str!("../../../../assets/flora/dab-hero.flora.json"),
        include_bytes!("../../../../assets/flora/dab-hero.flora.png"),
    ),
    (
        include_str!("../../../../assets/flora/dab-hero-lod2.flora.json"),
        include_bytes!("../../../../assets/flora/dab-hero-lod2.flora.png"),
    ),
    (
        include_str!("../../../../assets/flora/dab-hero-imp.flora.json"),
        include_bytes!("../../../../assets/flora/dab-hero-imp.flora.png"),
    ),
];

pub fn flora_catalog() -> &'static FloraCatalog {
    static CATALOG: OnceLock<FloraCatalog> = OnceLock::new();
    CATALOG.get_or_init(|| build_catalog().expect("shipped flora assets pack"))
}

fn build_catalog() -> Result<FloraCatalog, String> {
    let mut decoded = decode_shipped()?;
    decoded
        .sort_by(|a, b| b.0.texture_height.cmp(&a.0.texture_height).then(a.0.name.cmp(&b.0.name)));
    if decoded.len() > PACK_SLOTS.len() {
        return Err(format!(
            "{} flora assets exceed {} isolated slots",
            decoded.len(),
            PACK_SLOTS.len()
        ));
    }
    let mut sources = Vec::with_capacity(decoded.len());
    let mut entries = Vec::with_capacity(decoded.len());
    for ((asset, rgba), slot) in decoded.into_iter().zip(PACK_SLOTS) {
        if asset.texture_width > SLOT_SIZE || asset.texture_height > SLOT_SIZE {
            return Err(format!("{} exceeds its {SLOT_SIZE}px isolated slot", asset.name));
        }
        let x = slot.0 * SLOT_SIZE + (SLOT_SIZE - asset.texture_width) / 2;
        let y = slot.1 * SLOT_SIZE + if slot.1 == 0 { 0 } else { SLOT_SIZE - asset.texture_height };
        let region = texel_center_region(x, y, asset.texture_width, asset.texture_height);
        let levels = mips::build_asset_mip_chain(asset.texture_width, asset.texture_height, rgba);
        sources.push(PackedSource { x, y, levels });
        entries.push((asset, region));
    }
    let levels = build_atlas_levels(&sources);
    Ok(FloraCatalog { atlas_mips: Rgba8MipChain::new(levels, FLORA_MAX_SAMPLED_MIP), entries })
}

fn decode_shipped() -> Result<Vec<(FloraAsset, Vec<u8>)>, String> {
    SHIPPED
        .iter()
        .map(|(json, png_bytes)| {
            let asset: FloraAsset =
                serde_json::from_str(json).map_err(|error| format!("flora json: {error}"))?;
            asset.validate().map_err(|reason| format!("{}: {reason}", asset.name))?;
            let mut reader = png::Decoder::new(std::io::Cursor::new(*png_bytes))
                .read_info()
                .map_err(|error| format!("png: {error}"))?;
            let mut rgba = vec![0u8; reader.output_buffer_size().unwrap_or_default()];
            let info = reader.next_frame(&mut rgba).map_err(|error| format!("png: {error}"))?;
            if info.color_type != png::ColorType::Rgba || info.bit_depth != png::BitDepth::Eight {
                return Err(format!("{}: texture must be RGBA8", asset.name));
            }
            if (info.width, info.height) != (asset.texture_width, asset.texture_height) {
                return Err(format!("{}: texture dimensions disagree with json", asset.name));
            }
            rgba.truncate((info.width * info.height * 4) as usize);
            Ok((asset, rgba))
        })
        .collect()
}

fn texel_center_region(x: u32, y: u32, width: u32, height: u32) -> FloraRegion {
    let page = FLORA_ATLAS_SIZE as f32;
    FloraRegion {
        u_offset: (x as f32 + 0.5) / page,
        v_offset: (y as f32 + 0.5) / page,
        u_scale: width.saturating_sub(1) as f32 / page,
        v_scale: height.saturating_sub(1) as f32 / page,
    }
}

fn build_atlas_levels(sources: &[PackedSource]) -> Vec<Rgba8MipLevel> {
    let mut levels = Vec::new();
    let mut atlas_size = FLORA_ATLAS_SIZE;
    for level in 0..=FLORA_ATLAS_SIZE.ilog2() {
        let mut rgba = vec![0u8; (atlas_size * atlas_size * 4) as usize];
        let mut occupied = vec![false; (atlas_size * atlas_size) as usize];
        for source in sources {
            blit_source(&mut rgba, &mut occupied, atlas_size, source, level);
        }
        for source in sources {
            blit_source_gutter(&mut rgba, &occupied, atlas_size, source, level);
        }
        rgba[0..4].copy_from_slice(&[255, 255, 255, 255]);
        levels.push(Rgba8MipLevel::new(atlas_size, atlas_size, rgba));
        atlas_size = (atlas_size / 2).max(1);
    }
    levels
}

fn blit_source(
    atlas: &mut [u8],
    occupied: &mut [bool],
    atlas_size: u32,
    source: &PackedSource,
    level: u32,
) {
    let mip = &source.levels[level.min(source.levels.len() as u32 - 1) as usize];
    let (x, y) = (source.x >> level, source.y >> level);
    for row in 0..mip.height().min(atlas_size.saturating_sub(y)) {
        for column in 0..mip.width().min(atlas_size.saturating_sub(x)) {
            let dst_pixel = ((y + row) * atlas_size + x + column) as usize;
            let src = ((row * mip.width() + column) * 4) as usize;
            atlas[dst_pixel * 4..dst_pixel * 4 + 4].copy_from_slice(&mip.rgba()[src..src + 4]);
            occupied[dst_pixel] = true;
        }
    }
}

fn blit_source_gutter(
    atlas: &mut [u8],
    occupied: &[bool],
    atlas_size: u32,
    source: &PackedSource,
    level: u32,
) {
    let mip = &source.levels[level.min(source.levels.len() as u32 - 1) as usize];
    let (x, y) = (source.x >> level, source.y >> level);
    let left = x.saturating_sub(1);
    let top = y.saturating_sub(1);
    let right = (x + mip.width()).min(atlas_size - 1);
    let bottom = (y + mip.height()).min(atlas_size - 1);
    for dst_y in top..=bottom {
        for dst_x in left..=right {
            let dst_pixel = (dst_y * atlas_size + dst_x) as usize;
            if occupied[dst_pixel] {
                continue;
            }
            let src_x = dst_x.saturating_sub(x).min(mip.width() - 1);
            let src_y = dst_y.saturating_sub(y).min(mip.height() - 1);
            let src = ((src_y * mip.width() + src_x) * 4) as usize;
            atlas[dst_pixel * 4..dst_pixel * 4 + 4].copy_from_slice(&mip.rgba()[src..src + 4]);
        }
    }
}

struct PackedSource {
    x: u32,
    y: u32,
    levels: Vec<Rgba8MipLevel>,
}
