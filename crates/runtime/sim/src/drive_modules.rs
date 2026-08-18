//! Module-health projection for the drive step: which modules still work and the
//! partial-damage fractions that shape one tick of hull motion. Split from `tank_drive.rs` to
//! keep the drive step within the reviewability budget.

use game_core::{ModuleSlot, TankSpec, TrackHealth, TrackSide, track_traction_fraction};

/// One side's drive contribution for a fixed tick: either rolling (with a traction fraction,
/// `1.0` when healthy — exact, so the healthy path stays bit-identical) or thrown.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TrackSideDrive {
    Rolling(f32),
    Thrown,
}

impl TrackSideDrive {
    /// Traction fraction, `0.0` for a thrown side.
    pub fn traction(self) -> f32 {
        match self {
            Self::Rolling(fraction) => fraction,
            Self::Thrown => 0.0,
        }
    }

    pub fn is_rolling(self) -> bool {
        matches!(self, Self::Rolling(_))
    }
}

/// Deterministic per-side track state for one fixed tick.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TrackDriveStatus {
    pub left: TrackSideDrive,
    pub right: TrackSideDrive,
}

impl TrackDriveStatus {
    pub const fn healthy() -> Self {
        Self { left: TrackSideDrive::Rolling(1.0), right: TrackSideDrive::Rolling(1.0) }
    }

    pub const fn broken() -> Self {
        Self { left: TrackSideDrive::Thrown, right: TrackSideDrive::Thrown }
    }

    pub const fn from_suspension_ok(ok: bool) -> Self {
        if ok { Self::healthy() } else { Self::broken() }
    }

    /// Project the authoritative per-side HP pool into the drive tick: a drained (0 HP) side is
    /// thrown; any live side rolls at its graded traction fraction.
    pub fn from_track_health(tracks: &TrackHealth) -> Self {
        let side = |s: TrackSide| {
            let hp = tracks.hp(s);
            if hp == 0 {
                TrackSideDrive::Thrown
            } else {
                TrackSideDrive::Rolling(track_traction_fraction(hp))
            }
        };
        Self { left: side(TrackSide::Left), right: side(TrackSide::Right) }
    }

    pub(crate) fn left_ok(self) -> bool {
        self.left.is_rolling()
    }

    pub(crate) fn right_ok(self) -> bool {
        self.right.is_rolling()
    }

    pub(crate) fn any_ok(self) -> bool {
        self.left_ok() || self.right_ok()
    }

    pub(crate) fn both_ok(self) -> bool {
        self.left_ok() && self.right_ok()
    }

    /// Combined traction of the two sides for the damaged-but-not-thrown tier — the AVERAGE, so a
    /// single damaged side barely dents mobility while two damaged sides compound (both turn and
    /// top-speed reads worse). Both healthy → `(1.0 + 1.0) * 0.5 = 1.0` exactly, so the drive
    /// scaling is a bit-exact no-op on a healthy hull. Only meaningful when neither side is thrown.
    pub(crate) fn rolling_traction(self) -> f32 {
        (self.left.traction() + self.right.traction()) * 0.5
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
    /// `driver_effectiveness` is the crew-vitals read for the driver's station (1.0 whole; see
    /// `game_core::CrewVitals`) — a parameter, not a global, so a missed call site is a compile
    /// error instead of a silent server/predictor desync.
    pub fn from_module_hp(
        tracks: TrackDriveStatus,
        live: [u32; game_core::MODULE_SLOT_COUNT],
        spec: &TankSpec,
        driver_effectiveness: f32,
    ) -> Self {
        let live_hp = |slot: ModuleSlot| live[slot.wire_index()];
        let full_hp = |slot: ModuleSlot| spec.module_health.hit_points(slot);
        let gun_full = full_hp(ModuleSlot::Gun).max(1) as f32;
        // A covered driver's station drives at a penalty on BOTH throttle and turn — someone
        // else is on the sticks — but never to zero: the floors below the module fractions
        // still hold, and crew state multiplies them rather than replacing them.
        let driver = driver_effectiveness.clamp(0.0, 1.0).max(0.35);
        Self {
            tracks,
            engine_ok: live_hp(ModuleSlot::Engine) > 0,
            turret_ok: live_hp(ModuleSlot::Turret) > 0,
            gun_damage_fraction: (1.0 - live_hp(ModuleSlot::Gun) as f32 / gun_full).clamp(0.0, 1.0),
            engine_power_fraction: game_core::engine_power_fraction(
                live_hp(ModuleSlot::Engine),
                full_hp(ModuleSlot::Engine),
            ) * driver,
            suspension_agility: game_core::suspension_agility_fraction(
                live_hp(ModuleSlot::Suspension),
                full_hp(ModuleSlot::Suspension),
            ) * driver,
        }
    }

    /// Every module at full health, the crew whole.
    pub fn healthy(spec: &TankSpec) -> Self {
        Self::from_module_hp(
            TrackDriveStatus::healthy(),
            spec.module_health.hit_points_by_slot(),
            spec,
            1.0,
        )
    }
}
