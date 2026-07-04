//! Module-health projection for the drive step: which modules still work and the
//! partial-damage fractions that shape one tick of hull motion. Split from `tank_drive.rs` to
//! keep the drive step within the reviewability budget.

use game_core::{ModuleSlot, TankSpec, TrackDamageMask, TrackSide};

/// Deterministic per-side track availability for one fixed tick.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TrackDriveStatus {
    pub left_ok: bool,
    pub right_ok: bool,
}

impl TrackDriveStatus {
    pub const fn healthy() -> Self {
        Self { left_ok: true, right_ok: true }
    }

    pub const fn broken() -> Self {
        Self { left_ok: false, right_ok: false }
    }

    pub const fn from_suspension_ok(ok: bool) -> Self {
        if ok { Self::healthy() } else { Self::broken() }
    }

    pub const fn from_track_damage(mask: TrackDamageMask) -> Self {
        Self {
            left_ok: !mask.is_broken(TrackSide::Left),
            right_ok: !mask.is_broken(TrackSide::Right),
        }
    }

    pub(crate) fn any_ok(self) -> bool {
        self.left_ok || self.right_ok
    }

    pub(crate) fn both_ok(self) -> bool {
        self.left_ok && self.right_ok
    }
}

/// Which modules still work, plus the partial-damage fractions that shape the drive: gun damage
/// inflates dispersion, engine damage drains drive power, suspension damage dulls the turn.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DriveModuleStatus {
    pub tracks: TrackDriveStatus,
    pub engine_ok: bool,
    pub turret_ok: bool,
    pub gun_damage_fraction: f32,
    /// `P/v` drive-power fraction from partial engine damage (1.0 healthy, floored — game_core).
    pub engine_power_fraction: f32,
    /// Turn-rate and yaw-spool fraction from partial suspension damage.
    pub suspension_agility: f32,
}

impl DriveModuleStatus {
    /// Build from live module HP (in `ModuleSlot::ALL` wire order) plus the spec's full pools —
    /// the one construction shared by the server projection and the client predictor, so both
    /// always drive identical hulls.
    pub fn from_module_hp(
        tracks: TrackDriveStatus,
        live: [u32; game_core::MODULE_SLOT_COUNT],
        spec: &TankSpec,
    ) -> Self {
        let live_hp = |slot: ModuleSlot| live[slot.wire_index()];
        let full_hp = |slot: ModuleSlot| spec.module_health.hit_points(slot);
        let gun_full = full_hp(ModuleSlot::Gun).max(1) as f32;
        Self {
            tracks,
            engine_ok: live_hp(ModuleSlot::Engine) > 0,
            turret_ok: live_hp(ModuleSlot::Turret) > 0,
            gun_damage_fraction: (1.0 - live_hp(ModuleSlot::Gun) as f32 / gun_full)
                .clamp(0.0, 1.0),
            engine_power_fraction: game_core::engine_power_fraction(
                live_hp(ModuleSlot::Engine),
                full_hp(ModuleSlot::Engine),
            ),
            suspension_agility: game_core::suspension_agility_fraction(
                live_hp(ModuleSlot::Suspension),
                full_hp(ModuleSlot::Suspension),
            ),
        }
    }

    /// Every module at full health.
    pub fn healthy(spec: &TankSpec) -> Self {
        Self::from_module_hp(
            TrackDriveStatus::healthy(),
            spec.module_health.hit_points_by_slot(),
            spec,
        )
    }
}
