//! What ground a tank can drive — the numbers every layer has to agree about.
//!
//! These lived in three places at once. The physics controller had 0.6 ("the classic ~60 % tank
//! gradeability"), the map contract's drive graph had 0.55, and the road check had 0.5 — three
//! hand-written answers to one physical question, in three crates, none of them referring to the
//! others. Nothing was wrong with any single number; what was wrong is that a map could refuse
//! ground the tank drives, and nobody would ever see the two constants side by side.
//!
//! The gap was academic while the terrain grid was 5 m: a coarse grid averages slopes away, so
//! almost nothing landed between 0.55 and 0.6. Measured across the shipped maps at 2.5 m it stops
//! being academic — Prokhorovka's share of ground in that disputed band roughly quadruples
//! (0.079 % → 0.290 % of drive-graph edges, ~1 730 edges the gate calls a wall and the hull
//! climbs). Densifying the terrain is what makes one number necessary.

/// The steepest face a tank climbs under its own drive, rise over run.
///
/// ~0.6 is about 31°, the classic ~60 % tank gradeability. This is the AUTHORITY: the physics
/// controller's climb limit and the map contract's drive-graph wall are both this number, so a
/// map can never refuse ground the game would let a hull drive.
///
/// Deliberately NOT the momentum-climb ceiling (`physics`, 0.68). A committed diagonal run-up can
/// scrabble above this, but that is a skilled exception; a map's connectivity must be provable by
/// a tank driving normally, not by one taking a run at the hill.
pub const MAX_CLIMB_GRADE: f32 = 0.6;

/// The steepest grade a ROAD may reach before the map report complains.
///
/// Lower than [`MAX_CLIMB_GRADE`] on purpose, and that is the whole point of it being a separate
/// number: a road is a promise of comfortable movement, not of barely-possible movement. A route
/// that needs the hull's full gradeability is a climb, and if it is drawn as a road the map is
/// lying to whoever reads it.
pub const ROAD_COMFORT_GRADE: f32 = 0.5;

/// A road at the hull's climb limit is not a road, it is a climb drawn as one. Checked at COMPILE
/// time rather than in a test: the ordering is a property of the two literals, and a test that
/// compares two constants is a test clippy is right to call pointless.
const _: () = assert!(ROAD_COMFORT_GRADE < MAX_CLIMB_GRADE);
