use bevy_ecs::prelude::*;
use game_core::{
    ArmorBreachSet, MODULE_SLOT_COUNT, TankId, TeamId, TrackDamageMask, TrackSide, VehicleKind,
};

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

/// Bitmask of teams that can currently see this tank (LOS spotting, protocol v16). The HUD gates
/// enemy overlays — floating HP bars and minimap blips — on the player team's bit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Component)]
pub struct Spotted(pub u8);

/// Live module HP in `ModuleSlot::ALL` wire order, replicated from the snapshot. Partial damage
/// drives presentation cues (a wounded suspension softens the sprung-hull attitude).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Component)]
pub struct ModuleHitPoints(pub [u32; MODULE_SLOT_COUNT]);

/// Side-specific track damage bitmask replicated from the authoritative simulation.
#[derive(Debug, Clone, PartialEq, Default, Component)]
pub struct VehicleDamage {
    pub mask: u8,
    pub break_t: [Option<f32>; 2],
    pub armor_breaches: ArmorBreachSet,
    pub engine_fire: bool,
    /// A holed fuel tank burns as itself (v27) — independent of the engine's own fire.
    pub fuel_fire: bool,
}

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

/// Render-only gun recoil: firing throws the barrel back along its axis and a critically damped
/// spring runs it back into battery. Like [`TrackAnimation`] this is presentation state derived
/// from replicated events — it never leaves the presentation world and is kept out of the
/// snapshot sync bundle so it persists across frames.
#[derive(Debug, Clone, Copy, PartialEq, Default, Component)]
pub struct GunRecoil {
    /// Meters the barrel currently sits back from battery (>= 0).
    pub offset_m: f32,
    velocity_mps: f32,
}

/// Recoil spring frequency (rad/s): back in ~50 ms, returned to battery in ~a third of a second —
/// the long-recoil cycle of a mid-century high-pressure gun, readable at battle camera range.
const RECOIL_OMEGA: f32 = 14.0;
/// Rearward barrel speed injected per shot (m/s). With critical damping the stroke peaks at
/// `v0 / (omega * e)` ≈ 0.32 m — right for a D-10T-class recoil visible over the fender line.
const RECOIL_IMPULSE_MPS: f32 = 12.0;

impl GunRecoil {
    /// One shot: throw the barrel rearward. Stacks (a fast autoloader keeps it dancing) but the
    /// spring's stroke stays bounded by the clamp in [`GunRecoil::step`].
    pub fn kick(&mut self) {
        self.velocity_mps += RECOIL_IMPULSE_MPS;
    }

    /// Advance the recoil spring one presented frame (critically damped return to battery).
    pub fn step(&mut self, dt: f32) {
        let dt = dt.clamp(0.0, 0.05);
        let accel =
            -RECOIL_OMEGA * RECOIL_OMEGA * self.offset_m - 2.0 * RECOIL_OMEGA * self.velocity_mps;
        self.velocity_mps += accel * dt;
        self.offset_m = (self.offset_m + self.velocity_mps * dt).clamp(0.0, 0.6);
    }
}

/// Flat view of a presentation entity handed to the renderer and HUD. The render path reads this
/// instead of `net::TankSnapshot`, so the persistent ECS — not the raw snapshot buffer — is the
/// presentation source of truth.
#[derive(Debug, Clone, PartialEq)]
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
    /// Bitmask of teams that can see this tank (protocol v16); gates enemy HUD overlays.
    pub spotted_by_teams_mask: u8,
    /// Live module HP in `ModuleSlot::ALL` wire order (partial-damage presentation cues).
    pub module_hit_points: [u32; MODULE_SLOT_COUNT],
    pub track_damage_mask: u8,
    pub track_break_t: [Option<f32>; 2],
    pub engine_fire: bool,
    /// A holed fuel tank burns as itself (v27) — independent of the engine's own fire.
    pub fuel_fire: bool,
    pub armor_breaches: ArmorBreachSet,
    /// Per-side track distance (metres) for spinning wheels and scrolling track links.
    pub track_left_m: f32,
    pub track_right_m: f32,
    /// Sprung-hull attitude (presentation only): pitch (+nose up), roll (+right side up), and the
    /// vertical spring offset over the replicated hull height.
    pub attitude_pitch_rad: f32,
    pub attitude_roll_rad: f32,
    pub attitude_heave_m: f32,
    /// Filtered longitudinal acceleration (m/s^2) - the render-side track-tension cue.
    pub accel_long_mps2: f32,
    /// Meters the barrel sits back from battery this frame (render-side recoil animation).
    pub gun_recoil_m: f32,
}
