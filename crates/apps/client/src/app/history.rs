//! The battle history on disk (interface program P5): one JSON per battle under `battles/`
//! beside `garage.json`, written once at the battle's end from the ledger's records — the
//! same words the results page read — plus an index for the BATTLES page (P4); on the garage
//! file's pattern: a version, a tolerant load (a corrupt or foreign file is an empty history,
//! never a panic), an atomic write.

use std::path::{Path, PathBuf};

use game_core::{DamageEvent, KillEvent, TankId, TeamId, VehicleKind};
use net::{RosterEntry, SpottingRecord};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use terrain::MapId;

use super::ClientApp;
use super::ledger::{BattleEnd, BattleLedger, OwnShot, SpottedSpan};
use crate::hud::BattleHudOutcome;

pub(crate) const HISTORY_VERSION: u32 = 1;

/// One battle as written: the ledger's records and the roster, nothing derived.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct BattleRecord {
    pub version: u32,
    /// Unix seconds when the end word arrived.
    pub ended_at: i64,
    pub map: String,
    pub vehicle: String,
    pub player: TankId,
    pub player_team: u16,
    pub outcome: String,
    pub end_tick: u64,
    pub damage: Vec<DamageEvent>,
    pub kills: Vec<KillEvent>,
    pub shots: Vec<OwnShot>,
    pub spotted: Vec<SpottedSpan>,
    pub observers: Vec<SpottingRecord>,
    pub roster: Vec<RosterEntry>,
}

/// One line of the index: enough for the BATTLES page without opening the file.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct HistoryEntry {
    pub file: String,
    pub ended_at: i64,
    pub map: String,
    pub vehicle: String,
    pub outcome: String,
    pub kills: u32,
    pub damage_dealt: u32,
}

/// The index, newest first.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub(crate) struct HistoryIndex {
    pub version: u32,
    pub entries: Vec<HistoryEntry>,
}

pub(crate) fn outcome_slug(outcome: BattleHudOutcome) -> &'static str {
    match outcome {
        BattleHudOutcome::Victory => "victory",
        BattleHudOutcome::Defeat => "defeat",
        BattleHudOutcome::Draw => "draw",
        BattleHudOutcome::ConnectionLost => "connection-lost",
        BattleHudOutcome::BattleOver => "battle-over",
    }
}

pub(crate) fn outcome_from_slug(slug: &str) -> BattleHudOutcome {
    match slug {
        "victory" => BattleHudOutcome::Victory,
        "defeat" => BattleHudOutcome::Defeat,
        "draw" => BattleHudOutcome::Draw,
        "connection-lost" => BattleHudOutcome::ConnectionLost,
        _ => BattleHudOutcome::BattleOver,
    }
}

/// `battles/` beside `garage.json`.
pub(crate) fn history_dir() -> PathBuf {
    super::garage::persistence::config_dir()
        .map_or_else(|| PathBuf::from("battles"), |dir| dir.join("battles"))
}

/// The day and the hour the battle ended, on the player's clock.
pub(crate) fn date_word(ended_at: i64) -> String {
    chrono::DateTime::<chrono::Utc>::from_timestamp(ended_at, 0).map_or_else(
        || "-".to_string(),
        |time| time.with_timezone(&chrono::Local).format("%Y-%m-%d %H:%M").to_string(),
    )
}

/// The file a battle is written to: the end's seconds and the map, unique by the seconds.
fn battle_file_name(ended_at: i64, map: &str) -> String {
    format!("{ended_at}_{map}.json")
}

impl BattleRecord {
    /// The ledger's records, once ended; `None` while the battle runs.
    pub(crate) fn from_ledger(
        ledger: &BattleLedger,
        roster: &[RosterEntry],
        map: MapId,
        vehicle: VehicleKind,
        player_team: TeamId,
        ended_at: i64,
    ) -> Option<Self> {
        let end = ledger.ended()?;
        Some(Self {
            version: HISTORY_VERSION,
            ended_at,
            map: map.slug().to_string(),
            vehicle: vehicle.slug().to_string(),
            player: ledger.player(),
            player_team: player_team.0,
            outcome: outcome_slug(end.outcome).to_string(),
            end_tick: end.tick,
            damage: ledger.damage().copied().collect(),
            kills: ledger.kills().copied().collect(),
            shots: ledger.shots().to_vec(),
            spotted: ledger.spotted_spans().to_vec(),
            observers: ledger.observers().to_vec(),
            roster: roster.to_vec(),
        })
    }

    /// The ledger read back: the results page over a stored battle is the live page.
    pub(crate) fn ledger(&self) -> BattleLedger {
        BattleLedger::from_records(
            self.player,
            &self.damage,
            &self.kills,
            &self.shots,
            &self.spotted,
            &self.observers,
            Some(BattleEnd { tick: self.end_tick, outcome: outcome_from_slug(&self.outcome) }),
        )
    }

    pub(crate) fn player_team(&self) -> TeamId {
        TeamId(self.player_team)
    }

    fn entry(&self, file: String) -> HistoryEntry {
        let own = self.ledger().own();
        HistoryEntry {
            file,
            ended_at: self.ended_at,
            map: self.map.clone(),
            vehicle: self.vehicle.clone(),
            outcome: self.outcome.clone(),
            kills: own.kills,
            damage_dealt: own.damage_dealt,
        }
    }
}

/// A JSON file read tolerantly: missing, malformed or foreign is `None`, with a warning for
/// the malformed case so a broken file is visible without being fatal.
fn load_json<T: DeserializeOwned>(path: &Path, what: &str) -> Option<T> {
    let raw = std::fs::read_to_string(path).ok()?;
    match serde_json::from_str::<T>(&raw) {
        Ok(value) => Some(value),
        Err(error) => {
            tracing::warn!(%error, path = %path.display(), "{what} is unreadable; ignoring it");
            None
        }
    }
}

/// A JSON file written atomically: a sibling `.tmp`, renamed over the target.
fn store_json<T: Serialize>(path: &Path, value: &T, what: &str) -> bool {
    if let Some(parent) = path.parent()
        && let Err(error) = std::fs::create_dir_all(parent)
    {
        tracing::warn!(%error, "could not create the {what} directory");
        return false;
    }
    let json = match serde_json::to_string_pretty(value) {
        Ok(json) => json,
        Err(error) => {
            tracing::warn!(%error, "could not serialize the {what}");
            return false;
        }
    };
    let tmp = path.with_extension("json.tmp");
    if let Err(error) = std::fs::write(&tmp, json) {
        tracing::warn!(%error, "could not write the {what} temp");
        return false;
    }
    if let Err(error) = std::fs::rename(&tmp, path) {
        tracing::warn!(%error, "could not replace the {what}");
        let _ = std::fs::remove_file(&tmp);
        return false;
    }
    true
}

/// The history at a directory: the index it holds, the writer, the reader.
#[derive(Debug, Clone)]
pub(crate) struct BattleHistory {
    dir: PathBuf,
    index: HistoryIndex,
}

impl BattleHistory {
    /// Open the history at `dir`, reading its index; a missing, corrupt or foreign index is
    /// an empty history.
    pub(crate) fn open(dir: PathBuf) -> Self {
        let index = load_json::<HistoryIndex>(&dir.join("index.json"), "battle index")
            .filter(|index| index.version == HISTORY_VERSION)
            .unwrap_or_default();
        Self { dir, index }
    }

    /// Newest first.
    pub(crate) fn entries(&self) -> &[HistoryEntry] {
        &self.index.entries
    }

    /// Write one battle: its file, then the index with the new line first. A second battle
    /// ending in the same second takes the next name.
    pub(crate) fn write(&mut self, record: &BattleRecord) {
        let mut file = battle_file_name(record.ended_at, &record.map);
        let mut suffix = 1;
        while self.dir.join(&file).exists() {
            file = format!("{}_{}_{suffix}.json", record.ended_at, record.map);
            suffix += 1;
        }
        if !store_json(&self.dir.join(&file), record, "battle record") {
            return;
        }
        self.index.version = HISTORY_VERSION;
        self.index.entries.insert(0, record.entry(file));
        store_json(&self.dir.join("index.json"), &self.index, "battle index");
    }

    /// The battle behind an index line, read back; a missing, corrupt or foreign file is
    /// `None`.
    pub(crate) fn read(&self, entry: usize) -> Option<BattleRecord> {
        let line = self.index.entries.get(entry)?;
        load_json::<BattleRecord>(&self.dir.join(&line.file), "battle record")
            .filter(|record| record.version == HISTORY_VERSION)
    }
}

impl ClientApp {
    /// Turn on the history at `dir`, reading its index (the real startup path; tests never
    /// touch the player's battles).
    pub(super) fn enable_history_persistence(&mut self, dir: PathBuf) {
        self.history = Some(BattleHistory::open(dir));
    }

    /// The battle's end, written once (P5): the ledger's records, the roster, the map, the
    /// hull — at the outcome's edge, after the end word named the observers.
    pub(super) fn write_battle_record(&mut self) {
        if self.history.is_none() {
            return;
        }
        let roster = self.session.roster();
        let (map, vehicle, team) =
            (self.session.map_id(), self.garage.selected_vehicle(), self.player_team());
        let Some(record) = BattleRecord::from_ledger(
            &self.ledger,
            &roster,
            map,
            vehicle,
            team,
            chrono::Utc::now().timestamp(),
        ) else {
            return;
        };
        if let Some(history) = &mut self.history {
            history.write(&record);
        }
    }

    /// The history as held; the locks read it, the garage's STATISTICS tab (G10) next.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn history(&self) -> Option<&BattleHistory> {
        self.history.as_ref()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use game_core::{BattleEventId, DamageCause, ShellId, ShotFired};

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("wot-history-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    fn record(ended_at: i64) -> BattleRecord {
        let me = TankId(1);
        let mut ledger = BattleLedger::new(me);
        let snapshot = net::Snapshot {
            server_tick: 1_200,
            damage_events: vec![DamageEvent {
                source: me,
                target: TankId(9),
                damage_hp: 240,
                penetrated: true,
                cause: DamageCause::Shell,
                event_id: BattleEventId(31),
                occurred_tick: 1_200,
                shell_id: Some(ShellId(4)),
                ..Default::default()
            }],
            shots_fired: vec![ShotFired { shooter: me, shell_id: ShellId(4) }],
            ..Default::default()
        };
        ledger.ingest_snapshot(&snapshot, me);
        ledger.ingest_kills(&[KillEvent {
            victim: TankId(9),
            killer: Some(me),
            cause: DamageCause::Shell,
            occurred_tick: 2_400,
        }]);
        ledger.spotted(1_140, true);
        ledger.end(4_000, BattleHudOutcome::Victory);
        ledger.name_observers(vec![SpottingRecord {
            observer: TankId(9),
            distance_m: 308.5,
            from_tick: 1_100,
            to_tick: 1_640,
        }]);
        let roster = vec![
            RosterEntry {
                tank_id: me,
                team: TeamId(1),
                vehicle: VehicleKind::BENCHMARK,
                seat: 0,
                crew_kind: net::CrewKind::Human,
            },
            RosterEntry {
                tank_id: TankId(9),
                team: TeamId(2),
                vehicle: VehicleKind::BENCHMARK,
                seat: 1,
                crew_kind: net::CrewKind::Bot,
            },
        ];
        BattleRecord::from_ledger(
            &ledger,
            &roster,
            MapId::default(),
            VehicleKind::BENCHMARK,
            TeamId(1),
            ended_at,
        )
        .expect("ended")
    }

    /// P5: a battle is written once at its end — its file and one index line, newest first —
    /// and read back as the same ledger; a missing history is empty; a corrupt index or a
    /// corrupt battle file degrades to an empty history or a missing battle, never a panic;
    /// a foreign version is ignored; the write is atomic (no temp lingers).
    #[test]
    fn a_battle_is_written_once_at_its_end_and_a_corrupt_file_degrades_to_an_empty_history() {
        let dir = temp_dir("write");
        let mut history = BattleHistory::open(dir.clone());
        assert!(history.entries().is_empty(), "no history yet");
        let first = record(1_757_160_000);
        history.write(&first);
        let second = record(1_757_163_600);
        history.write(&second);
        assert_eq!(history.entries().len(), 2);
        assert_eq!(history.entries()[0].ended_at, second.ended_at, "newest first");
        assert_eq!(history.entries()[0].kills, 1);
        assert_eq!(history.entries()[0].damage_dealt, 240);
        assert_eq!(history.entries()[0].outcome, "victory");
        assert!(!dir.join("index.json.tmp").exists() && dir.read_dir().expect("dir").count() == 3);
        // Read back: the same ledger, the same page.
        let reopened = BattleHistory::open(dir.clone());
        assert_eq!(reopened.entries(), history.entries());
        let back = reopened.read(1).expect("the first battle");
        assert_eq!(back, first);
        let ledger = back.ledger();
        assert_eq!(ledger.own().damage_dealt, 240);
        assert_eq!(ledger.observers().len(), 1);
        assert_eq!(ledger.ended().map(|end| end.outcome), Some(BattleHudOutcome::Victory));
        // The same second twice: the next name, never an overwrite.
        history.write(&first);
        assert_eq!(history.entries().len(), 3);
        assert_ne!(history.entries()[0].file, history.entries()[2].file);
        // A corrupt battle file is a missing battle; a corrupt index is an empty history.
        std::fs::write(dir.join(&reopened.entries()[1].file), "{ not json").expect("scribble");
        assert!(reopened.read(1).is_none(), "a corrupt file is a missing battle, never a panic");
        std::fs::write(dir.join("index.json"), "{ not json").expect("scribble");
        assert!(BattleHistory::open(dir.clone()).entries().is_empty());
        let foreign =
            HistoryIndex { version: HISTORY_VERSION + 1, entries: history.entries().to_vec() };
        store_json(&dir.join("index.json"), &foreign, "battle index");
        assert!(
            BattleHistory::open(dir.clone()).entries().is_empty(),
            "a foreign version is ignored"
        );
        assert!(date_word(first.ended_at).contains('-'));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
