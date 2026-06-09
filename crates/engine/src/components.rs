use bevy_ecs::prelude::*;
use game_core::{TankId, VehicleKind};

/// Render-side clock, advanced once per presented frame. Lives as an ECS resource so future
/// presentation systems (animation, fade timers) read one shared time source.
#[derive(Debug, Clone, Copy, PartialEq, Default, Resource)]
pub struct Time {
    pub tick: u64,
    pub delta_seconds: f32,
    pub elapsed_seconds: f32,
}

/// Stable identity tying a presentation entity back to its networked tank.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Component)]
pub struct TankEntity {
    pub id: TankId,
}

/// Hull pose in world space.
#[derive(Debug, Clone, Copy, PartialEq, Default, Component)]
pub struct RenderTransform {
    pub translation: [f32; 3],
    pub hull_yaw_rad: f32,
}

/// Turret heading relative to the hull.
#[derive(Debug, Clone, Copy, PartialEq, Default, Component)]
pub struct TurretYaw(pub f32);

/// Gun elevation.
#[derive(Debug, Clone, Copy, PartialEq, Default, Component)]
pub struct GunPitch(pub f32);

/// Current hit points; the max comes from the vehicle profile, so only the live value is stored.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Component)]
pub struct Health {
    pub hit_points: u32,
}

/// Vehicle kind as a component (mesh + armor/profile selection). Newtype so we can derive
/// `Component` without an orphan impl on `game_core::VehicleKind`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Component)]
pub struct Vehicle(pub VehicleKind);

/// Bitmask of destroyed module slots, used by the renderer for damage tinting.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Component)]
pub struct DestroyedModules(pub u8);

/// Flat view of a presentation entity handed to the renderer and HUD. The render path reads this
/// instead of `net::TankSnapshot`, so the persistent ECS — not the raw snapshot buffer — is the
/// presentation source of truth.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PresentationTank {
    pub id: TankId,
    pub vehicle: VehicleKind,
    pub translation: [f32; 3],
    pub hull_yaw_rad: f32,
    pub turret_yaw_rad: f32,
    pub gun_pitch_rad: f32,
    pub hit_points: u32,
    pub destroyed_modules_mask: u8,
}
