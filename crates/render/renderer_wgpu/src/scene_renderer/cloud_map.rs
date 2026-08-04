//! The baked cloud-coverage tile the ground passes sample for wandering cloud shade
//! (`shadow_common.wgsl::cloud_shadow`). The procedural version evaluated a domain-warped,
//! three-octave value-noise field per fragment — ~6 lattice evaluations across every terrain
//! and scene pixel, the measured ~5 ms that kept cloud shade out of the shipped canonical look
//! (art-direction D21). The field is a pure function of its UV, so it is baked ONCE here on
//! the CPU into a seamlessly tiling R8 texture and the per-fragment cost collapses to one
//! repeat-sampled tap.
//!
//! Seamless by construction, not by blending: every octave's lattice is wrapped modulo an
//! integer period, and the domain warp is itself periodic, so opposite edges of the tile are
//! the same field — no mirror seams, no cross-fade mush.

/// Texture edge in texels. At 8 noise cells per tile this leaves 64 texels per finest-octave
/// cell — the field is smooth well past the smoothstep threshold's needs; 256 KiB resident.
pub(crate) const CLOUD_MAP_SIZE: u32 = 512;

/// How many cloud-UV units (the shader's `base_uv` domain, 1 unit = 1 base-octave cell) one
/// repeat of the tile spans. Kept in lockstep with `CLOUD_TILE_SPAN` in shadow_common.wgsl —
/// locked by `the_cloud_tile_span_matches_the_shader`.
pub(crate) const CLOUD_TILE_SPAN_UV: f32 = 8.0;

/// Octave weights, carried over from the procedural field (0.46 / 0.34 / 0.2 at relative
/// scales 1 / ~2 / ~4): broad banks, mid clumps, fine erosion of their edges.
const OCTAVES: [(f32, u64, f32); 3] =
    [(1.0, 0xC10D_0001, 0.46), (2.0, 0xC10D_0002, 0.34), (4.0, 0xC10D_0003, 0.2)];
/// Domain-warp amplitude in UV units — the procedural field's 0.72, the number that stopped
/// value noise from showing its square interpolation cells as giant rectangles.
const WARP_AMPLITUDE: f32 = 0.72;

/// splitmix64 over the wrapped lattice coordinates and an octave seed — the same construction
/// `terrain::ground::value_noise` trusts on the CPU.
fn corner(ix: i64, iz: i64, period: i64, seed: u64) -> f32 {
    let wrap = |v: i64| -> u64 { v.rem_euclid(period) as u64 };
    let mut h = wrap(ix).wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ wrap(iz).rotate_left(32) ^ seed;
    h = (h ^ (h >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    h = (h ^ (h >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    ((h ^ (h >> 31)) >> 40) as f32 / ((1u64 << 24) - 1) as f32
}

/// Deterministic 2D value noise in [0, 1] whose lattice wraps modulo `period` — shifting the
/// input by `period` cells reproduces the value exactly, which is what makes the tile seamless.
fn periodic_value_noise(x: f32, z: f32, period: i64, seed: u64) -> f32 {
    let (x0, z0) = (x.floor(), z.floor());
    let (fx, fz) = (x - x0, z - z0);
    let (sx, sz) = (fx * fx * (3.0 - 2.0 * fx), fz * fz * (3.0 - 2.0 * fz));
    let (ix, iz) = (x0 as i64, z0 as i64);
    let a = corner(ix, iz, period, seed);
    let b = corner(ix + 1, iz, period, seed);
    let c = corner(ix, iz + 1, period, seed);
    let d = corner(ix + 1, iz + 1, period, seed);
    let top = a + (b - a) * sx;
    let bottom = c + (d - c) * sx;
    top + (bottom - top) * sz
}

/// The coverage field at a point of the UV-unit domain. Periodic with `CLOUD_TILE_SPAN_UV` in
/// both axes (locked by `the_coverage_field_tiles_seamlessly`).
fn coverage_at(u: f32, v: f32) -> f32 {
    let cells = CLOUD_TILE_SPAN_UV as i64;
    let warp_x = periodic_value_noise(u, v, cells, 0x57A2_7001) - 0.5;
    let warp_z = periodic_value_noise(u, v, cells, 0x57A2_7002) - 0.5;
    let (wu, wv) = (u + warp_x * WARP_AMPLITUDE, v + warp_z * WARP_AMPLITUDE);
    OCTAVES
        .iter()
        .map(|&(frequency, seed, weight)| {
            let period = cells * frequency as i64;
            periodic_value_noise(wu * frequency, wv * frequency, period, seed) * weight
        })
        .sum()
}

/// Bake the tiling coverage field into R8 texels (row-major, `CLOUD_MAP_SIZE`²).
pub(crate) fn bake_cloud_coverage() -> Vec<u8> {
    let size = CLOUD_MAP_SIZE as usize;
    let mut texels = Vec::with_capacity(size * size);
    for tz in 0..size {
        for tx in 0..size {
            // Texel centres; the sampler's linear filtering interpolates between them and the
            // repeat addressing closes the last half-texel against the (identical) first row.
            let u = (tx as f32 + 0.5) / size as f32 * CLOUD_TILE_SPAN_UV;
            let v = (tz as f32 + 0.5) / size as f32 * CLOUD_TILE_SPAN_UV;
            texels.push((coverage_at(u, v) * 255.0).round().clamp(0.0, 255.0) as u8);
        }
    }
    texels
}

/// Upload the baked tile and build its repeat-addressed sampler (group-2 bindings 5–6).
pub(crate) fn create_cloud_resources(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
) -> (wgpu::TextureView, wgpu::Sampler) {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("cloud_coverage_map"),
        size: wgpu::Extent3d {
            width: CLOUD_MAP_SIZE,
            height: CLOUD_MAP_SIZE,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::R8Unorm,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    queue.write_texture(
        wgpu::TexelCopyTextureInfo {
            texture: &texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        &bake_cloud_coverage(),
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(CLOUD_MAP_SIZE),
            rows_per_image: Some(CLOUD_MAP_SIZE),
        },
        wgpu::Extent3d { width: CLOUD_MAP_SIZE, height: CLOUD_MAP_SIZE, depth_or_array_layers: 1 },
    );
    let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        label: Some("cloud_coverage_sampler"),
        address_mode_u: wgpu::AddressMode::Repeat,
        address_mode_v: wgpu::AddressMode::Repeat,
        address_mode_w: wgpu::AddressMode::Repeat,
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        ..Default::default()
    });
    (texture.create_view(&wgpu::TextureViewDescriptor::default()), sampler)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The whole point of the periodic lattice: stepping one full tile in either axis lands on
    /// the identical field, so the repeat-addressed sampler never crosses a seam.
    #[test]
    fn the_coverage_field_tiles_seamlessly() {
        for (u, v) in [(0.13, 0.62), (3.7, 1.9), (7.99, 7.01), (2.5, 6.25), (0.0, 4.4)] {
            let here = coverage_at(u, v);
            let east = coverage_at(u + CLOUD_TILE_SPAN_UV, v);
            let south = coverage_at(u, v + CLOUD_TILE_SPAN_UV);
            assert!((here - east).abs() < 1.0e-6, "x seam at ({u}, {v}): {here} vs {east}");
            assert!((here - south).abs() < 1.0e-6, "z seam at ({u}, {v}): {here} vs {south}");
        }
    }

    /// The field must hold real banks AND real open sky through the shader's smoothstep window
    /// (0.40..0.72 plus the profile bias) — a flat grey tile would shade nothing or everything.
    /// Determinism is the bake's contract with the one-look policy: every machine ships the
    /// identical texture.
    #[test]
    fn the_baked_tile_is_deterministic_with_banks_and_holes() {
        let tile = bake_cloud_coverage();
        assert_eq!(tile.len(), (CLOUD_MAP_SIZE * CLOUD_MAP_SIZE) as usize);
        assert_eq!(tile, bake_cloud_coverage(), "the bake must be deterministic");

        let min = *tile.iter().min().expect("non-empty") as f32 / 255.0;
        let max = *tile.iter().max().expect("non-empty") as f32 / 255.0;
        let mean = tile.iter().map(|&t| t as f32 / 255.0).sum::<f32>() / tile.len() as f32;
        assert!(min < 0.35, "the field needs open sky below the threshold, min {min}");
        assert!(max > 0.72, "the field needs full banks above the threshold, max {max}");
        assert!((0.4..=0.6).contains(&mean), "coverage must stay balanced, mean {mean}");
    }

    /// The shader divides its cloud UV by CLOUD_TILE_SPAN before the repeat-sampled tap; the
    /// bake stretches the same span across the texture. One number, two homes — locked.
    #[test]
    fn the_cloud_tile_span_matches_the_shader() {
        let expected = format!("const CLOUD_TILE_SPAN: f32 = {CLOUD_TILE_SPAN_UV:.1};");
        assert!(
            crate::shader_library::SHADOW_COMMON_WGSL.contains(&expected),
            "shadow_common.wgsl must declare {expected}"
        );
    }

    /// CLOUD SHADE FALLS ON EVERYTHING THE SUN LIGHTS — honesty doctrine, one sun for one world.
    /// Terrain and statics took the cloud term from the day it shipped; vehicles did not, so a
    /// tank sitting in a bank of moving shade stayed at full key while the field around it went
    /// dark. Nothing in the policy asked for that; it was simply missed, which is exactly the
    /// kind of omission a per-pass list catches and a screenshot does not.
    #[test]
    fn every_sunlit_pass_takes_the_cloud_shade() {
        for (pass, source) in [
            ("terrain", crate::scene_renderer::ground::terrain_shader_source()),
            ("scene", crate::scene_pipeline::scene_shader_source()),
            ("vehicle", crate::vehicle_pipeline::vehicle_shader_source()),
        ] {
            assert!(
                source.contains("cloud_shadow(input.world_pos)"),
                "the {pass} pass shades with the sun but never multiplies it by \
                 cloud_shadow(input.world_pos) — its surfaces would ignore the cloud layer the \
                 rest of the world obeys"
            );
        }
    }
}
