//! Cover is a body, not a veto (the one program's X6).
//!
//! A wall used to be a refusal in the settle: the move was thrown away, the velocity into it
//! zeroed after the tick's acceleration had already been read — no dive, no torque, no bill, and a
//! hull pinned between a pusher and a wall lost its velocity every tick. Now every standing solid
//! within reach is an immovable body in the roster solve: the hull meets it through a contact,
//! spends its momentum there, and the settle sees the deceleration as any other.

use game_core::{ContactFootprint, HullPlan, TankSpec, VehicleKind};
use glam::Vec3;
use physics::{
    ContactBody, ContactCache, SOLID_ID_BASE, TankControlInput, TankControllerSettings,
    TankFootprint, TankKinematicState, TankWorldObstacles, advance_tank_on_world, resolve_contacts,
    settle_tank_on_world, solid_bodies_near,
};
use terrain::{HeightMap, StaticCoverKind, StaticCoverObject};

const DT: f32 = 1.0 / 60.0;

fn tenement() -> StaticCoverObject {
    StaticCoverObject {
        id: "tenement".into(),
        name: "tenement".into(),
        kind: StaticCoverKind::CityBuilding,
        center: [60.0, 5.0, 80.0],
        half_extents_m: [10.0, 5.0, 6.0],
        yaw_rad: 0.0,
    }
}

fn hull_body(state: &TankKinematicState, spec: &TankSpec) -> ContactBody {
    ContactBody {
        id: 1,
        position: state.position,
        velocity: state.velocity,
        yaw_rad: state.yaw_rad,
        yaw_rate_rad_s: state.yaw_rate_rad_s,
        footprint: TankFootprint::from_plan(spec.hull_plan()),
        mass_kg: spec.mass_kg,
        movable: true,
        solid: false,
    }
}

/// The world one hull is solved in: its spec and settings, the flat ground, the cover.
struct Rig<'a> {
    spec: &'a TankSpec,
    settings: TankControllerSettings,
    map: HeightMap,
    cover: &'a [StaticCoverObject],
    running_gear: ContactFootprint,
}

impl Rig<'_> {
    /// One solved tick, the way the authority and the predictor run it: advance, the roster
    /// solve with the standing solids within reach, settle. Returns the pairs that pressed and
    /// whether the backstop veto had anything to refuse.
    fn solved_tick(
        &self,
        state: &mut TankKinematicState,
        cache: &mut ContactCache,
        input: TankControlInput,
    ) -> (Vec<physics::ContactPair>, bool) {
        let hull = TankFootprint::from_plan(self.spec.hull_plan());
        let obstacles = TankWorldObstacles::new(self.cover, hull);
        let phase = advance_tank_on_world(
            state,
            input,
            &self.settings,
            Some(&self.map),
            obstacles,
            Some(&self.running_gear),
            DT,
        );
        let mut bodies = vec![hull_body(state, self.spec)];
        bodies.extend(solid_bodies_near(self.cover, &bodies, DT));
        let report = resolve_contacts(&bodies, cache, DT);
        state.velocity += report.bodies[0].delta_velocity;
        state.yaw_rate_rad_s += report.bodies[0].delta_yaw_rate_rad_s;
        // What the settle will integrate, before the backstop can touch it.
        let intended = state.position + Vec3::new(state.velocity.x, 0.0, state.velocity.z) * DT;
        settle_tank_on_world(
            state,
            &self.settings,
            phase,
            Some(&self.map),
            obstacles,
            Some(&self.running_gear),
            DT,
        );
        let refused = (state.position.x - intended.x).abs() > 1.0e-4
            || (state.position.z - intended.z).abs() > 1.0e-4;
        (report.pairs, refused)
    }
}

/// The row's lock: a 14 m/s wall hit is a contact — one pair with the hull's whole momentum in
/// it, the nose dives through the same spring a brake dives it, and the backstop veto in the
/// settle finds nothing to refuse on any tick of the charge.
#[test]
fn a_fourteen_metre_per_second_wall_hit_is_a_contact_that_dives_the_nose() {
    let spec = TankSpec::t54_1951();
    let wall = tenement();
    let rig = Rig {
        spec: &spec,
        settings: TankControllerSettings::from_spec(&spec),
        map: HeightMap::flat(121, 121, 1.0, 0.0).expect("flat"),
        cover: std::slice::from_ref(&wall),
        running_gear: ContactFootprint::for_vehicle(VehicleKind::T54_1951),
    };
    let mut state = TankKinematicState {
        position: Vec3::new(60.0, 0.0, 60.0),
        velocity: Vec3::new(0.0, 0.0, 14.0),
        ..Default::default()
    };
    let mut cache = ContactCache::default();
    let coast = TankControlInput { throttle: 0.0, steer: 0.0, brake: 0.0 };
    let (mut hit_impulse, mut deepest_dive, mut refused_ever) = (0.0_f32, 0.0_f32, false);
    // The momentum the hull BRINGS to the contact: a coasting hull sheds speed on its own
    // (engine braking, the governor above the top speed), so the wall is owed what arrives.
    let mut momentum_at_contact = 0.0_f32;
    for _ in 0..90 {
        let speed_before = state.velocity.z;
        let (pairs, refused) = rig.solved_tick(&mut state, &mut cache, coast);
        refused_ever |= refused;
        if momentum_at_contact == 0.0 && !pairs.is_empty() {
            momentum_at_contact = spec.mass_kg * speed_before;
        }
        for pair in &pairs {
            assert!(pair.b >= 1 || pair.a >= 1, "the only other body is the wall");
            // Summed over the charge: the contact takes the momentum over the ticks it takes to
            // close, and all of it goes through the wall.
            hit_impulse += pair.normal_impulse_ns;
        }
        deepest_dive = deepest_dive.min(state.dive_pitch_rad);
    }
    let face_z = wall.center[2] - wall.half_extents_m[2];
    let nose = state.position.z + HullPlan::for_vehicle(VehicleKind::T54_1951).half_length_m;
    assert!(nose <= face_z + 0.03, "the hull stops at the wall face, nose at {nose} vs {face_z}");
    assert!(state.velocity.z.abs() < 0.05, "...and stays stopped, got {} m/s", state.velocity.z);
    assert!(momentum_at_contact > 0.5 * spec.mass_kg * 14.0, "the hull must still be charging");
    assert!(
        hit_impulse > 0.9 * momentum_at_contact,
        "the wall must take the charge's momentum through the contact: {hit_impulse} N·s of          {momentum_at_contact}"
    );
    assert!(
        deepest_dive < -0.003,
        "a wall hit dives the nose like a brake does, deepest {deepest_dive} rad"
    );
    assert!(!refused_ever, "the backstop veto must have had nothing to refuse in a solved charge");
}

/// The gather is deterministic and the ids are the cover's: the same wall gets the same id on
/// every side of the wire, and a wall out of reach is not a body.
#[test]
fn the_solids_within_reach_are_the_covers_own_and_no_more() {
    let spec = TankSpec::t54_1951();
    let far = StaticCoverObject { id: "far".into(), center: [300.0, 5.0, 300.0], ..tenement() };
    let cover = [tenement(), far];
    let state = TankKinematicState {
        position: Vec3::new(60.0, 0.0, 70.0),
        velocity: Vec3::new(0.0, 0.0, 5.0),
        ..Default::default()
    };
    let hulls = [hull_body(&state, &spec)];
    let solids = solid_bodies_near(&cover, &hulls, DT);
    assert_eq!(solids.len(), 1, "only the tenement is within reach");
    assert_eq!(solids[0].id, SOLID_ID_BASE, "the id is the cover index under the solid bit");
    assert!(solids[0].solid && !solids[0].movable);
    assert!((solids[0].position.y - 0.0).abs() < 1.0e-6, "planted at the box's bottom");
    assert!((solids[0].footprint.height_m - 10.0).abs() < 1.0e-6);
}
