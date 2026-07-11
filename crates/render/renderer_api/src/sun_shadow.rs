//! Focused directional-sun shadow: the light's view-projection for a single, tight, texel-snapped
//! orthographic shadow map centred on the action. Backend-neutral (pure `glam`), so the matrix and
//! its anti-shimmer texel snap are unit-tested without a GPU. See `docs/shadow-policy.md`.

use glam::{Mat4, Vec3, Vec4, Vec4Swizzles};

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
        // A 64 m half-box (128 m across) at 4096 texels = 3.1 cm texels. Wider than a pure hero-shot
        // box, because on the battlefield the whole near/mid field must ground — a low hull under the
        // sun, the buildings it fights around, and the hillsides raking into shadow — not only the
        // ~30 m bubble around one tank. 3.1 cm still resolves running-gear detail through the 3×3 PCF
        // (a halved map on integrated GPUs lands at 6.3 cm, acceptably soft). Depth reach 100 m clears
        // hills along the sun axis over the wider footprint.
        Self { focus_radius_m: 64.0, depth_radius_m: 100.0, resolution: 4096 }
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

    /// The far cascade derived from this (near) cascade: a 4.5× focus box (64 m → 288 m half-box,
    /// so a receiver ~490 m out under a 200 m-forward centre still lands inside) at half the
    /// resolution — at ~0.56 m/texel the far field reads terrain and building shadows, which is
    /// all it needs; vehicle-scale detail lives in the near box. The deeper sun-axis reach clears
    /// hills across the wider footprint. Beyond this box the aerial-perspective haze owns the
    /// image, so a third cascade buys nothing (see docs/shadow-policy.md).
    pub fn far_cascade(&self) -> SunShadowParams {
        SunShadowParams {
            focus_radius_m: self.focus_radius_m * 4.5,
            // The sun-axis reach must contain EVERY potential occluder on a 1000 m map, not
            // just the box's own footprint: geometry between the light and the box's near
            // plane pancakes to depth 0, and under a grazing golden-evening sun (~14°) the
            // whole western map overlaps the eastern field's shadow UVs — a too-short reach
            // turns that pancake into a blanket of false shadow across the far field. 1100 m
            // covers a worst-case focus at the far map edge; the wider NDC depth span also
            // scales the fixed NDC bias to ~1.8 m of world slack, right for 0.5-1.1 m texels.
            depth_radius_m: 1100.0,
            resolution: (self.resolution / 2).max(512),
        }
    }
}

/// A stable up vector for the light view, avoiding degeneracy when the sun is near-vertical.
fn stable_up(sun_dir: Vec3) -> Vec3 {
    if sun_dir.y.abs() > 0.99 { Vec3::Z } else { Vec3::Y }
}

/// The auto shadow-focus centre for a chase camera: push the focus box forward along the camera's
/// **horizontal** look direction so the coverage lands on the field the player is looking at, not on
/// the empty ground behind a chase camera. `forward_offset_m` is how far ahead of the eye to centre
/// the box; the vertical stays at the eye height (the box's depth reach spans down to the ground).
///
/// The look direction is recovered by unprojecting the screen centre through the inverse
/// view-projection — a near and a far point on the centre ray — and dropping the vertical component.
/// A degenerate matrix (or straight-down view) yields no horizontal direction, so the eye position
/// is returned unchanged.
pub fn forward_shadow_focus(
    camera_pos: [f32; 3],
    view_proj: [[f32; 4]; 4],
    forward_offset_m: f32,
) -> [f32; 3] {
    let inv = Mat4::from_cols_array_2d(&view_proj).inverse();
    let unproject = |ndc_z: f32| {
        let clip = inv * Vec4::new(0.0, 0.0, ndc_z, 1.0);
        if clip.w.abs() < 1.0e-6 { Vec3::ZERO } else { clip.truncate() / clip.w }
    };
    let ray = unproject(1.0) - unproject(0.0);
    let horizontal = Vec3::new(ray.x, 0.0, ray.z);
    let Some(dir) = horizontal.try_normalize() else {
        return camera_pos;
    };
    (Vec3::from_array(camera_pos) + dir * forward_offset_m).to_array()
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
    fn forward_focus_pushes_the_box_along_the_horizontal_look_direction() {
        use crate::{Camera, view_projection_matrix};
        // A chase camera behind and above a tank at the origin, looking forward-down toward +Z.
        let camera =
            Camera { eye: [0.0, 6.0, -10.0], target: [0.0, 1.5, 0.0], vertical_fov_degrees: 45.0 };
        let view_proj = view_projection_matrix(&camera, 16.0 / 9.0, 0.1, 1000.0);
        let focus = forward_shadow_focus(camera.eye, view_proj, 40.0);
        // The box centre moves ~40 m ahead in +Z (the look direction), stays at eye height, and does
        // not drift sideways — coverage lands out in the field, not behind the camera.
        assert!(focus[2] > camera.eye[2] + 30.0, "focus should lead the eye in +Z: {focus:?}");
        assert!((focus[0] - camera.eye[0]).abs() < 1.0e-3, "no sideways drift: {focus:?}");
        assert!(
            (focus[1] - camera.eye[1]).abs() < 1.0e-3,
            "vertical stays at eye height: {focus:?}"
        );
        // The horizontal displacement is exactly the requested offset (the vertical is dropped).
        let dx = focus[0] - camera.eye[0];
        let dz = focus[2] - camera.eye[2];
        assert!(((dx * dx + dz * dz).sqrt() - 40.0).abs() < 1.0e-2, "offset magnitude: {focus:?}");
    }

    #[test]
    fn forward_focus_on_a_degenerate_matrix_returns_the_eye() {
        // A straight-down view (no horizontal look direction) leaves the focus at the eye rather than
        // producing a NaN centre.
        let eye = [3.0, 20.0, 7.0];
        assert_eq!(forward_shadow_focus(eye, [[0.0; 4]; 4], 40.0), eye);
    }

    #[test]
    fn the_far_cascade_footprint_strictly_contains_the_near_box() {
        // The cascade handoff contract: every world point the near map covers (its NDC inside
        // [-1, 1]) also lands strictly inside the far map, despite the two boxes being centred at
        // different forward offsets (40 m vs 200 m along the look). Sampled over a ground/height
        // grid spanning well past both boxes, under the usual oblique sun.
        let near = SunShadowParams::default();
        let far = near.far_cascade();
        let near_m = sun_light_view_projection(KEY, [40.0, 0.0, 0.0], near);
        let far_m = sun_light_view_projection(KEY, [200.0, 0.0, 0.0], far);
        let mut covered = 0;
        for xi in -12..=18 {
            for zi in -12..=12 {
                for y in [0.0, 12.0] {
                    let p = Vec3::new(xi as f32 * 20.0, y, zi as f32 * 20.0);
                    let near_ndc = project(&near_m, p);
                    if near_ndc.x.abs() > 1.0 || near_ndc.y.abs() > 1.0 {
                        continue;
                    }
                    covered += 1;
                    let far_ndc = project(&far_m, p);
                    assert!(
                        far_ndc.x.abs() < 1.0 && far_ndc.y.abs() < 1.0,
                        "near-covered point {p:?} must land inside the far box, got {far_ndc:?}"
                    );
                }
            }
        }
        assert!(covered > 30, "the grid must actually exercise the near box ({covered} points)");
    }

    #[test]
    fn a_point_250m_out_is_outside_the_near_box_and_inside_the_far_box() {
        // The capability the far cascade adds: a receiver/occluder 250 m from the near focus —
        // dead ground for the old single 64 m box — projects inside the far cascade's NDC.
        let near = SunShadowParams::default();
        let far = near.far_cascade();
        let near_m = sun_light_view_projection(KEY, [40.0, 0.0, 0.0], near);
        let far_m = sun_light_view_projection(KEY, [200.0, 0.0, 0.0], far);
        let p = Vec3::new(290.0, 0.0, 0.0);
        let near_ndc = project(&near_m, p);
        let far_ndc = project(&far_m, p);
        assert!(
            near_ndc.x.abs() > 1.0 || near_ndc.y.abs() > 1.0,
            "250 m past the near focus must miss the near box: {near_ndc:?}"
        );
        assert!(
            far_ndc.x.abs() < 1.0 && far_ndc.y.abs() < 1.0,
            "250 m past the near focus must land in the far box: {far_ndc:?}"
        );
    }

    #[test]
    fn the_far_cascade_snaps_to_its_own_texel_grid() {
        // The anti-shimmer contract holds per cascade: the far matrix snaps on the far map's
        // (coarser) texel grid, so the far field does not crawl as the camera moves either.
        let far = SunShadowParams::default().far_cascade();
        let m = sun_light_view_projection(KEY, [340.3, 1.1, 300.7], far);
        let ndc = project(&m, Vec3::new(340.3, 1.1, 300.7));
        let texels = ndc * (0.5 * far.resolution as f32);
        assert!((texels.x - texels.x.round()).abs() < 1.0e-3, "x off grid: {}", texels.x);
        assert!((texels.y - texels.y.round()).abs() < 1.0e-3, "y off grid: {}", texels.y);
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
