use game_core::HitboxProfile;
use glam::{Vec2, Vec3};
use terrain::StaticCoverObject;

pub(crate) const TANK_COLLISION_RADIUS_M: f32 = 1.6;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TankFootprint {
    pub half_width_m: f32,
    pub half_length_m: f32,
}

impl TankFootprint {
    pub fn from_hitbox(hitbox: HitboxProfile) -> Self {
        Self {
            half_width_m: hitbox.half_width_m.max(0.01),
            half_length_m: hitbox.half_length_m.max(0.01),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TankObstacle {
    pub center: Vec3,
    pub yaw_rad: f32,
    pub footprint: TankFootprint,
}

impl TankObstacle {
    pub fn new(center: Vec3, yaw_rad: f32, footprint: TankFootprint) -> Self {
        Self { center, yaw_rad, footprint }
    }

    pub fn from_hitbox(center: Vec3, yaw_rad: f32, hitbox: HitboxProfile) -> Self {
        Self::new(center, yaw_rad, TankFootprint::from_hitbox(hitbox))
    }
}

#[derive(Debug, Clone, Copy)]
pub struct TankWorldObstacles<'a> {
    pub cover: &'a [StaticCoverObject],
    pub tank_footprint: TankFootprint,
    pub tanks: &'a [TankObstacle],
    /// The map's standing water, if any: wading drag and the riverbed traction cut read the
    /// depth of water over the terrain contact (see [`crate::water`]). `None` keeps the dry
    /// path bit-identical (replay-locked).
    pub water: Option<terrain::WaterBody>,
}

impl<'a> TankWorldObstacles<'a> {
    pub fn new(
        cover: &'a [StaticCoverObject],
        tank_footprint: TankFootprint,
        tanks: &'a [TankObstacle],
    ) -> Self {
        Self { cover, tank_footprint, tanks, water: None }
    }

    pub fn with_water(mut self, water: Option<terrain::WaterBody>) -> Self {
        self.water = water;
        self
    }
}

pub fn default_tank_footprint() -> TankFootprint {
    TankFootprint { half_width_m: TANK_COLLISION_RADIUS_M, half_length_m: TANK_COLLISION_RADIUS_M }
}

/// Whether two hull footprints (OBBs in the XZ plane) touch within `slop_m`. This is the same
/// SAT used for movement blocking, with footprint `a` inflated by the slop, so "touching" can
/// never disagree with the shape that movement collided. Ramming uses a per-tick dynamic slop
/// (base + closing distance per tick) so fast closing hulls cannot tunnel past the contact.
pub fn tank_footprints_touch(a: &TankObstacle, b: &TankObstacle, slop_m: f32) -> bool {
    let inflated = TankObstacle {
        footprint: TankFootprint {
            half_width_m: a.footprint.half_width_m + slop_m.max(0.0),
            half_length_m: a.footprint.half_length_m + slop_m.max(0.0),
        },
        ..*a
    };
    obstacles_overlap(&inflated, b)
}

/// Zero the world-axis velocity components that the resolver had to drop back to `previous`,
/// leaving the sliding axis intact. The axis-separated resolver only ever holds whole world axes,
/// so killing exactly those axes keeps the surviving velocity tangent to the obstacle (the hull
/// slides along a wall instead of either sticking or keeping a phantom into-wall component).
pub(crate) fn trim_velocity(
    previous: Vec3,
    attempted: Vec3,
    resolved: Vec3,
    velocity: Vec3,
) -> Vec3 {
    let mut trimmed = velocity;
    if axis_blocked(previous.x, attempted.x, resolved.x) {
        trimmed.x = 0.0;
    }
    if axis_blocked(previous.z, attempted.z, resolved.z) {
        trimmed.z = 0.0;
    }
    trimmed
}

/// True when a move along one world axis was attempted but the resolver pinned it back to the
/// previous value (i.e. that axis is blocked by an obstacle).
fn axis_blocked(previous: f32, attempted: f32, resolved: f32) -> bool {
    (attempted - previous).abs() > 1.0e-6 && (resolved - previous).abs() <= 1.0e-5
}

/// XZ-plane separating-axis overlap between two oriented hull footprints.
pub(crate) fn obstacles_overlap(a: &TankObstacle, b: &TankObstacle) -> bool {
    let center_a = Vec2::new(a.center.x, a.center.z);
    let center_b = Vec2::new(b.center.x, b.center.z);
    let [right_a, forward_a] = footprint_axes(a.yaw_rad);
    let [right_b, forward_b] = footprint_axes(b.yaw_rad);
    let delta = center_b - center_a;
    for axis in [right_a, forward_a, right_b, forward_b] {
        let radius_a = a.footprint.half_width_m * axis.dot(right_a).abs()
            + a.footprint.half_length_m * axis.dot(forward_a).abs();
        let radius_b = b.footprint.half_width_m * axis.dot(right_b).abs()
            + b.footprint.half_length_m * axis.dot(forward_b).abs();
        if delta.dot(axis).abs() >= radius_a + radius_b - 1.0e-5 {
            return false;
        }
    }
    true
}

fn footprint_axes(yaw_rad: f32) -> [Vec2; 2] {
    let forward = Vec2::new(yaw_rad.sin(), yaw_rad.cos());
    let right = Vec2::new(forward.y, -forward.x);
    [right, forward]
}
