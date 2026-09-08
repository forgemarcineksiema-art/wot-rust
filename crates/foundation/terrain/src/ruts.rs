//! Ruts with MEMORY (the one program's T8). `GroundProperties::rut_depth_m` said how deep a
//! surface remembers a track and only the mark's opacity read it: a column of tanks left a
//! road that faded in a minute. The rut field is the client's own ledger of where tracks have
//! pressed soft ground — no gameplay, no wire, never folded into `sample_height` — and the
//! ground mesh reads it: every pass presses a little more, up to the softest ground's memory,
//! and the trough stays as long as the battle does.
//!
//! **The ledger is a RASTER, and that is the whole point.** It used to be a ring of 2 048
//! segments that overwrote its oldest press, and the ground mesh is patched by CUTTING the
//! base triangles a rut touches and drawing a finer patch in their place. The cut was one-way
//! while the patch was replaced wholesale and covered only the CURRENT touched set — so every
//! press the ring forgot left its cells cut with nothing standing in for them: a permanent
//! hole in the ground. Measured on Orliny, 90 s of one AI battle: zero holes for twenty
//! seconds, then the ring wraps and 7 068 base triangles — about 22 000 m² of ground — are
//! gone by second 76, growing at ~380 m²/s for the rest of the battle.
//!
//! A raster cannot forget: the memory of the ground is now permanent, and reading a depth is
//! O(1) instead of a walk over every press in the battle. What the PICTURE can afford is a
//! separate question with its own answer — `scene_build::RUT_PATCH_CELL_BUDGET` spends the
//! patch on the cells nearest the eye, and `renderer_wgpu::set_ground_cut` puts back what
//! falls outside it, so bounding the patch can never dig a hole again. The touched set also
//! follows the TRACK now instead of each press's bounding box, which is what made the patches
//! rectangles.
//!
//! Cost: tiles are allocated only where tracks have run — 8 m × 8 m of ground per KiB. The
//! measured 24 000 m² above is ~375 KiB; a 1 km² map driven over end to end is the ceiling at
//! 15.6 MiB.

use std::collections::{BTreeMap, BTreeSet};

/// One pass of one track presses this much into soft ground.
pub const RUT_PASS_DEPTH_M: f32 = 0.03;
/// Half the width of a track's rut.
pub const RUT_HALF_WIDTH_M: f32 = 0.30;
/// The raster's cell. Finer than the ground patch's own mesh step (0.3125 m on the 2.5 m
/// grid), so the drawn trough never aliases the memory it reads, and fine enough that the
/// 0.60 m trough is carried by several samples rather than one.
pub const RUT_RASTER_CELL_M: f32 = 0.25;
/// The deepest rut the raster stores; depth is quantised to a byte over this range. The
/// deepest ground memory in the fleet's materials today is dirt's 0.09 m, so this is head
/// room, not a limit anything reaches.
pub const RUT_RASTER_MAX_DEPTH_M: f32 = 0.25;
/// One step of the depth quantisation — the tolerance every depth read carries.
pub const RUT_DEPTH_QUANT_STEP_M: f32 = RUT_RASTER_MAX_DEPTH_M / 255.0;
/// Ground that remembers less than this takes no press at all (rock).
const MIN_REMEMBERED_DEPTH_M: f32 = 0.01;
/// A tile's side in raster cells: 32 × 32 cells is 8 m × 8 m of ground in one KiB.
const TILE: usize = 32;

/// One 8 m × 8 m patch of the ledger. Allocated the first time a track runs through it and
/// never dropped — the mesh's cut is one-way, so the memory that drives it must be too.
#[derive(Debug, Clone, PartialEq, Eq)]
struct RutTile {
    depth_q: [u8; TILE * TILE],
}

impl RutTile {
    fn empty() -> Self {
        Self { depth_q: [0; TILE * TILE] }
    }
}

/// The client's ledger of pressed ground.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct RutField {
    tiles: BTreeMap<(i32, i32), RutTile>,
}

impl RutField {
    /// Press one track run into ground that remembers `cap_m` at most (its `rut_depth_m`);
    /// ground that remembers nothing takes no press and allocates nothing.
    ///
    /// The cap is applied AS the ground is pressed rather than when it is read: a column
    /// deepens a rut to the softest memory under it and no further, and a later press over
    /// harder ground can never make a trough shallower than it already is.
    pub fn press(&mut self, from: [f32; 2], to: [f32; 2], cap_m: f32) {
        // NaN first, on its own: it is neither above nor below the threshold, and a press it
        // let through would poison a tile for the rest of the battle.
        if cap_m.is_nan() || cap_m < MIN_REMEMBERED_DEPTH_M {
            return;
        }
        let finite = from.iter().chain(to.iter()).all(|value| value.is_finite());
        if !finite {
            return;
        }
        let pass_m = RUT_PASS_DEPTH_M.min(cap_m);
        let cap_m = cap_m.min(RUT_RASTER_MAX_DEPTH_M);
        let low = [from[0].min(to[0]) - RUT_HALF_WIDTH_M, from[1].min(to[1]) - RUT_HALF_WIDTH_M];
        let high = [from[0].max(to[0]) + RUT_HALF_WIDTH_M, from[1].max(to[1]) + RUT_HALF_WIDTH_M];
        for iz in cell_index(low[1])..=cell_index(high[1]) {
            for ix in cell_index(low[0])..=cell_index(high[0]) {
                let distance = distance_to_run(from, to, cell_centre(ix), cell_centre(iz));
                if distance >= RUT_HALF_WIDTH_M {
                    continue;
                }
                // The trough across the track's width, exactly as the analytic ledger drew it.
                let across = distance / RUT_HALF_WIDTH_M;
                let pressed = pass_m * (1.0 - across * across);
                if pressed <= 0.0 {
                    continue;
                }
                let slot = self.slot_mut(ix, iz);
                let deepened = (dequantise(*slot) + pressed).min(cap_m);
                // Never shallower than it already was — not even by a quantisation step.
                // `touched_cells` is monotone BECAUSE of this line and the tiles that stay.
                *slot = quantise(deepened).max(*slot);
            }
        }
    }

    pub fn is_empty(&self) -> bool {
        self.tiles.is_empty()
    }

    /// Bytes the ledger holds — the instrument for the memory this costs a battle.
    pub fn footprint_bytes(&self) -> usize {
        self.tiles.len() * std::mem::size_of::<RutTile>()
    }

    /// The rut's depth at a point, read bilinearly off the raster so the trough the mesh
    /// draws is smooth rather than stepped at the raster's own cell.
    pub fn depth_at(&self, x: f32, z: f32) -> f32 {
        if self.tiles.is_empty() || !x.is_finite() || !z.is_finite() {
            return 0.0;
        }
        // Samples live at cell CENTRES, so the lattice the interpolation walks is offset by
        // half a cell from the cell grid.
        let (fx, fz) = (x / RUT_RASTER_CELL_M - 0.5, z / RUT_RASTER_CELL_M - 0.5);
        let (base_x, base_z) = (fx.floor(), fz.floor());
        let (tx, tz) = (fx - base_x, fz - base_z);
        let (ix, iz) = (base_x as i32, base_z as i32);
        let depth = |dx: i32, dz: i32| dequantise(self.depth_q_at(ix + dx, iz + dz));
        let near = depth(0, 0) + (depth(1, 0) - depth(0, 0)) * tx;
        let far = depth(0, 1) + (depth(1, 1) - depth(0, 1)) * tx;
        near + (far - near) * tz
    }

    /// The base-grid cells (of `cell_m`, indices up to `max_x`/`max_z` inclusive) any press
    /// touches — the ground bakes them at patch resolution so the trough can show.
    ///
    /// MONOTONE: the raster never forgets a press and no tile is ever dropped, so this set
    /// only grows for the life of a battle. The ground mesh's cut is one-way; a set that
    /// could shrink would leave the cells it dropped cut with no patch to stand in for them
    /// (see this module's header for the 22 000 m² that cost).
    pub fn touched_cells(
        &self,
        cell_m: f32,
        max_x: usize,
        max_z: usize,
    ) -> BTreeSet<(usize, usize)> {
        let mut cells = BTreeSet::new();
        if cell_m.is_nan() || cell_m <= 0.0 {
            return cells;
        }
        for (&(tile_x, tile_z), tile) in &self.tiles {
            for (offset, &depth_q) in tile.depth_q.iter().enumerate() {
                if depth_q == 0 {
                    continue;
                }
                let ix = tile_x * TILE as i32 + (offset % TILE) as i32;
                let iz = tile_z * TILE as i32 + (offset / TILE) as i32;
                // The cell's own extent widened by one raster cell each way: that is exactly
                // how far the bilinear read above reaches, and the patch must cover every
                // point that can read a depth.
                let span = |index: i32| {
                    ((index - 1) as f32 * RUT_RASTER_CELL_M, (index + 2) as f32 * RUT_RASTER_CELL_M)
                };
                let (x0, x1) = span(ix);
                let (z0, z1) = span(iz);
                let base =
                    |value: f32, max: usize| ((value / cell_m).floor().max(0.0) as usize).min(max);
                for z in base(z0, max_z)..=base(z1, max_z) {
                    for x in base(x0, max_x)..=base(x1, max_x) {
                        cells.insert((x, z));
                    }
                }
            }
        }
        cells
    }

    fn slot_mut(&mut self, ix: i32, iz: i32) -> &mut u8 {
        let (tile_x, offset_x) = split_index(ix);
        let (tile_z, offset_z) = split_index(iz);
        let tile = self.tiles.entry((tile_x, tile_z)).or_insert_with(RutTile::empty);
        &mut tile.depth_q[offset_z * TILE + offset_x]
    }

    fn depth_q_at(&self, ix: i32, iz: i32) -> u8 {
        let (tile_x, offset_x) = split_index(ix);
        let (tile_z, offset_z) = split_index(iz);
        self.tiles.get(&(tile_x, tile_z)).map_or(0, |tile| tile.depth_q[offset_z * TILE + offset_x])
    }
}

/// The raster cell an ordinate falls in (negative coordinates included — a map's origin is a
/// corner, not a guarantee).
fn cell_index(value: f32) -> i32 {
    (value / RUT_RASTER_CELL_M).floor() as i32
}

/// The world ordinate of a raster cell's centre.
fn cell_centre(index: i32) -> f32 {
    (index as f32 + 0.5) * RUT_RASTER_CELL_M
}

/// A raster index split into (tile, offset within the tile).
fn split_index(index: i32) -> (i32, usize) {
    (index.div_euclid(TILE as i32), index.rem_euclid(TILE as i32) as usize)
}

/// Depth to its stored byte. Rounds DOWN, never to nearest: the ground's memory (`cap_m`) is
/// a ceiling the mesh must not cross, and rounding to nearest put a capped rut 0.2 mm past
/// dirt's 9 cm — which `scene_build`'s "never past the ground's memory" lock caught.
fn quantise(depth_m: f32) -> u8 {
    let scaled = (depth_m / RUT_RASTER_MAX_DEPTH_M).clamp(0.0, 1.0) * 255.0;
    scaled.floor() as u8
}

fn dequantise(depth_q: u8) -> f32 {
    f32::from(depth_q) * RUT_DEPTH_QUANT_STEP_M
}

/// Planar distance from a point to a press's run.
fn distance_to_run(from: [f32; 2], to: [f32; 2], x: f32, z: f32) -> f32 {
    let (ax, az) = (from[0], from[1]);
    let (dx, dz) = (to[0] - ax, to[1] - az);
    let len_sq = dx * dx + dz * dz;
    let t = if len_sq <= 1.0e-9 {
        0.0
    } else {
        (((x - ax) * dx + (z - az) * dz) / len_sq).clamp(0.0, 1.0)
    };
    let (px, pz) = (ax + dx * t, az + dz * t);
    ((x - px) * (x - px) + (z - pz) * (z - pz)).sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The centre of a run reads a quarter-cell off the line at worst, so one pass over a
    /// point ON the line reads the trough profile at 0.125 m rather than the full press.
    fn one_pass_on_the_line() -> f32 {
        let across = (RUT_RASTER_CELL_M * 0.5) / RUT_HALF_WIDTH_M;
        RUT_PASS_DEPTH_M * (1.0 - across * across)
    }

    /// T8: a pass presses a little, a column presses to the ground's memory and no further,
    /// the press is a trough across the track's width and nothing beyond it, and rock takes
    /// nothing.
    #[test]
    fn passes_add_up_to_the_grounds_memory_and_no_further() {
        let mut field = RutField::default();
        field.press([10.0, 0.0], [10.0, 40.0], 0.09);
        let single = field.depth_at(10.0, 20.0);
        assert!(
            (single - one_pass_on_the_line()).abs() <= RUT_DEPTH_QUANT_STEP_M,
            "one pass reads the trough at the raster's own sample: {single}"
        );
        assert!(
            field.depth_at(10.0 + RUT_HALF_WIDTH_M * 0.5, 20.0) < single,
            "shallower off centre"
        );
        for _ in 0..4 {
            field.press([10.0, 0.0], [10.0, 40.0], 0.09);
        }
        assert!(
            (field.depth_at(10.0, 20.0) - 0.09).abs() <= RUT_DEPTH_QUANT_STEP_M,
            "five passes cap at dirt"
        );
        assert_eq!(
            field.depth_at(10.0 + RUT_HALF_WIDTH_M + RUT_RASTER_CELL_M + 0.01, 20.0),
            0.0,
            "and nothing beyond the track's width"
        );
        assert_eq!(field.depth_at(10.0, 45.0), 0.0, "nor past the run's end");

        let mut rock = RutField::default();
        rock.press([50.0, 0.0], [50.0, 40.0], 0.0);
        assert_eq!(rock.depth_at(50.0, 20.0), 0.0, "rock remembers nothing");
        assert!(rock.is_empty(), "and allocates nothing");

        let cells = field.touched_cells(5.0, 39, 39);
        assert!(cells.contains(&(1, 4)) && cells.contains(&(2, 4)), "the run's cells");
        assert!(!cells.contains(&(5, 4)));
    }

    /// The hole bug's lock (T8, 2026-09-08). The ground mesh's cut is ONE-WAY — the renderer
    /// degenerates the base triangles a rut touches and nothing ever restores them — while
    /// the patch that stands in for them is replaced wholesale and covers only the touched
    /// set. So the touched set may never lose a cell: the old 2 048-press ring did, and every
    /// cell it dropped became a permanent hole in the ground (7 068 triangles in 76 s of one
    /// measured battle). Here a column presses far past that old cap, criss-crossing, and the
    /// set is checked for shrinkage after every single press.
    #[test]
    fn the_touched_set_never_loses_a_cell_however_long_the_battle_runs() {
        let mut field = RutField::default();
        let mut seen: BTreeSet<(usize, usize)> = BTreeSet::new();
        // Well past the 2 048 presses the ring used to hold, over ground wide enough that the
        // oldest runs are nowhere near the newest.
        for pass in 0..3_000u32 {
            let z = (pass % 300) as f32 * 0.5;
            let x = 20.0 + ((pass / 300) as f32) * 4.0;
            field.press([x, z], [x, z + 1.5], 0.09);
            let cells = field.touched_cells(2.5, 399, 399);
            assert!(
                seen.is_subset(&cells),
                "press {pass}: the touched set lost {} cell(s) — every one of them is a hole",
                seen.difference(&cells).count()
            );
            seen = cells;
        }
        assert!(seen.len() > 500, "the run really did cover ground: {} cells", seen.len());
        // And the first press is still remembered at the end, which the ring could not do.
        assert!(field.depth_at(20.0, 0.75) > 0.0, "the oldest press is still in the ground");
    }

    /// The touched set follows the TRACK, not each press's bounding box. The old ledger
    /// marked every cell inside a run's AABB, so a diagonal run cut a filled rectangle out of
    /// the base and the patch drew one — the rectangles reported from the game. A long
    /// diagonal must leave the cells well off its line alone.
    #[test]
    fn a_diagonal_run_marks_its_track_and_not_the_rectangle_around_it() {
        let mut field = RutField::default();
        field.press([10.0, 10.0], [50.0, 50.0], 0.09);
        let cells = field.touched_cells(2.5, 399, 399);
        assert!(cells.contains(&(12, 12)), "the line's own cells are marked");
        // The corners of the run's bounding box are 28 m off the line; the old AABB ledger
        // marked them, and everything between.
        for corner in [(4usize, 19usize), (19, 4)] {
            assert!(
                !cells.contains(&corner),
                "the bounding box's corner {corner:?} is not the track"
            );
        }
        let boxed = (4..=20usize).count() * (4..=20usize).count();
        assert!(
            cells.len() < boxed / 2,
            "the track is a band, not a filled box: {} cells vs {boxed} boxed",
            cells.len()
        );
    }

    /// The ledger costs memory only where tracks have run, and it says how much.
    #[test]
    fn the_ledger_allocates_only_where_the_tracks_ran() {
        let mut field = RutField::default();
        assert_eq!(field.footprint_bytes(), 0, "an untouched battle costs nothing");
        field.press([100.0, 100.0], [100.0, 140.0], 0.09);
        let one_lane = field.footprint_bytes();
        assert!(one_lane > 0 && one_lane <= 8 * 1024, "a 40 m lane is a few tiles: {one_lane} B");
        field.press([900.0, 900.0], [900.0, 902.0], 0.09);
        assert!(
            field.footprint_bytes() > one_lane,
            "ground a kilometre away allocates its own tile, not the span between"
        );
    }
}
