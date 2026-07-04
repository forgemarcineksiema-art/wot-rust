//! Focused directional-sun shadow: the light's view-projection for a single, tight, texel-snapped
//! orthographic shadow map centred on the action. Backend-neutral (pure `glam`), so the matrix and
//! its anti-shimmer texel snap are unit-tested without a GPU. See `docs/shadow-policy.md`.

use glam::{Mat4, Vec3, Vec4Swizzles};

/// Parameters for the focused sun-shadow map.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SunShadowParams {
    /// Half-size of the square focus box on the ground, in metres. Smaller = crisper (higher texel
    /// density) but less coverage; ~24–48 m suits a low vehicle read from driving distance.
    pub focus_radius_m: f32,
    /// How far along the sun axis the ortho frustum reaches from the focus centre — must clear the
    /// tallest occluder above and the ground below.
    pub depth_radius_m: f32,
    /// Shadow-map resolution in texels (square).
    pub resolution: u32,
}

impl Default for SunShadowParams {
    fn default() -> Self {
        // 4096 texels over a 64 m box = 1.6 cm texels: running-gear detail (spokes, rims, links)
        // lives at the 2–9 cm scale, and a coarser map crawls jagged shadow edges across the
        // wheel faces on every camera move. Depth reach 80 m still clears hills along the sun
        // axis while halving the depth-bias world slack.
        Self { focus_radius_m: 32.0, depth_radius_m: 80.0, resolution: 4096 }
    }
}

impl SunShadowParams {
    /// World-space size of one shadow texel (metres) — the snap increment.
    pub fn texel_world_size(&self) -> f32 {
        2.0 * self.focus_radius_m / self.resolution.max(1) as f32
    }

    /// Shadow-map UV size of one texel (for PCF offsets): `1 / resolution`.
    pub fn texel_uv_size(&self) -> f32 {
        1.0 / self.resolution.max(1) as f32
    }
}

/// A stable up vector for the light view, avoiding degeneracy when the sun is near-vertical.
fn stable_up(sun_dir: Vec3) -> Vec3 {
    if sun_dir.y.abs() > 0.99 { Vec3::Z } else { Vec3::Y }
}

/// The light view-projection for the focused sun shadow map, **texel-snapped** to kill edge shimmer
/// as the focus centre (camera/player) moves. `key_direction` points *towards* the sun (matches
/// [`crate::SceneLighting::key_direction`]); `focus_center` is the world point the box centres on.
/// Returns a WebGPU `[0, 1]`-depth matrix as column-major `[[f32; 4]; 4]`.
pub fn sun_light_view_projection(
    key_direction: [f32; 3],
    focus_center: [f32; 3],
    params: SunShadowParams,
) -> [[f32; 4]; 4] {
    let sun_dir = Vec3::from_array(key_direction).normalize_or(Vec3::Y);
    let center = Vec3::from_array(focus_center);
    let eye = center + sun_dir * params.depth_radius_m;
    let view = Mat4::look_at_rh(eye, center, stable_up(sun_dir));
    let r = params.focus_radius_m.max(1.0e-3);
    // WebGPU [0, 1]-depth orthographic box covering the sun-axis span through the focus box.
    let proj = Mat4::orthographic_rh(-r, r, -r, r, 0.0, 2.0 * params.depth_radius_m);

    // Texel snap: project the focus centre to NDC, round its xy onto the shadow-texel grid, and fold
    // the residual back into the projection as an NDC translation so the whole map lands on grid.
    let clip = proj * view * center.extend(1.0);
    let ndc = clip.xy() / clip.w;
    let half_res = 0.5 * params.resolution.max(1) as f32;
    let snapped = (ndc * half_res).round() / half_res;
    let offset = snapped - ndc;
    let mut proj = proj;
    proj.w_axis.x += offset.x;
    proj.w_axis.y += offset.y;
    (proj * view).to_cols_array_2d()
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::{Vec2, Vec4};

    const KEY: [f32; 3] = [0.62, 0.52, 0.34];

    fn project(m: &[[f32; 4]; 4], p: Vec3) -> Vec2 {
        let clip = Mat4::from_cols_array_2d(m) * Vec4::new(p.x, p.y, p.z, 1.0);
        clip.xy() / clip.w
    }

    #[test]
    fn params_texel_sizes_track_resolution_and_radius() {
        let p = SunShadowParams { focus_radius_m: 32.0, depth_radius_m: 120.0, resolution: 2048 };
        assert!((p.texel_world_size() - 64.0 / 2048.0).abs() < 1.0e-6);
        assert!((p.texel_uv_size() - 1.0 / 2048.0).abs() < 1.0e-9);
    }

    #[test]
    fn the_focus_centre_projects_onto_the_shadow_texel_grid() {
        // The anti-shimmer contract: after snapping, the focus centre lands on an integer texel, so
        // the map does not crawl under sub-texel camera motion.
        let params = SunShadowParams::default();
        let m = sun_light_view_projection(KEY, [340.3, 1.1, 300.7], params);
        let ndc = project(&m, Vec3::new(340.3, 1.1, 300.7));
        let half_res = 0.5 * params.resolution as f32;
        let texels = ndc * half_res;
        assert!((texels.x - texels.x.round()).abs() < 1.0e-3, "x off grid: {}", texels.x);
        assert!((texels.y - texels.y.round()).abs() < 1.0e-3, "y off grid: {}", texels.y);
    }

    #[test]
    fn the_focus_centre_maps_near_the_shadow_map_centre() {
        // Snapping only nudges by up to one texel, so the focus centre stays essentially centred.
        let m = sun_light_view_projection(KEY, [0.0, 0.0, 0.0], SunShadowParams::default());
        let ndc = project(&m, Vec3::ZERO);
        assert!(ndc.length() < 0.01, "focus centre should sit near NDC origin, got {ndc:?}");
    }

    #[test]
    fn a_sub_texel_focus_shift_keeps_both_maps_grid_aligned() {
        // Two focus centres a fraction of a texel apart both land on the integer grid — the property
        // that makes the shadow stable frame to frame.
        let params = SunShadowParams::default();
        let step = params.texel_world_size() * 0.25;
        for focus in [[10.0, 0.5, 10.0], [10.0 + step, 0.5, 10.0 + step]] {
            let m = sun_light_view_projection(KEY, focus, params);
            let ndc = project(&m, Vec3::from_array(focus));
            let texels = ndc * (0.5 * params.resolution as f32);
            assert!((texels.x - texels.x.round()).abs() < 1.0e-3);
            assert!((texels.y - texels.y.round()).abs() < 1.0e-3);
        }
    }
}
