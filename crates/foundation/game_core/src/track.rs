use serde::{Deserialize, Serialize};

use crate::ShellType;

/// Full track-band HP for one side. A track is a per-side pool: hits subtract, the crew regens
/// it back, and a pool at zero is a THROWN track (latched broken until re-seated — see
/// `sim::repair`). The pool is the single source of truth; the wire `TrackDamageMask` and every
/// broken-mask consumer are derived from `hp == 0`.
pub const TRACK_HP_MAX: u8 = 100;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TrackSide {
    Left,
    Right,
}

/// The three states a side's pool can be in, read off its HP. `Damaged` is the new intermediate
/// tier: still driving, but degraded and one solid hit from breaking.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrackSeverity {
    Healthy,
    Damaged,
    Broken,
}

impl TrackSeverity {
    pub const fn from_hp(hp: u8) -> Self {
        if hp == 0 {
            Self::Broken
        } else if hp >= TRACK_HP_MAX {
            Self::Healthy
        } else {
            Self::Damaged
        }
    }
}

/// Per-side track HP — the authoritative track state on a hull. `serde(default)` (via [`Default`])
/// keeps pre-two-tier fixtures loading as fully healthy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrackHealth {
    left_hp: u8,
    right_hp: u8,
}

impl Default for TrackHealth {
    fn default() -> Self {
        Self::healthy()
    }
}

impl TrackHealth {
    pub const fn healthy() -> Self {
        Self { left_hp: TRACK_HP_MAX, right_hp: TRACK_HP_MAX }
    }

    /// Rebuild from the replicated `[left, right]` HP pair (protocol v22).
    pub const fn from_hp_pair(pair: [u8; 2]) -> Self {
        Self { left_hp: pair[0], right_hp: pair[1] }
    }

    pub const fn hp_pair(self) -> [u8; 2] {
        [self.left_hp, self.right_hp]
    }

    pub const fn hp(self, side: TrackSide) -> u8 {
        match side {
            TrackSide::Left => self.left_hp,
            TrackSide::Right => self.right_hp,
        }
    }

    fn hp_mut(&mut self, side: TrackSide) -> &mut u8 {
        match side {
            TrackSide::Left => &mut self.left_hp,
            TrackSide::Right => &mut self.right_hp,
        }
    }

    pub const fn severity(self, side: TrackSide) -> TrackSeverity {
        TrackSeverity::from_hp(self.hp(side))
    }

    pub const fn is_broken(self, side: TrackSide) -> bool {
        self.hp(side) == 0
    }

    pub const fn any_broken(self) -> bool {
        self.left_hp == 0 || self.right_hp == 0
    }

    pub const fn all_broken(self) -> bool {
        self.left_hp == 0 && self.right_hp == 0
    }

    /// Subtract `amount` from a side's pool (saturating at 0 = broken). A pool that reaches 0
    /// stays there until [`reseat`](Self::reseat) — the drive treats it as a thrown track.
    pub fn damage(&mut self, side: TrackSide, amount: u8) {
        let hp = self.hp_mut(side);
        *hp = hp.saturating_sub(amount);
    }

    pub fn damage_both(&mut self, amount: u8) {
        self.damage(TrackSide::Left, amount);
        self.damage(TrackSide::Right, amount);
    }

    /// Crew regen of a DAMAGED pool back toward full (capped at [`TRACK_HP_MAX`]). Callers must
    /// not heal a broken (0 HP) side — that is a re-seat, not a regen (see `sim::repair`).
    pub fn heal(&mut self, side: TrackSide, amount: u8) {
        let hp = self.hp_mut(side);
        *hp = hp.saturating_add(amount).min(TRACK_HP_MAX);
    }

    /// Throw a side outright (pool to 0), regardless of its current HP.
    pub fn break_side(&mut self, side: TrackSide) {
        *self.hp_mut(side) = 0;
    }

    pub fn break_both(&mut self) {
        self.left_hp = 0;
        self.right_hp = 0;
    }

    /// Crew re-seat: the side is whole again (pool back to full). See `sim::repair` for timing.
    pub fn reseat(&mut self, side: TrackSide) {
        *self.hp_mut(side) = TRACK_HP_MAX;
    }

    /// The derived broken-only view every legacy mask consumer (wire, render sag, hull lean)
    /// keeps reading — a side appears in the mask exactly when its pool is at 0.
    pub fn broken_mask(self) -> TrackDamageMask {
        let mut mask = TrackDamageMask::healthy();
        if self.is_broken(TrackSide::Left) {
            mask.damage(TrackSide::Left);
        }
        if self.is_broken(TrackSide::Right) {
            mask.damage(TrackSide::Right);
        }
        mask
    }
}

/// Drive-power / turn fraction a damaged track still delivers: `1.0` at full pool (exact, so the
/// healthy path stays bit-identical), curving down as the pool drains. The curve is deliberately
/// flat near full — the first light hit is imperceptible — and bites as HP falls, so two damaged
/// sides handle noticeably worse. A broken (0 HP) side never routes through here; the drive gate
/// treats it as thrown.
pub fn track_traction_fraction(hp: u8) -> f32 {
    /// Traction lost by a fully-drained-but-not-broken pool (hp -> 1).
    const TRACK_TRACTION_LOSS: f32 = 0.68;
    /// Curve exponent: >1 keeps the first hit gentle and makes later damage bite.
    const TRACK_DAMAGE_EXP: f32 = 2.4;
    let drained = ((TRACK_HP_MAX - hp) as f32 / TRACK_HP_MAX as f32).clamp(0.0, 1.0);
    1.0 - TRACK_TRACTION_LOSS * drained.powf(TRACK_DAMAGE_EXP)
}

/// Track-band HP a single hit strips, as a function of the shell that struck it (protocol D4:
/// "wytrzymałość zależna od pocisku"). A solid near-normal AP round strips the whole pool in one
/// (immediate throw, so tracking stays a real tactic); an oblique or ricocheting hit only
/// degrades it; an HE splash chips it. Deterministic — a pure function of the resolved hit.
pub fn track_hit_damage(
    caliber_mm: f32,
    impact_angle_deg: f32,
    shell_type: ShellType,
    penetrated: bool,
    ricocheted: bool,
) -> u8 {
    /// Flat chunk an HE burst (splash / non-pen) chips off the band.
    const TRACK_HE_DAMAGE: f32 = 22.0;
    if shell_type == ShellType::HighExplosive && !penetrated {
        return TRACK_HE_DAMAGE as u8;
    }
    // Kinetic: the caliber sets the raw bite (a 100 mm class round throws a full pool head-on —
    // the headroom keeps a clean hit above MAX out to ~25° before obliquity drops it into the
    // damage-only band), obliquity sheds it (the band skids the round aside), and a ricochet
    // barely scores it.
    const TRACK_AP_BITE: f32 = 1.2;
    let angle_factor = impact_angle_deg.to_radians().cos().clamp(0.0, 1.0).powf(1.5);
    let ricochet_factor = if ricocheted { 0.35 } else { 1.0 };
    let raw = caliber_mm * TRACK_AP_BITE * angle_factor * ricochet_factor;
    raw.clamp(0.0, TRACK_HP_MAX as f32) as u8
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct TrackDamageMask(u8);

impl TrackDamageMask {
    pub const LEFT_BIT: u8 = 1 << 0;
    pub const RIGHT_BIT: u8 = 1 << 1;
    pub const BOTH_BITS: u8 = Self::LEFT_BIT | Self::RIGHT_BIT;

    pub const LEFT: Self = Self(Self::LEFT_BIT);
    pub const RIGHT: Self = Self(Self::RIGHT_BIT);
    pub const BOTH: Self = Self(Self::BOTH_BITS);

    pub const fn healthy() -> Self {
        Self(0)
    }

    pub const fn from_bits(bits: u8) -> Self {
        Self(bits & Self::BOTH_BITS)
    }

    pub const fn bits(self) -> u8 {
        self.0
    }

    pub const fn is_broken(self, side: TrackSide) -> bool {
        self.0 & side.bit() != 0
    }

    pub fn damage(&mut self, side: TrackSide) {
        self.0 |= side.bit();
    }

    pub fn damage_both(&mut self) {
        self.0 |= Self::BOTH_BITS;
    }

    /// Crew repair: the side is whole again (see `sim::repair` for the timing).
    pub fn repair(&mut self, side: TrackSide) {
        self.0 &= !side.bit();
    }

    pub const fn all_broken(self) -> bool {
        self.0 & Self::BOTH_BITS == Self::BOTH_BITS
    }

    pub const fn any_broken(self) -> bool {
        self.0 & Self::BOTH_BITS != 0
    }
}

impl TrackSide {
    pub const fn bit(self) -> u8 {
        match self {
            Self::Left => TrackDamageMask::LEFT_BIT,
            Self::Right => TrackDamageMask::RIGHT_BIT,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn healthy_pool_is_full_and_unbroken() {
        let tracks = TrackHealth::healthy();
        assert_eq!(tracks.hp(TrackSide::Left), TRACK_HP_MAX);
        assert_eq!(tracks.severity(TrackSide::Left), TrackSeverity::Healthy);
        assert!(!tracks.any_broken());
        assert_eq!(tracks.broken_mask(), TrackDamageMask::healthy());
    }

    #[test]
    fn a_partial_hit_damages_but_does_not_break() {
        let mut tracks = TrackHealth::healthy();
        tracks.damage(TrackSide::Left, 40);
        assert_eq!(tracks.severity(TrackSide::Left), TrackSeverity::Damaged);
        assert!(!tracks.is_broken(TrackSide::Left));
        assert_eq!(tracks.broken_mask(), TrackDamageMask::healthy(), "damaged is not yet broken");
        assert_eq!(tracks.severity(TrackSide::Right), TrackSeverity::Healthy);
    }

    #[test]
    fn draining_the_pool_breaks_the_side_and_shows_in_the_mask() {
        let mut tracks = TrackHealth::healthy();
        tracks.damage(TrackSide::Left, TRACK_HP_MAX);
        assert_eq!(tracks.severity(TrackSide::Left), TrackSeverity::Broken);
        assert!(tracks.is_broken(TrackSide::Left));
        assert_eq!(tracks.broken_mask(), TrackDamageMask::LEFT);
    }

    #[test]
    fn traction_is_exactly_one_at_full_and_drops_off() {
        assert_eq!(track_traction_fraction(TRACK_HP_MAX), 1.0, "healthy must be bit-exact 1.0");
        let one_hit = track_traction_fraction(66);
        let two_hit = track_traction_fraction(32);
        assert!(one_hit > 0.93, "one light hit is nearly imperceptible (~-5%): {one_hit}");
        assert!(two_hit < 0.80, "a second hit bites (~-27%): {two_hit}");
        assert!(two_hit < one_hit);
    }

    #[test]
    fn solid_head_on_ap_throws_the_track_in_one() {
        let chunk = track_hit_damage(100.0, 5.0, ShellType::ArmorPiercing, true, false);
        assert!(chunk >= TRACK_HP_MAX, "a clean AP hit strips the whole pool: {chunk}");
    }

    #[test]
    fn oblique_and_ricochet_only_degrade() {
        let oblique = track_hit_damage(100.0, 55.0, ShellType::ArmorPiercing, true, false);
        let ricochet = track_hit_damage(100.0, 20.0, ShellType::ArmorPiercing, false, true);
        assert!(oblique > 0 && oblique < TRACK_HP_MAX, "oblique degrades but does not throw: {oblique}");
        assert!(ricochet > 0 && ricochet < TRACK_HP_MAX, "a ricochet barely scores: {ricochet}");
    }
}
