//! Thin client presentation ECS built on `bevy_ecs`. It projects the snapshot buffer into a
//! persistent world of presentation entities that the renderer and HUD read from. It is not a
//! general-purpose engine and does not own gameplay truth — see `docs/architecture.md`.

mod attitude;
mod components;
mod sync_cues;
mod world;

pub use attitude::{AttitudeSample, HullAttitude, TankMotion};
pub use components::{
    DestroyedModules, GunPitch, GunRecoil, Health, ModuleHitPoints, PresentationTank,
    RenderTransform, TankEntity, Team, Time, TurretYaw, Vehicle, VehicleDamage,
};
pub use world::PresentationWorld;
