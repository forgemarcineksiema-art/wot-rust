#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RapierPhysicsRole {
    Broadphase,
    Raycasts,
    WorldCollisions,
    SimpleRigidBodies,
    TriggerVolumes,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CustomPhysicsRole {
    TankController,
    Traction,
    HullRotation,
    TurretRotation,
    GunDepression,
    Ballistics,
    ArmorPenetration,
    DamageModules,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PhysicsOwner {
    Rapier,
    CustomDeterministic,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PhysicsOwnershipPolicy;

impl PhysicsOwnershipPolicy {
    pub const fn rapier_roles(&self) -> [RapierPhysicsRole; 5] {
        [
            RapierPhysicsRole::Broadphase,
            RapierPhysicsRole::Raycasts,
            RapierPhysicsRole::WorldCollisions,
            RapierPhysicsRole::SimpleRigidBodies,
            RapierPhysicsRole::TriggerVolumes,
        ]
    }

    pub const fn owner_of(&self, _role: CustomPhysicsRole) -> PhysicsOwner {
        PhysicsOwner::CustomDeterministic
    }
}
