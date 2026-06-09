use physics::{
    CustomPhysicsRole, PhysicsOwner, PhysicsOwnershipPolicy, RapierPhysicsRole, TankControlInput,
    TankControllerSettings, TankKinematicState, step_custom_tank_controller,
};

#[test]
fn rapier_is_limited_to_world_queries_and_simple_bodies() {
    let policy = PhysicsOwnershipPolicy;

    assert_eq!(
        policy.rapier_roles(),
        [
            RapierPhysicsRole::Broadphase,
            RapierPhysicsRole::Raycasts,
            RapierPhysicsRole::WorldCollisions,
            RapierPhysicsRole::SimpleRigidBodies,
            RapierPhysicsRole::TriggerVolumes,
        ]
    );
}

#[test]
fn tank_gameplay_physics_is_owned_by_custom_code() {
    let policy = PhysicsOwnershipPolicy;

    for role in [
        CustomPhysicsRole::TankController,
        CustomPhysicsRole::Traction,
        CustomPhysicsRole::HullRotation,
        CustomPhysicsRole::TurretRotation,
        CustomPhysicsRole::GunDepression,
        CustomPhysicsRole::Ballistics,
        CustomPhysicsRole::ArmorPenetration,
        CustomPhysicsRole::DamageModules,
    ] {
        assert_eq!(policy.owner_of(role), PhysicsOwner::CustomDeterministic);
    }
}

#[test]
fn custom_tank_controller_replays_same_inputs_exactly() {
    let inputs = [
        TankControlInput { throttle: 1.0, steer: 0.25, brake: 0.0 },
        TankControlInput { throttle: 0.5, steer: -0.1, brake: 0.0 },
        TankControlInput { throttle: 0.0, steer: 0.0, brake: 1.0 },
    ];

    let first = replay_controller(inputs);
    let second = replay_controller(inputs);

    assert_eq!(first, second);
}

fn replay_controller(inputs: [TankControlInput; 3]) -> TankKinematicState {
    let settings = TankControllerSettings::arcade_default();
    let mut state = TankKinematicState::default();
    for input in inputs {
        step_custom_tank_controller(&mut state, input, &settings, 1.0 / 60.0);
    }
    state
}
