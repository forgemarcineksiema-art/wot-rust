//! The hero under the cursor (interface program G8): the cursor's ray through the garage camera
//! against the hero's armour volumes — the same trace a shell meets, at the parked pose with
//! the turret where the player left it — and the drag that turns the turret. A press on the
//! hero starts one of two drags: on a turret plate it turns the turret and nothing else; on any
//! other plate it orbits the camera, and a press that never travelled is a click on the module
//! the plate belongs to.

use game_core::math::{HullPose, wrap_angle};
use game_core::{ArmorFacing, ArmorZone, TankId};
use glam::{Mat4, Vec3, Vec4};
use renderer_api::{CameraProjectionPolicy, view_projection_matrix};
use sim::{SegmentImpact, ShellTraceWorld, TraceTank, segment_impact};

use super::GarageState;
use super::draft::FitSlot;
use super::types::Drag;

/// What the cursor's ray met on the hero.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct HeroHit {
    pub zone: ArmorZone,
    /// The loadout slot the struck plate belongs to, as the eye reads it.
    pub slot: Option<FitSlot>,
    /// Whether the plate rides the turret: a drag there turns it.
    pub turret: bool,
    /// The trace's own record of the plate (G11): what the inspector asks the resolver with.
    pub facing: ArmorFacing,
    pub hit_position: Vec3,
    pub plate_normal: Vec3,
    pub impact_angle_degrees: f32,
    pub thickness_scale: f32,
    /// The ray's direction at the plate.
    pub direction: Vec3,
}

/// Radians of turret per pixel of drag.
const TURRET_DRAG_RAD_PER_PX: f32 = 0.006;
/// A press that travelled less than this before its release is a click, not a drag.
pub(super) const CLICK_TRAVEL_PX: f32 = 6.0;
/// Farther than the hall is long: the ray meets the hero or nothing.
const RAY_LENGTH_M: f32 = 80.0;

/// The slot a plate belongs to, as the eye reads the hero: the turret's plates are the turret,
/// the mantlet is the gun, the deck and the rear are the engine, the running gear is the
/// suspension, the hull's front and flanks are the hull. The radio has no plate of its own.
pub(super) fn slot_of_zone(zone: ArmorZone) -> Option<FitSlot> {
    Some(match zone {
        ArmorZone::Mantlet => FitSlot::Gun,
        ArmorZone::TurretFront
        | ArmorZone::TurretSide
        | ArmorZone::TurretRear
        | ArmorZone::Roof
        | ArmorZone::Cupola => FitSlot::Turret,
        ArmorZone::HullDeck | ArmorZone::HullRear => FitSlot::Engine,
        ArmorZone::LeftTrack | ArmorZone::RightTrack | ArmorZone::Skirt => FitSlot::Suspension,
        ArmorZone::UpperGlacis
        | ArmorZone::LowerPlate
        | ArmorZone::HullSide
        | ArmorZone::GlacisPort => FitSlot::Hull,
    })
}

/// Whether a zone traverses with the turret.
fn is_turret_zone(zone: ArmorZone) -> bool {
    matches!(
        zone,
        ArmorZone::Mantlet
            | ArmorZone::TurretFront
            | ArmorZone::TurretSide
            | ArmorZone::TurretRear
            | ArmorZone::Roof
            | ArmorZone::Cupola
    )
}

impl GarageState {
    /// The cursor's ray in the hall — eye and unit direction — through the orbit camera the
    /// frame is drawn with, in the viewport the screen is laid out in. `None` off the frame.
    pub(super) fn hero_ray(&self) -> Option<(Vec3, Vec3)> {
        let clip = self.cursor_clip;
        if clip[0].abs() > 1.0 || clip[1].abs() > 1.0 {
            return None;
        }
        let aspect = self.viewport_px[0].max(1.0) / self.viewport_px[1].max(1.0);
        let projection = CameraProjectionPolicy::webgpu_default();
        let view_proj = view_projection_matrix(
            &self.orbit_camera(),
            aspect,
            projection.near_plane_m(),
            projection.far_plane_m(),
        );
        let inverse = Mat4::from_cols_array_2d(&view_proj).inverse();
        let unproject = |ndc_z: f32| {
            let p = inverse * Vec4::new(clip[0], clip[1], ndc_z, 1.0);
            (p.w.abs() > 1.0e-6).then(|| p.truncate() / p.w)
        };
        let near = unproject(0.0)?;
        let far = unproject(1.0)?;
        let direction = (far - near).try_normalize()?;
        Some((near, direction))
    }

    /// The hero plate under the cursor, through the shell trace's own volumes at the parked
    /// pose and the turret's current traverse. `None` over the floor, the walls or the air.
    pub(super) fn hero_hit(&self) -> Option<HeroHit> {
        let (origin, direction) = self.hero_ray()?;
        let pose = self.drive_in_pose();
        let position = Vec3::new(0.0, scene_build::hangar::TURNTABLE_TOP_M, pose.z);
        let tank = TraceTank::for_kind(
            TankId(0),
            position,
            HullPose::level(pose.yaw_rad),
            self.hero_turret_yaw,
            self.selected_vehicle(),
        );
        let tanks = [tank];
        let world = ShellTraceWorld {
            projectile_radius_m: 0.0,
            tanks: &tanks,
            blockers: &[],
            heightmap: None,
            cover: &[],
            rubble: &[],
            water: terrain::WaterView::DRY,
        };
        match segment_impact(origin, origin + direction * RAY_LENGTH_M, direction, &world)? {
            SegmentImpact::Tank {
                zone,
                facing,
                impact_angle_degrees,
                hit_position,
                plate_normal,
                thickness_scale,
                ..
            } => Some(HeroHit {
                zone,
                slot: slot_of_zone(zone),
                turret: is_turret_zone(zone),
                facing,
                hit_position,
                plate_normal,
                impact_angle_degrees,
                thickness_scale,
                direction,
            }),
            _ => None,
        }
    }

    /// The turret's traverse on the parked hero, hull-relative — the render and the inspector
    /// draw it, the pick traces against it.
    pub(in crate::app) fn hero_turret_yaw(&self) -> f32 {
        self.hero_turret_yaw
    }

    /// A press on the scene: on a turret plate it takes the turret, anywhere else the camera —
    /// and remembers the module under the press, in case it never travels.
    pub(super) fn press_scene(&mut self) {
        self.drag_travel_px = 0.0;
        self.hero_press = None;
        self.idle_seconds = 0.0;
        self.drag = match self.hero_hit() {
            Some(hit) if hit.turret => {
                self.hero_press = Some(hit);
                Drag::Turret
            }
            Some(hit) => {
                self.hero_press = Some(hit);
                Drag::Camera
            }
            None => Drag::Camera,
        };
    }

    /// The press is over: whatever drag it started ends, and if it never travelled and began
    /// on the hero, the plate it began on is the click.
    pub(super) fn end_press(&mut self) -> Option<HeroHit> {
        let click = self.hero_press.take().filter(|_| self.drag_travel_px < CLICK_TRAVEL_PX);
        self.drag = Drag::None;
        click
    }

    /// The turret drag: `dx` pixels of travel turn the turret and nothing else.
    pub(super) fn turn_turret(&mut self, dx: f32) {
        self.hero_turret_yaw = wrap_angle(self.hero_turret_yaw + dx * TURRET_DRAG_RAD_PER_PX);
    }
}

/// Scan the frame for a cursor position whose hero hit satisfies `pick` — the locks' way to
/// find a plate, and the review goldens' (G11); the cursor is left there, raw — the caller
/// feeds it through `set_cursor`.
pub(super) fn hero_point_where(
    state: &mut GarageState,
    pick: impl Fn(HeroHit) -> bool,
) -> Option<[f32; 2]> {
    for iy in 0..40 {
        for ix in 0..64 {
            let clip = [-0.9 + 1.8 * ix as f32 / 63.0, 0.9 - 1.8 * iy as f32 / 39.0];
            state.cursor_clip = clip;
            if state.hero_hit().is_some_and(&pick) {
                return Some(clip);
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use game_core::VehicleKind;

    /// The ray through the camera lands on the hero where the eye sees it: every module's
    /// plates are reachable from the hero framing, the floor beside it is nothing, and the
    /// zone the ray names is the slot the eye reads.
    #[test]
    fn the_cursor_ray_meets_the_heros_plates_and_names_their_slots() {
        let mut state = GarageState::default();
        state.open();
        state.select_vehicle(VehicleKind::BENCHMARK);
        for slot in
            [FitSlot::Turret, FitSlot::Gun, FitSlot::Hull, FitSlot::Engine, FitSlot::Suspension]
        {
            assert!(
                hero_point_where(&mut state, |hit| hit.slot == Some(slot)).is_some(),
                "{slot:?} has no plate under the hero framing"
            );
        }
        assert!(hero_point_where(&mut state, |hit| hit.turret).is_some());
        state.set_cursor([0.95, 0.95]);
        assert_eq!(state.hero_hit(), None, "the roof of the hall is not the hero");
        state.set_cursor([3.0, 3.0]);
        assert_eq!(state.hero_ray(), None, "off the frame there is no ray");
        for zone in [ArmorZone::Mantlet, ArmorZone::TurretSide, ArmorZone::Cupola] {
            assert!(is_turret_zone(zone));
        }
        assert!(!is_turret_zone(ArmorZone::HullDeck));
        assert_eq!(slot_of_zone(ArmorZone::HullDeck), Some(FitSlot::Engine));
    }
}
