//! The battle damage log: a short left-edge feed of the player's dealt and taken hits, so the
//! last few seconds of a fight can be read back without a scoreboard. Entries age out after a
//! few seconds and the list stays capped — the log is a pulse, not a ledger.

use std::collections::VecDeque;

use game_core::{DamageEvent, ModuleSlot, TankId, TrackSide, VehicleKind};
use net::TankSnapshot;
use renderer_api::HudVertex;

use super::theme::{color as theme, tagged};

/// Dealt rows read as quiet readout off-white; taken rows read as signal red. Unique bytes per
/// class — the HUD tests tag features by exact vertex-color equality.
pub(crate) const DMG_DEALT_COLOR: [f32; 4] = tagged(theme::READOUT, 0.90);
pub(crate) const DMG_TAKEN_COLOR: [f32; 4] = [0.90, 0.36, 0.30, 0.92];

const LOG_CAP: usize = 6;
const LOG_TTL_S: f32 = 8.0;
/// Left-edge region, clear of the HP bar (top-left) and the speed readout (bottom-left).
const LOG_LEFT_X: f32 = -0.97;
const LOG_TOP_Y: f32 = -0.25;
const LOG_ROW_PITCH: f32 = 0.058;
const LOG_TEXT_SIZE: f32 = 0.036;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LogDirection {
    Dealt,
    Taken,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct DamageLogEntry {
    pub direction: LogDirection,
    pub damage_hp: u32,
    pub module: Option<ModuleSlot>,
    /// A track band this event struck: `(side, broke)`. Set even when `damage_hp` is 0 (a clean
    /// track break deals no HP), which is why such an event still earns a row.
    pub track: Option<(TrackSide, bool)>,
    /// The other party (target when dealt, attacker when taken), if still known.
    pub other_vehicle: Option<VehicleKind>,
    pub age_s: f32,
}

/// Rolling feed of the player's recent damage exchanges.
#[derive(Debug, Default)]
pub(crate) struct DamageLog {
    entries: VecDeque<DamageLogEntry>,
}

impl DamageLog {
    /// Ingest a snapshot's damage events: rows for damage the player dealt or took. Zero-damage
    /// events (bounces) stay out of the log — the hit-direction arcs and confirm ticks carry
    /// those.
    pub(crate) fn ingest(
        &mut self,
        events: &[DamageEvent],
        player: TankId,
        tanks: &[TankSnapshot],
    ) {
        for event in events {
            let direction = if event.source == player && event.target != player {
                LogDirection::Dealt
            } else if event.target == player && event.source != player {
                LogDirection::Taken
            } else {
                continue;
            };
            let track = event.track_hit.map(|hit| (hit.side, hit.broke));
            // A pure bounce stays out — the arcs and confirm ticks carry those. But a track hit
            // earns a row even at 0 HP: it is exactly the "your track is gone" event the log used
            // to swallow.
            if event.damage_hp == 0 && track.is_none() {
                continue;
            }
            let other_id =
                if direction == LogDirection::Dealt { event.target } else { event.source };
            let other_vehicle =
                tanks.iter().find(|tank| tank.tank_id == other_id).map(|tank| tank.vehicle);
            self.entries.push_front(DamageLogEntry {
                direction,
                damage_hp: event.damage_hp,
                module: event.module,
                track,
                other_vehicle,
                age_s: 0.0,
            });
        }
        self.entries.truncate(LOG_CAP);
    }

    pub(crate) fn tick(&mut self, dt: f32) {
        for entry in &mut self.entries {
            entry.age_s += dt;
        }
        self.entries.retain(|entry| entry.age_s < LOG_TTL_S);
    }

    /// Newest first, ready for `push_damage_log`.
    pub(crate) fn visible(&self) -> Vec<DamageLogEntry> {
        self.entries.iter().copied().collect()
    }
}

/// Draw the log rows top-down from the newest: damage number, module icon when a module was
/// hurt, and the other vehicle's short name.
pub(crate) fn push_damage_log(
    vertices: &mut Vec<HudVertex>,
    entries: &[DamageLogEntry],
    aspect: f32,
) {
    for (row, entry) in entries.iter().take(LOG_CAP).enumerate() {
        let y = LOG_TOP_Y - row as f32 * LOG_ROW_PITCH;
        let mut color = match entry.direction {
            LogDirection::Dealt => DMG_DEALT_COLOR,
            LogDirection::Taken => DMG_TAKEN_COLOR,
        };
        // The last second of life fades out instead of popping off the screen.
        color[3] *= ((LOG_TTL_S - entry.age_s) / 1.0).clamp(0.0, 1.0);
        if color[3] <= 0.0 {
            continue;
        }

        // The leading slot: the damage number, or a short "TRK" tag for a 0-HP track hit.
        if entry.damage_hp > 0 {
            let digits = crate::hud::number::digit_count(entry.damage_hp.min(9_999)) as f32;
            let number_right = LOG_LEFT_X + digits * LOG_TEXT_SIZE * 0.6;
            crate::hud::number::push_number(
                vertices,
                entry.damage_hp.min(9_999),
                number_right,
                y,
                LOG_TEXT_SIZE,
                aspect,
                color,
            );
        } else if entry.track.is_some() {
            crate::hud::font::push_text(
                vertices,
                "TRK",
                LOG_LEFT_X,
                y,
                LOG_TEXT_SIZE,
                aspect,
                color,
            );
        }

        let mut x = LOG_LEFT_X + 4.0 * LOG_TEXT_SIZE * 0.6 + 0.012;
        // A track hit always shows the suspension icon; a plain module hit shows its own.
        let icon_module =
            if entry.track.is_some() { Some(ModuleSlot::Suspension) } else { entry.module };
        if let Some(module) = icon_module {
            let icon = crate::hud::icons::HudIcon::for_module(module);
            crate::hud::font::push_icon(vertices, icon, x, y + 0.008, 0.045, aspect, color);
            x += 0.035;
        }
        if let Some((side, _broke)) = entry.track {
            let tag = match side {
                TrackSide::Left => "L",
                TrackSide::Right => "R",
            };
            crate::hud::font::push_text(vertices, tag, x, y, LOG_TEXT_SIZE, aspect, color);
            x += 0.022;
        }
        if let Some(kind) = entry.other_vehicle {
            crate::hud::font::push_text(
                vertices,
                kind.short_name(),
                x,
                y,
                LOG_TEXT_SIZE,
                aspect,
                color,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use game_core::{TankId, TeamId, VehicleKind};
    use glam::Vec3;

    use super::*;

    fn tank(id: u64, vehicle: VehicleKind) -> TankSnapshot {
        TankSnapshot {
            tank_id: TankId(id),
            team: TeamId(1),
            vehicle,
            position: [0.0, 0.0, 0.0],
            yaw_rad: 0.0,
            hull_pitch_rad: 0.0,
            hull_roll_rad: 0.0,
            turret_yaw_rad: 0.0,
            turret_yaw_velocity_rad_s: 0.0,
            gun_pitch_rad: 0.0,
            hit_points: 100,
            reload_remaining_s: 0.0,
            aim_dispersion_mrad: 0.0,
            module_hit_points: vehicle.spec().module_health.hit_points_by_slot(),
            destroyed_modules_mask: 0,
            track_damage_mask: 0,
            track_hp: [game_core::TRACK_HP_MAX; 2],
            ammo_counts: game_core::AmmoLoadout::default().counts,
            selected_ammo: 0,
            spotted_by_teams_mask: 0,
            armor_breaches: Default::default(),
            track_break_t: [None, None],
            engine_fire: false,
            fuel_fire: false,
            rack_fire_remaining_s: None,
        }
    }

    fn event(source: u64, target: u64, damage: u32) -> DamageEvent {
        DamageEvent {
            source: TankId(source),
            target: TankId(target),
            hit_position: Vec3::ZERO,
            damage_hp: damage,
            penetrated: damage > 0,
            ..Default::default()
        }
    }

    #[test]
    fn ingest_classifies_dealt_and_taken_and_skips_bounces() {
        let mut log = DamageLog::default();
        let tanks = [tank(1, VehicleKind::T54_1951), tank(2, VehicleKind::IS3)];
        log.ingest(
            &[event(1, 2, 320), event(2, 1, 150), event(2, 1, 0), event(3, 4, 90)],
            TankId(1),
            &tanks,
        );

        let rows = log.visible();
        assert_eq!(rows.len(), 2, "bounces and third-party exchanges stay out");
        // Newest first: the taken 150 was pushed after the dealt 320.
        assert_eq!(rows[0].direction, LogDirection::Taken);
        assert_eq!(rows[0].damage_hp, 150);
        assert_eq!(rows[0].other_vehicle, Some(VehicleKind::IS3));
        assert_eq!(rows[1].direction, LogDirection::Dealt);
        assert_eq!(rows[1].other_vehicle, Some(VehicleKind::IS3));
    }

    #[test]
    fn a_zero_hp_track_break_still_earns_a_row() {
        let mut log = DamageLog::default();
        let tanks = [tank(1, VehicleKind::T54_1951), tank(2, VehicleKind::IS3)];
        let mut tracked = event(2, 1, 0); // taken, dealt no HP
        tracked.track_hit =
            Some(game_core::TrackHit { side: game_core::TrackSide::Left, broke: true });
        log.ingest(&[tracked], TankId(1), &tanks);

        let rows = log.visible();
        assert_eq!(rows.len(), 1, "a 0-HP track break is not a silent bounce");
        assert_eq!(rows[0].track, Some((game_core::TrackSide::Left, true)));
        assert_eq!(rows[0].damage_hp, 0, "and it carries no HP number");
    }

    #[test]
    fn the_log_caps_its_rows_and_stale_entries_expire() {
        let mut log = DamageLog::default();
        let tanks = [tank(1, VehicleKind::T54_1951), tank(2, VehicleKind::IS3)];
        let events: Vec<DamageEvent> = (0..10).map(|i| event(1, 2, 100 + i)).collect();
        log.ingest(&events, TankId(1), &tanks);
        assert_eq!(log.visible().len(), LOG_CAP, "the log is a pulse, not a ledger");

        log.tick(LOG_TTL_S + 0.1);
        assert!(log.visible().is_empty(), "aged rows leave the feed");
    }

    #[test]
    fn dealt_and_taken_rows_draw_in_distinct_colors_in_the_left_mid_region() {
        let entries = [
            DamageLogEntry {
                direction: LogDirection::Dealt,
                damage_hp: 320,
                module: Some(game_core::ModuleSlot::Engine),
                track: None,
                other_vehicle: Some(VehicleKind::IS3),
                age_s: 0.0,
            },
            DamageLogEntry {
                direction: LogDirection::Taken,
                damage_hp: 150,
                module: None,
                track: None,
                other_vehicle: Some(VehicleKind::TigerI),
                age_s: 0.0,
            },
        ];
        let mut v = Vec::new();
        push_damage_log(&mut v, &entries, 16.0 / 9.0);

        let dealt: Vec<_> = v.iter().filter(|vert| vert.color == DMG_DEALT_COLOR).collect();
        let taken: Vec<_> = v.iter().filter(|vert| vert.color == DMG_TAKEN_COLOR).collect();
        assert!(!dealt.is_empty() && !taken.is_empty(), "both classes draw");
        let in_region = |vert: &&renderer_api::HudVertex| {
            vert.position[0] >= LOG_LEFT_X - 0.01
                && vert.position[0] < -0.4
                && vert.position[1] <= LOG_TOP_Y + 0.05
                && vert.position[1] > -0.65
        };
        assert!(dealt.iter().all(in_region) && taken.iter().all(in_region));
    }
}
