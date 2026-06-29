use bevy_ecs::prelude::*;
use game_core::{TankId, TeamId, TrackDamageMask, TrackSide, VehicleKind};

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

/// Replicated team identity (protocol v12). The HUD reads it to draw enemy-only overlays.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Component)]
pub struct Team(pub TeamId);

/// Bitmask of destroyed module slots, used by the renderer for damage tinting.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Component)]
pub struct DestroyedModules(pub u8);

/// Side-specific track damage bitmask replicated from the authoritative simulation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Component)]
pub struct TrackDamage(pub u8);

/// Per-side track distance travelled (metres), accumulated from the hull's frame-to-frame pose so
/// the renderer can spin the wheels and scroll the track links. This is a render-only cue derived
/// from motion — it is not gameplay state and never leaves the presentation world. Kept out of the
/// snapshot sync bundle so it persists and accumulates across frames instead of being overwritten.
#[derive(Debug, Clone, Copy, PartialEq, Default, Component)]
pub struct TrackAnimation {
    pub left_m: f32,
    pub right_m: f32,
    last_translation: [f32; 3],
    last_yaw: f32,
    seeded: bool,
}

impl TrackAnimation {
    /// Fold one frame of hull motion into the per-side track distance. Forward travel advances both
    /// tracks; yaw advances the outer track and retards the inner, so a pivot runs them opposite
    /// ways. `half_gauge` is roughly the track half-spacing (the turn lever arm).
    pub fn accumulate(
        &mut self,
        translation: [f32; 3],
        yaw_rad: f32,
        half_gauge: f32,
        damage: TrackDamageMask,
    ) {
        use game_core::math::wrap_angle;

        if self.seeded {
            // Forward displacement = delta projected onto the hull heading (sin, cos in XZ).
            let dx = translation[0] - self.last_translation[0];
            let dz = translation[2] - self.last_translation[2];
            let ds = dx * yaw_rad.sin() + dz * yaw_rad.cos();
            let dyaw = wrap_angle(yaw_rad - self.last_yaw);
            if !damage.is_broken(TrackSide::Left) {
                self.left_m += ds - dyaw * half_gauge;
            }
            if !damage.is_broken(TrackSide::Right) {
                self.right_m += ds + dyaw * half_gauge;
            }
        }
        self.last_translation = translation;
        self.last_yaw = yaw_rad;
        self.seeded = true;
    }
}

/// Flat view of a presentation entity handed to the renderer and HUD. The render path reads this
/// instead of `net::TankSnapshot`, so the persistent ECS — not the raw snapshot buffer — is the
/// presentation source of truth.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PresentationTank {
    pub id: TankId,
    pub team: TeamId,
    pub vehicle: VehicleKind,
    pub translation: [f32; 3],
    pub hull_yaw_rad: f32,
    pub turret_yaw_rad: f32,
    pub gun_pitch_rad: f32,
    pub hit_points: u32,
    pub destroyed_modules_mask: u8,
    pub track_damage_mask: u8,
    /// Per-side track distance (metres) for spinning wheels and scrolling track links.
    pub track_left_m: f32,
    pub track_right_m: f32,
}
