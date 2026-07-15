//! The thrown track lying on the ground (Honest Steel epilogue, Inna Liga D6; re-cut for
//! Fizyczny Świat): the loop that left the wheels. A real shed track is a CONTINUOUS band of
//! several hundred kilos of steel — links touching link-to-link at the belt's own pitch, the
//! full loop's worth of them, lying flat along the line of travel in a lazy S with one crumple
//! where it folded and a curl at the far end. Posed once at the break (deterministic from tank
//! and side, like the turret pop-off) and inert forever after. Instanced from the same unit
//! link mesh the live track scrolls, so the shed steel IS the track's steel — and the live
//! belt on that side draws nothing, because the loop is HERE now.

use game_core::{TankId, TrackSide, VehicleKind};
use glam::{Mat4, Vec3};
use terrain::HeightMap;

/// Hard cap on links per shed band (a T-54 loop is ~90; headroom for longer runs). The real
/// count comes from the vehicle's own belt so the steel on the ground is the steel missing
/// from the wheels.
pub(crate) const MAX_RIBBON_LINKS: usize = 112;
/// Shed ribbons alive at once across the battle; past the budget the oldest is recycled.
pub(crate) const MAX_TRACK_RIBBONS: usize = 6;
/// Fallbacks for a vehicle without authored gear kinematics.
const FALLBACK_LINK_COUNT: usize = 90;
const FALLBACK_PITCH_M: f32 = 0.15;
/// Rest height of a link lying flat on the soil.
const LINK_REST_Y_M: f32 = 0.05;

#[derive(Debug, Clone)]
pub struct TrackRibbon {
    pub tank_id: TankId,
    pub vehicle: VehicleKind,
    pub side: TrackSide,
    /// Seconds since the throw — drives the hanging remnant's slide-off (phase 2).
    pub age_s: f32,
    links: Vec<Mat4>,
}

impl TrackRibbon {
    /// Pose the shed band once, at the moment the track throws. The hull rolled off the loop,
    /// so the band starts a little AHEAD of the hull's centre on the broken side and trails
    /// back along the line of travel; the shape is integrated curvature — a gentle wander, one
    /// hard crumple where the band folded as the hull left it, and a tight curl at the tail.
    /// Deterministic from (tank, side): every client lays the same steel.
    pub fn shed(
        tank_id: TankId,
        vehicle: VehicleKind,
        side: TrackSide,
        hull_position: Vec3,
        hull_yaw_rad: f32,
        heightmap: Option<&HeightMap>,
    ) -> Self {
        let hitbox = game_core::HitboxProfile::for_vehicle(vehicle);
        let (count, pitch) = vehicle_geometry::RunningGearKinematics::for_vehicle(vehicle)
            .map(|kin| {
                let count = kin.link_count().min(MAX_RIBBON_LINKS);
                (count, kin.belt_length() / kin.link_count().max(1) as f32)
            })
            .unwrap_or((FALLBACK_LINK_COUNT, FALLBACK_PITCH_M));
        let total_m = count as f32 * pitch;

        let (sin, cos) = hull_yaw_rad.sin_cos();
        let rotate = |local: Vec3| {
            Vec3::new(cos * local.x + sin * local.z, local.y, -sin * local.x + cos * local.z)
        };
        let x_sign = match side {
            TrackSide::Left => -1.0,
            TrackSide::Right => 1.0,
        };
        let mut seed = tank_id.0 ^ (((x_sign > 0.0) as u64) << 32) ^ 0x7124_8B0F_5EED_0001;
        let mut next =
            move |scale: f32, base: f32| base + game_core::math::next_hash_unit(&mut seed) * scale;
        // The lazy wander: low curvature, one slow half-wave over the band's length.
        let wander_k = next(0.06, 0.05);
        let wander_phase = next(std::f32::consts::TAU, 0.0);
        // The crumple: a hard localized bend (integrates to ~110-150 degrees) somewhere past
        // the middle — the fold the band takes as the last road wheel rolls off it.
        let fold_at = total_m * next(0.2, 0.55);
        // The fold always breaks OUTWARD: the hull rolls off toward its own line of travel,
        // the band stays on the outside — positive local curvature bends toward -x, so the
        // outward sign is the opposite of the side's. (Keeps the shed steel reading as
        // "the LEFT track" at a glance, mass on the flank it left.)
        let fold_sign = -x_sign;
        let fold_k = fold_sign * next(1.2, 2.4);
        // The tail curl: the loose end always comes to rest bent.
        let curl_k = fold_sign * -next(0.5, 1.0);
        let curl_from = total_m - next(0.5, 1.1);

        // Integrate the band link by link in hull-local space: x lateral, -z backwards.
        let mut local =
            Vec3::new(x_sign * hitbox.half_width_m * 0.86, 0.0, hitbox.half_length_m * 0.35);
        let mut heading = std::f32::consts::PI; // facing local -Z: trailing behind
        let links = (0..count)
            .map(|index| {
                let along = index as f32 * pitch;
                let flat = hull_position + rotate(local);
                let ground_y = heightmap
                    .and_then(|map| map.sample_height(flat.x, flat.z))
                    .unwrap_or(hull_position.y);
                let position = Vec3::new(flat.x, ground_y + LINK_REST_Y_M, flat.z);
                // The link's yaw IS the band's tangent — a chain, not scattered dominoes.
                let transform = Mat4::from_translation(position)
                    * Mat4::from_rotation_y(hull_yaw_rad + heading);
                // Advance along the integrated heading.
                let (step_sin, step_cos) = heading.sin_cos();
                local += Vec3::new(step_sin, 0.0, step_cos) * pitch;
                let mut curvature = wander_k * (along * 0.5 + wander_phase).sin();
                let from_fold = (along - fold_at) / 0.45;
                curvature += fold_k * (-from_fold * from_fold).exp();
                if along > curl_from {
                    curvature += curl_k;
                }
                heading += curvature * pitch;
                transform
            })
            .collect();
        Self { tank_id, vehicle, side, age_s: 0.0, links }
    }

    pub fn link_transforms(&self) -> &[Mat4] {
        &self.links
    }
}

/// Dark bare steel — shed links stop wearing the vehicle's paint the moment they leave it.
pub(crate) const SHED_STEEL_TINT: [f32; 3] = [0.16, 0.15, 0.14];

/// The hanging remnant as render objects (phase 2): the last stretch of the thrown band still
/// draped over the drive sprocket, posed hull-local by the kernel and carried by the given
/// hull transform. `slide_m` > 0 is the slide-off in progress; an empty result means the
/// remnant has fully joined the band on the field.
pub fn thrown_remnant_objects(
    catalog: &mut crate::VehicleAssetCatalog,
    tank_id: TankId,
    vehicle: VehicleKind,
    side: TrackSide,
    hull_transform: Mat4,
    slide_m: f32,
) -> Vec<renderer_api::RenderObject> {
    let Some(kin) = vehicle_geometry::RunningGearKinematics::for_vehicle(vehicle) else {
        return Vec::new();
    };
    let side_sign = match side {
        TrackSide::Left => -1.0,
        TrackSide::Right => 1.0,
    };
    let placements = vehicle_geometry::thrown_remnant_placements(&kin, side_sign, slide_m);
    if placements.is_empty() {
        return Vec::new();
    }
    let Some(entry) = catalog.vehicle_entry(vehicle) else {
        return Vec::new();
    };
    let Some(gear) = entry.running_gear else {
        return Vec::new();
    };
    placements
        .iter()
        .map(|placement| renderer_api::RenderObject {
            tank_id: Some(tank_id),
            mesh: gear.link,
            material: entry.material,
            transform: (hull_transform * placement.transform).to_cols_array_2d(),
            tint: SHED_STEEL_TINT,
        })
        .collect()
}

/// The shed band as render objects: the same unit link mesh the live track scrolls, instanced
/// along the frozen curve. THE one production path — the battle frame and the review renders
/// both draw the thrown steel through here.
pub fn ribbon_render_objects(
    catalog: &mut crate::VehicleAssetCatalog,
    ribbon: &TrackRibbon,
) -> Vec<renderer_api::RenderObject> {
    let Some(entry) = catalog.vehicle_entry(ribbon.vehicle) else {
        return Vec::new();
    };
    let Some(gear) = entry.running_gear else {
        return Vec::new();
    };
    ribbon
        .link_transforms()
        .iter()
        .map(|transform| renderer_api::RenderObject {
            tank_id: Some(ribbon.tank_id),
            mesh: gear.link,
            material: entry.material,
            transform: transform.to_cols_array_2d(),
            tint: SHED_STEEL_TINT,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_shed_band_is_the_whole_loop_lying_continuously_on_the_ground() {
        let map = HeightMap::flat(129, 129, 1.0, 2.0).expect("valid map");
        // Hull rides suspension at y=2.8, facing +Z; the band must sit on the 2.0 ground.
        let ribbon = TrackRibbon::shed(
            TankId(7),
            VehicleKind::T54_1951,
            TrackSide::Left,
            Vec3::new(60.0, 2.8, 60.0),
            0.0,
            Some(&map),
        );
        let kin = vehicle_geometry::RunningGearKinematics::for_vehicle(VehicleKind::T54_1951)
            .expect("T-54 gear");
        // The steel on the ground is the steel missing from the wheels: the FULL loop.
        assert_eq!(ribbon.link_transforms().len(), kin.link_count().min(MAX_RIBBON_LINKS));

        let positions: Vec<Vec3> =
            ribbon.link_transforms().iter().map(|l| l.w_axis.truncate()).collect();
        let pitch = kin.belt_length() / kin.link_count() as f32;
        for pair in positions.windows(2) {
            let gap = (pair[1] - pair[0]).length();
            // Continuous band: consecutive links touch at the belt's own pitch — the shed
            // track is a ribbon, never a trail of scattered dominoes.
            assert!(
                gap <= pitch * 1.05 + 1.0e-4,
                "links touch link-to-link: gap {gap} vs pitch {pitch}"
            );
        }
        for position in &positions {
            assert!(
                (position.y - (2.0 + LINK_REST_Y_M)).abs() < 1.0e-3,
                "every link rests on the sampled ground, got y {}",
                position.y
            );
        }
        // It came OFF the left side: the head of the band (the links that just left the
        // wheels) and its centre of mass lie on the left flank — the folded tail may
        // honestly wander across the centreline behind the hull.
        for position in &positions[..positions.len() / 4] {
            assert!(position.x < 60.0, "the band's head lies under the left wheels");
        }
        let centroid_x = positions.iter().map(|p| p.x).sum::<f32>() / positions.len() as f32;
        assert!(centroid_x < 60.0, "the band's mass lies on the left flank: {centroid_x}");
        // It trails the line of travel: the band's centre of mass sits clearly BEHIND the
        // hull centre even though the first links start under the forward wheels.
        let centroid_z = positions.iter().map(|p| p.z).sum::<f32>() / positions.len() as f32;
        assert!(centroid_z < 60.0 - 1.0, "the band trails behind: centroid z {centroid_z}");
        // And it is not a ruler line: the wander plus the crumple bend it visibly.
        let xs: Vec<f32> = positions.iter().map(|p| p.x).collect();
        let spread = xs.iter().copied().fold(f32::MIN, f32::max)
            - xs.iter().copied().fold(f32::MAX, f32::min);
        assert!(spread > 0.4, "shed steel wanders and crumples: lateral spread {spread}");
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
