//! The garage hangar: a single static interior scene the owned tank is parked in, replacing the
//! battlefield while the garage is open. Modelled as a working repair shop — a cool concrete floor,
//! ribbed metal walls with near-black upper reaches, roof trusses under REAL skylight openings, a
//! cold-daylight doorway in back, a turntable spot, and workshop props (`hangar_props`: crane,
//! wheel/track stacks, workbench, barrels, oil stains). The room is built from *solid slabs*
//! surrounding the play volume: each slab's inner surface is an ordinary outward-facing face, so
//! back-face culling keeps exactly the walls seen from inside. The hero vehicle throws a real
//! contact shadow on the turntable from the workshop sun key — no faked shadow disc.

use glam::{Mat3, Vec3};
use renderer_api::SceneVertex;

use crate::tank_mesh::push_oriented_box;

/// Interior half-extent (x/z) of the hall. Sized so the full camera boom range fits inside at every
/// angle without clipping the walls — see `garage/camera.rs`. `pub(super)` so the props hug the walls.
/// A working MAINTENANCE HALL sized for the roomy 17 m pull-back; the volume is earned by the
/// gallery band, crane rails, lamp rig and occupied bays (`hangar_gallery`, `hangar_props`).
pub(super) const HALF: f32 = 18.0;
pub(super) const WALL_HEIGHT: f32 = 12.6;
pub(super) const SLAB: f32 = 0.15;
/// Height where the gunmetal lower wall meets the shadowed upper wall. The two bands **abut** at
/// this seam — they must never overlap, or their coplanar inner faces z-fight (a moiré band).
pub(super) const WALL_SEAM: f32 = 5.6;
/// Top surface of the turntable the tank rests on, metres above the floor.
pub const TURNTABLE_TOP_M: f32 = 0.12;
const TURNTABLE_RADIUS_M: f32 = 5.2;

// Workshop palette: warm cast concrete and painted gunmetal panels. The upper band and roof fall
// into SHADOW, not void — a step darker than the lower walls, never near-black.
const CONCRETE: [f32; 3] = [0.30, 0.29, 0.28];
const METAL: [f32; 3] = [0.24, 0.245, 0.255];
const UPPER_WALL: [f32; 3] = [0.155, 0.16, 0.175];
const RIB: [f32; 3] = [0.28, 0.29, 0.31];
const ROOF: [f32; 3] = [0.125, 0.13, 0.14];
const TRUSS: [f32; 3] = [0.17, 0.17, 0.18];
const TURNTABLE: [f32; 3] = [0.34, 0.34, 0.35];
const MARKING: [f32; 3] = [0.62, 0.55, 0.20];
// Wall dressing: panel joints recessed a shade darker, a girt rail riding the band seam.
const PANEL_JOINT: [f32; 3] = [0.19, 0.195, 0.205];
const GIRT: [f32; 3] = [0.27, 0.275, 0.29];
// The bay gate: framed, closed, segmented — cool sheet steel, never a glowing plate.
const GATE_FRAME: [f32; 3] = [0.145, 0.15, 0.16];
const GATE_SLAT: [f32; 3] = [0.215, 0.22, 0.235];
const GATE_SLAT_ALT: [f32; 3] = [0.235, 0.24, 0.255];
// The glowing "frosted panes" are GONE (Hala 2.0 T1 correction, user verdict 2026-08-05): a
// flat HDR slab is not a window, and its bloom halo read as fog. Real windows are framed
// openings with glass and the day behind them — T4 scope; until then the wall over the gate
// is honestly a wall, and daylight enters through the skylights only.
// Floor dressing: expansion joints, the worn drive lane in from the gate, and its track wear.
const FLOOR_JOINT: [f32; 3] = [0.235, 0.23, 0.225];
const DRIVE_LANE: [f32; 3] = [0.272, 0.265, 0.256];
const TRACK_WEAR: [f32; 3] = [0.24, 0.234, 0.226];
// Turntable dressing: the pit rim it sits in, its radial plate seams, the centre hub.
const TURNTABLE_RIM: [f32; 3] = [0.225, 0.23, 0.24];
const TURNTABLE_SEAM: [f32; 3] = [0.27, 0.27, 0.28];
const TURNTABLE_HUB: [f32; 3] = [0.305, 0.305, 0.315];

/// Pivot the garage orbit camera looks at: roughly the centre of a parked tank.
pub fn hangar_camera_pivot() -> Vec3 {
    Vec3::new(0.0, TURNTABLE_TOP_M + 1.3, 0.0)
}

/// The garage's rest framing — the numbers the hangar opens with, and the ONE place they live.
/// The live orbit camera, the review golden and the human-review example all read these, so a
/// reframing moves the played picture and the locked picture together.
pub const HERO_ORBIT_YAW: f32 = 0.60;
/// Lowered from 0.28. At 0.28 rad the camera tilted 16 deg down through a 32 deg lens, which put
/// the TOP of the frame exactly on the horizon through the pivot: everything above the eye — the
/// roof, the trusses, the skylight strips, the frosted panes over the bay gate, both high-bay
/// lamps — fell outside the shot. That is the arithmetic behind D20's "0.00% of the hero frame
/// sits above the bright threshold": the room's light sources are all real and all emissive, and
/// the lens was pointed under every one of them. At 0.13 the frame reaches roughly 9 deg above
/// the eye and the daylight over the gate comes into shot.
pub const HERO_ORBIT_PITCH: f32 = 0.13;
pub const HERO_ORBIT_DISTANCE: f32 = 14.0;
/// The heading the hero is PARKED at, which only means anything against [`HERO_ORBIT_YAW`]: the
/// camera orbits to a bearing, the tank sits at a heading, and the angle between them is the shot.
///
/// It used to equal `HERO_ORBIT_YAW` exactly. A tank whose heading matches the camera's bearing
/// faces the lens dead-on, so the hero shot was a head-on elevation with the gun barrel bisecting
/// the hull and hiding the glacis behind it — the one angle at which a T-54 and a Centurion are
/// hardest to tell apart. Offset by ~0.65 rad it reads as the three-quarter the comment beside it
/// always claimed: front, flank and the length of the running gear in one silhouette.
pub const HERO_PARK_YAW: f32 = HERO_ORBIT_YAW + 0.65;
/// A long lens: the hangar is a studio, and a studio does not read at a battle FOV.
pub const HERO_FOV_DEGREES: f32 = 32.0;

/// What shows THROUGH the roof's skylight openings: the day outside. The renderer paints it as
/// the interior background (the hangar runs no sky dome), so it is the colour of the sky in
/// every garage frame.
///
/// It lives here because four callers need the same answer — the live client
/// (`garage_render::ensure_scene`), the review view set (`review_views::hangar_review_views`)
/// and both hangar probes. It used to be four copies of a literal, and the moment the roof
/// gained real openings the game moved to daylight while the goldens and the probes kept the
/// old near-black: the locked picture stopped being the played picture, which is the one thing
/// a review artifact may never do.
pub const INTERIOR_BACKGROUND: (f64, f64, f64) = (1.30, 1.38, 1.55);

/// Direction from the pivot to the eye for an orbit yaw/pitch. Shared so the live camera and
/// every offscreen review of it cannot disagree about where the camera is.
pub fn orbit_direction(yaw: f32, pitch: f32) -> Vec3 {
    Vec3::new(pitch.cos() * yaw.sin(), pitch.sin(), pitch.cos() * yaw.cos())
}

/// The eye the garage rests at: the hero framing applied to the turntable pivot.
pub fn hero_orbit_eye() -> Vec3 {
    hangar_camera_pivot() + orbit_direction(HERO_ORBIT_YAW, HERO_ORBIT_PITCH) * HERO_ORBIT_DISTANCE
}

/// The world point the garage pins the sun-shadow boxes to: the turntable the hero stands on.
/// The orbit camera's "forward" sweeps a full circle, so the battle path's forward-offset
/// shadow-focus heuristic would walk the boxes off the subject.
pub fn hangar_shadow_focus() -> [f32; 3] {
    [0.0, TURNTABLE_TOP_M, 0.0]
}

/// The near shadow box the garage asks for, as a half-size in metres.
///
/// Pinning the box's CENTRE to the turntable was only half the job: its SIZE stayed the
/// battlefield's 64 m half-box, so a 36 m room sat inside a 128 m box and the hall received 576
/// of 2048 texels — 7.9% of the map, the other 92% spent on empty ground outside the walls. At a
/// 14 m hero boom one 6.25 cm texel covered 8.4 screen pixels, which is what a staircased
/// skylight shaft on the floor actually is.
///
/// 30 m is the smallest box that still contains the whole hall under every garage rig (the
/// steepest, `garage_workshop`, needs 28.3 m — see `the_near_shadow_box_contains_the_whole_hall`)
/// plus the cascade's containment margin. Tighter would sharpen the turntable further but push
/// the far corners of the room out to the coarse far cascade, trading a staircase for a seam.
pub fn hangar_shadow_radius_m() -> f32 {
    30.0
}

/// The garage's bloom depth (Hala 2.0 T1): the quality table's "integrated budget" 3-mip
/// chain, enough for pane and lamp glow. Scene data like the shadow radius above — the live
/// garage and every offscreen review of it must read the same number, or the locked picture
/// stops being the played picture.
pub fn hangar_bloom_mips() -> u32 {
    3
}

/// Interior of the hangar shell as `(half_extent_xz, height)`. Used by the CLIENT camera
/// invariant test to prove the whole orbit range stays inside the room — cross-crate now, so
/// it cannot hide behind cfg(test).
pub fn hangar_interior() -> (f32, f32) {
    (HALF, WALL_HEIGHT)
}

/// The roof's skylight bands as `(x centre, x half-width)` — REAL openings cut through the
/// roof slab, not bright plates under it. The first is the SUN band, placed by physics: the
/// `garage_hero` key direction leaves the turntable centre through the roof plane at
/// x ≈ −7.2, z ≈ +6.7, so the band straddles that exit lane and the key genuinely falls on
/// the hero — a real beam, a real contact shadow on the deck (the whole point of D20's fix;
/// locked by `the_workshop_sun_reaches_the_turntable_through_a_real_opening`). The other two
/// bands rhythm the roof toward the second bay, so the openings read as architecture, not as
/// one hole drilled for a light rig.
const SKYLIGHT_BANDS: [(f32, f32); 3] = [(-6.85, 3.2), (1.8, 1.6), (10.8, 1.6)];
/// The bands' half-length along z; the roof stays solid in a border along both end walls.
const SKYLIGHT_HALF_Z: f32 = 13.0;

/// The roof: solid strips between the skylight bands, end caps closing each band short of the
/// walls, and thin glazing mullions across the openings (they cast the honest striped shadows
/// a glazed roof throws). Through the openings the renderer's interior background shows — set
/// to daylight sky by the client, so an opening reads as sky, not void.
fn push_skylight_roof(v: &mut Vec<SceneVertex>, i: &mut Vec<u32>) {
    let roof_y = WALL_HEIGHT + SLAB;
    // Solid strips spanning the full hall depth, between and outside the bands.
    let mut edges = vec![-HALF];
    for (cx, hx) in SKYLIGHT_BANDS {
        edges.push(cx - hx);
        edges.push(cx + hx);
    }
    edges.push(HALF);
    for pair in edges.chunks(2) {
        let (lo, hi) = (pair[0], pair[1]);
        slab(v, i, [(lo + hi) / 2.0, roof_y, 0.0], [(hi - lo) / 2.0, SLAB, HALF], ROOF);
    }
    // End caps and mullions per band.
    let cap_c = (SKYLIGHT_HALF_Z + HALF) / 2.0;
    let cap_h = (HALF - SKYLIGHT_HALF_Z) / 2.0;
    for (cx, hx) in SKYLIGHT_BANDS {
        for sign in [-1.0_f32, 1.0] {
            slab(v, i, [cx, roof_y, sign * cap_c], [hx, SLAB, cap_h], ROOF);
        }
        // Mullion phase is offset by half a pitch: the hero-centre sun ray crosses the roof
        // plane near z ≈ 6.4, and a bar there would put the one guaranteed-clear ray of the
        // sun lock behind glazing framing forever.
        for k in -4i32..=3 {
            slab(v, i, [cx, WALL_HEIGHT + 0.05, k as f32 * 3.25 + 1.625], [hx, 0.08, 0.11], TRUSS);
        }
    }
}

/// The hall is static and its GI bake is honest work (a hemisphere of rays per vertex), so the
/// whole build runs once per process and callers take a copy of the cached result.
static MESH: std::sync::OnceLock<(Vec<SceneVertex>, Vec<u32>)> = std::sync::OnceLock::new();
/// Guards [`prewarm`] so the worker is spawned once however many times it is asked for.
static PREWARM: std::sync::Once = std::sync::Once::new();

/// Build the static hangar mesh. The tank is parked at the origin on top of the turntable
/// (`TURNTABLE_TOP_M`), so place the parked vehicle's `position.y` at that height.
///
/// **This blocks until the hall exists**, which is why [`prewarm`] is called at startup: the
/// build is seconds of work, and the first caller used to be `ensure_scene`, inside the frame
/// that opens the garage.
pub fn hangar_scene_mesh() -> (Vec<SceneVertex>, Vec<u32>) {
    MESH.get_or_init(build_hangar_scene_mesh).clone()
}

/// Start the hall's build on a worker and return immediately. Idempotent, and safe to call
/// before anything wants the mesh: a caller that arrives while the worker is still going simply
/// waits for it in `get_or_init` instead of building a second copy.
///
/// It exists because the build is not cheap and the comment that said it was cost the player a
/// frozen frame. Measured with the gather serial and allocating per ray: **1 902 / 1 919 /
/// 1 940 ms in release**, run synchronously inside `ensure_scene` on the first garage entry —
/// an order of magnitude past the battlefield stall that earned the scene cache its own
/// app-lifetime bake. The gather runs on worker lanes now and the ray scratch is hoisted out
/// of the traversal, which takes it to **531 ms** in release; this takes the rest out of the
/// frame entirely, the way `poll_map_prebake` already takes the map bake out of the Battle
/// press.
pub fn prewarm() {
    PREWARM.call_once(|| {
        // A failed spawn is not an error worth propagating: the lazy path in
        // `hangar_scene_mesh` still builds the hall, exactly as it did before this existed.
        let _ = std::thread::Builder::new()
            .name("hangar-bake".to_string())
            .spawn(|| MESH.get_or_init(build_hangar_scene_mesh));
    });
}

/// Whether the hall is already in the cache — the prewarm's observable, so a caller can tell
/// "the worker finished" from "the worker is still going" without blocking on the answer.
pub fn is_baked() -> bool {
    MESH.get().is_some()
}

/// The uncached build. `pub(crate)` so the bake's own tests can run it twice: through
/// [`hangar_scene_mesh`] they cannot, because the `OnceLock` hands both calls the same value —
/// which is exactly how `the_bake_is_deterministic` came to compare a clone with itself.
pub(crate) fn build_hangar_scene_mesh() -> (Vec<SceneVertex>, Vec<u32>) {
    let mut v = Vec::new();
    let mut i = Vec::new();
    let h = WALL_HEIGHT / 2.0;

    // Shell: floor, ceiling, four walls. The lower walls are gunmetal; a near-black upper band
    // above the doorway line lets the roof fall into shadow so the lit bay reads as the subject.
    // The lower/upper bands abut exactly at `WALL_SEAM` and the upper band runs to the ceiling, so
    // no two wall faces are ever coplanar (which would z-fight) and there is no gap to the roof.
    let lower_c = WALL_SEAM / 2.0;
    let lower_h = WALL_SEAM / 2.0;
    let upper_c = (WALL_SEAM + WALL_HEIGHT) / 2.0;
    let upper_h = (WALL_HEIGHT - WALL_SEAM) / 2.0;
    // Sealed concrete carries a faint sheen that catches the worklight pools; the painted
    // lower panels a satin step less; the shadowed upper band and roof stay matte.
    slab_finished(&mut v, &mut i, [0.0, -SLAB, 0.0], [HALF, SLAB, HALF], CONCRETE, 0.08);
    push_skylight_roof(&mut v, &mut i);
    for cz in [-HALF, HALF] {
        slab_finished(&mut v, &mut i, [0.0, lower_c, cz], [HALF, lower_h, SLAB], METAL, 0.12);
        slab(&mut v, &mut i, [0.0, upper_c, cz], [HALF, upper_h, SLAB], UPPER_WALL);
    }
    for cx in [-HALF, HALF] {
        slab_finished(&mut v, &mut i, [cx, lower_c, 0.0], [SLAB, lower_h, HALF], METAL, 0.12);
        slab(&mut v, &mut i, [cx, upper_c, 0.0], [SLAB, upper_h, HALF], UPPER_WALL);
    }
    // Everything past the bare shell is "furniture" and takes the corner-shade bake at the end.
    let furniture_start = v.len();

    // Panel joints: recessed vertical lines rhythm the lower band into sheet-steel panels, and a
    // girt rail rides the band seam — a wall of PANELS, not one smeared plate.
    let joint_h = WALL_SEAM / 2.0 - 0.05;
    for step in -6i32..=6 {
        let along = step as f32 * 2.8;
        for sign in [-1.0_f32, 1.0] {
            let wall = sign * (HALF - SLAB - 0.03);
            slab(&mut v, &mut i, [wall, joint_h, along], [0.035, joint_h, 0.05], PANEL_JOINT);
            slab(&mut v, &mut i, [along, joint_h, wall], [0.05, joint_h, 0.035], PANEL_JOINT);
        }
    }
    for (cx, cz, hx, hz) in [
        (0.0, -(HALF - SLAB - 0.05), HALF - 0.3, 0.05_f32),
        (0.0, HALF - SLAB - 0.05, HALF - 0.3, 0.05),
        (-(HALF - SLAB - 0.05), 0.0, 0.05, HALF - 0.3),
        (HALF - SLAB - 0.05, 0.0, 0.05, HALF - 0.3),
    ] {
        slab_finished(&mut v, &mut i, [cx, WALL_SEAM, cz], [hx, 0.07, hz], GIRT, 0.3);
    }

    // Vertical wall ribs (pilasters) proud of the side and back walls, spaced across the bay as a
    // fraction of the hall so they stay evenly distributed at any hall size.
    for k in [-0.8_f32, -0.4, 0.0, 0.4, 0.8] {
        let z = k * HALF;
        slab(&mut v, &mut i, [-(HALF - 0.2), h, z], [0.12, h - 0.4, 0.35], RIB);
        slab(&mut v, &mut i, [HALF - 0.2, h, z], [0.12, h - 0.4, 0.35], RIB);
    }
    for k in [-0.8_f32, 0.8] {
        slab(&mut v, &mut i, [k * HALF, h, -(HALF - 0.2)], [0.35, h - 0.4, 0.12], RIB);
    }

    // Roof trusses spanning the bay, backlit by the REAL skylight bands cut through the roof
    // above them, so the trusses read as dark bars against daylight.
    for k in [-0.8_f32, -0.4, 0.0, 0.4, 0.8] {
        slab(&mut v, &mut i, [0.0, WALL_HEIGHT - 0.3, k * HALF], [HALF - 0.5, 0.12, 0.18], TRUSS);
    }

    // The bay gate the tank rolled in through: a framed, closed, segmented steel door on the back
    // wall — jambs, lintel, alternating slats. The old glowing doorway plate read as a broken
    // texture; a real gate explains the drive-in and grounds the back of the frame.
    push_bay_gate(&mut v, &mut i);

    // Floor: expansion joints score the slab into cast bays; the drive lane in from the gate is
    // worn a step darker with two track-polished strips — the floor tells the room's story.
    for step in -5i32..=5 {
        let along = step as f32 * 3.4;
        slab(&mut v, &mut i, [along, 0.003, 0.0], [0.03, 0.003, HALF - 0.4], FLOOR_JOINT);
        slab(&mut v, &mut i, [0.0, 0.003, along], [HALF - 0.4, 0.003, 0.03], FLOOR_JOINT);
    }
    {
        let lane_center = -(HALF + TURNTABLE_RADIUS_M) / 2.0;
        let lane_half = (HALF - TURNTABLE_RADIUS_M) / 2.0;
        slab_finished(
            &mut v,
            &mut i,
            [0.0, 0.0015, lane_center],
            [2.6, 0.0015, lane_half],
            DRIVE_LANE,
            0.05,
        );
        for x in [-1.35_f32, 1.35] {
            slab_finished(
                &mut v,
                &mut i,
                [x, 0.0025, lane_center],
                [0.55, 0.0015, lane_half],
                TRACK_WEAR,
                0.05,
            );
        }
    }

    // Parking-bay markings flanking the turntable, flush with the floor.
    for x in [-6.8_f32, 6.8] {
        slab(&mut v, &mut i, [x, 0.004, 0.0], [0.14, 0.005, HALF - 2.0], MARKING);
    }

    // Turntable: a rim ring recessed under the deck, the deck itself, radial plate seams and a
    // centre hub — machinery, not a sticker. (No faked shadow disc — the hero vehicle casts a
    // real contact shadow here.)
    let rim_start = v.len();
    push_cylinder(
        &mut v,
        &mut i,
        Vec3::ZERO,
        TURNTABLE_RADIUS_M + 0.35,
        TURNTABLE_TOP_M - 0.02,
        48,
        TURNTABLE_RIM,
    );
    for vertex in &mut v[rim_start..] {
        vertex.gloss = 0.2;
    }
    let deck_start = v.len();
    push_cylinder(&mut v, &mut i, Vec3::ZERO, TURNTABLE_RADIUS_M, TURNTABLE_TOP_M, 48, TURNTABLE);
    // Machined deck steel: the glossiest ground plane in the hall, so the worklight pools and
    // the hero's contact shadow both read on it.
    for vertex in &mut v[deck_start..] {
        vertex.gloss = 0.35;
    }
    for seam in 0..4 {
        let angle = seam as f32 * std::f32::consts::FRAC_PI_4;
        rotated_slab(
            &mut v,
            &mut i,
            [0.0, TURNTABLE_TOP_M + 0.001, 0.0],
            [TURNTABLE_RADIUS_M - 0.15, 0.001, 0.02],
            angle,
            TURNTABLE_SEAM,
        );
    }
    let hub_start = v.len();
    push_cylinder(&mut v, &mut i, Vec3::ZERO, 0.9, TURNTABLE_TOP_M + 0.004, 24, TURNTABLE_HUB);
    for vertex in &mut v[hub_start..] {
        vertex.gloss = 0.3;
    }

    // The floor drain beside the drive lane: a recessed near-black strip under a steel grating.
    slab(&mut v, &mut i, [3.4, 0.001, -11.0], [0.10, 0.002, 2.2], [0.10, 0.10, 0.11]);
    for step in 0..9 {
        let z = -13.0 + step as f32 * 0.5;
        slab_finished(
            &mut v,
            &mut i,
            [3.4, 0.004, z],
            [0.11, 0.003, 0.03],
            [0.16, 0.17, 0.18],
            0.3,
        );
    }

    super::hangar_gallery::push_gallery(&mut v, &mut i);
    super::hangar_props::push_props(&mut v, &mut i);

    bake_corner_shade(&mut v[furniture_start..]);

    // Hala 2.0 T1: subdivide for bake resolution and gather one bounce of light into the
    // bounce lane (see hangar_bake.rs). After the corner shade, so the bake reads final
    // albedos; cached by `hangar_scene_mesh` because the gather is real work.
    super::hangar_bake::bake_bounce_lane(&mut v, &mut i);

    (v, i)
}

/// Analytic, view-independent corner shade baked into the vertex colours of the hall's
/// FURNITURE (everything after the bare shell): a piece darkens as it approaches the shell's
/// concave junctions and its own floor contact — the cheap standing ambient occlusion a
/// one-room interior earns without any runtime cost. The shell slabs themselves are excluded:
/// a single box has vertices only at its corners, so vertex-baked shade would interpolate to
/// one flat darkening instead of a gradient (the shell's corners belong to SSAO). The plane a
/// vertex FACES ALONG is excluded via its normal (a deck top is not occluded by the floor).
/// Emissive vertices (any channel past 1.0 — skylights, lamp faces) are left untouched, and
/// the bake is bounded: no vertex drops below 0.8x its authored colour.
fn bake_corner_shade(vertices: &mut [SceneVertex]) {
    let ease = |distance: f32, reach: f32| (distance / reach).clamp(0.0, 1.0);
    for vertex in vertices {
        if vertex.color.iter().any(|&c| c > 1.0) {
            continue;
        }
        let [x, y, z] = vertex.position;
        let [nx, ny, nz] = vertex.normal;
        let d_wall_x = ease(HALF - x.abs(), 1.4);
        let d_wall_z = ease(HALF - z.abs(), 1.4);
        let d_floor = ease(y, 1.2);
        let d_ceiling = ease(WALL_HEIGHT - y, 1.6);
        let open = if ny.abs() > 0.7 {
            // Floor/ceiling: occluded by the walls it runs into.
            d_wall_x.min(d_wall_z)
        } else if nx.abs() > 0.7 {
            // An x-facing wall: occluded by floor, ceiling and the crossing walls.
            d_wall_z.min(d_floor).min(d_ceiling)
        } else if nz.abs() > 0.7 {
            d_wall_x.min(d_floor).min(d_ceiling)
        } else {
            // Props and slopes: grounded by their floor contact and any wall they hug.
            d_floor.min(d_wall_x).min(d_wall_z)
        };
        let t = open * open * (3.0 - 2.0 * open);
        let shade = 0.80 + 0.20 * t;
        for channel in &mut vertex.color {
            *channel *= shade;
        }
    }
}

/// The closed segmented bay gate on the back wall: jambs and a lintel framing six alternating
/// sheet-steel slats, everything proud of the wall plane so nothing z-fights.
fn push_bay_gate(v: &mut Vec<SceneVertex>, i: &mut Vec<u32>) {
    let wall_z = -(HALF - SLAB);
    let gate_half_w = 4.6;
    let gate_top = 5.0;
    // Jambs + lintel.
    for x in [-(gate_half_w + 0.25), gate_half_w + 0.25] {
        slab(
            v,
            i,
            [x, gate_top / 2.0 + 0.2, wall_z + 0.10],
            [0.22, gate_top / 2.0 + 0.2, 0.10],
            GATE_FRAME,
        );
    }
    slab(v, i, [0.0, gate_top + 0.28, wall_z + 0.10], [gate_half_w + 0.47, 0.16, 0.10], GATE_FRAME);
    // Six horizontal slats, alternating tone so the segments read from across the bay; sheet
    // steel takes a low satin finish.
    let slat_h = gate_top / 6.0;
    for slat in 0..6 {
        let color = if slat % 2 == 0 { GATE_SLAT } else { GATE_SLAT_ALT };
        slab_finished(
            v,
            i,
            [0.0, slat_h * (slat as f32 + 0.5), wall_z + 0.06],
            [gate_half_w, slat_h / 2.0 - 0.015, 0.05],
            color,
            0.2,
        );
    }
}

/// [`slab`] rotated around Y — for the turntable's radial plate seams.
fn rotated_slab(
    vertices: &mut Vec<SceneVertex>,
    indices: &mut Vec<u32>,
    center: [f32; 3],
    half: [f32; 3],
    yaw_rad: f32,
    color: [f32; 3],
) {
    push_oriented_box(
        vertices,
        indices,
        Vec3::from_array(center),
        Vec3::from_array(half),
        Mat3::from_rotation_y(yaw_rad),
        color,
    );
}

/// [`slab`] plus a material finish stamped onto the vertices it appended — the hangar's route
/// into the `SceneVertex.gloss` lane (satin rails, sealed concrete, glazed lamp shades).
pub(super) fn slab_finished(
    vertices: &mut Vec<SceneVertex>,
    indices: &mut Vec<u32>,
    center: [f32; 3],
    half: [f32; 3],
    color: [f32; 3],
    gloss: f32,
) {
    let start = vertices.len();
    slab(vertices, indices, center, half, color);
    for vertex in &mut vertices[start..] {
        vertex.gloss = gloss;
    }
}

/// An axis-aligned solid box (every face winds CCW outward for back-face culling). Shared with
/// `hangar_props`.
pub(super) fn slab(
    vertices: &mut Vec<SceneVertex>,
    indices: &mut Vec<u32>,
    center: [f32; 3],
    half: [f32; 3],
    color: [f32; 3],
) {
    push_oriented_box(
        vertices,
        indices,
        Vec3::from_array(center),
        Vec3::from_array(half),
        Mat3::IDENTITY,
        color,
    );
}

/// A low cylinder resting on the floor: a top cap (normal +Y) plus an outward-facing side ring.
/// The bottom is omitted — it sits on the floor slab and is never seen. Shared with `hangar_props`.
pub(super) fn push_cylinder(
    vertices: &mut Vec<SceneVertex>,
    indices: &mut Vec<u32>,
    base_center: Vec3,
    radius: f32,
    height: f32,
    segments: u32,
    color: [f32; 3],
) {
    let top_y = base_center.y + height;
    let up = [0.0, 1.0, 0.0];

    let center_index = vertices.len() as u32;
    vertices.push(SceneVertex::new([base_center.x, top_y, base_center.z], up, color));
    let rim_start = vertices.len() as u32;
    for s in 0..segments {
        let theta = s as f32 / segments as f32 * std::f32::consts::TAU;
        let (sin, cos) = theta.sin_cos();
        vertices.push(SceneVertex::new(
            [base_center.x + radius * cos, top_y, base_center.z + radius * sin],
            up,
            color,
        ));
    }
    for s in 0..segments {
        let a = rim_start + s;
        let b = rim_start + (s + 1) % segments;
        indices.extend_from_slice(&[center_index, b, a]);
    }

    for s in 0..segments {
        let t0 = s as f32 / segments as f32 * std::f32::consts::TAU;
        let t1 = (s + 1) as f32 / segments as f32 * std::f32::consts::TAU;
        let (s0, c0) = t0.sin_cos();
        let (s1, c1) = t1.sin_cos();
        let p_top_0 = [base_center.x + radius * c0, top_y, base_center.z + radius * s0];
        let p_top_1 = [base_center.x + radius * c1, top_y, base_center.z + radius * s1];
        let p_bot_0 = [base_center.x + radius * c0, base_center.y, base_center.z + radius * s0];
        let p_bot_1 = [base_center.x + radius * c1, base_center.y, base_center.z + radius * s1];
        let n0 = [c0, 0.0, s0];
        let n1 = [c1, 0.0, s1];
        let base = vertices.len() as u32;
        vertices.push(SceneVertex::new(p_bot_0, n0, color));
        vertices.push(SceneVertex::new(p_bot_1, n1, color));
        vertices.push(SceneVertex::new(p_top_1, n1, color));
        vertices.push(SceneVertex::new(p_top_0, n0, color));
        indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Whether `color` is `base` scaled by one uniform corner-shade factor in `[0.8, 1.0]` —
    /// the bake multiplies all channels equally, so hue identity survives it.
    fn is_shade_of(color: [f32; 3], base: [f32; 3]) -> bool {
        let k = color[0] / base[0];
        (0.79..=1.001).contains(&k) && (0..3).all(|i| (color[i] - base[i] * k).abs() < 1.0e-4)
    }

    #[test]
    fn hangar_mesh_is_nonempty_and_indices_are_in_range() {
        let (vertices, indices) = hangar_scene_mesh();
        assert!(!vertices.is_empty() && !indices.is_empty());
        assert_eq!(indices.len() % 3, 0, "triangle list");
        assert!(indices.iter().all(|&i| (i as usize) < vertices.len()));
    }

    #[test]
    fn hangar_encloses_the_parked_tank() {
        let (vertices, _) = hangar_scene_mesh();
        let pivot = hangar_camera_pivot();
        let any = |pred: fn(&[f32; 3]) -> bool| vertices.iter().any(|v| pred(&v.position));
        assert!(any(|p| p[0] < -HALF + 1.0), "left wall");
        assert!(any(|p| p[0] > HALF - 1.0), "right wall");
        assert!(any(|p| p[2] < -HALF + 1.0), "back wall");
        assert!(any(|p| p[2] > HALF - 1.0), "front wall");
        assert!(any(|p| p[1] <= 0.0), "floor at or below the tank");
        assert!(any(|p| p[1] >= WALL_HEIGHT - 0.5), "ceiling above the tank");
        assert!(pivot.y > 0.0 && pivot.y < WALL_HEIGHT, "camera pivot sits inside the room");
    }

    #[test]
    fn no_faked_shadow_disc_remains() {
        let (vertices, _) = hangar_scene_mesh();
        // The old near-black shadow disc sat just above the turntable; nothing that dark may
        // hover there — the vehicle casts a REAL contact shadow from the skylight sun now.
        let disc = vertices.iter().any(|v| {
            v.color[0] < 0.08
                && v.position[1] > TURNTABLE_TOP_M
                && v.position[1] < TURNTABLE_TOP_M + 0.02
        });
        assert!(!disc, "the faked shadow disc must be gone");
    }

    /// Ray-vs-mesh (Möller–Trumbore over the triangle list): does a ray from `origin` along
    /// `dir` hit any hangar triangle?
    fn ray_hits_mesh(
        origin: [f32; 3],
        dir: [f32; 3],
        vertices: &[SceneVertex],
        indices: &[u32],
    ) -> bool {
        let o = Vec3::from_array(origin);
        let d = Vec3::from_array(dir).normalize();
        indices.chunks(3).any(|tri| {
            let a = Vec3::from_array(vertices[tri[0] as usize].position);
            let b = Vec3::from_array(vertices[tri[1] as usize].position);
            let c = Vec3::from_array(vertices[tri[2] as usize].position);
            let (e1, e2) = (b - a, c - a);
            let p = d.cross(e2);
            let det = e1.dot(p);
            if det.abs() < 1.0e-8 {
                return false;
            }
            let inv = 1.0 / det;
            let s = o - a;
            let u = s.dot(p) * inv;
            if !(0.0..=1.0).contains(&u) {
                return false;
            }
            let q = s.cross(e1);
            let w = d.dot(q) * inv;
            if w < 0.0 || u + w > 1.0 {
                return false;
            }
            e2.dot(q) * inv > 1.0e-3
        })
    }

    /// THE lock behind D20's fix: the workshop sun is REAL. The `garage_hero` key direction,
    /// followed up from the turntable deck, must leave the hall through a genuine roof opening
    /// — the roof comment claimed "sun through the skylights" for months while a solid slab
    /// blocked every ray, which is why the hero never threw a contact shadow. Reads the LIVE
    /// lighting profile, so a relight that moves the key forces whoever moves it to move the
    /// sun band too.
    #[test]
    fn the_workshop_sun_reaches_the_turntable_through_a_real_opening() {
        let (vertices, indices) = hangar_scene_mesh();
        let key = renderer_api::SceneLighting::garage_hero().key_direction;

        // Rays start just above the turntable's dressing (plate seams, hub), where the hull
        // stands — the lock is "the sun reaches the hero's station", not "the deck sticker".
        let deck = TURNTABLE_TOP_M + 0.6;

        // The hero's centre stands in the sun, exactly.
        assert!(
            !ray_hits_mesh([0.0, deck, 0.0], key, &vertices, &indices),
            "the key must reach the turntable centre unobstructed"
        );
        // Across the deck, most of the sun arrives; mullions, trusses and the lamp rig are
        // allowed to stripe it (that is what a glazed workshop roof looks like).
        let (mut clear, mut total) = (0, 0);
        for gx in -2i32..=2 {
            for gz in -2i32..=2 {
                total += 1;
                let origin = [gx as f32 * 1.5, deck, gz as f32 * 1.5];
                if !ray_hits_mesh(origin, key, &vertices, &indices) {
                    clear += 1;
                }
            }
        }
        assert!(
            clear * 10 >= total * 6,
            "most of the deck stands in the sun: {clear}/{total} rays clear"
        );
        // And the hall still HAS a roof: straight up from the hero is covered — the openings
        // are windows in a roof, not a missing lid.
        assert!(
            ray_hits_mesh([0.0, deck, 0.0], [0.0, 1.0, 0.0], &vertices, &indices),
            "straight overhead stays roofed"
        );
    }

    /// The garage's shadow boxes pin to the turntable, and the pin is inside the hall.
    #[test]
    fn the_shadow_focus_is_the_turntable() {
        let [x, y, z] = hangar_shadow_focus();
        assert_eq!([x, z], [0.0, 0.0], "the focus is the turntable centre");
        assert!((y - TURNTABLE_TOP_M).abs() < 1.0e-6);
        assert!(x.abs() < HALF && z.abs() < HALF && y < WALL_HEIGHT);
    }

    /// THE GARAGE'S SHADOW BOX IS SIZED TO THE ROOM. Two halves of one contract, because
    /// failing either way ruins the picture:
    ///
    /// - too LARGE and the texels are wasted on ground outside the walls (the 64 m battlefield
    ///   box gave the hall 7.9% of the map, and staircased every skylight shaft);
    /// - too SMALL and the hall's own corners fall out of the near cascade onto the coarse far
    ///   one, which trades the staircase for a visible quality seam across the walls.
    ///
    /// So: every corner of the hall must project INSIDE the near box (with the cascade's
    /// containment margin to spare), under every garage light rig — and the box must still be
    /// small enough to beat the battlefield default by a real factor.
    #[test]
    fn the_near_shadow_box_contains_the_whole_hall() {
        use glam::Mat4;
        use renderer_api::{SceneLighting, SunShadowParams, sun_light_view_projection};

        let radius = hangar_shadow_radius_m();
        let params = SunShadowParams { focus_radius_m: radius, ..SunShadowParams::default() };
        let focus = hangar_shadow_focus();
        // The near cascade hands a fragment to the far cascade once its UV leaves this margin
        // (CASCADE_MARGIN_UV in renderer_wgpu's shadow.rs); in NDC that is 4% of the half-box.
        const MARGIN_NDC: f32 = 1.0 - 2.0 * 0.02;

        for rig in [
            SceneLighting::garage_hero(),
            SceneLighting::garage_workshop(),
            SceneLighting::garage_studio(),
        ] {
            let m = Mat4::from_cols_array_2d(&sun_light_view_projection(
                rig.key_direction,
                focus,
                params,
            ));
            for x in [-HALF, HALF] {
                for z in [-HALF, HALF] {
                    for y in [0.0, WALL_HEIGHT] {
                        let clip = m * glam::Vec4::new(x, y, z, 1.0);
                        let ndc = clip.truncate() / clip.w;
                        assert!(
                            ndc.x.abs() <= MARGIN_NDC && ndc.y.abs() <= MARGIN_NDC,
                            "hall corner ({x}, {y}, {z}) falls out of the near shadow box \
                             (ndc {ndc:?}) under key {:?} — the walls would take the far \
                             cascade and read a seam",
                            rig.key_direction
                        );
                    }
                }
            }
        }

        // And it is genuinely tighter than the battlefield box it replaces: same 2048² map,
        // smaller footprint, finer texels. The numbers this whole change exists for.
        let battlefield = SunShadowParams::default();
        assert!(
            radius < battlefield.focus_radius_m,
            "a garage box no smaller than the battlefield's buys nothing"
        );
        let gain = battlefield.texel_world_size() / params.texel_world_size();
        assert!(
            gain >= 2.0,
            "the garage box must be worth the wiring: only {gain:.1}x finer texels"
        );
        assert!(
            params.texel_world_size() < 0.03,
            "hangar texel {:.4} m regressed past 3 cm",
            params.texel_world_size()
        );
    }

    /// The back of the frame is a CLOSED bay gate, not a glowing plate: nothing on the back
    /// wall below the seam may run brighter than the walls' own palette (the only hot
    /// emitters in the hall are the worklamp faces).
    #[test]
    fn the_bay_gate_is_steel_not_a_lightbox() {
        let (vertices, _) = hangar_scene_mesh();
        let back_wall_glow = vertices.iter().any(|v| {
            v.position[2] < -(HALF - 1.0)
                && v.position[1] < WALL_SEAM
                && v.color.iter().any(|&c| c > 0.8)
        });
        assert!(!back_wall_glow, "the gate must read as steel, not a glowing doorway");
        // The gate itself is present: framed slats proud of the back wall plane (hue-matched —
        // the corner-shade bake scales the authored colours).
        let slats = vertices
            .iter()
            .filter(|v| is_shade_of(v.color, GATE_SLAT) || is_shade_of(v.color, GATE_SLAT_ALT))
            .count();
        assert!(slats >= 6 * 24, "six framed gate slats, got {slats} vertices");
    }

    /// The turntable is machinery: a rim ring wider than the deck and radial plate seams on
    /// top of it — not a flat sticker disc.
    #[test]
    fn the_turntable_has_a_rim_and_plate_seams() {
        let (vertices, _) = hangar_scene_mesh();
        let rim = vertices.iter().any(|v| {
            let r = (v.position[0] * v.position[0] + v.position[2] * v.position[2]).sqrt();
            is_shade_of(v.color, TURNTABLE_RIM) && r > TURNTABLE_RADIUS_M + 0.2
        });
        assert!(rim, "a rim ring extends past the deck");
        let seams = vertices.iter().filter(|v| is_shade_of(v.color, TURNTABLE_SEAM)).count();
        assert!(seams >= 4 * 8, "radial plate seams dress the deck, got {seams}");
    }

    #[test]
    fn the_two_wall_bands_abut_without_overlapping() {
        // Regression for the z-fighting moiré band: the gunmetal and near-black wall slabs must
        // meet edge-to-edge at `WALL_SEAM`, never share coplanar inner faces over an overlap.
        let (vertices, _) = hangar_scene_mesh();
        // Vertices on the right side-wall inner plane (x == HALF - SLAB).
        let on_plane = |c: [f32; 3]| {
            vertices
                .iter()
                .filter(move |v| (v.position[0] - (HALF - SLAB)).abs() < 1.0e-4 && v.color == c)
                .map(|v| v.position[1])
        };
        let metal_top = on_plane(METAL).fold(f32::MIN, f32::max);
        let upper_bottom = on_plane(UPPER_WALL).fold(f32::MAX, f32::min);
        assert!(metal_top > 0.0, "the gunmetal band should reach the inner wall plane");
        assert!(
            (metal_top - upper_bottom).abs() < 1.0e-4,
            "wall bands must abut at the seam, not overlap: metal top {metal_top}, upper bottom {upper_bottom}"
        );
        // The upper band runs all the way to the ceiling — no gap to the roof.
        let upper_top = on_plane(UPPER_WALL).fold(f32::MIN, f32::max);
        assert!((upper_top - WALL_HEIGHT).abs() < 1.0e-4, "upper wall must reach the roof");
    }

    #[test]
    fn workshop_props_add_geometry_beyond_the_bare_shell() {
        let (with_props, _) = hangar_scene_mesh();
        // The gallery band, both bays, the stores zone and the lamp rig dwarf the shed shell.
        assert!(with_props.len() > 6000, "the hall is furnished, got {}", with_props.len());
        // Props sit outside the turntable, clear of the hero vehicle.
        assert!(
            with_props.iter().any(|v| v.position[0].abs() > TURNTABLE_RADIUS_M + 1.0),
            "props stand off the turntable"
        );
    }

    /// The 36 m volume is EARNED: the band between the wall seam and the roof carries the
    /// catwalk, crane rails, lamp rig and signage — not empty black air.
    #[test]
    fn the_upper_band_is_inhabited() {
        let (vertices, _) = hangar_scene_mesh();
        let inhabited = vertices
            .iter()
            .filter(|v| {
                let [x, y, z] = v.position;
                y > WALL_SEAM
                    && y < WALL_HEIGHT - 0.6
                    && x.abs() < HALF - SLAB - 0.01
                    && z.abs() < HALF - SLAB - 0.01
            })
            .count();
        assert!(inhabited >= 800, "the upper band is dressed, got {inhabited} vertices");
    }

    /// Every hot lamp face hangs where the `garage_hero` light rig says a light is: the pools
    /// and the housings are twins, or the room reads as haunted. NO exceptions any more — the
    /// glowing "frosted panes" that used to be excused from the pairing are gone (a flat HDR
    /// slab is not a window; readable-light correction, 2026-08-05), so every emissive vertex
    /// in the hall below the skylights is a worklamp face with a housing and a rig pool.
    #[test]
    fn lamp_faces_run_hot_and_hang_from_housings() {
        let (vertices, _) = hangar_scene_mesh();
        let rig = renderer_api::SceneLighting::garage_hero().local_lights;
        let hot: Vec<&SceneVertex> = vertices
            .iter()
            .filter(|v| v.color.iter().any(|&c| c > 1.3) && v.position[1] < WALL_HEIGHT - 0.6)
            .collect();
        assert!(hot.len() >= 6 * 4, "the lamp rig has hot faces, got {}", hot.len());
        for vertex in &hot {
            assert!(
                vertex.position[2] > -(HALF - 1.0),
                "an emitter on the back wall is a lightbox, not a window: {:?}",
                vertex.position
            );
            let near_light = rig.iter().any(|light| {
                light.radius_m > 0.0
                    && (vertex.position[0] - light.position[0]).abs() < 1.6
                    && (vertex.position[2] - light.position[2]).abs() < 1.6
            });
            assert!(
                near_light,
                "hot face at {:?} hangs away from every rig light",
                vertex.position
            );
        }
    }

    /// The extinguishers are the hall's ONE saturated red accent — nothing else may reach for
    /// that chroma, or the accent stops meaning anything.
    #[test]
    fn the_extinguishers_are_the_only_saturated_red() {
        let (vertices, _) = hangar_scene_mesh();
        let red: Vec<_> = vertices
            .iter()
            .filter(|v| v.color[0] > 0.42 && v.color[0] > 2.5 * v.color[1] && v.color[0] <= 1.0)
            .collect();
        assert!(!red.is_empty(), "the extinguishers exist");
        for vertex in &red {
            let [x, _, z] = vertex.position;
            let by_gate = (x - -5.6).abs() < 0.6 && (z + (HALF - 0.45)).abs() < 0.6;
            let by_bench = (x - (HALF - 0.45)).abs() < 0.6 && (z - 3.8).abs() < 0.6;
            assert!(by_gate || by_bench, "stray saturated red at {:?}", vertex.position);
        }
    }

    /// The far side of the hall reads OCCUPIED: the second bay and its furniture put real
    /// geometry in the +x/+z quadrant, off the hero's turntable.
    #[test]
    fn the_second_bay_reads_occupied() {
        let (vertices, _) = hangar_scene_mesh();
        let occupied = vertices
            .iter()
            .filter(|v| {
                let [x, y, z] = v.position;
                x > 7.0 && z > 5.5 && y > 0.05 && y < 4.0
            })
            .count();
        assert!(occupied >= 400, "the second bay is furnished, got {occupied} vertices");
    }

    /// The corner-shade bake grounds the furniture without touching the emitters: a barrel's
    /// floor contact is darker than its upper rim, every shade factor stays within the bounded
    /// 0.8..=1.0 band, and hot faces (skylights, lamp panels) keep their authored colours.
    #[test]
    fn corner_shade_grounds_the_furniture_without_touching_emitters() {
        let (vertices, _) = hangar_scene_mesh();
        // The barrels hug the left wall: their base vertices sit at the floor AND the wall.
        const BARREL: [f32; 3] = [0.30, 0.34, 0.30];
        let barrels: Vec<_> = vertices.iter().filter(|v| is_shade_of(v.color, BARREL)).collect();
        assert!(!barrels.is_empty(), "the barrels survive the bake hue-intact");
        let darkest =
            barrels.iter().min_by(|a, b| a.color[0].partial_cmp(&b.color[0]).unwrap()).unwrap();
        let brightest =
            barrels.iter().max_by(|a, b| a.color[0].partial_cmp(&b.color[0]).unwrap()).unwrap();
        assert!(
            darkest.position[1] < brightest.position[1],
            "the floor contact is the darkest part of a barrel"
        );
        assert!(darkest.color[0] >= BARREL[0] * 0.8 - 1.0e-4, "the bake is bounded at 0.8x");
        // Emitters keep their authored bits: the lamp faces run hot and survive the bake
        // exactly as authored (the bake skips every emissive face).
        assert!(
            vertices.iter().any(|v| v.color[0] > 1.3 && v.color[1] > 1.3),
            "the lamp rig's hot faces stay authored-hot"
        );
    }

    /// The hall's SIZE budget. It used to be the whole of what this file measured, under a
    /// comment claiming "the hall re-bakes on every garage entry" — which was false (the
    /// `OnceLock` bakes once) and, worse, pointed the budget at a proxy: vertex count is not
    /// what the player waits for. The cost is measured next door, in
    /// `the_hall_is_built_off_the_thread_that_asks_for_it` and `the_cold_bake_is_measured`.
    #[test]
    fn the_hangar_stays_inside_its_size_budget() {
        let (vertices, indices) = hangar_scene_mesh();
        assert!(vertices.len() < 30_000, "hangar vertex budget: {}", vertices.len());
        assert!(indices.len() < 120_000, "hangar index budget: {}", indices.len());
    }

    /// THE LOCK BEHIND `prewarm`: asking for the hall must not build it on the asking thread.
    ///
    /// The bound is three orders of magnitude under the build it replaces, so it cannot flake
    /// on a slow machine and cannot pass if someone makes `prewarm` synchronous again — which
    /// is the whole failure this exists to prevent. The hall itself is proven to arrive by
    /// every other test in this file; here the only question is who builds it.
    #[test]
    fn the_hall_is_built_off_the_thread_that_asks_for_it() {
        let asked = std::time::Instant::now();
        prewarm();
        let cost_to_the_caller = asked.elapsed();
        assert!(
            cost_to_the_caller < std::time::Duration::from_millis(50),
            "prewarm built the hall on the caller's thread: it cost {cost_to_the_caller:?}"
        );
        // And the worker really is building the same hall — after the blocking call the cache
        // is warm however the race went.
        let (vertices, _) = hangar_scene_mesh();
        assert!(!vertices.is_empty());
        assert!(is_baked(), "the cache is warm once the mesh has been handed out");
    }

    /// The cold build, measured through the private builder so the `OnceLock` cannot hide it,
    /// and PRINTED — a number nobody writes down is a number nobody can be held to.
    ///
    /// The ceiling catches an order-of-magnitude regression (a ray count, an edge limit or a
    /// lost worker lane), not a tuning wobble: tests run at `opt-level = 1`, where the same
    /// work costs several times its release price, and CI machines vary. What the number is
    /// FOR is the release measurement in `prewarm`'s docs.
    #[test]
    fn the_cold_bake_is_measured() {
        let started = std::time::Instant::now();
        let (vertices, indices) = build_hangar_scene_mesh();
        let cost = started.elapsed();
        println!(
            "HANGAR COLD BAKE: {:?} for {} vertices / {} triangles",
            cost,
            vertices.len(),
            indices.len() / 3
        );
        assert!(
            cost < std::time::Duration::from_secs(20),
            "the hangar bake regressed by an order of magnitude: {cost:?}"
        );
    }

    /// Nothing may invade the hero's stage: the turntable's air column stays clear for the
    /// vehicle and the orbit camera, and the drive lane stays clear for the roll-in.
    #[test]
    fn nothing_invades_the_orbit_or_the_drive_lane() {
        let (vertices, _) = hangar_scene_mesh();
        for vertex in &vertices {
            let [x, y, z] = vertex.position;
            let r = (x * x + z * z).sqrt();
            assert!(
                !(r < TURNTABLE_RADIUS_M + 0.5 && y > 0.5 && y < 8.4),
                "geometry invades the turntable air column at {:?}",
                vertex.position
            );
            assert!(
                !(x.abs() < 2.7
                    && (-(HALF - 0.5)..-TURNTABLE_RADIUS_M).contains(&z)
                    && y > 0.4
                    && y < 4.4),
                "geometry blocks the drive lane at {:?}",
                vertex.position
            );
        }
    }
}
