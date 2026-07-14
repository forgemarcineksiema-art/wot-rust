//! The thrown track lying on the ground (Honest Steel epilogue, Inna Liga D6): the third beat
//! of de-track readability. The dangle and the lean already say "broken"; this says "thrown" —
//! a ribbon of shoe links shed behind the broken side in a lazy S-curve, posed once at the
//! break (deterministic from tank and side, like the turret pop-off) and inert forever after.
//! Instanced from the same unit link mesh the live track scrolls, so the shed steel IS the
//! track's steel.

use game_core::{TankId, TrackSide, VehicleKind};
use glam::{Mat4, Vec3};
use terrain::HeightMap;

/// Links in one shed ribbon; at ~0.32 m spacing the ribbon reads ~4 m of thrown track.
pub(crate) const RIBBON_LINK_COUNT: usize = 13;
/// Shed ribbons alive at once across the battle; past the budget the oldest is recycled.
/// 6 ribbons x 13 links rides inside the vehicle instance budget's slack.
pub(crate) const MAX_TRACK_RIBBONS: usize = 6;
const LINK_SPACING_M: f32 = 0.32;
/// Rest height of a link lying flat on the soil.
const LINK_REST_Y_M: f32 = 0.05;

#[derive(Debug, Clone)]
pub(crate) struct TrackRibbon {
    pub tank_id: TankId,
    pub vehicle: VehicleKind,
    pub side: TrackSide,
    links: Vec<Mat4>,
}

impl TrackRibbon {
    /// Pose the shed ribbon once, at the moment the track throws: it starts under the broken
    /// side and trails backwards in an S-curve — the shape a ribbon takes when the hull keeps
    /// rolling off it. Deterministic from (tank, side): every client lays the same steel.
    pub fn shed(
        tank_id: TankId,
        vehicle: VehicleKind,
        side: TrackSide,
        hull_position: Vec3,
        hull_yaw_rad: f32,
        heightmap: Option<&HeightMap>,
    ) -> Self {
        let hitbox = game_core::HitboxProfile::for_vehicle(vehicle);
        let (sin, cos) = hull_yaw_rad.sin_cos();
        let rotate = |local: Vec3| {
            Vec3::new(cos * local.x + sin * local.z, local.y, -sin * local.x + cos * local.z)
        };
        let x_sign = match side {
            TrackSide::Left => -1.0,
            TrackSide::Right => 1.0,
        };
        let mut seed = tank_id.0 ^ (((x_sign > 0.0) as u64) << 32) ^ 0x7124_8B0F_5EED_0001;
        let phase = game_core::math::next_hash_unit(&mut seed) * std::f32::consts::TAU;
        let sway = 0.28 + game_core::math::next_hash_unit(&mut seed) * 0.22;

        let start =
            hull_position + rotate(Vec3::new(x_sign * hitbox.half_width_m * 0.86, 0.0, 0.0));
        let links = (0..RIBBON_LINK_COUNT)
            .map(|index| {
                let along = index as f32 * LINK_SPACING_M;
                // Behind the hull, with a lateral S: two half-waves over the ribbon's run.
                let s = (along * 1.6 + phase).sin() * sway;
                let local = Vec3::new(x_sign * s, 0.0, -along);
                let flat = start + rotate(Vec3::new(local.x, 0.0, local.z));
                let ground_y = heightmap
                    .and_then(|map| map.sample_height(flat.x, flat.z))
                    .unwrap_or(hull_position.y);
                let position = Vec3::new(flat.x, ground_y + LINK_REST_Y_M, flat.z);
                // Each link yaws with the curve's tangent plus a hashed wobble — shed steel is
                // never a perfect chain.
                let tangent = (along * 1.6 + phase).cos() * sway * 1.6 * x_sign;
                let wobble = (game_core::math::next_hash_unit(&mut seed) - 0.5) * 0.5;
                let yaw = hull_yaw_rad + tangent + wobble;
                Mat4::from_translation(position) * Mat4::from_rotation_y(yaw)
            })
            .collect();
        Self { tank_id, vehicle, side, links }
    }

    pub fn link_transforms(&self) -> &[Mat4] {
        &self.links
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_shed_ribbon_lies_on_the_ground_behind_the_broken_side() {
        let map = HeightMap::flat(65, 65, 1.0, 2.0).expect("valid map");
        // Hull rides suspension at y=2.8, facing +Z; the ribbon must sit on the 2.0 ground.
        let ribbon = TrackRibbon::shed(
            TankId(7),
            VehicleKind::T54_1951,
            TrackSide::Left,
            Vec3::new(30.0, 2.8, 30.0),
            0.0,
            Some(&map),
        );
        assert_eq!(ribbon.link_transforms().len(), RIBBON_LINK_COUNT);
        for link in ribbon.link_transforms() {
            let position = link.w_axis.truncate();
            assert!(
                (position.y - (2.0 + LINK_REST_Y_M)).abs() < 1.0e-3,
                "every link rests on the sampled ground, got y {}",
                position.y
            );
            assert!(position.z <= 30.0 + 1.0e-3, "the ribbon trails BEHIND the hull");
            assert!(position.x < 30.0, "a left-side throw lands on the left flank");
        }
        // The S-curve actually sways — the ribbon is not a ruler line.
        let xs: Vec<f32> = ribbon.link_transforms().iter().map(|l| l.w_axis.x).collect();
        let spread = xs.iter().copied().fold(f32::MIN, f32::max)
            - xs.iter().copied().fold(f32::MAX, f32::min);
        assert!(spread > 0.15, "shed steel takes an S, not a line: lateral spread {spread}");
    }

    #[test]
    fn the_pose_is_deterministic_per_tank_and_side_and_differs_between_them() {
        let ribbon = |tank: u64, side: TrackSide| {
            TrackRibbon::shed(
                TankId(tank),
                VehicleKind::T54_1951,
                side,
                Vec3::new(10.0, 0.5, 10.0),
                0.7,
                None,
            )
        };
        let a = ribbon(3, TrackSide::Left);
        let b = ribbon(3, TrackSide::Left);
        assert_eq!(a.link_transforms(), b.link_transforms(), "every client lays the same steel");
        let right = ribbon(3, TrackSide::Right);
        assert_ne!(
            a.link_transforms()[2],
            right.link_transforms()[2],
            "the two sides shed differently"
        );
    }
}
