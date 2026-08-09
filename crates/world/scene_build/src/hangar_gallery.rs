//! The hangar's upper band: the set pieces that make a 36 m hall read as a working maintenance
//! facility instead of an empty cathedral — a catwalk gallery with its stair, crane rails the
//! girder actually rides, the worklamp housings (geometry twins of the `garage_hero` light rig:
//! the light pools where a lamp hangs), and the stencil signage. All procedural slabs and
//! cylinders, baked into the same static hangar mesh.

use renderer_api::SceneVertex;

use super::hangar::{Finish, HALF, SLAB, WALL_HEIGHT, finish, slab};

// Gallery palette (colors linear; the material each piece is MADE of is stamped per section
// with `finish` — see `hangar::Finish`, and the audit finding that this whole hall shipped
// with no surface role and no specular on seven vertices in ten).
const CATWALK_DECK: [f32; 3] = [0.21, 0.215, 0.225];
const KICK_PLATE: [f32; 3] = [0.145, 0.15, 0.16];
const RAIL: [f32; 3] = [0.30, 0.31, 0.33];
const COLUMN: [f32; 3] = [0.28, 0.29, 0.31];
const LAMP_STEM: [f32; 3] = [0.16, 0.17, 0.18];
const LAMP_SHADE: [f32; 3] = [0.17, 0.18, 0.19];
/// Lamp faces run hot (channels past 1.0) so the tone curve blooms them — the same emissive
/// convention as the skylight strips.
const LAMP_FACE: [f32; 3] = [1.55, 1.40, 1.10];
const STRIP_FACE: [f32; 3] = [1.45, 1.42, 1.30];
const CHEVRON_DARK: [f32; 3] = [0.08, 0.08, 0.08];
const CHEVRON_YELLOW: [f32; 3] = [0.62, 0.55, 0.20];
const BANNER: [f32; 3] = [0.16, 0.20, 0.15];
const BANNER_BORDER: [f32; 3] = [0.30, 0.33, 0.27];
const BANNER_PATCH: [f32; 3] = [0.58, 0.56, 0.48];

/// Top surface of the catwalk deck, just under the wall-band seam so the girt rail reads as its
/// support line.
const DECK_Y: f32 = 4.9;
/// Where the worklamps hang — must agree with `SceneLighting::garage_hero()`'s rig (locked by
/// `lamp_faces_run_hot_and_hang_from_housings`).
pub(super) const HIGH_BAY_LAMPS: [[f32; 3]; 2] = [[-3.6, 9.8, 1.8], [3.6, 9.8, -1.8]];
pub(super) const STORES_LAMP: [f32; 3] = [-14.5, 6.2, 10.0];
pub(super) const SECOND_BAY_LAMP: [f32; 3] = [10.5, 8.6, 9.5];
pub(super) const BENCH_STRIP_LAMP: [f32; 3] = [16.0, 3.4, 6.0];

pub(super) fn push_gallery(v: &mut Vec<SceneVertex>, i: &mut Vec<u32>) {
    push_catwalk(v, i);
    push_crane_rails(v, i);
    push_worklamps(v, i);
    push_signage(v, i);
    push_cable_trays(v, i);
}

/// The catwalk gallery: a deck along the back wall turning up the left wall, kick plates,
/// railing, support columns, and a stair flight down toward the stores zone. The decks ABUT at
/// the corner (never overlap — coplanar top faces would z-fight).
fn push_catwalk(v: &mut Vec<SceneVertex>, i: &mut Vec<u32>) {
    let start = v.len();
    let deck_c = DECK_Y - 0.06;
    let inner = HALF - 0.25 - 2.2; // inner edge plane of the 2.2 m-deep deck
    // Back-wall run spans the full width; the left-wall leg starts where the back run ends.
    slab(v, i, [0.0, deck_c, -(HALF - 1.35)], [HALF - 0.25, 0.06, 1.1], CATWALK_DECK);
    let leg_z = (-inner + 6.0) / 2.0;
    let leg_half = (6.0 + inner) / 2.0;
    slab(v, i, [-(HALF - 1.35), deck_c, leg_z], [1.1, 0.06, leg_half], CATWALK_DECK);

    // Kick plates + two-rail railing along both inner edges.
    let rail_runs: [([f32; 3], [f32; 3]); 2] = [
        ([0.0, 0.0, -inner], [HALF - 0.25, 0.0, 0.03]),
        ([-inner, 0.0, leg_z], [0.03, 0.0, leg_half]),
    ];
    for (center, half) in rail_runs {
        slab(v, i, [center[0], DECK_Y + 0.06, center[2]], [half[0], 0.12, half[2]], KICK_PLATE);
        for rail_y in [DECK_Y + 0.55, DECK_Y + 0.95] {
            slab(
                v,
                i,
                [center[0], rail_y, center[2]],
                [half[0].max(0.03), 0.03, half[2].max(0.03)],
                RAIL,
            );
        }
    }
    // Railing posts every ~2.2 m plus support columns every ~4.4 m under the deck.
    let mut post = |x: f32, z: f32| {
        slab(v, i, [x, DECK_Y + 0.5, z], [0.03, 0.5, 0.03], RAIL);
    };
    let posts = ((HALF - 0.25) * 2.0 / 2.2) as i32;
    for k in 0..=posts {
        let x = -(HALF - 0.25) + k as f32 * 2.2;
        if x <= HALF - 0.25 {
            post(x, -inner);
        }
    }
    let leg_posts = (leg_half * 2.0 / 2.2) as i32;
    for k in 0..=leg_posts {
        let z = -inner + k as f32 * 2.2;
        if z <= 6.0 {
            post(-inner, z);
        }
    }
    let col_half = (DECK_Y - 0.12) / 2.0;
    for k in 0..=((HALF - 1.0) / 4.4) as i32 {
        for x in [-(k as f32) * 4.4, k as f32 * 4.4] {
            slab(v, i, [x, col_half, -(inner + 1.0)], [0.09, col_half, 0.09], COLUMN);
        }
    }

    // The stair: ten steps descending from the left deck's south end toward the stores zone.
    for step in 0..10 {
        let k = step as f32;
        slab(
            v,
            i,
            [-(HALF - 1.35), DECK_Y - 0.46 * (k + 1.0) + 0.045, 6.0 + 0.30 * (k + 1.0)],
            [0.55, 0.045, 0.14],
            CATWALK_DECK,
        );
    }
    // Grating deck, kick plates, rail, columns and stair: bare section steel, never painted.
    finish(&mut v[start..], Finish::MACHINED_STEEL);
}

/// Continuous crane rails along both side walls at girder height, on corbel brackets — the
/// overhead girder finally rides something.
fn push_crane_rails(v: &mut Vec<SceneVertex>, i: &mut Vec<u32>) {
    let start = v.len();
    let rail_y = WALL_HEIGHT - 2.5 + 0.34 + 0.16; // just under the girder's cross rail
    for sign in [-1.0_f32, 1.0] {
        let x = sign * (HALF - 1.1);
        slab(v, i, [x, rail_y, 0.0], [0.14, 0.16, HALF - 0.6], RAIL);
        for k in [-0.8_f32, -0.4, 0.0, 0.4, 0.8] {
            let bracket_x = sign * (HALF - 0.62);
            slab(v, i, [bracket_x, rail_y - 0.05, k * HALF], [0.48, 0.10, 0.28], COLUMN);
        }
    }
    // A crane rail is polished by the wheels that ride it — the brightest steel in the hall.
    finish(&mut v[start..], Finish::MACHINED_STEEL);
}

/// The worklamp housings: geometry twins of the `garage_hero` light rig. Every hot face hangs
/// within arm's reach of its rig position, so light pools exactly where a lamp is seen.
fn push_worklamps(v: &mut Vec<SceneVertex>, i: &mut Vec<u32>) {
    let start = v.len();
    // High-bay pendants over the turntable + the two zone pendants, all dropped from the roof.
    for [x, y, z] in HIGH_BAY_LAMPS.iter().copied().chain([STORES_LAMP, SECOND_BAY_LAMP]) {
        let stem_top = WALL_HEIGHT - 0.15;
        let stem_half = (stem_top - (y + 0.2)) / 2.0;
        slab(v, i, [x, y + 0.2 + stem_half, z], [0.04, stem_half, 0.04], LAMP_STEM);
        slab(v, i, [x, y + 0.1, z], [0.42, 0.10, 0.42], LAMP_SHADE);
        slab(v, i, [x, y - 0.015, z], [0.34, 0.015, 0.34], LAMP_FACE);
    }
    // The workbench strip light: a wall-bracketed tube shade with a hot underside.
    let [x, y, z] = BENCH_STRIP_LAMP;
    slab(
        v,
        i,
        [(x + HALF - SLAB) / 2.0, y + 0.3, z],
        [(HALF - SLAB - x) / 2.0, 0.04, 0.04],
        LAMP_STEM,
    );
    slab(v, i, [x, y + 0.15, z], [0.15, 0.05, 1.1], LAMP_SHADE);
    slab(v, i, [x, y + 0.08, z], [0.10, 0.015, 1.05], STRIP_FACE);
    // Stems and shades are enamelled pressings. The hot FACES take no finish at all: `finish`
    // skips emissive vertices, because a lamp face is a light and multiplying a light by a
    // paint treatment is a category error.
    finish(&mut v[start..], Finish::PAINTED_STEEL);
}

/// Stencil signage, no lettering: hazard chevrons on the gate lintel, a bordered banner with a
/// concentric stencil patch hung off the front trusses, and small marker plates over the bays.
fn push_signage(v: &mut Vec<SceneVertex>, i: &mut Vec<u32>) {
    let start = v.len();
    // Hazard chevrons across the gate lintel (alternating yellow/near-black), proud of it.
    let lintel_z = -(HALF - SLAB) + 0.22;
    for step in 0..8 {
        let x = -4.05 + step as f32 * 1.16;
        let color = if step % 2 == 0 { CHEVRON_YELLOW } else { CHEVRON_DARK };
        slab(v, i, [x, 5.28, lintel_z], [0.55, 0.14, 0.02], color);
    }
    // The banner: a dark board with a lighter border and concentric stencil squares, hung from
    // the front-wall truss on two straps.
    let banner_z = HALF * 0.8 - 0.4;
    for x in [-1.2_f32, 1.2] {
        slab(v, i, [x, WALL_HEIGHT - 1.05, banner_z], [0.03, 0.75, 0.02], LAMP_STEM);
    }
    slab(v, i, [0.0, WALL_HEIGHT - 2.7, banner_z], [1.6, 0.9, 0.03], BANNER);
    for (dy, half) in [(0.82_f32, [1.52, 0.06, 0.035]), (-0.82, [1.52, 0.06, 0.035])] {
        slab(v, i, [0.0, WALL_HEIGHT - 2.7 + dy, banner_z], half, BANNER_BORDER);
    }
    for dx in [-1.52_f32, 1.52] {
        slab(v, i, [dx, WALL_HEIGHT - 2.7, banner_z], [0.06, 0.9, 0.035], BANNER_BORDER);
    }
    slab(v, i, [0.0, WALL_HEIGHT - 2.7, banner_z - 0.045], [0.55, 0.55, 0.012], BANNER_PATCH);
    slab(v, i, [0.0, WALL_HEIGHT - 2.7, banner_z - 0.055], [0.33, 0.33, 0.008], BANNER);
    // Marker plates over the two side bays: the same concentric-stencil motif, small.
    for (x, z) in [(-(HALF - SLAB - 0.06), 10.0), (HALF - SLAB - 0.06, 9.5)] {
        let sign = if x < 0.0 { 1.0 } else { -1.0 };
        slab(v, i, [x, 4.6, z], [0.02, 0.34, 0.34], BANNER);
        slab(v, i, [x + sign * 0.015, 4.6, z], [0.012, 0.2, 0.2], BANNER_PATCH);
    }
    // Chevrons, banner and marker plates are stencil over plate — the same paint the bay
    // markings on the floor are, at the same low sheen.
    finish(&mut v[start..], Finish::PAINT_MARK);
}

/// Cable trays: a U-profile run along the right wall feeding the workbench wall, vertical
/// conduit drops, and a ceiling tray out to the turntable lamps — the shop's wiring story.
fn push_cable_trays(v: &mut Vec<SceneVertex>, i: &mut Vec<u32>) {
    let start = v.len();
    let wall_x = HALF - SLAB - 0.10;
    slab(v, i, [wall_x, 3.4, 0.0], [0.12, 0.02, 10.0], LAMP_STEM);
    for lip in [-0.10_f32, 0.10] {
        slab(v, i, [wall_x + lip, 3.47, 0.0], [0.02, 0.07, 10.0], LAMP_STEM);
    }
    // Conduit drops to the bench line and the tool board.
    for z in [5.2_f32, 6.8] {
        slab(v, i, [wall_x, 2.15, z], [0.03, 1.25, 0.03], LAMP_STEM);
    }
    // Ceiling tray from the wall out to the high-bay pendants' drop line.
    let tray_half = (wall_x - 3.6) / 2.0;
    slab(v, i, [3.6 + tray_half, WALL_HEIGHT - 0.3, -1.8], [tray_half, 0.03, 0.06], LAMP_STEM);
    // Galvanised tray and conduit: mill finish, never sprayed with the wall behind it.
    finish(&mut v[start..], Finish::MACHINED_STEEL);
}
