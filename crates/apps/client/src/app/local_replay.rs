//! The offline battle's replay (P10, the substrate of W3/S7).
//!
//! `WOT_RECORD=<path>` has always written the REMOTE session's accepted frames — and the game
//! the owner actually plays is the offline AI battle, which has no wire and so wrote nothing.
//! A battle you cannot watch back is a battle you can only argue about from memory. This
//! records the local host into exactly the same append-only frame stream
//! ([`net::recording`]): the world's opening word, the seat, the roster, one snapshot per
//! authoritative delivery, the end word. A reader feeds those frames through the same ingest
//! the live client uses, so a recorded offline battle cannot drift from a watched one — the
//! whole point of the format.
//!
//! What it is NOT: a re-simulation. The frames carry what the CLIENT was told, already filtered
//! by the host's per-viewer spotting — a replay of the player's battle shows what the player
//! could see, and nothing they could not.

use std::io::BufWriter;

use game_core::{MatchWeather, TankId};
use net::{ProtocolMessage, Snapshot, recording::FrameRecorder};
use terrain::MapId;

/// The session id every locally recorded frame carries. Offline play has no connection, so the
/// stream needs one constant word for the reader's session check — distinctive on purpose, so a
/// local recording is never mistaken for a captured wire session.
pub(super) const LOCAL_REPLAY_SESSION_ID: u64 = 0x10CA_1000_0000_0001;

pub(super) struct LocalReplayRecorder {
    path: String,
    /// `None` once a write has failed: a broken disk must cost the battle nothing but the
    /// recording, so the first error disarms the recorder and the game plays on.
    recorder: Option<FrameRecorder<BufWriter<std::fs::File>>>,
    opened: bool,
    closed: bool,
    snapshots: u64,
}

impl LocalReplayRecorder {
    /// Armed by `WOT_RECORD=<path>`; `None` otherwise. The file is NOT created here — the same
    /// variable arms the remote session's recorder, and a client that turns out to be joining a
    /// server must not have truncated the file the remote recorder is writing. The offline
    /// battle claims the path only when it actually opens one.
    pub(super) fn armed() -> Option<Self> {
        let path = std::env::var("WOT_RECORD").ok()?;
        Some(Self { path, recorder: None, opened: false, closed: false, snapshots: 0 })
    }

    /// The file being written, or `None` while nothing has been recorded into it yet — the HUD
    /// must not claim a recording that has not started.
    pub(super) fn active_path(&self) -> Option<&str> {
        (self.opened && self.recorder.is_some()).then_some(self.path.as_str())
    }

    fn write(&mut self, message: &ProtocolMessage) {
        let Some(recorder) = self.recorder.as_mut() else {
            return;
        };
        if let Err(error) = recorder.record(message) {
            tracing::error!(path = self.path, %error, "replay: recording stopped");
            self.recorder = None;
        }
    }

    /// The battle's opening words: the world to generate, the seat to watch it from, the roster
    /// to name it by. Written once — a second BATTLE from the garage keeps the first recording
    /// rather than interleaving two battles into one unreadable stream.
    pub(super) fn open_battle(
        &mut self,
        map_id: MapId,
        weather: MatchWeather,
        assigned_tank: TankId,
        time_limit_tick: Option<u64>,
        roster: Vec<net::RosterEntry>,
    ) {
        if self.opened {
            return;
        }
        match std::fs::File::create(&self.path) {
            Ok(file) => self.recorder = Some(FrameRecorder::new(BufWriter::new(file))),
            Err(error) => {
                tracing::error!(path = self.path, %error, "WOT_RECORD: the replay was not opened");
                self.closed = true;
                return;
            }
        }
        self.opened = true;
        let map_content_hash = map_forge::battlefield_hash(&map_forge::battlefield(map_id));
        self.write(&ProtocolMessage::ServerHello {
            session_id: LOCAL_REPLAY_SESSION_ID,
            protocol_version: net::PROTOCOL_VERSION,
            map_id,
            weather,
            map_content_hash,
        });
        self.write(&ProtocolMessage::StartBattle {
            session_id: LOCAL_REPLAY_SESSION_ID,
            assigned_tank,
            server_tick: 0,
            time_limit_tick,
        });
        self.write(&ProtocolMessage::BattleRoster {
            session_id: LOCAL_REPLAY_SESSION_ID,
            entries: roster,
        });
        tracing::info!(path = self.path, "replay: recording the offline battle");
    }

    /// One authoritative delivery. The local host has no wire, so its per-tick snapshot IS the
    /// delivery — recorded before the client folds it in, so the file holds the state the frame
    /// was drawn from.
    pub(super) fn record_snapshot(&mut self, snapshot: &Snapshot) {
        if !self.opened || self.closed {
            return;
        }
        self.write(&ProtocolMessage::Snapshot(snapshot.clone()));
        self.snapshots += 1;
    }

    /// The end word, then the buffer to disk — on the battle's edge, once.
    pub(super) fn end_battle(&mut self, winning_team: Option<u16>) {
        if !self.opened || self.closed {
            return;
        }
        self.closed = true;
        self.write(&ProtocolMessage::BattleEnded {
            session_id: LOCAL_REPLAY_SESSION_ID,
            winning_team,
            spotting_log: Vec::new(),
        });
        self.flush();
    }

    /// Push the buffered frames at the file. A recording is only worth what survives the exit.
    /// A run cut short (`WOT_EXIT_AFTER_S`, alt-F4) ends here with NO end word: the file then
    /// says truthfully that the battle was still going, instead of inventing a draw.
    pub(super) fn flush(&mut self) {
        let Some(recorder) = self.recorder.as_mut() else {
            return;
        };
        match recorder.flush() {
            Ok(()) => tracing::info!(
                path = self.path,
                snapshots = self.snapshots,
                ended = self.closed,
                "replay written"
            ),
            Err(error) => {
                tracing::error!(path = self.path, %error, "replay: the tail was not flushed")
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The offline battle's replay is the SAME stream the wire writes: the reader that plays a
    /// recorded network session back must accept a recorded AI battle without knowing which it
    /// is. That is the whole reason the format is shared — lock the words and their order.
    #[test]
    fn the_offline_battle_records_the_same_frames_a_wire_session_would() {
        let dir = std::env::temp_dir().join(format!("wot-local-replay-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("temp dir");
        let path = dir.join("battle.replay");
        let mut recorder = LocalReplayRecorder {
            path: path.to_string_lossy().into_owned(),
            recorder: None,
            opened: false,
            closed: false,
            snapshots: 0,
        };
        assert_eq!(recorder.active_path(), None, "nothing recorded yet is not a recording");

        let map_id = MapId::default();
        recorder.open_battle(map_id, MatchWeather::default(), TankId(7), Some(1_234), Vec::new());
        assert!(recorder.active_path().is_some());
        for tick in 0..3 {
            recorder.record_snapshot(&Snapshot { server_tick: tick, ..Snapshot::default() });
        }
        recorder.end_battle(Some(1));
        // Twice is once: a second end word would make the file say the battle ended twice.
        recorder.end_battle(Some(0));
        assert_eq!(recorder.snapshots, 3);

        let file = std::fs::File::open(&path).expect("replay file");
        let frames = net::recording::read_recording(file).expect("readable recording");
        assert_eq!(frames.len(), 7, "hello, seat, roster, 3 snapshots, end: {frames:?}");
        match &frames[0] {
            ProtocolMessage::ServerHello { map_id: recorded, map_content_hash, .. } => {
                assert_eq!(*recorded, map_id);
                assert_eq!(
                    *map_content_hash,
                    map_forge::battlefield_hash(&map_forge::battlefield(map_id)),
                    "the replay names the world it was played on"
                );
            }
            other => panic!("first frame must be the world's word: {other:?}"),
        }
        match &frames[1] {
            ProtocolMessage::StartBattle { assigned_tank, time_limit_tick, .. } => {
                assert_eq!(*assigned_tank, TankId(7));
                assert_eq!(*time_limit_tick, Some(1_234));
            }
            other => panic!("second frame must be the seat: {other:?}"),
        }
        assert!(matches!(&frames[2], ProtocolMessage::BattleRoster { .. }));
        for (index, frame) in frames[3..6].iter().enumerate() {
            match frame {
                ProtocolMessage::Snapshot(snapshot) => {
                    assert_eq!(snapshot.server_tick, index as u64);
                }
                other => panic!("frame {index} must be a snapshot: {other:?}"),
            }
        }
        assert!(
            matches!(&frames[6], ProtocolMessage::BattleEnded { winning_team: Some(1), .. }),
            "the end word carries the winner the battle actually had"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A run cut short (`WOT_EXIT_AFTER_S`, alt-F4) must leave a READABLE file — every frame
    /// that was recorded, and no end word for a battle that never ended.
    #[test]
    fn a_run_cut_short_flushes_what_it_had_and_invents_no_ending() {
        let dir = std::env::temp_dir().join(format!("wot-local-replay-cut-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("temp dir");
        let path = dir.join("cut.replay");
        let mut recorder = LocalReplayRecorder {
            path: path.to_string_lossy().into_owned(),
            recorder: None,
            opened: false,
            closed: false,
            snapshots: 0,
        };
        recorder.open_battle(
            MapId::default(),
            MatchWeather::default(),
            TankId(1),
            None,
            Vec::new(),
        );
        recorder.record_snapshot(&Snapshot::default());
        recorder.flush();

        let file = std::fs::File::open(&path).expect("replay file");
        let frames = net::recording::read_recording(file).expect("readable recording");
        assert_eq!(frames.len(), 4, "hello, seat, roster, one snapshot");
        assert!(
            !frames.iter().any(|frame| matches!(frame, ProtocolMessage::BattleEnded { .. })),
            "a battle that was still going has no end word"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}
