//! The FX pass frame budget, locked as executable numbers — and, at last, compared against the
//! buffer that has to hold them.
//!
//! Every capped FX pool is filled to its cap and rendered through the REAL append paths, and the
//! summed worst-case vertex counts are locked. A Honest Steel phase that adds stamps or raises a
//! cap updates the locked number in the same diff, with the measurement that justifies it — the
//! budget moves consciously, never by drift.
//!
//! What this file used to do, and what was wrong with it. It locked ONE number, 35,070, and
//! never asked whether 35,070 vertices fit the GPU buffer they are uploaded into. They did not:
//! the buffer held 13,107. `set_fx` answered the overflow with a bare `return`, so the whole FX
//! layer — tracers, blasts, ruts, the holes in the hulls — went off the screen somewhere around
//! the fortieth ground impact of a match and stayed off, silently, while the client kept
//! building the vertices every frame and discarding them. A budget nobody compares to a capacity
//! is a number, not a budget.
//!
//! The worst case it locked was also understated twice over, both times by the fixture rather
//! than the code: the "battered" tank was battered with `DecalKind::Penetration`, which
//! deliberately emits no quads at all, so the entire fleet's decals counted as ZERO; and the
//! scar pool alternated high-explosive with armour-piercing marks, when the HE family is the
//! expensive one and a shelled field is full of them.
//!
//! So the budget is now split by what it means to lose it. ESSENTIAL is what carries the game —
//! blasts, tracers, the perforations in a hull — and the buffer is sized so it can never be
//! truncated. GROUND is the marks left in the mud, which are scenery: they are given whatever
//! the frame has left and drop their oldest to fit (`TerrainScars::append_quads_within`).
//! Between them the frame is bounded by construction rather than by hope.

use glam::Vec3;
use terrain::HeightMap;

use super::FxSystem;
use super::particle::{MAX_PARTICLES, Particle};
use super::terrain_scars::{MAX_TERRAIN_SCARS, TerrainScars};
use super::track_marks::{MAX_TRACK_MARKS, TrackMarks};

/// The battle roster the budget is sized for: 7v7.
const BATTLE_TANKS: usize = 14;

/// The locked worst case for the layers that must never be truncated: the full particle pool.
/// Since Inny Poziom Z5 no hit mark costs an FX vertex: penetrations are analytic apertures,
/// scuffs and gouges are wound records the vehicle shader shades — the fleet's 2 688 decal
/// vertices (16 marks x 2 stamps x 6 x 14 tanks) left this batch for the armor-damage buffer.
const FX_ESSENTIAL_VERTEX_BUDGET: usize = 12_288;

/// The locked worst case for the ground marks: the crater pool at its cap, every mark a fresh
/// high-explosive scorch (the expensive family), plus the rut pool at its cap.
const FX_GROUND_VERTEX_BUDGET: usize = 43_668;

/// Slack the buffer keeps for shell tracers, the one uncapped source: they are bounded by live
/// shells (4 s lifetime x fire cadence), not by a pool. Three stretched quads per shell, 18
/// vertices, and a 7v7 cannot plausibly hold more than two rounds per tank in the air at once.
const FX_TRACER_ALLOWANCE: usize = BATTLE_TANKS * 2 * 18;

fn puff(position: Vec3) -> Particle {
    Particle {
        position,
        velocity_mps: Vec3::ZERO,
        gravity_factor: 0.0,
        drag_per_s: 0.0,
        age_s: 0.0,
        ttl_s: 10.0,
        size_begin_m: 1.0,
        size_end_m: 1.0,
        color_begin: [1.0; 4],
        color_end: [0.0; 4],
        stretch_s: 0.0,
        seat: None,
    }
}

/// The particle pool at cap, as this frame's batch.
fn essential_particle_vertices() -> usize {
    let mut fx = FxSystem::default();
    for index in 0..MAX_PARTICLES {
        fx.spawn(puff(Vec3::new(index as f32 * 0.5, 1.0, 30.0)));
    }
    fx.vertices(Vec3::ZERO, Vec3::Z, &|_| None).len()
}

/// The crater pool at cap, every mark a fresh high-explosive scorch — the expensive family, and
/// the one a shelled field is actually full of.
fn saturated_scars() -> TerrainScars {
    let map = HeightMap::flat(65, 65, 5.0, 0.0).expect("flat map");
    let mut scars = TerrainScars::default();
    for index in 0..MAX_TERRAIN_SCARS {
        scars.record(
            &game_core::ShellImpact {
                owner: Some(game_core::TankId(1)),
                position: Vec3::new(5.0 + index as f32 * 2.0, 0.0, 40.0),
                surface: game_core::ImpactSurface::Terrain,
                shell_type: game_core::ShellType::HighExplosive,
                direction: Vec3::new(0.0, -0.4, 0.9),
                caliber_mm: 122.0,
                ..Default::default()
            },
            &map,
        );
    }
    scars
}

/// One rut through the REAL emitter, so the per-mark cost is measured rather than assumed. The
/// pool's contribution is that times its cap: ruts are uniform, unlike the scorch marks.
fn rut_vertices() -> usize {
    let map = HeightMap::flat(65, 65, 5.0, 0.0).expect("flat map");
    let mut marks = TrackMarks::default();
    marks.record_segment(Vec3::new(10.0, 0.0, 40.0), Vec3::new(12.0, 0.0, 40.0), 0.5, 0.25, &map);
    let mut batch = Vec::new();
    marks.append_quads_within(&mut batch, usize::MAX);
    batch.len()
}

#[test]
fn the_fx_frame_worst_case_is_locked() {
    let particles = essential_particle_vertices();
    // The fleet's scuffs and gouges are wound RECORDS the vehicle shader reads (Z5), not FX
    // quads: their cost left this batch for the armor-damage buffer.
    let essential = particles;
    assert_eq!(
        essential, FX_ESSENTIAL_VERTEX_BUDGET,
        "the ESSENTIAL FX worst case moved (particles {particles}); if \
         the change is intentional, update FX_ESSENTIAL_VERTEX_BUDGET in the same diff, with the \
         measurement that justifies the new number",
    );

    let mut scar_batch = Vec::new();
    saturated_scars().append_quads(&mut scar_batch);
    let per_rut = rut_vertices();
    assert!(per_rut > 0, "a rut that emits nothing would make this lock meaningless");
    let ruts = per_rut * MAX_TRACK_MARKS;
    let ground = scar_batch.len() + ruts;
    assert_eq!(
        ground,
        FX_GROUND_VERTEX_BUDGET,
        "the GROUND FX worst case moved (scorch marks {} + ruts {ruts})",
        scar_batch.len(),
    );
}

/// The assertion this budget never made, and the whole reason the FX layer used to vanish
/// mid-battle: what the frame builds has to FIT THE BUFFER THAT HOLDS IT.
///
/// Only the essential half is asserted to fit, because only the essential half is undroppable.
/// The ground marks are explicitly allowed to exceed what is left — that is what
/// `append_quads_within` is for — but the blasts, tracers and perforations must always have
/// room, in the worst frame the pools can produce.
#[test]
fn the_essential_fx_layers_always_fit_the_gpu_buffer() {
    let needed = FX_ESSENTIAL_VERTEX_BUDGET + FX_TRACER_ALLOWANCE;
    assert!(
        needed <= renderer_wgpu::fx_vertex_budget(),
        "a saturated battle submits {needed} undroppable FX vertices ({FX_ESSENTIAL_VERTEX_BUDGET} \
         pooled + {FX_TRACER_ALLOWANCE} of tracers) but the FX buffer holds only {}; grow \
         FX_VERTEX_CAPACITY before shipping this, because the alternative is `set_fx` truncating \
         a tracer off the screen",
        renderer_wgpu::fx_vertex_budget(),
    );
}

/// And the other half of the contract: the ground marks give way, so the frame as a whole stays
/// inside the buffer even when the pools want far more than fits. The worst case genuinely does
/// want more — that is not a hypothetical — so this is the mechanism that keeps `set_fx` from
/// ever having to truncate anything.
#[test]
fn the_ground_marks_give_way_so_the_frame_fits() {
    let budget = renderer_wgpu::fx_vertex_budget();
    assert!(
        FX_ESSENTIAL_VERTEX_BUDGET + FX_GROUND_VERTEX_BUDGET > budget,
        "the pools now fit the buffer whole; this test is guarding a squeeze that no longer \
         happens, so either the caps shrank or the buffer grew and this lock should be revisited",
    );

    // The frame the client builds: essentials take what they need, the ground takes the rest.
    // Saturating, exactly as `fx_frame_vertices_into` does — if the essentials ever outgrew the
    // buffer the test above is the one that should say so, not an underflow here.
    let ground_budget = budget.saturating_sub(FX_ESSENTIAL_VERTEX_BUDGET + FX_TRACER_ALLOWANCE);
    let mut frame = Vec::new();
    let dropped = saturated_scars().append_quads_within(&mut frame, ground_budget);
    assert!(dropped > 0, "a saturated crater pool must have had to drop marks to fit");
    assert!(
        frame.len() <= ground_budget,
        "the ground marks spent {} of a {ground_budget} vertex allowance",
        frame.len(),
    );
    assert!(
        !frame.is_empty(),
        "the squeeze took EVERY scorch mark; the frame should thin the ground, not clear it",
    );
    assert!(
        frame.len() + FX_ESSENTIAL_VERTEX_BUDGET + FX_TRACER_ALLOWANCE <= budget,
        "the budgeted frame still overruns the buffer",
    );
}
