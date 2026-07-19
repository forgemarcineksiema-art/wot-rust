use glam::Vec3;

use crate::{LocalLight, SceneLighting};

fn scalar(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

fn vector(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    Vec3::from_array(a).lerp(Vec3::from_array(b), t).to_array()
}

fn direction(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    Vec3::from_array(a)
        .lerp(Vec3::from_array(b), t)
        .normalize_or(Vec3::from_array(a).normalize_or(Vec3::Y))
        .to_array()
}

fn shortest_angle_lerp(a: f32, b: f32, t: f32) -> f32 {
    let tau = std::f32::consts::TAU;
    let delta = (b - a + std::f32::consts::PI).rem_euclid(tau) - std::f32::consts::PI;
    a + delta * t
}

fn local_light(a: LocalLight, b: LocalLight, t: f32) -> LocalLight {
    LocalLight {
        position: vector(a.position, b.position, t),
        radius_m: scalar(a.radius_m, b.radius_m, t),
        rgb: vector(a.rgb, b.rgb, t),
        intensity: scalar(a.intensity, b.intensity, t),
    }
}

impl SceneLighting {
    /// Blend two authored looks without introducing non-unit light directions or rotating a storm
    /// front the long way around. Outdoor timelines use this once per one-second keyframe.
    pub fn lerp(self, other: Self, amount: f32) -> Self {
        let t = amount.clamp(0.0, 1.0);
        Self {
            ambient_rgb: vector(self.ambient_rgb, other.ambient_rgb, t),
            ground_ambient_rgb: vector(self.ground_ambient_rgb, other.ground_ambient_rgb, t),
            key_direction: direction(self.key_direction, other.key_direction, t),
            key_rgb: vector(self.key_rgb, other.key_rgb, t),
            fill_direction: direction(self.fill_direction, other.fill_direction, t),
            fill_rgb: vector(self.fill_rgb, other.fill_rgb, t),
            rim_direction: direction(self.rim_direction, other.rim_direction, t),
            rim_rgb: vector(self.rim_rgb, other.rim_rgb, t),
            sky_zenith_rgb: vector(self.sky_zenith_rgb, other.sky_zenith_rgb, t),
            sky_horizon_rgb: vector(self.sky_horizon_rgb, other.sky_horizon_rgb, t),
            fog_density: scalar(self.fog_density, other.fog_density, t),
            fog_height_falloff: scalar(self.fog_height_falloff, other.fog_height_falloff, t),
            exposure: scalar(self.exposure, other.exposure, t),
            black_point: scalar(self.black_point, other.black_point, t),
            saturation: scalar(self.saturation, other.saturation, t),
            contrast: scalar(self.contrast, other.contrast, t),
            cloud_coverage_bias: scalar(self.cloud_coverage_bias, other.cloud_coverage_bias, t),
            cloud_scale: scalar(self.cloud_scale, other.cloud_scale, t),
            cloud_opacity: scalar(self.cloud_opacity, other.cloud_opacity, t),
            cloud_drift: scalar(self.cloud_drift, other.cloud_drift, t),
            cloud_shadow_strength: scalar(
                self.cloud_shadow_strength,
                other.cloud_shadow_strength,
                t,
            ),
            fog_sun_scatter: scalar(self.fog_sun_scatter, other.fog_sun_scatter, t),
            sun_softness: scalar(self.sun_softness, other.sun_softness, t),
            valley_haze_density: scalar(self.valley_haze_density, other.valley_haze_density, t),
            valley_haze_height_m: scalar(self.valley_haze_height_m, other.valley_haze_height_m, t),
            god_ray_strength: scalar(self.god_ray_strength, other.god_ray_strength, t),
            cloud_sheet_opacity: scalar(self.cloud_sheet_opacity, other.cloud_sheet_opacity, t),
            cloud_sheet_scale: scalar(self.cloud_sheet_scale, other.cloud_sheet_scale, t),
            storm_front_dir_rad: shortest_angle_lerp(
                self.storm_front_dir_rad,
                other.storm_front_dir_rad,
                t,
            ),
            storm_front_strength: scalar(self.storm_front_strength, other.storm_front_strength, t),
            bloom_weight: scalar(self.bloom_weight, other.bloom_weight, t),
            vignette: scalar(self.vignette, other.vignette, t),
            local_lights: std::array::from_fn(|index| {
                local_light(self.local_lights[index], other.local_lights[index], t)
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blended_directions_remain_normalized_and_angles_take_short_arc() {
        let a = SceneLighting::bystra_clear_afternoon();
        let mut b = SceneLighting::bystra_rain();
        b.storm_front_dir_rad = std::f32::consts::TAU - 0.1;
        let mut start = a;
        start.storm_front_dir_rad = 0.1;
        let blended = start.lerp(b, 0.5);
        assert!((Vec3::from_array(blended.key_direction).length() - 1.0).abs() < 1.0e-5);
        assert!(blended.storm_front_dir_rad.abs() < 0.11);
    }
}
