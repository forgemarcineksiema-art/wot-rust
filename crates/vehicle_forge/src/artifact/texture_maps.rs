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
    const SIZE: u32 = 512;
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
    let variation = grain(x, y, 5);
    [174 + variation, 183 + variation, 153 + variation, 255]
}

fn normal_pixel(x: u32, y: u32) -> [u8; 4] {
    let variation = grain(x, y, 2);
    [128 + variation, 128 + variation / 2, 255, 255]
}

fn ao_pixel(x: u32, y: u32) -> [u8; 4] {
    let variation = grain(x, y, 4);
    [238 - variation, 174 + variation, 18, 255]
}

fn cavity_pixel(x: u32, y: u32) -> [u8; 4] {
    let value = 238 - grain(x, y, 3);
    [value, value, value, 255]
}

fn grain(x: u32, y: u32, amplitude: u8) -> u8 {
    let mut value = x.wrapping_mul(1_103_515_245) ^ y.wrapping_mul(12_345);
    value ^= value >> 16;
    (value % (u32::from(amplitude) + 1)) as u8
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn forge_maps_are_clean_512px_material_inputs() {
        let maps = bake_default_set().expect("maps bake");
        for map in maps {
            let decoder = png::Decoder::new(std::io::Cursor::new(map.bytes()));
            let reader = decoder.read_info().expect("png header");
            assert_eq!(reader.info().width, 512);
            assert_eq!(reader.info().height, 512);
        }
    }
}
