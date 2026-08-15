//! L: the garage answers for the battle (Hala 3.0).
//!
//! L1 — the hero comes back WEARING the fight: the damage masks, the thrown belt and the
//! hit decals it earned on the field ride into the hall with it, captured at the same
//! `open_garage` seam the dust uses (J2). A clean vehicle renders clean — the dirty state
//! is EARNED, never decorative (the Valve rule the plan locks the whole stage to).
//!
//! L2 — the repair moment: "napraw" plays a short beat (the hull rises off its springs a
//! few centimetres, the shop answers with a clunk at each end) and the state crosses
//! damaged → clean on screen. The lock: after the beat there is ZERO battle residue.

use net::TankSnapshot;

use crate::vehicle::variation::{HitDecal, VehicleVariation};

/// How long the repair beat plays — inside the plan's 2–5 s window.
pub(in crate::app) const REPAIR_BEAT_S: f32 = 3.2;
/// How high the jack lifts the hull at the beat's crest.
pub(in crate::app) const REPAIR_LIFT_M: f32 = 0.03;

/// The battle state the hero carries into the hall: exactly the fields the vehicle render
/// path reads for damage, plus the decal history the snapshot never carries (scars are
/// client-side memory, `ClientApp::tank_scars`).
#[derive(Debug, Clone, PartialEq)]
pub(in crate::app) struct FieldWear {
    vehicle: game_core::VehicleKind,
    hit_points: u32,
    module_hit_points: [u32; game_core::MODULE_SLOT_COUNT],
    destroyed_modules_mask: u8,
    track_damage_mask: u8,
    track_break_t: [Option<f32>; 2],
    decals: Vec<HitDecal>,
}

impl FieldWear {
    /// Capture the player's tank as it left the field. `scars` is its decal history, if any.
    pub(in crate::app) fn from_battle(
        snapshot: &TankSnapshot,
        scars: Option<&VehicleVariation>,
    ) -> Self {
        Self {
            vehicle: snapshot.vehicle,
            hit_points: snapshot.hit_points,
            module_hit_points: snapshot.module_hit_points,
            destroyed_modules_mask: snapshot.destroyed_modules_mask,
            track_damage_mask: snapshot.track_damage_mask,
            track_break_t: snapshot.track_break_t,
            decals: scars.map(|s| s.decals().to_vec()).unwrap_or_default(),
        }
    }

    /// Whether the field actually left a mark — an unscathed return renders clean and the
    /// repair action has nothing to offer.
    pub(in crate::app) fn is_marked(&self) -> bool {
        self.destroyed_modules_mask != 0
            || self.track_damage_mask != 0
            || self.track_break_t.iter().any(Option::is_some)
            || !self.decals.is_empty()
    }

    /// Merge the wear into the parked preview snapshot — only onto the vehicle that earned
    /// it (a roster switch parks a different, clean machine).
    pub(in crate::app) fn apply(&self, snapshot: &mut TankSnapshot) {
        if snapshot.vehicle != self.vehicle {
            return;
        }
        snapshot.hit_points = self.hit_points.max(1);
        snapshot.module_hit_points = self.module_hit_points;
        snapshot.destroyed_modules_mask = self.destroyed_modules_mask;
        snapshot.track_damage_mask = self.track_damage_mask;
        snapshot.track_break_t = self.track_break_t;
    }

    pub(in crate::app) fn decals(&self) -> &[HitDecal] {
        &self.decals
    }

    pub(in crate::app) fn vehicle(&self) -> game_core::VehicleKind {
        self.vehicle
    }
}

impl super::GarageState {
    /// L1 capture seam: the hero returned from the field wearing this.
    pub(in crate::app) fn wear_from_the_field(&mut self, wear: FieldWear) {
        self.wear = Some(wear);
    }

    /// The wear the parked hero currently carries, if it earned any.
    pub(in crate::app) fn field_wear(&self) -> Option<&FieldWear> {
        self.wear.as_ref()
    }

    /// Whether the PARKED hero visibly wears the fight — wear present, actually marked, and
    /// belonging to the machine on the turntable. Drives the repair offer and the HUD tag.
    pub(in crate::app) fn hero_is_marked(&self) -> bool {
        self.wear
            .as_ref()
            .is_some_and(|wear| wear.is_marked() && wear.vehicle() == self.selected_vehicle())
    }

    /// L2: start the repair beat. Only a marked hero has anything to repair, and a beat
    /// already playing keeps playing. Returns whether the beat actually started.
    pub(in crate::app) fn start_repair(&mut self) -> bool {
        if !self.hero_is_marked() || self.repair.is_some() {
            return false;
        }
        self.repair = Some(0.0);
        true
    }

    /// Advance the repair beat. Returns true on the frame the beat COMPLETES — the state
    /// crosses damaged → clean and the caller answers with the finishing clunk. After
    /// completion there is zero battle residue, by construction.
    pub(in crate::app) fn tick_repair(&mut self, dt: f32) -> bool {
        let Some(elapsed) = self.repair.as_mut() else {
            return false;
        };
        *elapsed += dt;
        // R3: the mechanic's round clock pauses for the length of the beat — he steps toward
        // the ring to work instead of walking his round, and resumes exactly where he left.
        self.mechanic_pause_s += dt;
        if *elapsed >= REPAIR_BEAT_S {
            self.repair = None;
            self.wear = None;
            return true;
        }
        false
    }

    /// The live work cue for the mechanic (R3), while a beat plays: its elapsed seconds and
    /// the one beat length everything answers to.
    pub(in crate::app) fn repair_work_cue(&self) -> Option<scene_build::hangar_mechanic::WorkCue> {
        self.repair.map(|elapsed_s| scene_build::hangar_mechanic::WorkCue {
            elapsed_s,
            beat_s: REPAIR_BEAT_S,
        })
    }

    /// Total time the mechanic's round clock has been paused by repair beats (R3).
    pub(in crate::app) fn mechanic_pause_s(&self) -> f32 {
        self.mechanic_pause_s
    }

    /// The jack's lift this frame: a smooth rise-and-settle bump over the beat, zero when
    /// the shop is idle. The hull visibly comes off its springs and sits back down.
    pub(in crate::app) fn repair_lift_m(&self) -> f32 {
        let Some(elapsed) = self.repair else {
            return 0.0;
        };
        // A half-sine bump: up through the first half, settling through the second.
        REPAIR_LIFT_M * (std::f32::consts::PI * (elapsed / REPAIR_BEAT_S).clamp(0.0, 1.0)).sin()
    }

    pub(in crate::app) fn repair_active(&self) -> bool {
        self.repair.is_some()
    }

    /// A vehicle switch parks a DIFFERENT machine: it arrives clean, and any beat mid-play
    /// belongs to the tank that just rolled out.
    pub(in crate::app) fn clear_wear_for_new_vehicle(&mut self) {
        self.wear = None;
        self.repair = None;
    }
}

#[cfg(test)]
mod tests {
    use game_core::VehicleKind;

    use super::super::GarageState;
    use super::*;
    use crate::vehicle::variation::{DecalFrame, DecalKind};

    fn worn_snapshot() -> TankSnapshot {
        let mut snapshot =
            crate::app::garage_render::garage_preview_snapshot(VehicleKind::PLAYABLE[0]);
        snapshot.hit_points = 320;
        snapshot.destroyed_modules_mask = 0b0000_0100;
        snapshot.track_damage_mask = 0b01;
        snapshot.track_break_t = [Some(0.35), None];
        snapshot
    }

    fn scarred_variation() -> VehicleVariation {
        let mut variation = VehicleVariation::default();
        variation.record_hit(HitDecal {
            local_position: [0.5, 1.0, 0.2],
            local_normal: [0.0, 0.0, 1.0],
            radius: 0.2,
            age_s: 0.0,
            kind: DecalKind::Penetration,
            frame: DecalFrame::Hull,
            patch: None,
        });
        variation
    }

    /// L1 CONTRACT: the wear the field wrote is exactly the wear the hall shows — masks,
    /// thrown belt and decal history all carried; and it lands ONLY on the vehicle that
    /// earned it, a roster sibling stays clean.
    #[test]
    fn the_hall_shows_what_the_field_wrote() {
        let wear = FieldWear::from_battle(&worn_snapshot(), Some(&scarred_variation()));
        assert!(wear.is_marked());
        assert_eq!(wear.decals().len(), 1);

        let mut parked =
            crate::app::garage_render::garage_preview_snapshot(VehicleKind::PLAYABLE[0]);
        wear.apply(&mut parked);
        assert_eq!(parked.destroyed_modules_mask, 0b0000_0100);
        assert_eq!(parked.track_damage_mask, 0b01);
        assert_eq!(parked.track_break_t, [Some(0.35), None]);
        assert_eq!(parked.hit_points, 320);

        let mut sibling =
            crate::app::garage_render::garage_preview_snapshot(VehicleKind::PLAYABLE[1]);
        wear.apply(&mut sibling);
        assert_eq!(sibling.destroyed_modules_mask, 0, "a different machine parks clean");
        assert_eq!(sibling.track_break_t, [None, None]);
    }

    /// The Valve rule, negatively: an unscathed return is NOT marked — nothing to show,
    /// nothing to repair.
    #[test]
    fn an_unscathed_return_is_not_marked() {
        let clean = crate::app::garage_render::garage_preview_snapshot(VehicleKind::PLAYABLE[0]);
        let wear = FieldWear::from_battle(&clean, None);
        assert!(!wear.is_marked());
        let mut garage = GarageState::default();
        garage.wear_from_the_field(wear);
        assert!(!garage.start_repair(), "a clean hero offers no repair");
    }

    /// L2 CONTRACT: the beat starts only on a marked hero, lifts the hull mid-play, runs
    /// 2–5 s, and completing it leaves ZERO battle residue — no wear, no lift, no beat.
    #[test]
    fn the_repair_beat_ends_with_zero_battle_residue() {
        assert!((2.0..=5.0).contains(&REPAIR_BEAT_S), "the plan's beat window");
        let mut garage = GarageState::default();
        assert!(!garage.start_repair(), "no wear, no beat");

        garage.wear_from_the_field(FieldWear::from_battle(
            &worn_snapshot(),
            Some(&scarred_variation()),
        ));
        assert!(garage.start_repair());
        assert!(!garage.start_repair(), "a beat mid-play keeps playing");

        let mut finished = false;
        let mut lifted = false;
        for _ in 0..400 {
            if garage.repair_lift_m() > 0.02 {
                lifted = true;
            }
            if garage.tick_repair(1.0 / 60.0) {
                finished = true;
                break;
            }
        }
        assert!(finished, "the beat completes");
        assert!(lifted, "the hull visibly came off its springs");
        assert!(garage.field_wear().is_none(), "zero battle residue: wear gone");
        assert!(!garage.repair_active() && garage.repair_lift_m() == 0.0, "shop idle again");
        assert!(!garage.tick_repair(1.0), "nothing left to tick");
    }

    /// A vehicle switch parks a different, clean machine — wear and any beat mid-play are
    /// dropped with the tank that rolled out.
    #[test]
    fn a_roster_switch_parks_a_clean_machine() {
        let mut garage = GarageState::default();
        garage.wear_from_the_field(FieldWear::from_battle(&worn_snapshot(), None));
        assert!(garage.start_repair());
        garage.clear_wear_for_new_vehicle();
        assert!(garage.field_wear().is_none() && !garage.repair_active());
    }
}
