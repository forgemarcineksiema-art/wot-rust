use std::io;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ForgeTextureManifest {
    file: String,
    semantic: String,
    channels: String,
    bytes: usize,
}

impl ForgeTextureManifest {
    pub fn file(&self) -> &str {
        &self.file
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

pub fn bake_default_set() -> Result<Vec<BakedTextureMap>, io::Error> {
    Ok(vec![
        bake_map("albedo.png", "albedo", "rgba", albedo_pixel)?,
        bake_map("normal.png", "normal", "xyz", normal_pixel)?,
        bake_map(
            "ao_roughness.png",
            "ao_roughness_metalness",
            "ao,roughness,metalness,unused",
            ao_pixel,
        )?,
        bake_map("cavity.png", "cavity", "cavity,cavity,cavity,unused", cavity_pixel)?,
    ])
}

fn bake_map(
    file: &str,
    semantic: &str,
    channels: &str,
    pixel: fn(u32, u32) -> [u8; 4],
) -> Result<BakedTextureMap, io::Error> {
    const SIZE: u32 = 32;
    let mut rgba = Vec::with_capacity((SIZE * SIZE * 4) as usize);
    for y in 0..SIZE {
        for x in 0..SIZE {
            rgba.extend_from_slice(&pixel(x, y));
        }
    }
    let bytes = encode_png(SIZE, SIZE, &rgba)?;
    let manifest = ForgeTextureManifest {
        file: file.to_string(),
        semantic: semantic.to_string(),
        channels: channels.to_string(),
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

fn albedo_pixel(x: u32, y: u32) -> [u8; 4] {
    let band = y / 7;
    let chip = ((x / 4 + y / 4) % 2) as u8 * 6;
    match band {
        0 => [112 + chip, 118 + chip, 104 + chip, 255],
        1 => [94 + chip, 99 + chip, 92 + chip, 255],
        2 => [42 + chip, 43 + chip, 40 + chip, 255],
        3 => [24 + chip, 23 + chip, 22 + chip, 255],
        _ => [12 + chip, 12 + chip, 13 + chip, 255],
    }
}

fn normal_pixel(x: u32, y: u32) -> [u8; 4] {
    let seam = if x.is_multiple_of(8) || y.is_multiple_of(8) { 6 } else { 0 };
    [128 + seam, 128, 255_u8.saturating_sub(seam / 2), 255]
}

fn ao_pixel(x: u32, y: u32) -> [u8; 4] {
    let recess = if x < 6 || y > 24 { 42 } else { 0 };
    let roughness = 155 + ((x + y) % 18) as u8;
    [220_u8.saturating_sub(recess), roughness, 18, 255]
}

fn cavity_pixel(x: u32, y: u32) -> [u8; 4] {
    let line = if x.is_multiple_of(8) || y.is_multiple_of(8) { 86 } else { 0 };
    let value = 226_u8.saturating_sub(line);
    [value, value, value, 255]
}
