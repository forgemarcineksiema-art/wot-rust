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
    /// Collapsed buildings, as GROUND (see [`terrain::RubbleMound`]). Empty on a battlefield
    /// nothing has knocked down yet, which keeps the untouched path bit-identical.
    pub rubble: &'a [terrain::RubbleMound],
    /// The map's ground rule (see [`terrain::GroundClassifier`]) — what the surface under the
    /// tracks IS. `None` is bare grass everywhere, which is exactly the model before material
    /// existed, so terrain-free modes and old fixtures stay bit-identical.
    pub ground: Option<&'a terrain::GroundClassifier>,
}

impl<'a> TankWorldObstacles<'a> {
    pub fn new(
        cover: &'a [StaticCoverObject],
        tank_footprint: TankFootprint,
        tanks: &'a [TankObstacle],
    ) -> Self {
        Self { cover, tank_footprint, tanks, water: None, rubble: &[], ground: None }
    }

    pub fn with_water(mut self, water: Option<terrain::WaterBody>) -> Self {
        self.water = water;
        self
    }

    pub fn with_rubble(mut self, rubble: &'a [terrain::RubbleMound]) -> Self {
        self.rubble = rubble;
        self
    }

    pub fn with_ground(mut self, ground: Option<&'a terrain::GroundClassifier>) -> Self {
        self.ground = ground;
        self
    }
}

pub fn default_tank_footprint() -> TankFootprint {
    TankFootprint { half_width_m: TANK_COLLISION_RADIUS_M, half_length_m: TANK_COLLISION_RADIUS_M }
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

/// How two overlapping footprints are into each other: the axis of LEAST penetration (the
/// minimum translation vector of the same SAT), pointing from `a` toward `b`, and the depth along
/// it. Separating them means moving `a` back along `normal` and `b` forward along it.
///
/// This is what turns a collision from a veto into a physical event. A blocking test only ever
/// needed "do these touch"; an impulse needs to know along WHICH direction the contact acts,
/// and a positional correction needs to know by how much.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FootprintContact {
    /// Unit contact normal in the XZ plane, from `a` toward `b`.
    pub normal: Vec2,
    /// Penetration depth along `normal`, always positive.
    pub depth_m: f32,
}

/// XZ-plane separating-axis test between two oriented hull footprints, reporting the contact when
/// they overlap. [`obstacles_overlap`] is this test's yes/no answer and stays bit-identical to it:
/// the separation threshold is the same, so no blocking verdict anywhere moves.
pub(crate) fn obstacles_contact(a: &TankObstacle, b: &TankObstacle) -> Option<FootprintContact> {
    let center_a = Vec2::new(a.center.x, a.center.z);
    let center_b = Vec2::new(b.center.x, b.center.z);
    let [right_a, forward_a] = footprint_axes(a.yaw_rad);
    let [right_b, forward_b] = footprint_axes(b.yaw_rad);
    let delta = center_b - center_a;
    let mut shallowest: Option<FootprintContact> = None;
    for axis in [right_a, forward_a, right_b, forward_b] {
        let radius_a = a.footprint.half_width_m * axis.dot(right_a).abs()
            + a.footprint.half_length_m * axis.dot(forward_a).abs();
        let radius_b = b.footprint.half_width_m * axis.dot(right_b).abs()
            + b.footprint.half_length_m * axis.dot(forward_b).abs();
        let separation = delta.dot(axis);
        let depth_m = radius_a + radius_b - separation.abs();
        if depth_m <= 1.0e-5 {
            return None;
        }
        if shallowest.is_none_or(|contact| depth_m < contact.depth_m) {
            // Orient the axis from `a` toward `b`, so the normal always says which way `b` lies.
            let normal = if separation < 0.0 { -axis } else { axis };
            shallowest = Some(FootprintContact { normal, depth_m });
        }
    }
    shallowest
}

/// XZ-plane separating-axis overlap between two oriented hull footprints.
pub(crate) fn obstacles_overlap(a: &TankObstacle, b: &TankObstacle) -> bool {
    obstacles_contact(a, b).is_some()
}

fn footprint_axes(yaw_rad: f32) -> [Vec2; 2] {
    let forward = Vec2::new(yaw_rad.sin(), yaw_rad.cos());
    let right = Vec2::new(forward.y, -forward.x);
    [right, forward]
}

#[cfg(test)]
mod contact_tests {
    use super::*;

    fn hull(x: f32, z: f32, yaw_rad: f32) -> TankObstacle {
        TankObstacle::new(
            Vec3::new(x, 0.0, z),
            yaw_rad,
            TankFootprint { half_width_m: 1.75, half_length_m: 3.2 },
        )
    }

    /// The contact must be the MINIMUM translation: pushing the pair apart by exactly `depth_m`
    /// along `normal` has to separate them, and no less would have. This is the property the
    /// positional correction leans on, so it is the property worth locking rather than any
    /// particular number.
    #[test]
    fn the_reported_contact_is_the_shallowest_way_out() {
        let mut state = 0x0c0f_feeeu32;
        let xorshift = |state: &mut u32| {
            *state ^= *state << 13;
            *state ^= *state >> 17;
            *state ^= *state << 5;
            (*state % 10_000) as f32 / 10_000.0
        };
        let mut seen = 0;
        for _ in 0..600 {
            let a = hull(0.0, 0.0, xorshift(&mut state) * std::f32::consts::TAU);
            let b = hull(
                (xorshift(&mut state) - 0.5) * 12.0,
                (xorshift(&mut state) - 0.5) * 12.0,
                xorshift(&mut state) * std::f32::consts::TAU,
            );
            let Some(contact) = obstacles_contact(&a, &b) else {
                assert!(!obstacles_overlap(&a, &b), "no contact must mean no overlap");
                continue;
            };
            seen += 1;
            assert!(contact.depth_m > 0.0);
            assert!((contact.normal.length() - 1.0).abs() < 1.0e-4, "the normal must be unit");

            // Separating by the full depth (plus a hair for the 1e-5 threshold) must clear it.
            let push = contact.normal * (contact.depth_m + 1.0e-3);
            let moved = TankObstacle { center: b.center + Vec3::new(push.x, 0.0, push.y), ..b };
            assert!(!obstacles_overlap(&a, &moved), "the MTV must actually separate the pair");

            // ...and separating by appreciably LESS must not: it is the minimum, not merely a way.
            let short = contact.normal * (contact.depth_m * 0.5);
            let barely = TankObstacle { center: b.center + Vec3::new(short.x, 0.0, short.y), ..b };
            assert!(obstacles_overlap(&a, &barely), "half the depth cannot already be clear");
        }
        assert!(seen > 30, "the sampling must actually produce contacts, saw {seen}");
    }

    /// The normal says which way `b` lies from `a`, which is what lets the solver push the pair
    /// apart rather than through each other.
    #[test]
    fn the_normal_points_from_a_toward_b() {
        let a = hull(0.0, 0.0, 0.0);
        let b = hull(2.0, 0.0, 0.0); // overlapping to +x (half_width 1.75 each)
        let contact = obstacles_contact(&a, &b).expect("these overlap");
        assert!(contact.normal.x > 0.9, "b lies to +x, got {:?}", contact.normal);

        let behind = hull(-2.0, 0.0, 0.0);
        let contact = obstacles_contact(&a, &behind).expect("these overlap");
        assert!(contact.normal.x < -0.9, "b lies to -x, got {:?}", contact.normal);
    }
}
