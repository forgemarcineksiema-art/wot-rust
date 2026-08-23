//! The garage hangar: a single static interior scene the owned tank is parked in, replacing the
//! battlefield while the garage is open. Hala 3.0 stage A1: a RECTANGULAR industrial nave — a
//! poured concrete floor, painted sheet walls, deep roof trusses under a SAWTOOTH (shed) roof whose
//! glazing bands are REAL openings, a bay gate at the end of the long axis standing ajar over a
//! wedge of daylight, a turntable spot, and workshop props (`hangar_props`: crane, wheel/track
//! stacks, workbench, barrels, oil stains). The room is built from *solid slabs* surrounding the
//! play volume: each slab's inner surface is an ordinary outward-facing face, so back-face culling
//! keeps exactly the walls seen from inside. The hero vehicle throws a real contact shadow on the
//! turntable from the workshop sun key — no faked shadow disc.

use glam::{Mat3, Vec3};
use renderer_api::{SceneLighting, SceneVertex};

use crate::tank_mesh::push_oriented_box;

/// Interior half-extents of the nave (Hala 3.0 A1): 22 m across, 44 m down the long axis. The
/// square 36×36 hall read as a box with a tank in it; a nave has a DIRECTION — the gate at one end,
/// the hero station on the axis, depth receding behind the hull in every hero frame. The camera
/// boom no longer fits at every angle by construction; [`max_orbit_boom`] clamps it against the
/// walls per angle instead (decision 2026-08-09: full 360° orbit, boom rides the geometry).
/// `pub(super)` so the props hug the walls.
pub(super) const HALF_X: f32 = 11.0;
pub(super) const HALF_Z: f32 = 22.0;
/// Height of the truss bottom chords — the working ceiling of the room. The shed roof rises
/// above it to [`SHED_RIDGE`]; everything human-scale (walls' seam, gate, catwalk, lamps) is
/// proportioned against THIS, not the ridge.
pub(super) const WALL_HEIGHT: f32 = 9.0;
/// Top of each sawtooth: the shed decks climb from the eaves to here, then drop back through
/// the glazing opening. Side walls run to the ridge so the sheds meet steel, not sky slivers.
pub(super) const SHED_RIDGE: f32 = 11.3;
pub(super) const SLAB: f32 = 0.15;
/// Height where the gunmetal lower wall meets the shadowed upper wall. The two bands **abut** at
/// this seam — they must never overlap, or their coplanar inner faces z-fight (a moiré band).
pub(super) const WALL_SEAM: f32 = 5.6;
/// Top surface of the turntable the tank rests on, metres above the floor. Hala 3.0 A2: the
/// station is a plate SUNK INTO the slab — 2 cm proud, a machined seam in the floor rather
/// than a 12 cm podium. A vehicle on a pedestal is a showroom; a vehicle on a flush ring is a
/// workshop that happens to be pointing a turntable at it.
pub const TURNTABLE_TOP_M: f32 = 0.02;
const TURNTABLE_RADIUS_M: f32 = 5.2;

// Workshop palette: warm cast concrete, lime-washed lower walls, painted gunmetal above. The
// upper band and roof fall into SHADOW, not void — a step darker, never near-black.
const CONCRETE: [f32; 3] = [0.30, 0.29, 0.28];
/// C2: the lower wall bands are WHITEWASHED — lime over the sheet, the working hall's oldest
/// coat, and it reads as OLD lime: years of shop dust in it, not a fresh remont. The first
/// candidate ([0.54, 0.535, 0.51]) measured the hero at 1.68x over the room against the 2.0x
/// floor — the fresh white competed with the subject, and the lock held; this tone puts the
/// room median back under the hero's shoulder. The reflection lock reads this as "what a
/// vertical surface sees across the bay".
const WHITEWASH_WALL: [f32; 3] = [0.38, 0.375, 0.355];
/// C2 palette: rust on worn machined steel — drain grates, handled link plates. A warm dead
/// tone, deliberately short of the extinguishers' saturated red (their lock stands).
const RUST: [f32; 3] = [0.295, 0.195, 0.14];
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
/// Dirty glazing (Światło służy czołgowi): NON-emissive on purpose — the first candidate
/// glowed past 1.0, the bake's emission boost turned the whole roof into a lamp, and two
/// bake-value locks rightly refused ("a roof with decks in it cannot out-lume the bare
/// day"). Cool glass tone; the daylight read comes from the gloss lane and the environment
/// reflection, and the BROKEN bays showing the raw sky are the roof's only true brights —
/// which is the story.
const GLAZE_PANE: [f32; 3] = [0.60, 0.64, 0.70];
/// Shard remnants in a broken bay's frame: glass tone, no glow.
const SHARD_GLASS: [f32; 3] = [0.55, 0.60, 0.62];
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
///
/// A1 reframe: the yaw drops from 0.60 toward the long axis (yaw 0 looks straight down the
/// nave), so the hero stands against 35 m of receding hall — trusses, shed light and the ajar
/// gate — instead of a wall 18 m behind it. Still enough of an angle that the shot is a
/// three-quarter, not an axial mugshot.
pub const HERO_ORBIT_YAW: f32 = 0.42;
/// Lowered from 0.28. At 0.28 rad the camera tilted 16 deg down through a 32 deg lens, which put
/// the TOP of the frame exactly on the horizon through the pivot: everything above the eye — the
/// roof, the trusses, the skylight strips, the frosted panes over the bay gate, both high-bay
/// lamps — fell outside the shot. That is the arithmetic behind D20's "0.00% of the hero frame
/// sits above the bright threshold": the room's light sources are all real and all emissive, and
/// the lens was pointed under every one of them. At 0.13 the frame reaches roughly 9 deg above
/// the eye and the shed daylight over the nave comes into shot.
pub const HERO_ORBIT_PITCH: f32 = 0.13;
/// The long axis has the room for it, and the fifteenth metre buys the frame the first roof
/// truss over the hull.
pub const HERO_ORBIT_DISTANCE: f32 = 15.0;
/// The heading the hero is PARKED at, which only means anything against [`HERO_ORBIT_YAW`]: the
/// camera orbits to a bearing, the tank sits at a heading, and the angle between them is the shot.
///
/// Never let it equal `HERO_ORBIT_YAW`: a tank whose heading matches the camera's bearing
/// faces the lens dead-on, so the hero shot becomes a head-on elevation with the gun barrel
/// bisecting the hull and hiding the glacis behind it — the one angle at which a T-54 and a
/// Centurion are hardest to tell apart. Offset by ~0.65 rad it reads as the three-quarter the comment beside it
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
/// and both hangar probes. Four callers, ONE source: the moment they drift apart the locked
/// picture stops being the played picture, which is the one thing a review artifact may
/// never do.
pub const INTERIOR_BACKGROUND: (f64, f64, f64) = (1.30, 1.38, 1.55);

/// [`INTERIOR_BACKGROUND`] per daylight (H1): the day outside is what the variant IS, so the
/// backdrop in the roof openings is its first witness. Day is the canonical constant above.
pub fn interior_background_for(light: HangarLight) -> (f64, f64, f64) {
    match light {
        HangarLight::Morning => (1.08, 1.20, 1.50),
        HangarLight::Day => INTERIOR_BACKGROUND,
        HangarLight::Evening => (1.42, 1.08, 0.72),
    }
}

/// The key's direction per daylight (H1), single-sourced for the rig, the shafts and the
/// sun-reach lock. Morning keeps the DAY bearing: the sheds' glazing faces one way (the
/// standing artistic license), so a morning sun on their blind side enters as sky glow along
/// the same fans, not as a mirrored beam through solid decking. Evening keeps the azimuth —
/// the mullion-clear lanes the E1 lock guards are z-rows, and swinging the bearing would put
/// blades on the bars — and drops the elevation: a low sun, longer travel.
pub fn hangar_key_direction(light: HangarLight) -> Vec3 {
    match light {
        HangarLight::Morning | HangarLight::Day => Vec3::new(-0.233, 0.892, 0.388),
        // Normalized from (-0.6, 1.75, 1.0) — the day derivation's (-0.6, 2.3, 1.0) with the
        // sun lower. Chosen by the same sweep that placed the day key (E1): candidates from
        // 1.25 to 1.90 were ray-tested against the real glazing, and 1.75 is the lowest sun
        // that still clears the turntable centre AND keeps 15/25 of the deck fan clear — the
        // exact 60% the day's sun-reach lock demands. Lower suns hit the mullion bars.
        HangarLight::Evening => Vec3::new(-0.285_310_2, 0.832_154_75, 0.475_517),
    }
}

/// The hall's rig per daylight (H1). [`HangarLight::Day`] IS `garage_hero()` — bit-identical,
/// locked by `the_canonical_daylight_is_the_golden_rig` — and the others are re-grades of it:
/// morning halves a cooled key and lets the lamps carry, evening warms and lowers the sun and
/// deepens the grade a step. All three stay inside the moody band.
pub fn hangar_lighting(light: HangarLight) -> SceneLighting {
    let mut rig = SceneLighting::garage_hero();
    match light {
        HangarLight::Day => {}
        HangarLight::Morning => {
            rig.key_rgb = [rig.key_rgb[0] * 0.48, rig.key_rgb[1] * 0.52, rig.key_rgb[2] * 0.60];
            rig.ambient_rgb =
                [rig.ambient_rgb[0] * 0.96, rig.ambient_rgb[1] * 1.02, rig.ambient_rgb[2] * 1.12];
            rig.sky_horizon_rgb = [0.17, 0.18, 0.21];
            rig.exposure = 1.06;
            rig.black_point = 0.022;
        }
        HangarLight::Evening => {
            rig.key_direction = hangar_key_direction(HangarLight::Evening).to_array();
            rig.key_rgb = [rig.key_rgb[0] * 1.12, rig.key_rgb[1] * 0.88, rig.key_rgb[2] * 0.58];
            rig.ambient_rgb =
                [rig.ambient_rgb[0] * 1.04, rig.ambient_rgb[1] * 0.94, rig.ambient_rgb[2] * 0.82];
            rig.sky_horizon_rgb = [0.225, 0.185, 0.155];
            rig.exposure = 1.08;
            rig.black_point = 0.024;
        }
    }
    rig
}

/// [`hangar_lighting`] on the presentation clock: the bench fluorescent's flicker (E2) rides
/// every daylight the same way — and at the frozen review second the factor is exactly 1.0,
/// so the goldens' canonical rig stays `garage_hero()` to the bit.
pub fn hangar_lighting_at(light: HangarLight, seconds: f32) -> SceneLighting {
    let mut rig = hangar_lighting(light);
    rig.local_lights[2].intensity *= renderer_api::fluorescent_flicker(seconds);
    rig
}

/// A module-inspection framing: the shot the camera flies to when a fitting slot is clicked.
/// Single-sourced here (A3) for the same reason the hero constants are: the live camera, the
/// review probe and the composition locks must read the SAME shot, or the background composed
/// for a framing quietly stops being the background the framing sees.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SlotFraming {
    pub yaw: f32,
    pub pitch: f32,
    pub distance: f32,
    /// Look-point offset off the turntable centre (lift to the turret, drop to the gear).
    pub pivot_offset: [f32; 3],
}

/// The four composed module framings (A3) plus the hull's. Yaws are bearings relative to the
/// parked hull (they moved with `HERO_PARK_YAW` in A1); each background is COMPOSED — what
/// stands behind the module is subject matter, not whatever wall happened to be there:
/// the gun looks down the lane at the ajar gate between the ammunition racks, the suspension's
/// low pass ends on the spare wheels and track links at the flow wall, the engine deck fronts
/// the second bay's gantry and block, the turret shot carries the hoist hook hanging over it.
pub const FRAMING_TURRET: SlotFraming =
    SlotFraming { yaw: 0.52, pitch: 0.55, distance: 5.5, pivot_offset: [0.0, 0.7, 0.0] };
pub const FRAMING_GUN: SlotFraming =
    SlotFraming { yaw: 0.0, pitch: 0.12, distance: 6.5, pivot_offset: [0.0, 0.25, 1.6] };
pub const FRAMING_HULL: SlotFraming =
    SlotFraming { yaw: 0.37, pitch: 0.14, distance: 6.0, pivot_offset: [0.0, -0.1, 0.4] };
pub const FRAMING_ENGINE: SlotFraming =
    SlotFraming { yaw: 3.17, pitch: 0.42, distance: 6.0, pivot_offset: [0.0, 0.2, -1.6] };
/// The A3 probe caught the old 1.37 yaw standing 0.30 rad off the hull's NOSE — a point-blank
/// glacis stare that had shipped unreviewed because no probe rendered this shot before. 1.67
/// puts the camera a real 0.60 rad off the hull: the wheel line reads, and the flow wall's
/// gear stock stands in the background.
pub const FRAMING_SUSPENSION: SlotFraming =
    SlotFraming { yaw: 1.67, pitch: -0.03, distance: 4.5, pivot_offset: [0.0, -0.55, 0.0] };

/// The composed framings by name, for the review probe and the composition lock.
pub fn slot_framings() -> [(&'static str, SlotFraming); 4] {
    [
        ("turret", FRAMING_TURRET),
        ("gun", FRAMING_GUN),
        ("engine", FRAMING_ENGINE),
        ("suspension", FRAMING_SUSPENSION),
    ]
}

/// Eye and look-target of a slot framing, from the same arithmetic the live camera runs.
pub fn slot_eye(framing: SlotFraming) -> (Vec3, Vec3) {
    let pivot = hangar_camera_pivot() + Vec3::from_array(framing.pivot_offset);
    let eye = pivot + orbit_direction(framing.yaw, framing.pitch) * framing.distance;
    (eye, pivot)
}

/// Where the heating duct breathes (E2): just proud of the grille the gallery hangs at the
/// stores corner — the client's steam emitter and the geometry agree on the source.
pub const STEAM_DUCT_OUTLET: [f32; 3] = [-(HALF_X - 0.55), 2.45, 18.6];

/// The workbench anchor on the bench wall: `hangar_props` builds the bench here, and the
/// audio bed's radio (G1) murmurs FROM here — the sound and the furniture are the same fact.
/// y is the bench top, where a radio would actually stand.
pub const WORKBENCH_ANCHOR: [f32; 3] = [HALF_X - 1.7, 1.05, 6.0];

/// Where the exhaust fan hangs on the stores wall (E2): shared by the static housing
/// (`hangar_gallery`), the dynamic blades below, and the steam duct that gives the fan its
/// reason — the hall breathes out through this corner. It hangs INSIDE the stores lamp's
/// pool (4.5 m from the emitter at [-8.5, 6.2, 17.0]): the first placement at x=6.5 sat in
/// the unlit band over the catwalk and the review lens could barely find it — a moving
/// piece nobody can see buys nothing, so the fan lives where its motion catches light.
pub const FAN_CENTER: [f32; 3] = [-8.5, 7.2, HALF_Z - SLAB - 0.16];

/// The exhaust fan's BLADES at `angle` radians (E2): the one piece of the hall that moves
/// every frame, so it rides the dynamic-mesh slot the garage previously cleared. Four
/// pitched blades and a hub, rebuilt per frame (~180 vertices — cheaper than a mote). The
/// housing is static gallery geometry; this is only what turns.
pub fn wall_fan_blades(angle: f32) -> (Vec<SceneVertex>, Vec<u32>) {
    const BLADE_COUNT: u32 = 4;
    let hub = Vec3::from_array(FAN_CENTER);
    let mut v = Vec::new();
    let mut i = Vec::new();
    for blade in 0..BLADE_COUNT {
        let a = angle + blade as f32 * std::f32::consts::TAU / BLADE_COUNT as f32;
        let spin = Mat3::from_rotation_z(a);
        // Radial arm in the wall plane, pitched about its own axis so it reads as a fan
        // blade, not a paddle.
        push_oriented_box(
            &mut v,
            &mut i,
            hub + spin * Vec3::new(0.32, 0.0, 0.0),
            Vec3::new(0.26, 0.09, 0.012),
            spin * Mat3::from_rotation_x(0.55),
            [0.16, 0.165, 0.175],
        );
    }
    push_oriented_box(
        &mut v,
        &mut i,
        hub,
        Vec3::new(0.08, 0.08, 0.04),
        Mat3::IDENTITY,
        [0.13, 0.135, 0.14],
    );
    for vertex in &mut v {
        vertex.surface = renderer_api::surface_role::PAINTED_STEEL;
        vertex.gloss = 0.22;
    }
    (v, i)
}

/// The sun shafts' blade quads (Hala 3.0 E1): translucent beams hanging under the glazing
/// bands, each a world-space quad from a segment of the glazing plane down along the light's
/// travel (−key) to just over the floor. Geometry lives HERE because only the hangar knows
/// where its glazing is and which way its sun leans — the client turns these into soft
/// additive FX quads, and the E1 lock holds each blade's top edge inside a real opening
/// (`the_shafts_hang_from_real_openings`): a beam from a solid roof would be the pane-glow
/// lie all over again.
pub fn sun_shaft_quads() -> Vec<[[f32; 3]; 4]> {
    sun_shaft_quads_for(HangarLight::Day)
}

/// [`sun_shaft_quads`] per daylight (H1). Morning has NO blades — the sun stands on the
/// sheds' blind side and a beam with no opening behind it is the pane-glow lie the E1 lock
/// exists to kill. Evening runs the same mullion-clear blade rows under its lower key:
/// longer travel, the same honest openings.
pub fn sun_shaft_quads_for(light: HangarLight) -> Vec<[[f32; 3]; 4]> {
    if light == HangarLight::Morning {
        return Vec::new();
    }
    let key = hangar_key_direction(light);
    BROKEN_PANES
        .iter()
        .map(|pane| {
            // Top edge ON the broken pane's centre: the shaft is the hole's own light.
            let top = pane.center_world();
            let half_w = (pane.x_half - 0.3).min(1.1);
            // The blades die out a metre over the floor — the classic read of a shaft
            // thinning into the air, and it keeps the beams off the bright floor pixels the
            // hero-over-room lock weighs most heavily.
            let travel = -key * ((top.y - 1.0) / key.y);
            let top_a = Vec3::new(top.x - half_w, top.y, top.z);
            let top_b = Vec3::new(top.x + half_w, top.y, top.z);
            [
                top_a.to_array(),
                top_b.to_array(),
                (top_b + travel).to_array(),
                (top_a + travel).to_array(),
            ]
        })
        .collect()
}

/// One broken glazing pane (Światło służy czołgowi, user direction 2026-08-10): visible sun
/// shafts enter the hall ONLY through these — a beam needs an APERTURE, and clean glazing
/// diffuses; the old five-blade set streamed rays through intact glass, which read as a
/// light show raining on the tank. Rare and natural: three panes out of a whole roof, each
/// aimed so its beam lands BESIDE the hero (locked), framing the tank instead of hitting it.
#[derive(Debug, Clone, Copy)]
struct BrokenPane {
    /// Which shed's glazing plane (a `SHED_STARTS` value).
    shed_start: f32,
    /// The pane's bay centre and half-width in x (between the vertical divisions).
    x_center: f32,
    x_half: f32,
    /// The pane's segment centre as a fraction of the glazing slope (between the bars).
    along_frac: f32,
}

impl BrokenPane {
    /// The pane's centre on the glazing plane, in world space — shared by the roof builder
    /// (which leaves this bay OPEN and hangs shards), the shaft builder (which hangs the
    /// blade here) and the locks.
    fn center_world(&self) -> Vec3 {
        let (glaze_rot, glaze_len) = glaze_plane();
        let mid_y = (WALL_HEIGHT + SHED_RIDGE) / 2.0;
        let mid_z = self.shed_start + SHED_DECK_RUN + SHED_GLAZE_RUN / 2.0;
        let offset = glaze_rot * Vec3::new(0.0, 0.0, self.along_frac * glaze_len);
        Vec3::new(self.x_center, mid_y + offset.y, mid_z + offset.z)
    }
}

/// The glazing plane's rotation and slope length — one derivation for the roof builder, the
/// panes and the shafts.
fn glaze_plane() -> (Mat3, f32) {
    let rise = SHED_RIDGE - WALL_HEIGHT;
    let angle = rise.atan2(SHED_GLAZE_RUN);
    (Mat3::from_rotation_x(angle), (SHED_GLAZE_RUN * SHED_GLAZE_RUN + rise * rise).sqrt())
}

/// Three broken panes: two in the sun shed (one each side of the hero, feet landing at
/// r ≈ 7.3 and 5.3 from the turntable), one in the far shed over the stores. Bay centres
/// sit between the real vertical divisions, segment fractions between the real bars.
const BROKEN_PANES: [BrokenPane; 3] = [
    BrokenPane { shed_start: -5.5, x_center: 4.7, x_half: 1.5, along_frac: -0.119 },
    BrokenPane { shed_start: -5.5, x_center: -7.55, x_half: 1.25, along_frac: 0.1545 },
    BrokenPane { shed_start: 5.5, x_center: 7.7, x_half: 1.5, along_frac: -0.119 },
];

/// Direction from the pivot to the eye for an orbit yaw/pitch. Shared so the live camera and
/// every offscreen review of it cannot disagree about where the camera is.
pub fn orbit_direction(yaw: f32, pitch: f32) -> Vec3 {
    Vec3::new(pitch.cos() * yaw.sin(), pitch.sin(), pitch.cos() * yaw.cos())
}

/// The eye the garage rests at: the hero framing applied to the turntable pivot.
pub fn hero_orbit_eye() -> Vec3 {
    hangar_camera_pivot() + orbit_direction(HERO_ORBIT_YAW, HERO_ORBIT_PITCH) * HERO_ORBIT_DISTANCE
}

/// The hero boom for `kind` (F3): [`HERO_ORBIT_DISTANCE`] was framed on the T-54's silhouette,
/// and a Jagdtiger at the same 15 m runs its barrel off the right edge of the frame (seen on
/// the first heavy-fleet renders). The span the lens must hold is derived from the SPEC — hull
/// length plus the stock barrel, the same numbers the hitbox and the module catalog already
/// carry — normalized to the T-54 the framing was designed on. Never closer than the designed
/// boom, and never past the wall clamp the live orbit flies.
pub fn hero_orbit_boom_for(kind: game_core::VehicleKind) -> f32 {
    let boom = HERO_ORBIT_DISTANCE * hero_span_m(kind) / HERO_FRAMED_SPAN_M;
    boom.clamp(HERO_ORBIT_DISTANCE, max_orbit_boom(HERO_ORBIT_YAW, HERO_ORBIT_PITCH))
}

/// The span the hero lens must hold for `kind`: hull length plus the stock barrel, from the
/// SPEC — the same numbers the hitbox and the module catalog already carry.
pub fn hero_span_m(kind: game_core::VehicleKind) -> f32 {
    2.0 * kind.spec().hitbox.half_length_m + kind.stock_barrel_length_m()
}

/// The silhouette span [`HERO_ORBIT_DISTANCE`]'s framing was DESIGNED to hold: the benchmark
/// vehicle's hull length plus its stock barrel. A literal, because vehicles are data and the
/// world layer names none (the `vehicle_dispatch` ratchet); the review set's
/// `the_hero_framed_span_is_the_benchmarks_own` lock pins this to the benchmark's own
/// spec-derived span, so the number cannot drift from the data it summarizes.
pub const HERO_FRAMED_SPAN_M: f32 = 11.885;

/// [`hero_orbit_eye`] at the per-vehicle boom: the same bearing, backed off far enough that
/// THIS vehicle's whole silhouette — gun included — stays in the hero frame.
pub fn hero_orbit_eye_for(kind: game_core::VehicleKind) -> Vec3 {
    hangar_camera_pivot()
        + orbit_direction(HERO_ORBIT_YAW, HERO_ORBIT_PITCH) * hero_orbit_boom_for(kind)
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
/// 30 m still contains the A1 nave under every garage rig: the room's farthest point from the
/// turntable is the ridge-height gate corner at √(11² + 22² + 11.3²) ≈ 27.1 m (the containment
/// test projects the RIDGE corners, not just the chord height — a corner list that stopped at
/// the eaves would under-measure the roof silently). It may not GROW: at the shipped 2048² map
/// the texel is 29.3 mm against the test's 30 mm ceiling — 0.7 mm of headroom, total.
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

/// Interior of the hangar shell as `(half_x, half_z, height)`. Used by the CLIENT camera
/// invariant test to prove the clamped orbit stays inside the room — cross-crate now, so
/// it cannot hide behind cfg(test). Height is the truss chord, not the ridge: the camera's
/// headroom question is "when do I hit steel", and the chords come first.
pub fn hangar_interior() -> (f32, f32, f32) {
    (HALF_X, HALF_Z, WALL_HEIGHT)
}

/// How much wall the eye keeps between itself and every shell plane, at any boom.
const ORBIT_WALL_MARGIN_M: f32 = 0.4;
/// The eye's ceiling: a metre under the truss chords, so the crane girder (riding just below
/// them) never crosses the lens at full pitch.
const ORBIT_CEILING_M: f32 = WALL_HEIGHT - 1.0;

/// The longest boom the orbit camera may extend at this yaw/pitch before the eye leaves the
/// hall — the A1 nave is 22 m across and the long-axis pull-back is 21 m+, so a single "fits
/// at every angle" range would surrender the depth the rectangle was built for. The invariant
/// moves instead: the EYE never enters a wall, and this is the one place that arithmetic lives
/// (decision 2026-08-09; the client clamps its boom with it, and the client's camera test
/// sweeps it against [`hangar_interior`]).
pub fn max_orbit_boom(yaw: f32, pitch: f32) -> f32 {
    let pivot = hangar_camera_pivot();
    let dir = orbit_direction(yaw, pitch);
    // Distance to each shell plane along the ray, for the planes the ray actually approaches.
    let mut boom = f32::INFINITY;
    let mut clip = |room: f32, component: f32| {
        if component > 1.0e-5 {
            boom = boom.min(room / component);
        }
    };
    clip(HALF_X - ORBIT_WALL_MARGIN_M - pivot.x * dir.x.signum(), dir.x.abs());
    clip(HALF_Z - ORBIT_WALL_MARGIN_M - pivot.z * dir.z.signum(), dir.z.abs());
    clip(ORBIT_CEILING_M - pivot.y, dir.y);
    clip(pivot.y - ORBIT_WALL_MARGIN_M, -dir.y);
    boom
}

/// The sawtooth roof, in plan. Each shed is one tooth along the long axis: a solid deck
/// climbing from the eaves ([`WALL_HEIGHT`]) to the ridge ([`SHED_RIDGE`]) over
/// [`SHED_DECK_RUN`] metres of z, then a GLAZING band — a REAL opening in the envelope, framed
/// by mullions — falling back to the eaves over [`SHED_GLAZE_RUN`] metres. Three full teeth;
/// flat aprons at both ends close the roof to the end walls (which is what lets the end walls
/// stop at the eaves with no gable geometry).
///
/// The middle shed is the SUN shed, placed by physics: the `garage_hero` key direction leaves
/// the turntable centre through its glazing plane at z ≈ 4.6, x ≈ −2.8 (see the derivation on
/// the key in `lighting.rs`), so the beam genuinely falls on the hero — a real beam, a real
/// contact shadow on the deck (locked by
/// `the_workshop_sun_reaches_the_turntable_through_a_real_opening`).
const SHED_STARTS: [f32; 3] = [-16.5, -5.5, 5.5];
/// Solid climbing deck of each tooth, in plan metres along z.
const SHED_DECK_RUN: f32 = 6.5;
/// Open glazed fall of each tooth, in plan metres along z.
const SHED_GLAZE_RUN: f32 = 4.5;

/// Height of the roof envelope over a given z: the deck's climb, the glazing's fall, or the
/// eaves on the end aprons. The containment/geometry tests read the envelope through this, so
/// the roof profile has one author.
pub(super) fn roof_envelope_y(z: f32) -> f32 {
    for start in SHED_STARTS {
        let local = z - start;
        if (0.0..SHED_DECK_RUN).contains(&local) {
            return WALL_HEIGHT + (SHED_RIDGE - WALL_HEIGHT) * (local / SHED_DECK_RUN);
        }
        if (SHED_DECK_RUN..SHED_DECK_RUN + SHED_GLAZE_RUN).contains(&local) {
            let fall = (local - SHED_DECK_RUN) / SHED_GLAZE_RUN;
            return SHED_RIDGE - (SHED_RIDGE - WALL_HEIGHT) * fall;
        }
    }
    WALL_HEIGHT
}

/// How much of the roof plane is open sky, as a fraction of its plan area.
///
/// The room's own answer to "what is overhead", and the reason it is a function rather than a
/// number: `SceneLighting::garage_hero`'s `sky_zenith_rgb` is what every polished surface in
/// the hall reflects upward, and it has to be this fraction of the daylight behind the
/// openings ([`INTERIOR_BACKGROUND`]) or the reflection disagrees with the roof it is a
/// reflection of. `the_rooms_reflection_is_the_room` holds the two together across the crate
/// boundary that stops the profile computing it directly.
pub fn skylight_open_fraction() -> f32 {
    let open = SHED_STARTS.len() as f32 * SHED_GLAZE_RUN * (2.0 * HALF_X);
    open / ((2.0 * HALF_X) * (2.0 * HALF_Z))
}

/// The roof: sloped solid decks (one per tooth), flat aprons to both end walls, and mullions
/// across each glazing opening (they cast the honest striped shadows a glazed roof throws).
/// Through the openings the renderer's interior background shows — set to daylight sky by the
/// client, so an opening reads as sky, not void.
fn push_shed_roof(v: &mut Vec<SceneVertex>, i: &mut Vec<u32>) {
    let rise = SHED_RIDGE - WALL_HEIGHT;
    // Sloped decks: an oriented box per tooth, rotated about x so its local z CLIMBS the run
    // (R_x(θ) maps local +z to (0, −sin θ, cos θ), so climbing toward +z takes −θ).
    let deck_angle = -rise.atan2(SHED_DECK_RUN);
    let deck_len = (SHED_DECK_RUN * SHED_DECK_RUN + rise * rise).sqrt();
    for start in SHED_STARTS {
        push_oriented_box(
            v,
            i,
            Vec3::new(0.0, (WALL_HEIGHT + SHED_RIDGE) / 2.0 + SLAB, start + SHED_DECK_RUN / 2.0),
            Vec3::new(HALF_X, SLAB, deck_len / 2.0),
            Mat3::from_rotation_x(deck_angle),
            ROOF,
        );
    }
    // Flat aprons from the outermost teeth to the end walls.
    let apron_low = (SHED_STARTS[0] - (-HALF_Z)) / 2.0;
    slab(v, i, [0.0, WALL_HEIGHT + SLAB, -HALF_Z + apron_low], [HALF_X, SLAB, apron_low], ROOF);
    let last_glaze_end = SHED_STARTS[2] + SHED_DECK_RUN + SHED_GLAZE_RUN;
    let apron_high = (HALF_Z - last_glaze_end) / 2.0;
    slab(v, i, [0.0, WALL_HEIGHT + SLAB, HALF_Z - apron_high], [HALF_X, SLAB, apron_high], ROOF);
    // Mullions across each glazing plane: bars spanning the width every ~1 m of fall, and
    // vertical divisions every 3.4 m of width — thin steel in the opening, phased so the sun
    // lock's guaranteed-clear centre ray (crossing at z ≈ start + 7.6 mid-shed) misses them.
    let glaze_angle = rise.atan2(SHED_GLAZE_RUN);
    let glaze_rot = Mat3::from_rotation_x(glaze_angle);
    let glaze_len = (SHED_GLAZE_RUN * SHED_GLAZE_RUN + rise * rise).sqrt();
    for start in SHED_STARTS {
        let glaze_mid_z = start + SHED_DECK_RUN + SHED_GLAZE_RUN / 2.0;
        let glaze_mid_y = (WALL_HEIGHT + SHED_RIDGE) / 2.0;
        for k in [-0.256_f32, 0.018, 0.291] {
            // Bars across the width, riding the glazing plane at fractions of its fall. The
            // phases sit at the MIDPOINTS between the sun lock's ray-row crossings (the rows
            // cross this plane near z − start ≈ 8.0, 9.2, 10.5, 11.7), so every bar keeps
            // ≥ 0.6 m clear of every row.
            let along = k * glaze_len;
            let offset = glaze_rot * Vec3::new(0.0, 0.0, along);
            push_oriented_box(
                v,
                i,
                Vec3::new(0.0, glaze_mid_y + offset.y, glaze_mid_z + offset.z),
                Vec3::new(HALF_X - 0.2, 0.05, 0.09),
                glaze_rot,
                TRUSS,
            );
        }
        // Vertical divisions along the slope. The sun lock's ray fan sweeps the lane
        // x ∈ (−5.7, 1.7) as it crosses the glazing, so every division stands outside it.
        for x in [-8.8_f32, -6.3, 3.2, 6.2, 9.2] {
            push_oriented_box(
                v,
                i,
                Vec3::new(x, glaze_mid_y, glaze_mid_z),
                Vec3::new(0.06, 0.05, glaze_len / 2.0),
                glaze_rot,
                TRUSS,
            );
        }
    }
}

/// The GLASS (Światło służy czołgowi, user direction 2026-08-10): every glazing bay-segment
/// carries a dirty pane — except the [`BROKEN_PANES`], which stay open with a shard fringe
/// on the frame. Visible sun shafts hang from those holes and ONLY those: clean glazing
/// diffuses, an aperture beams. Pushed after the shell's blanket finish so the panes keep
/// their own [`Finish::GLASS`] role — the role the light-passing rules key on (the shadow
/// caster cut and the sun-reach locks both treat GLASS as what it is: a thing light
/// crosses, not a thing that stops it).
fn push_glazing(v: &mut Vec<SceneVertex>, i: &mut Vec<u32>) {
    let (glaze_rot, glaze_len) = glaze_plane();
    let glaze_mid_y = (WALL_HEIGHT + SHED_RIDGE) / 2.0;
    let bay_edges = [-10.8_f32, -8.8, -6.3, 3.2, 6.2, 9.2, 10.8];
    let seg_edges = [-0.5_f32, -0.256, 0.018, 0.291, 0.5];
    let start_index = v.len();
    for start in SHED_STARTS {
        let glaze_mid_z = start + SHED_DECK_RUN + SHED_GLAZE_RUN / 2.0;
        for bay in bay_edges.windows(2) {
            let (x_center, x_half) = ((bay[0] + bay[1]) / 2.0, (bay[1] - bay[0]) / 2.0);
            for seg in seg_edges.windows(2) {
                let frac_center = (seg[0] + seg[1]) / 2.0;
                let broken = BROKEN_PANES.iter().any(|pane| {
                    (pane.shed_start - start).abs() < 0.1
                        && (pane.x_center - x_center).abs() < 0.2
                        && (pane.along_frac - frac_center).abs() < 0.05
                });
                let along_center = frac_center * glaze_len;
                let along_half = (seg[1] - seg[0]) / 2.0 * glaze_len;
                let offset = glaze_rot * Vec3::new(0.0, 0.0, along_center);
                let center = Vec3::new(x_center, glaze_mid_y + offset.y, glaze_mid_z + offset.z);
                if !broken {
                    push_oriented_box(
                        v,
                        i,
                        center,
                        Vec3::new(x_half - 0.05, 0.015, along_half - 0.05),
                        glaze_rot,
                        GLAZE_PANE,
                    );
                    continue;
                }
                // The shard fringe: slim jagged remnants along the frame edges — the honest
                // witness of the break, readable from the floor 9 m below.
                for (dx, da, half_x, yaw) in [
                    (-x_half + 0.35, -along_half + 0.22, 0.32, 0.35_f32),
                    (x_half - 0.4, -along_half + 0.3, 0.28, -0.5),
                    (-x_half + 0.55, along_half - 0.25, 0.24, 0.7),
                    (x_half - 0.3, along_half - 0.2, 0.3, -0.3),
                ] {
                    push_oriented_box(
                        v,
                        i,
                        center + glaze_rot * Vec3::new(dx, 0.0, da),
                        Vec3::new(half_x, 0.012, 0.16),
                        glaze_rot * Mat3::from_rotation_y(yaw),
                        SHARD_GLASS,
                    );
                }
            }
        }
    }
    finish(&mut v[start_index..], Finish::GLASS);
}

/// The hall is static and its GI bake is honest work (a hemisphere of rays per vertex), so the
/// whole build runs once per process and callers take a copy of the cached result.
/// Everything one hall build produces: the render mesh (vertices, indices) and the hero
/// probe's six-face irradiance cube, baked in the same pass over the same BVH.
type BakedHall = (Vec<SceneVertex>, Vec<u32>, [[f32; 3]; 6], super::hangar_bake::ReflectionCube);
/// One baked hall per daylight variant (H1) — same geometry, its own key, grade and GI.
static MESHES: [std::sync::OnceLock<BakedHall>; 3] =
    [std::sync::OnceLock::new(), std::sync::OnceLock::new(), std::sync::OnceLock::new()];
/// Guards [`prewarm`] so the worker is spawned once however many times it is asked for.
static PREWARM: std::sync::Once = std::sync::Once::new();

/// The hall's daylight (H1): what the day outside is doing, and therefore what the key, the
/// backdrop, the shafts and the whole GI bake are doing. Selection is the PLAYER'S OWN CLOCK
/// (plus a manual override) on the client; every review artifact pins [`HangarLight::Day`],
/// the canonical variant the goldens lock — the others answer to value-structure tests, not
/// to a tripled golden set.
///
/// Append-only: the bake cache and the saved override index by position.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HangarLight {
    /// Early light: the sun is on the WRONG side of the sheds (their glazing faces one way —
    /// the standing artistic license), so no direct blade enters; the hall wakes on sky glow
    /// and its own lamps.
    Morning,
    /// The canonical workshop day every golden locks. This variant IS `garage_hero()`.
    Day,
    /// A low warm sun through the same glazing fans: longer shafts, warmer backdrop, dusk.
    Evening,
}

impl HangarLight {
    pub const ALL: [HangarLight; 3] =
        [HangarLight::Morning, HangarLight::Day, HangarLight::Evening];

    fn index(self) -> usize {
        match self {
            HangarLight::Morning => 0,
            HangarLight::Day => 1,
            HangarLight::Evening => 2,
        }
    }
}

/// Build the static hangar mesh (the canonical [`HangarLight::Day`]). The tank is parked at
/// the origin on top of the turntable (`TURNTABLE_TOP_M`), so place the parked vehicle's
/// `position.y` at that height.
///
/// **This blocks until the hall exists**, which is why [`prewarm`] is called at startup: the
/// build is seconds of work, and inside the frame that opens the garage those seconds are a
/// hitch — never call it there.
pub fn hangar_scene_mesh() -> (Vec<SceneVertex>, Vec<u32>) {
    hangar_scene_mesh_for(HangarLight::Day)
}

/// [`hangar_scene_mesh`] under a chosen daylight (H1).
pub fn hangar_scene_mesh_for(light: HangarLight) -> (Vec<SceneVertex>, Vec<u32>) {
    let (vertices, indices, _, _) = baked_hall(light);
    (vertices.clone(), indices.clone())
}

fn baked_hall(light: HangarLight) -> &'static BakedHall {
    MESHES[light.index()].get_or_init(|| build_hangar_scene_mesh_for(light))
}

/// The hero probe baked beside the mesh (B2): the hall's bounced light at the station as a
/// six-axis irradiance cube (±x, ±y, ±z), the term the vehicle shader adds so the hero
/// finally receives the room's GI (G5). Blocks with the mesh; `prewarm` covers both.
pub fn hangar_hero_probe() -> [[f32; 3]; 6] {
    hangar_hero_probe_for(HangarLight::Day)
}

/// [`hangar_hero_probe`] under a chosen daylight (H1).
pub fn hangar_hero_probe_for(light: HangarLight) -> [[f32; 3]; 6] {
    baked_hall(light).2
}

/// The reflection cubemap baked beside the mesh (D1): the room as seen from the hero station,
/// prefiltered per mip. The garage swap uploads it as the interior environment cube; the
/// reflection lock samples it directly — the room's reflection IS this data, both ways.
pub fn hangar_reflection_cube() -> super::hangar_bake::ReflectionCube {
    hangar_reflection_cube_for(HangarLight::Day)
}

/// [`hangar_reflection_cube`] under a chosen daylight (H1).
pub fn hangar_reflection_cube_for(light: HangarLight) -> super::hangar_bake::ReflectionCube {
    baked_hall(light).3.clone()
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
        // H1: the worker bakes the CANONICAL day first (the variant the first garage entry
        // almost always wants), then the other two — a variant the clock lands on before its
        // bake finishes just waits in `get_or_init`, the same contract as before.
        let _ = std::thread::Builder::new().name("hangar-bake".to_string()).spawn(|| {
            for light in [HangarLight::Day, HangarLight::Morning, HangarLight::Evening] {
                baked_hall(light);
            }
        });
    });
}

/// Whether the canonical hall is already in the cache — the prewarm's observable, so a caller
/// can tell "the worker finished" from "the worker is still going" without blocking.
pub fn is_baked() -> bool {
    MESHES[HangarLight::Day.index()].get().is_some()
}

/// The uncached build, per daylight (H1): identical geometry, the variant's own rig through
/// the bake. `pub(crate)` so the bake's own tests can run it twice: through
/// [`hangar_scene_mesh`] they cannot, because the `OnceLock` hands both calls the same value —
/// which is exactly how `the_bake_is_deterministic` came to compare a clone with itself.
pub(crate) fn build_hangar_scene_mesh_for(light: HangarLight) -> BakedHall {
    let (mut v, mut i) = hangar_geometry(true);
    // Hala 2.0 T1: subdivide for bake resolution and gather one bounce of light into the
    // bounce lane (see hangar_bake.rs). After the corner shade, so the bake reads final
    // albedos; cached by `hangar_scene_mesh` because the gather is real work. B2 gathers the
    // hero probe in the same pass; D1 gathers the reflection cubemap — one BVH, one bake.
    let (probe, cube) =
        super::hangar_bake::bake_bounce_lane(&mut v, &mut i, &hangar_lighting(light));
    (v, i, probe, cube)
}

/// The hall's geometry before the bounce bake — one function, so the probe's ablation variant
/// (`furnished = false`, which skips EXACTLY the gallery and the props) stays a strict subset
/// of the shipped hall by construction rather than by parallel maintenance. Geometry is
/// identical across daylights (H1: "same geometry, its own rig"), so no variant enters here.
fn hangar_geometry(furnished: bool) -> (Vec<SceneVertex>, Vec<u32>) {
    let mut v = Vec::new();
    let mut i = Vec::new();

    // Shell: floor, shed roof, four walls. The lower walls are gunmetal; a near-black upper band
    // above the doorway line lets the roof fall into shadow so the lit bay reads as the subject.
    // The lower/upper bands abut exactly at `WALL_SEAM` and the upper band runs to the roof, so
    // no two wall faces are ever coplanar (which would z-fight) and there is no gap upward. The
    // SIDE walls climb to the ridge — they are the sheds' flanks — while the END walls stop at
    // the eaves, where the flat roof aprons meet them.
    let lower_c = WALL_SEAM / 2.0;
    let lower_h = WALL_SEAM / 2.0;
    let side_upper_c = (WALL_SEAM + SHED_RIDGE) / 2.0;
    let side_upper_h = (SHED_RIDGE - WALL_SEAM) / 2.0;
    let end_upper_c = (WALL_SEAM + WALL_HEIGHT) / 2.0;
    let end_upper_h = (WALL_HEIGHT - WALL_SEAM) / 2.0;
    // The floor is a poured slab and everything above it is sprayed sheet: two materials, and
    // the treatments that go with them (see `Finish`). Sealed concrete carries a faint sheen
    // that catches the worklight pools; the painted panels a satin step over it.
    let floor_start = v.len();
    slab(&mut v, &mut i, [0.0, -SLAB, 0.0], [HALF_X, SLAB, HALF_Z], CONCRETE);
    finish(&mut v[floor_start..], Finish::CONCRETE);
    let shell_steel = v.len();
    push_shed_roof(&mut v, &mut i);
    // Upper bands and roof: sprayed sheet, falling into shadow. The gate wall's upper band
    // joins them here — its LOWER pieces live in `push_gate_wall` with the whitewash.
    slab(&mut v, &mut i, [0.0, end_upper_c, HALF_Z], [HALF_X, end_upper_h, SLAB], UPPER_WALL);
    slab(&mut v, &mut i, [0.0, end_upper_c, -HALF_Z], [HALF_X, end_upper_h, SLAB], UPPER_WALL);
    for cx in [-HALF_X, HALF_X] {
        slab(&mut v, &mut i, [cx, side_upper_c, 0.0], [SLAB, side_upper_h, HALF_Z], UPPER_WALL);
    }
    finish(&mut v[shell_steel..], Finish::PAINTED_STEEL);
    // Lower bands: WHITEWASHED (C2) — lime over the sheet up to the seam, the coat that
    // bounces the worklight back into the room. The stores end wall in one piece; the GATE
    // end wall in pieces around its real opening (`push_gate_wall`).
    let whitewash_start = v.len();
    slab(&mut v, &mut i, [0.0, lower_c, HALF_Z], [HALF_X, lower_h, SLAB], WHITEWASH_WALL);
    push_gate_wall(&mut v, &mut i);
    for cx in [-HALF_X, HALF_X] {
        slab(&mut v, &mut i, [cx, lower_c, 0.0], [SLAB, lower_h, HALF_Z], WHITEWASH_WALL);
    }
    finish(&mut v[whitewash_start..], Finish::WHITEWASH);
    // Everything past the bare shell is "furniture" and takes the corner-shade bake at the end.
    let furniture_start = v.len();

    // Panel joints: recessed vertical lines rhythm the lower band into sheet-steel panels, and a
    // girt rail rides the band seam — a wall of PANELS, not one smeared plate.
    let joint_h = WALL_SEAM / 2.0 - 0.05;
    for step in -7i32..=7 {
        let along = step as f32 * 2.8;
        for sign in [-1.0_f32, 1.0] {
            let wall = sign * (HALF_X - SLAB - 0.03);
            slab(&mut v, &mut i, [wall, joint_h, along], [0.035, joint_h, 0.05], PANEL_JOINT);
        }
    }
    for step in -3i32..=3 {
        let along = step as f32 * 2.8;
        // The stores end wall takes its joints across the width; on the gate wall only the
        // panels flanking the opening carry one — a joint floating in the gateway is no joint.
        slab(
            &mut v,
            &mut i,
            [along, joint_h, HALF_Z - SLAB - 0.03],
            [0.05, joint_h, 0.035],
            PANEL_JOINT,
        );
        if along.abs() > GATE_HALF_W + 0.5 {
            slab(
                &mut v,
                &mut i,
                [along, joint_h, -(HALF_Z - SLAB - 0.03)],
                [0.05, joint_h, 0.035],
                PANEL_JOINT,
            );
        }
    }
    for (cx, cz, hx, hz) in [
        (0.0, -(HALF_Z - SLAB - 0.05), HALF_X - 0.3, 0.05_f32),
        (0.0, HALF_Z - SLAB - 0.05, HALF_X - 0.3, 0.05),
        (-(HALF_X - SLAB - 0.05), 0.0, 0.05, HALF_Z - 0.3),
        (HALF_X - SLAB - 0.05, 0.0, 0.05, HALF_Z - 0.3),
    ] {
        slab(&mut v, &mut i, [cx, WALL_SEAM, cz], [hx, 0.07, hz], GIRT);
    }

    // C2: the yellow safety band at hip height along the whitewashed walls — the working
    // hall's one warning colour on the walls (the extinguishers keep their red monopoly).
    // Proud of the panel joints so the painted band rides OVER them, the way a real one is
    // rolled straight across every joint in its path; the gate wall carries it only on the
    // flanks, clear of the opening.
    let stripe = v.len();
    for sign in [-1.0_f32, 1.0] {
        let wall = sign * (HALF_X - SLAB - 0.075);
        slab(&mut v, &mut i, [wall, 1.0, 0.0], [0.02, 0.09, HALF_Z - 0.4], MARKING);
    }
    slab(&mut v, &mut i, [0.0, 1.0, HALF_Z - SLAB - 0.075], [HALF_X - 0.4, 0.09, 0.02], MARKING);
    let flank_half = (HALF_X - GATE_HALF_W) / 2.0 - 0.3;
    for sign in [-1.0_f32, 1.0] {
        slab(
            &mut v,
            &mut i,
            [sign * (GATE_HALF_W + flank_half + 0.3), 1.0, -(HALF_Z - SLAB - 0.075)],
            [flank_half, 0.09, 0.02],
            MARKING,
        );
    }
    finish(&mut v[stripe..], Finish::PAINT_MARK);

    // Vertical wall ribs (pilasters) proud of the side and end walls, spaced as fractions of
    // each wall's own length so they stay evenly distributed.
    let rib_c = WALL_HEIGHT / 2.0;
    let rib_h = WALL_HEIGHT / 2.0 - 0.4;
    for k in [-0.8_f32, -0.4, 0.0, 0.4, 0.8] {
        let z = k * HALF_Z;
        slab(&mut v, &mut i, [-(HALF_X - 0.2), rib_c, z], [0.12, rib_h, 0.35], RIB);
        slab(&mut v, &mut i, [HALF_X - 0.2, rib_c, z], [0.12, rib_h, 0.35], RIB);
    }
    for k in [-0.8_f32, 0.8] {
        slab(&mut v, &mut i, [k * HALF_X, rib_c, -(HALF_Z - 0.2)], [0.35, rib_h, 0.12], RIB);
    }

    // DEEP roof trusses: real frames under the shed decks — parallel chords a metre apart,
    // verticals and falling diagonals between them.
    // Two frames per tooth, on the deck run where the envelope leaves them headroom;
    // they read as dark structure against the glazing daylight behind them.
    for start in SHED_STARTS {
        for local in [3.25_f32, 6.0] {
            let z = start + local;
            let depth = (roof_envelope_y(z) - 0.25 - (WALL_HEIGHT + 0.14)).clamp(0.7, 1.1);
            push_truss_frame(&mut v, &mut i, z, depth);
        }
    }

    // The bay gate the tank rolled in through: a framed, segmented steel door at the end of
    // the long axis, standing AJAR — its slat stack raised, real daylight standing in the gap
    // under it. The gate explains the drive-in; the opening explains the light on the lane.
    push_bay_gate(&mut v, &mut i);
    // The curtain at rest joins the PARKED mesh so the bake sees a closed-to-ajar gate (the
    // slats occlude bounce rays and stand in the reflection cube) and the honesty locks keep
    // a slat band that blocks the eye. The render paths split it back out
    // (`hangar_scene_mesh_without_gate`) and animate it through the dynamic-mesh slot.
    let (slat_v, slat_i) = bay_gate_slats(GATE_AJAR_M);
    let slat_base = v.len() as u32;
    v.extend(slat_v);
    i.extend(slat_i.iter().map(|idx| idx + slat_base));
    // Joints, girt, ribs, trusses and gate are all the same stock the walls are: rolled sheet
    // and section, primed and sprayed with them.
    finish(&mut v[furniture_start..], Finish::PAINTED_STEEL);
    // The glazing panes go in AFTER the blanket stamps so they keep their GLASS role — the
    // light-passing semantics the caster cut and the sun locks read.
    push_glazing(&mut v, &mut i);

    // Floor: expansion joints score the slab into cast bays; the drive lane in from the gate is
    // worn a step darker with two track-polished strips — the floor tells the room's story.
    let floor_dressing = v.len();
    for step in -6i32..=6 {
        let along = step as f32 * 3.4;
        slab(&mut v, &mut i, [0.0, 0.003, along], [HALF_X - 0.4, 0.003, 0.03], FLOOR_JOINT);
    }
    for step in -3i32..=3 {
        let along = step as f32 * 2.8;
        slab(&mut v, &mut i, [along, 0.003, 0.0], [0.03, 0.003, HALF_Z - 0.4], FLOOR_JOINT);
    }
    {
        let lane_center = -(HALF_Z + TURNTABLE_RADIUS_M) / 2.0;
        let lane_half = (HALF_Z - TURNTABLE_RADIUS_M) / 2.0;
        slab(&mut v, &mut i, [0.0, 0.0015, lane_center], [2.6, 0.0015, lane_half], DRIVE_LANE);
        for x in [-1.35_f32, 1.35] {
            slab(&mut v, &mut i, [x, 0.0025, lane_center], [0.55, 0.0015, lane_half], TRACK_WEAR);
        }
        // ...and the wear runs THROUGH the station (A2): the lane continues past the
        // turntable down the axis toward the working end. This hall serviced vehicles before
        // this one — the floor says so, and the through-line is what makes the ring read as a
        // station ON a route rather than a pedestal at a dead end.
        let through_end = 12.0;
        let through_c = (TURNTABLE_RADIUS_M + through_end) / 2.0;
        let through_half = (through_end - TURNTABLE_RADIUS_M) / 2.0;
        slab(&mut v, &mut i, [0.0, 0.0015, through_c], [2.6, 0.0015, through_half], DRIVE_LANE);
        for x in [-1.35_f32, 1.35] {
            slab(&mut v, &mut i, [x, 0.0025, through_c], [0.55, 0.0015, through_half], TRACK_WEAR);
        }
    }
    // Scored, worn and polished, but still the same slab — one material, and the story is the
    // albedo the builders above already carry.
    finish(&mut v[floor_dressing..], Finish::CONCRETE);

    // Parking-bay markings flanking the turntable, flush with the floor.
    let markings = v.len();
    for x in [-6.8_f32, 6.8] {
        slab(&mut v, &mut i, [x, 0.004, 0.0], [0.14, 0.005, 8.0], MARKING);
    }
    finish(&mut v[markings..], Finish::PAINT_MARK);

    // Turntable: a plate assembly SUNK INTO the slab (A2) — the rim is a recessed annulus
    // ring around the deck (the visible groove between plate and poured floor), the deck a
    // 2 cm machined plate, radial seams and a centre hub on top of it. Machinery in the
    // floor, not a podium under the vehicle. (No faked shadow disc — the hero vehicle casts
    // a real contact shadow here.)
    let rim_start = v.len();
    push_cylinder(
        &mut v,
        &mut i,
        Vec3::ZERO,
        TURNTABLE_RADIUS_M + 0.35,
        TURNTABLE_TOP_M - 0.012,
        48,
        TURNTABLE_RIM,
    );
    finish(&mut v[rim_start..], Finish { gloss: 0.2, ..Finish::MACHINED_STEEL });
    let deck_start = v.len();
    push_cylinder(&mut v, &mut i, Vec3::ZERO, TURNTABLE_RADIUS_M, TURNTABLE_TOP_M, 48, TURNTABLE);
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
    push_cylinder(&mut v, &mut i, Vec3::ZERO, 0.9, TURNTABLE_TOP_M + 0.004, 24, TURNTABLE_HUB);
    // Machined deck steel: the glossiest ground plane in the hall, so the worklight pools and
    // the hero's contact shadow both read on it. Deck, seams and hub are one plate assembly and
    // take one finish.
    finish(&mut v[deck_start..], Finish { gloss: 0.35, ..Finish::MACHINED_STEEL });

    // The floor drain beside the drive lane: a recessed near-black strip under a steel
    // grating. Every third grate bar wears RUST (C2) — standing water finds worn machined
    // steel first, and the drain is where the water stands.
    let drain = v.len();
    slab(&mut v, &mut i, [3.4, 0.001, -11.0], [0.10, 0.002, 2.2], [0.10, 0.10, 0.11]);
    for step in 0..9 {
        let z = -13.0 + step as f32 * 0.5;
        let bar = if step % 3 == 2 { RUST } else { [0.16, 0.17, 0.18] };
        slab(&mut v, &mut i, [3.4, 0.004, z], [0.11, 0.003, 0.03], bar);
    }
    finish(&mut v[drain..], Finish::MACHINED_STEEL);

    if furnished {
        super::hangar_gallery::push_gallery(&mut v, &mut i);
        super::hangar_props::push_props(&mut v, &mut i);
    }

    bake_corner_shade(&mut v[furniture_start..]);

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
        let d_wall_x = ease(HALF_X - x.abs(), 1.4);
        let d_wall_z = ease(HALF_Z - z.abs(), 1.4);
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

/// Half-width of the bay gate opening in the −z end wall.
pub(super) const GATE_HALF_W: f32 = 4.6;
/// Top of the gate opening.
const GATE_TOP: f32 = 5.0;
/// How far the gate stands OPEN: the slat stack is raised this high, and under it the opening
/// is a real hole in the shell — the day outside stands in it, and its light lies on the drive
/// lane as a wedge. E3 animates the slats; A1 parks them here.
pub const GATE_AJAR_M: f32 = 1.5;

/// Where the drive-in roll starts, derived from the gate rather than dialled: hull centre one
/// hull-length inside the opening, so the tank enters FROM the gate however long the hall is.
/// (`drive_in.rs` used to hard-code −13.0 against a 36 m hall — the tank materialised five
/// metres into the room.)
pub fn drive_in_start_z() -> f32 {
    -(HALF_Z - SLAB) + 4.0
}

/// The gate end wall's LOWER pieces, built around the REAL gate opening: flanking panels
/// floor to seam and the lintel band between the gate top and the seam — whitewashed with
/// the other lower bands (C2); the caller stamps the finish. The wall's upper band is built
/// beside the other upper bands in the shell block. (The stores end wall is a whole slab;
/// this one has a hole in it.)
fn push_gate_wall(v: &mut Vec<SceneVertex>, i: &mut Vec<u32>) {
    let cz = -HALF_Z;
    let side_w = (HALF_X - GATE_HALF_W) / 2.0;
    for sign in [-1.0_f32, 1.0] {
        let cx = sign * (GATE_HALF_W + side_w);
        slab(v, i, [cx, WALL_SEAM / 2.0, cz], [side_w, WALL_SEAM / 2.0, SLAB], WHITEWASH_WALL);
    }
    let lintel_c = (GATE_TOP + WALL_SEAM) / 2.0;
    let lintel_h = (WALL_SEAM - GATE_TOP) / 2.0;
    slab(v, i, [0.0, lintel_c, cz], [GATE_HALF_W, lintel_h, SLAB], WHITEWASH_WALL);
}

/// One deep truss frame across the nave at `z`: parallel chords `depth` apart, verticals, and
/// falling diagonals — section steel with real depth, the structure the shed decks ride.
fn push_truss_frame(v: &mut Vec<SceneVertex>, i: &mut Vec<u32>, z: f32, depth: f32) {
    let chord_half = HALF_X - 0.3;
    let bottom_y = WALL_HEIGHT + 0.07;
    let top_y = bottom_y + depth;
    slab(v, i, [0.0, bottom_y, z], [chord_half, 0.07, 0.09], TRUSS);
    slab(v, i, [0.0, top_y, z], [chord_half, 0.07, 0.09], TRUSS);
    for x in [-8.0_f32, -4.0, 0.0, 4.0, 8.0] {
        slab(v, i, [x, (bottom_y + top_y) / 2.0, z], [0.05, depth / 2.0 - 0.06, 0.05], TRUSS);
    }
    // Falling diagonals between the verticals, alternating direction toward the centre.
    let diag_len = (depth * depth + 16.0).sqrt();
    for (x0, lean) in [(-6.0_f32, 1.0_f32), (-2.0, 1.0), (2.0, -1.0), (6.0, -1.0)] {
        let angle = lean * depth.atan2(4.0);
        push_oriented_box(
            v,
            i,
            Vec3::new(x0, (bottom_y + top_y) / 2.0, z),
            Vec3::new(diag_len / 2.0 - 0.1, 0.045, 0.045),
            Mat3::from_rotation_z(angle),
            TRUSS,
        );
    }
}

/// The segmented bay gate's FRAME standing in the gate wall's opening: jambs and a lintel,
/// everything proud of the wall plane so nothing z-fights. The slat curtain itself is built by
/// [`bay_gate_slats`] — E3 animates it during the drive-in, so the frame is the static half
/// and the curtain the dynamic half, the same split the exhaust fan made in E2.
fn push_bay_gate(v: &mut Vec<SceneVertex>, i: &mut Vec<u32>) {
    let wall_z = -(HALF_Z - SLAB);
    // Jambs + lintel.
    for x in [-(GATE_HALF_W + 0.25), GATE_HALF_W + 0.25] {
        slab(
            v,
            i,
            [x, GATE_TOP / 2.0 + 0.2, wall_z + 0.10],
            [0.22, GATE_TOP / 2.0 + 0.2, 0.10],
            GATE_FRAME,
        );
    }
    slab(v, i, [0.0, GATE_TOP + 0.28, wall_z + 0.10], [GATE_HALF_W + 0.47, 0.16, 0.10], GATE_FRAME);
}

/// How far the gate opens for the drive-in: clearance over the tallest hull with margin, and
/// exactly the travel at which the five-slat curtain fills its head track — the stack lip
/// (`bay_gate_slats`) is DERIVED from this, so "fully open" and "curtain fully stacked" are
/// the same position by construction.
pub const GATE_DRIVE_OPEN_M: f32 = 3.6;

/// The crane trolley's travel along its girder (K1): a slow ping-pong between the bay ends
/// with the hoist riding under it — somebody is WORKING this hall even when nothing else
/// moves. 0.32 m/s: a powered trolley creeping under load, not a carnival ride. Pure
/// function of the presentation clock, like the fan and the gate.
pub fn crane_trolley_x_at(seconds: f32) -> f32 {
    const SPEED_M_S: f32 = 0.32;
    const REACH: f32 = 6.0;
    let phase = (seconds * SPEED_M_S).rem_euclid(4.0 * REACH);
    // Triangle wave: 0 -> +R -> 0 -> -R -> 0.
    if phase < REACH {
        phase
    } else if phase < 3.0 * REACH {
        2.0 * REACH - phase
    } else {
        phase - 4.0 * REACH
    }
}

/// The crane trolley, cable run and hook block at a moment on the presentation clock (K1).
/// Moved here from the static props (E2 gave the cable its sway; K1 gives the trolley its
/// travel): the trolley is bolted to its rail and rides it, the cable and hook hang free
/// and keep the E2 sway lane.
pub fn crane_trolley_at(seconds: f32) -> (Vec<SceneVertex>, Vec<u32>) {
    let mut v = Vec::new();
    let mut i = Vec::new();
    let beam_y = super::hangar_gallery::CRANE_GIRDER_Y;
    let x = crane_trolley_x_at(seconds);
    let z = -1.6;
    slab(&mut v, &mut i, [x, beam_y - 0.2, z], [0.5, 0.2, 0.5], [0.16, 0.17, 0.18]);
    let hanging = v.len();
    slab(&mut v, &mut i, [x, beam_y - 0.48, z], [0.03, 0.08, 0.03], [0.16, 0.17, 0.18]);
    slab(&mut v, &mut i, [x, beam_y - 0.68, z], [0.15, 0.15, 0.15], [0.24, 0.25, 0.27]);
    set_sway(&mut v[hanging..], 0.03);
    finish(&mut v, Finish::MACHINED_STEEL);
    (v, i)
}

/// The welding bay's screen corner (K1): where the spark fountain rises from and where the
/// glow quads stand — behind the screen at the second bay, so the WORK is implied and the
/// light is readable (the sparks are the source). Shared by the props (screen geometry),
/// the FX emitter and the glow builder.
pub const WELDING_CORNER: [f32; 3] = [-7.2, 1.1, 14.5];

/// The welding arc's duty cycle (K1): burns `BURN_S` out of every `PERIOD_S`, phased so the
/// goldens' frozen review second (12.0) falls in the QUIET half — the locked picture stays
/// the resting hall, and the arc is a live moment. Deterministic on the presentation clock.
pub fn welding_burn_at(seconds: f32) -> bool {
    const PERIOD_S: f32 = 9.0;
    const BURN_S: f32 = 2.2;
    seconds.rem_euclid(PERIOD_S) < BURN_S
}

/// The gate's slat curtain at `open_m` metres of clear opening (E3): five constant-height
/// sheet-steel slats riding a head track. Each slat rises with the opening until it reaches
/// its stacking position under the gate top, so the curtain compresses into an overlapped
/// stack SEGMENT BY SEGMENT — the top slat parks first, the bottom one last — and closing
/// peels them off one by one. The staggering is the clamp math, not a choreography.
///
/// Rest position is `open_m == GATE_AJAR_M`: the same five-slat band the hall carried when
/// the curtain was static geometry. Depth steps OUTWARD toward the bottom slat (a sectional
/// door's lower panel rides the outermost track — it has to pass the ones already stacked),
/// which also keeps every overlapped pair on its own plane. The slats ABUT in y — behind
/// them is the open day, and a gap in front of 1.4 HDR daylight blooms into a glowing
/// stripe (seen on the first A1 render).
pub fn bay_gate_slats(open_m: f32) -> (Vec<SceneVertex>, Vec<u32>) {
    let mut v = Vec::new();
    let mut i = Vec::new();
    let wall_z = -(HALF_Z - SLAB);
    let slat_h = (GATE_TOP - GATE_AJAR_M) / 5.0;
    // The lip each stacked slat shows: at GATE_DRIVE_OPEN_M the bottom slat's risen position
    // IS its stacking clamp, so the curtain exactly fills the track at full open.
    let lip = (GATE_TOP - GATE_DRIVE_OPEN_M - slat_h) / 4.0;
    let open = open_m.clamp(GATE_AJAR_M, GATE_DRIVE_OPEN_M);
    for slat in 0..5u32 {
        let color = if slat % 2 == 0 { GATE_SLAT } else { GATE_SLAT_ALT };
        let depth = 0.045 + (4 - slat) as f32 * 0.009;
        let risen = open + slat_h * (slat as f32 + 0.5);
        let stacked = GATE_TOP - (4 - slat) as f32 * lip - slat_h / 2.0;
        slab(
            &mut v,
            &mut i,
            [0.0, risen.min(stacked), wall_z + depth],
            [GATE_HALF_W, slat_h / 2.0, 0.035],
            color,
        );
    }
    // The same stock the frame and walls are; the dynamic curtain stamps it itself (the
    // static build's furniture stamp never sees these vertices once the gate animates).
    finish(&mut v, Finish::PAINTED_STEEL);
    (v, i)
}

/// The vertex-level footprint of the gate curtain inside the parked hall mesh: the thin air
/// slice of the gate opening the slats occupy at rest. Jambs sit outside it in x, the lintel
/// above it in y, the gate wall behind it in z — measured against those pieces' literal
/// extents, and locked by `the_gate_split_removes_the_curtain_and_nothing_else`.
fn in_gate_curtain_slice(position: [f32; 3]) -> bool {
    let [x, y, z] = position;
    x.abs() < GATE_HALF_W + 0.015
        && y > GATE_AJAR_M - 0.05
        && y < GATE_TOP + 0.01
        && z > -(HALF_Z - SLAB) + 0.005
        && z < -(HALF_Z - SLAB) + 0.2
}

/// The hall's sun-shadow penumbra radius, in shadow texels (Światło służy czołgowi) — read
/// by the live garage, the golden harness and the review probe, so the played softness and
/// the locked softness are one number. The hall's tight shadow box makes a ~1.8 cm texel;
/// at radius 9 the penumbra runs ~16 cm — shade with an edge you can stand in, not a razor.
/// Locked ≥ 8 by `the_light_serves_the_tank`.
pub const HANGAR_SHADOW_SOFTNESS: f32 = 9.0;

/// The SUN-SHADOW caster set (Światło służy czołgowi): the hall's indices minus the roof
/// clutter — thin members above the wall band (truss bars, glazing mullions, crane rails,
/// lamp stems: any triangle whose shortest edge is under 0.3 m) and every emissive pane
/// (glass passes light; a pane that cast a wall's shadow was the lattice's brightest lie).
/// The user's verdict, 2026-08-10: the floor carried a printed grid of razor bars and the
/// lights played the lead over the tank. The camera and the SSAO prepass keep the FULL mesh
/// — this trims what the SUN projects, not what the eye sees; the hall is presentation and
/// the honesty doctrine's "what blocks the shell blocks the eye" governs the battlefield,
/// not the furniture's light. The deck panels, walls and floor keep casting: a few large
/// soft shapes ARE the mood.
pub fn hangar_shadow_indices() -> Vec<u32> {
    hangar_shadow_indices_for(HangarLight::Day)
}

/// [`hangar_shadow_indices`] under a chosen daylight. Filters the WITHOUT-GATE mesh — the
/// exact vertex order the render paths upload to the statics slot — so the reduced index
/// set and the uploaded vertex buffer can never disagree (the gate split compacts and
/// re-indexes its vertices; filtering the full mesh here would index garbage).
pub fn hangar_shadow_indices_for(light: HangarLight) -> Vec<u32> {
    let (vertices, indices) = hangar_scene_mesh_without_gate_for(light);
    sun_caster_indices(&vertices, &indices)
}

/// The caster filter itself, over any (vertices, indices) pair — one implementation, so the
/// probe's ablation meshes trim their sun exactly the way the shipped hall trims its own.
fn sun_caster_indices(vertices: &[SceneVertex], indices: &[u32]) -> Vec<u32> {
    let mut out = Vec::with_capacity(indices.len());
    for tri in indices.chunks_exact(3) {
        let p: [Vec3; 3] = [0, 1, 2].map(|k| Vec3::from_array(vertices[tri[k] as usize].position));
        let above_band = p.iter().all(|v| v.y > 6.5);
        let emissive = tri.iter().any(|&idx| vertices[idx as usize].color.iter().any(|c| *c > 1.0));
        let glass = tri
            .iter()
            .any(|&idx| vertices[idx as usize].surface == renderer_api::surface_role::GLASS);
        let shortest_edge =
            (p[0] - p[1]).length().min((p[1] - p[2]).length()).min((p[2] - p[0]).length());
        let thin_roof_member = above_band && shortest_edge < 0.3;
        if thin_roof_member || (above_band && (emissive || glass)) {
            continue;
        }
        out.extend_from_slice(tri);
    }
    out
}

/// The parked hall WITHOUT the gate curtain (E3): the static buffer the client and every
/// review render upload, with the slats re-emitted per frame through the dynamic-mesh slot
/// (`bay_gate_slats`) so the gate can move during the drive-in. [`hangar_scene_mesh`] stays
/// the complete parked hall — the honesty locks, the bake and the containment tests keep
/// measuring the mesh with its gate in.
pub fn hangar_scene_mesh_without_gate() -> (Vec<SceneVertex>, Vec<u32>) {
    hangar_scene_mesh_without_gate_for(HangarLight::Day)
}

/// [`hangar_scene_mesh_without_gate`] under a chosen daylight (H1).
pub fn hangar_scene_mesh_without_gate_for(light: HangarLight) -> (Vec<SceneVertex>, Vec<u32>) {
    let (vertices, indices) = hangar_scene_mesh_for(light);
    strip_gate_curtain(&vertices, &indices)
}

/// Remove the gate-curtain slats from a baked hall mesh, compacting the vertex buffer — the
/// one strip the shipped upload and the probe's ablation meshes share, so "without the gate"
/// means the same triangles everywhere.
fn strip_gate_curtain(vertices: &[SceneVertex], indices: &[u32]) -> (Vec<SceneVertex>, Vec<u32>) {
    let mut keep = vec![u32::MAX; vertices.len()];
    let mut out_v = Vec::with_capacity(vertices.len());
    let mut out_i = Vec::with_capacity(indices.len());
    for tri in indices.chunks_exact(3) {
        if tri.iter().all(|&idx| in_gate_curtain_slice(vertices[idx as usize].position)) {
            continue;
        }
        for &idx in tri {
            if keep[idx as usize] == u32::MAX {
                keep[idx as usize] = out_v.len() as u32;
                out_v.push(vertices[idx as usize]);
            }
            out_i.push(keep[idx as usize]);
        }
    }
    (out_v, out_i)
}

/// PROBE-ONLY (Hala v4 P1): the hall with its gallery and props left out — same shell, same
/// fixtures, same bounce bake and the same gate-curtain strip the shipped mesh gets, so the
/// delta between this and the shipped hall prices exactly the furnishings' fill. The game
/// never uploads it; `perf_capture` renders it to attribute the garage frame. Returns the
/// mesh with its own sun-caster subset, because the shipped caster indices are built against
/// the shipped vertex buffer and would index garbage against this one.
pub fn hangar_probe_mesh_unfurnished() -> (Vec<SceneVertex>, Vec<u32>, Vec<u32>) {
    let (mut v, mut i) = hangar_geometry(false);
    let _ =
        super::hangar_bake::bake_bounce_lane(&mut v, &mut i, &hangar_lighting(HangarLight::Day));
    let (v, i) = strip_gate_curtain(&v, &i);
    let casters = sun_caster_indices(&v, &i);
    (v, i, casters)
}

/// PROBE-ONLY (Hala v4 P1): a bare concrete slab of the hall's footprint and nothing else —
/// the reference floor under the probe's "vehicle only" block, so the hero's own fill can be
/// told apart from the room's. No bake: the block prices the vehicle, not the slab's light.
pub fn hangar_probe_mesh_floor_slab() -> (Vec<SceneVertex>, Vec<u32>) {
    let mut v = Vec::new();
    let mut i = Vec::new();
    slab(&mut v, &mut i, [0.0, -SLAB, 0.0], [HALF_X, SLAB, HALF_Z], CONCRETE);
    finish(&mut v[..], Finish::CONCRETE);
    (v, i)
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

// Never stamp a sheen without naming the material: that is what leaves a vertex carrying
// `surface_role::LEGACY` while wearing a finish — a wall that reflects like steel and is shaded
// like nothing. `Finish` + `finish` are the one path, and the two lanes travel together.

/// What a hall surface is MADE OF, as the two lanes that decide how it answers light: which
/// procedural treatment dresses its albedo (`surface`) and how sharp its specular is (`gloss`).
///
/// The pair is one value because setting them apart is how the hall ended up with neither. Every
/// vertex in this room carried `surface_role::LEGACY` — measured, all 17 771 of them — so
/// `surface_treatment` never ran indoors and the whole hall wore the interior arm of
/// `material_detail`: ONE octave of ±3.5% noise, no normal perturbation, concrete and painted
/// steel and workbench timber and rubber resolving to the same flat fill. And 70.1% of those
/// vertices carried gloss 0, which skips the shader's specular block and its environment
/// reflection outright. Art-direction rule 5 asks every surface for two octaves; the garage was
/// the one environment in the game answering with none.
#[derive(Clone, Copy, PartialEq)]
pub(super) struct Finish {
    pub(super) surface: f32,
    /// Public so a one-off piece can vary the sheen of a named material without inventing a
    /// material for it: `Finish { gloss: 0.45, ..Finish::PAINTED_STEEL }` is an enamelled
    /// bottle, and it is still painted steel.
    pub(super) gloss: f32,
}

impl Finish {
    /// The poured floor slab, sealed: the largest single surface the garage camera sees.
    pub(super) const CONCRETE: Self =
        Self { surface: renderer_api::surface_role::CONCRETE, gloss: 0.08 };
    /// Sprayed sheet steel — walls, roof, ribs, trusses, the bay gate, the shop's own furniture.
    pub(super) const PAINTED_STEEL: Self =
        Self { surface: renderer_api::surface_role::PAINTED_STEEL, gloss: 0.12 };
    /// Bare or galvanised steel a machine touched: rails, the crane girder, shelving, jack
    /// stands, the turntable deck. Brighter than paint because it was never painted.
    pub(super) const MACHINED_STEEL: Self =
        Self { surface: renderer_api::surface_role::PAINTED_STEEL, gloss: 0.30 };
    /// Sawn timber — the workbench top, pallets, crates, the tool board. Takes the world's
    /// existing plank treatment rather than a garage-only copy of it: a board is a board.
    pub(super) const TIMBER: Self =
        Self { surface: renderer_api::surface_role::PLANK, gloss: 0.05 };
    /// Tyre rubber and its kin. Deliberately near-featureless and near-matte — a road wheel's
    /// tread is the one thing in this room that genuinely has no grain to catch. Its own role
    /// since C2, instead of wearing painted steel's spray tooth at zero gloss.
    pub(super) const RUBBER: Self =
        Self { surface: renderer_api::surface_role::RUBBER, gloss: 0.02 };
    /// Lime whitewash over sheet (C2): the lower wall bands. Chalky matte — lime has no sheen
    /// to give a worklamp, which is half of why it reads as a WORKING hall's coat.
    pub(super) const WHITEWASH: Self =
        Self { surface: renderer_api::surface_role::WHITEWASH, gloss: 0.04 };
    /// Floor paint and stencil: bay markings, hazard chevrons, signage. Concrete's treatment,
    /// because that is what the paint is lying on and what shows through it.
    pub(super) const PAINT_MARK: Self =
        Self { surface: renderer_api::surface_role::CONCRETE, gloss: 0.06 };
    /// Proofed duck cloth — the tarped mound in stores. Takes the world's PLASTER treatment:
    /// fine grain over half-metre blotches is exactly what folded canvas reads as, and adding a
    /// role to say the same thing would be a role for one prop.
    pub(super) const CANVAS: Self =
        Self { surface: renderer_api::surface_role::PLASTER, gloss: 0.10 };
    /// Dirty glazing (Światło służy czołgowi): high gloss — glass IS its sheen — and the
    /// role the light-passing rules key on (caster cut, sun-reach locks).
    pub(super) const GLASS: Self = Self { surface: renderer_api::surface_role::GLASS, gloss: 0.75 };
}

/// Give the vertices a builder just appended a sway amplitude (E2): hanging pieces — the
/// hoist hook, the banner, the second bay's chain — ride the same wind lane the meadow's
/// blade tips do, at centimetre amplitudes. The hall's draft has a source: the gate is ajar.
pub(super) fn set_sway(vertices: &mut [SceneVertex], sway: f32) {
    for vertex in vertices {
        vertex.sway = sway;
    }
}

/// Stamp a finish onto the vertices a builder has just appended.
///
/// The hall's established idiom — the turntable's gloss has been set by walking `v[start..]`
/// since it was built — extended to carry the material with the finish so the two cannot be set
/// apart. EMISSIVE faces are skipped, the same convention `bake_corner_shade` and the bounce
/// bake honour: a lamp face is a light, and multiplying a light by a paint treatment is a
/// category error.
pub(super) fn finish(vertices: &mut [SceneVertex], finish: Finish) {
    for vertex in vertices {
        if vertex.color.iter().any(|&c| c > 1.0) {
            continue;
        }
        vertex.surface = finish.surface;
        vertex.gloss = finish.gloss;
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

    /// Hala v4 P1: the probe's unfurnished hall is the shipped geometry minus EXACTLY the
    /// gallery and the props — a literal prefix of the same build (the two pushes it skips
    /// come last), checked pre-bake so this costs a mesh assembly, not a BVH gather.
    #[test]
    fn the_probe_ablation_hall_is_a_strict_prefix_of_the_shipped_geometry() {
        let (full_v, full_i) = hangar_geometry(true);
        let (bare_v, bare_i) = hangar_geometry(false);
        assert!(bare_i.len() < full_i.len(), "the furnishings must cost triangles");
        assert!(bare_v.len() < full_v.len(), "the furnishings must cost vertices");
        assert_eq!(&full_v[..bare_v.len()], &bare_v[..], "vertex prefix drifted");
        assert_eq!(&full_i[..bare_i.len()], &bare_i[..], "index prefix drifted");
    }

    /// Hala v4 P1: the probe's reference slab spans the hall footprint and nothing above it —
    /// a floor to park the hero on, not a second room.
    #[test]
    fn the_probe_floor_slab_covers_the_hall_footprint_and_nothing_more() {
        let (v, i) = hangar_probe_mesh_floor_slab();
        assert!(!v.is_empty() && i.len() % 3 == 0);
        assert!(i.iter().all(|&idx| (idx as usize) < v.len()));
        let max_x = v.iter().map(|p| p.position[0].abs()).fold(0.0f32, f32::max);
        let max_z = v.iter().map(|p| p.position[2].abs()).fold(0.0f32, f32::max);
        let max_y = v.iter().map(|p| p.position[1]).fold(f32::MIN, f32::max);
        assert!((max_x - HALF_X).abs() < 1.0e-4, "slab spans the hall in x: {max_x}");
        assert!((max_z - HALF_Z).abs() < 1.0e-4, "slab spans the hall in z: {max_z}");
        assert!(max_y <= 1.0e-4, "a floor, not a room: top at {max_y}");
    }

    #[test]
    fn hangar_encloses_the_parked_tank() {
        let (vertices, _) = hangar_scene_mesh();
        let pivot = hangar_camera_pivot();
        let any = |pred: fn(&[f32; 3]) -> bool| vertices.iter().any(|v| pred(&v.position));
        assert!(any(|p| p[0] < -HALF_X + 1.0), "left wall");
        assert!(any(|p| p[0] > HALF_X - 1.0), "right wall");
        assert!(any(|p| p[2] < -HALF_Z + 1.0), "gate wall");
        assert!(any(|p| p[2] > HALF_Z - 1.0), "stores wall");
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

    /// [`ray_hits_mesh`] that treats EMISSIVE triangles as transparent: glass passes light —
    /// the same principle the shadow caster cut established. The sun-reach locks ask "does
    /// the LIGHT arrive", and a dirty pane dims light without stopping it; steel stops it.
    fn ray_hits_opaque(
        origin: [f32; 3],
        dir: [f32; 3],
        vertices: &[SceneVertex],
        indices: &[u32],
    ) -> bool {
        let opaque: Vec<u32> = indices
            .chunks_exact(3)
            .filter(|tri| {
                !tri.iter().any(|&idx| {
                    let vertex = &vertices[idx as usize];
                    vertex.color.iter().any(|c| *c > 1.0)
                        || vertex.surface == renderer_api::surface_role::GLASS
                })
            })
            .flatten()
            .copied()
            .collect();
        ray_hits_mesh(origin, dir, vertices, &opaque)
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

        // The hero's centre stands in the sun, exactly. Through GLASS if glass is in the
        // way: the panes are emissive and light-passing (the same principle the shadow
        // caster cut established), so the reach test skips them — steel still blocks.
        assert!(
            !ray_hits_opaque([0.0, deck, 0.0], key, &vertices, &indices),
            "the key must reach the turntable centre unobstructed"
        );
        // Across the deck, most of the sun arrives; mullions, trusses and the lamp rig are
        // allowed to stripe it (that is what a glazed workshop roof looks like).
        let (mut clear, mut total) = (0, 0);
        for gx in -2i32..=2 {
            for gz in -2i32..=2 {
                total += 1;
                let origin = [gx as f32 * 1.5, deck, gz as f32 * 1.5];
                if !ray_hits_opaque(origin, key, &vertices, &indices) {
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

    /// EVERY SURFACE IN THIS ROOM NAMES WHAT IT IS MADE OF.
    ///
    /// The measurement that put this here: all 17 771 of the hall's vertices carried
    /// `surface_role::LEGACY`, so `surface_treatment` never ran in the garage at all and the
    /// whole room wore the interior arm of `material_detail` — ONE octave of ±3.5% noise, with
    /// `detail_normal` early-returning indoors so not even that caught light. Concrete, sprayed
    /// sheet, workbench timber, tyre rubber and canvas resolved to the same flat fill. It is the
    /// only environment in the game that shipped with no material treatment whatsoever, and
    /// art-direction rule 5 asks every surface for two octaves.
    ///
    /// Emissive faces are the one exemption, and they are exempt by construction: `finish` skips
    /// them, because a lamp face is a light and multiplying a light by a paint treatment is a
    /// category error.
    #[test]
    fn every_surface_in_the_hall_names_its_material() {
        let (vertices, _) = hangar_scene_mesh();
        let untreated: Vec<&SceneVertex> = vertices
            .iter()
            .filter(|v| v.surface == renderer_api::surface_role::LEGACY)
            .filter(|v| v.color.iter().all(|&c| c <= 1.0))
            .collect();
        assert!(
            untreated.is_empty(),
            "{} non-emissive vertices carry no surface role — they render as flat fill. First \
             at {:?}",
            untreated.len(),
            untreated[0].position
        );
        // ...and the roles it does name are a real set, not one role stamped everywhere: a
        // workshop is concrete AND sheet AND timber, and if it collapses to one the treatment
        // is decoration rather than material.
        let mut roles: Vec<u32> = vertices.iter().map(|v| v.surface as u32).collect();
        roles.sort_unstable();
        roles.dedup();
        // Raised 4 → 6 with C2: concrete, painted steel, whitewash, plank, plaster-as-canvas
        // and rubber — a workshop's real material set, not one treatment stamped everywhere.
        assert!(roles.len() >= 6, "the hall wears {} distinct materials, want 6+", roles.len());
    }

    /// AND ANSWERS LIGHT WITH A FINISH. 70.1% of the hall's vertices carried gloss 0, which
    /// makes `scene.wgsl` skip its specular block AND its environment reflection outright —
    /// measured, and the other half of why a room lit by lamps read as dead.
    ///
    /// The bound is on the share, not on every vertex: rubber genuinely has no sheen and the
    /// role exists to say so. What may not come back is a hall where most of the steel answers
    /// a worklamp with nothing.
    #[test]
    fn the_hall_answers_light_with_a_finish() {
        let (vertices, _) = hangar_scene_mesh();
        let matte = vertices.iter().filter(|v| v.gloss <= 0.001).count();
        let share = matte as f32 / vertices.len() as f32;
        assert!(
            share < 0.10,
            "{:.1}% of the hall is fully matte (was 70.1%) — that much of the room skips the \
             specular block entirely",
            share * 100.0
        );
        // The finishes are graded, not one number applied everywhere: paint, mill steel and
        // machined deck plate answer a lamp differently or the grading buys nothing.
        let mut levels: Vec<u32> =
            vertices.iter().map(|v| (v.gloss * 100.0).round() as u32).collect();
        levels.sort_unstable();
        levels.dedup();
        assert!(levels.len() >= 5, "the hall wears {} distinct finishes, want 5+", levels.len());
    }

    /// THE ROOM IS WHAT THE ROOM REFLECTS. `env_sky` is the only environment term the scene and
    /// vehicle shaders have, and indoors it is fed by the profile's two sky colours — so those
    /// two numbers ARE the hall, as far as the turntable deck, the rails and every painted
    /// panel on the hero are concerned.
    ///
    /// They were a leftover outdoor gradient: 0.12 overhead against 0.17 sideways, while the
    /// roof openings show 1.30/1.38/1.55. That is a reflection seven to ten times darker than
    /// the daylight standing above it, and ordered the wrong way round — outdoors the horizon
    /// out-lumes the zenith, under a glazed roof it cannot.
    ///
    /// This holds the profile to the ROOF: the overhead colour must be the skylights' area
    /// share of the daylight behind them, within the margin the mullions, trusses and crane
    /// girder hanging under the openings account for. Move a band, widen one, or repaint the
    /// day outside, and this fails until the reflection follows.
    #[test]
    fn the_rooms_reflection_is_the_room() {
        let rig = renderer_api::SceneLighting::garage_hero();
        let open = skylight_open_fraction();
        assert!(
            (0.15..0.45).contains(&open),
            "a roof that is nearly all glass or nearly all slab is a different room: {open}"
        );

        // Overhead: the openings' share of the day behind them, minus what hangs under them.
        // The floor is 60% of the geometric estimate — below it the reflection is darker than
        // the roof can possibly be; above it, brighter than a roof with bars across it.
        let daylight = [
            INTERIOR_BACKGROUND.0 as f32,
            INTERIOR_BACKGROUND.1 as f32,
            INTERIOR_BACKGROUND.2 as f32,
        ];
        for (channel, day) in rig.sky_zenith_rgb.iter().zip(daylight) {
            let geometric = day * open;
            assert!(
                *channel >= geometric * 0.6 && *channel <= geometric * 1.15,
                "the overhead reflection must be the skylights' share of the day: {channel} \
                 against {open:.3} x {day} = {geometric:.3}"
            );
        }

        // ...and it out-lumes the walls, which is the inverse of the outdoor rule and the whole
        // difference between a roof with holes in it and a sky.
        let luma = |c: [f32; 3]| 0.2126 * c[0] + 0.7152 * c[1] + 0.0722 * c[2];
        assert!(
            luma(rig.sky_zenith_rgb) > luma(rig.sky_horizon_rgb) * 1.5,
            "a glazed roof must out-lume the gunmetal wall: overhead {:?}, across {:?}",
            rig.sky_zenith_rgb,
            rig.sky_horizon_rgb
        );
        // The wall's reflection stays in the wall's own range: since C2 the lower bands are
        // whitewashed, so "the wall" a vertical surface sees across the bay is lime white
        // below the seam and dark sheet above — the horizon term must stay under the BRIGHT
        // band's albedo (it is a wall being seen, not a light).
        let wall = luma(WHITEWASH_WALL);
        assert!(
            luma(rig.sky_horizon_rgb) < wall,
            "the sideways reflection may not out-lume the wall's own albedo ({wall:.3})"
        );

        // D1 MADE THE LOCK LITERAL: interiors reflect the CUBEMAP now (the profile's two sky
        // colours keep only the outdoor fallback duty), so the rest of this test asks the
        // cube itself — and holds its PHYSICS, not the retired approximation. First
        // measurement recorded the difference honestly: the blurred upward texel reads 0.97
        // where the old zenith claim said 0.34, because the cube weights the cone actually
        // overhead (the sun shed's glazing) while the derivation averaged the whole roof.
        let cube = hangar_reflection_cube();
        let cube_luma =
            |sample: [f32; 4]| 0.2126 * sample[0] + 0.7152 * sample[1] + 0.0722 * sample[2];
        let up_luma = cube_luma(cube.sample(Vec3::Y, 4));
        let day_luma = luma([
            INTERIOR_BACKGROUND.0 as f32,
            INTERIOR_BACKGROUND.1 as f32,
            INTERIOR_BACKGROUND.2 as f32,
        ]);
        assert!(
            up_luma < day_luma,
            "a roof with decks in it cannot out-lume the bare day: {up_luma:.3} vs {day_luma:.3}"
        );
        // A mirror looking through the sun shed's glazing sees the DAY, not a wall: the sharp
        // mip along the known-open lane carries daylight-class radiance.
        let glazing_luma = cube_luma(cube.sample(Vec3::new(-2.4, 8.3, 4.0).normalize(), 0));
        assert!(
            glazing_luma > 0.8,
            "the glazing texels of the room's reflection must carry the day: {glazing_luma:.3}"
        );
        // ...straight down is the lit deck — a surface, present and lit, not a void...
        let down_luma = cube_luma(cube.sample(Vec3::NEG_Y, 0));
        assert!(
            down_luma > 0.02,
            "the deck under the probe must reflect as a lit surface: {down_luma:.3}"
        );
        // ...and the roofward blur out-lumes the floorward blur: the room is lit through its
        // roof, and its reflection says so.
        let down_blur = cube_luma(cube.sample(Vec3::NEG_Y, 4));
        assert!(
            up_luma > down_blur,
            "the day through the roof must out-lume the deck's return: up {up_luma:.3} vs \
             down {down_blur:.3}"
        );
    }

    /// The garage's shadow boxes pin to the turntable, and the pin is inside the hall.
    #[test]
    fn the_shadow_focus_is_the_turntable() {
        let [x, y, z] = hangar_shadow_focus();
        assert_eq!([x, z], [0.0, 0.0], "the focus is the turntable centre");
        assert!((y - TURNTABLE_TOP_M).abs() < 1.0e-6);
        assert!(x.abs() < HALF_X && z.abs() < HALF_Z && y < WALL_HEIGHT);
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
    ///
    /// MEASURED THROUGH THE SHIPPED RESOLUTION. This test used to build its params from
    /// `SunShadowParams::default()`, whose resolution is 4096, and assert a texel under 3 cm —
    /// while the game ships `LightingQuality::canonical().shadow_resolution` = 2048. So it
    /// carried 2x of headroom the player never had: 14.6 mm of tested texel against 29.3 mm of
    /// played texel, and a drop to 1024 would have left it green at 4096 while the garage got
    /// 58 mm. The containment half of the contract is resolution-independent and was always
    /// right; the SHARPNESS half was measuring a picture nobody renders.
    #[test]
    fn the_near_shadow_box_contains_the_whole_hall() {
        use glam::Mat4;
        use renderer_api::{
            LightingQuality, SceneLighting, SunShadowParams, sun_light_view_projection,
        };

        let radius = hangar_shadow_radius_m();
        let shipped = LightingQuality::canonical().shadow_resolution;
        let params = SunShadowParams {
            focus_radius_m: radius,
            resolution: shipped,
            ..SunShadowParams::default()
        };
        let focus = hangar_shadow_focus();
        // The near cascade hands a fragment to the far cascade once its UV leaves this margin
        // (CASCADE_MARGIN_UV in renderer_wgpu's shadow.rs); in NDC that is 4% of the half-box.
        const MARGIN_NDC: f32 = 1.0 - 2.0 * 0.02;

        // The extreme points of the shell: every wall corner at floor and eaves height, plus
        // the RIDGE line ends of each shed tooth — the roof rises past the eaves, and a corner
        // list that stopped at the wall height would under-measure the roof silently.
        let mut extremes: Vec<[f32; 3]> = Vec::new();
        for x in [-HALF_X, HALF_X] {
            for z in [-HALF_Z, HALF_Z] {
                extremes.push([x, 0.0, z]);
                extremes.push([x, WALL_HEIGHT, z]);
            }
            for start in SHED_STARTS {
                extremes.push([x, SHED_RIDGE, start + SHED_DECK_RUN]);
            }
        }
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
            for [x, y, z] in &extremes {
                let clip = m * glam::Vec4::new(*x, *y, *z, 1.0);
                let ndc = clip.truncate() / clip.w;
                assert!(
                    ndc.x.abs() <= MARGIN_NDC && ndc.y.abs() <= MARGIN_NDC,
                    "hall extreme ({x}, {y}, {z}) falls out of the near shadow box \
                     (ndc {ndc:?}) under key {:?} — the walls would take the far \
                     cascade and read a seam",
                    rig.key_direction
                );
            }
        }

        // And it is genuinely tighter than the battlefield box it replaces: same map, smaller
        // footprint, finer texels. Both sides measured at the SHIPPED resolution.
        let battlefield = SunShadowParams { resolution: shipped, ..SunShadowParams::default() };
        assert!(
            radius < battlefield.focus_radius_m,
            "a garage box no smaller than the battlefield's buys nothing"
        );
        let gain = battlefield.texel_world_size() / params.texel_world_size();
        assert!(
            gain >= 2.0,
            "the garage box must be worth the wiring: only {gain:.1}x finer texels"
        );
        println!(
            "GARAGE SHADOW: {:.1} mm per texel at the shipped {shipped}² map ({gain:.1}x the \
             battlefield box), one cascade",
            params.texel_world_size() * 1000.0
        );
        assert!(
            params.texel_world_size() < 0.030,
            "hangar texel {:.1} mm regressed past 30 mm",
            params.texel_world_size() * 1000.0
        );
    }

    /// THE FAR CASCADE IS REDUNDANT IN THIS ROOM, and that is a claim the geometry can carry
    /// rather than a setting somebody chose.
    ///
    /// `the_near_shadow_box_contains_the_whole_hall` proves, corner by corner and rig by rig,
    /// that nothing in the hall ever leaves the near box. A second cascade can therefore only
    /// answer questions no fragment in this room asks — and the one the garage was encoding
    /// spanned 30 x 4.5 = 135 m of half-size at half the resolution, so 264 mm per texel over a
    /// room 36 m wide, redrawing the hall and the hero's 204 draws every frame to fill it.
    ///
    /// This holds the two together: the client may drop the cascade only while the box that
    /// makes it redundant still contains the room with margin.
    #[test]
    fn one_cascade_is_enough_for_a_room_that_fits_in_its_near_box() {
        use renderer_api::SunShadowParams;
        let near = SunShadowParams {
            focus_radius_m: hangar_shadow_radius_m(),
            ..SunShadowParams::default()
        };
        // The hall's furthest point from the turntable: the ridge-height gate-end corner.
        let corner = (HALF_X * HALF_X + HALF_Z * HALF_Z + SHED_RIDGE * SHED_RIDGE).sqrt();
        assert!(
            corner < near.focus_radius_m,
            "the hall's far corner is {corner:.1} m out and the near box only reaches {:.1} m — \
             it needs the far cascade after all, and the client must stop dropping it",
            near.focus_radius_m
        );
        // ...and the cascade being dropped really was covering nothing but air.
        let far = near.far_cascade();
        assert!(
            far.focus_radius_m > corner * 2.0,
            "a far cascade this tight might have carried something: {:.0} m against a {corner:.0} m room",
            far.focus_radius_m
        );
    }

    /// The end of the drive lane is a real AJAR gate, not a glowing plate and not a mural:
    /// nothing on the gate wall below the seam runs brighter than the walls' own palette (the
    /// only hot emitters in the hall are the worklamp faces), the raised slat stack is present,
    /// and under it the opening is a genuine hole in the shell — a ray out of the hall through
    /// the gap escapes, a ray at slat height does not. The daylight wedge on the lane is the
    /// renderer's background standing in a real opening, exactly like the shed glazing.
    #[test]
    fn the_bay_gate_stands_ajar_over_a_real_opening() {
        let (vertices, indices) = hangar_scene_mesh();
        let gate_wall_glow = vertices.iter().any(|v| {
            v.position[2] < -(HALF_Z - 1.0)
                && v.position[1] < WALL_SEAM
                && v.color.iter().any(|&c| c > 0.8)
        });
        assert!(!gate_wall_glow, "the gate must read as steel, not a glowing doorway");
        // The raised stack: framed slats proud of the gate wall plane (hue-matched — the
        // corner-shade bake scales the authored colours).
        let slats = vertices
            .iter()
            .filter(|v| is_shade_of(v.color, GATE_SLAT) || is_shade_of(v.color, GATE_SLAT_ALT))
            .count();
        assert!(slats >= 5 * 24, "five framed gate slats, got {slats} vertices");
        // The opening is real: out of the hall under the stack, steel at slat height.
        let out = [0.0, 0.0, -1.0];
        assert!(
            !ray_hits_mesh([0.0, GATE_AJAR_M * 0.6, -10.0], out, &vertices, &indices),
            "under the ajar stack the gateway is open to the day"
        );
        assert!(
            ray_hits_mesh([0.0, (GATE_AJAR_M + GATE_TOP) / 2.0, -10.0], out, &vertices, &indices),
            "above the ajar line the gate is closed steel"
        );
    }

    /// A2: THE STATION IS A FLUSH RING, NOT A PODIUM. Every vertex of the turntable assembly
    /// (rim annulus, deck plate, seams, hub) stays within a hand's breadth of the slab — the
    /// 12 cm pedestal is gone, and the vehicle stands on the workshop floor, on machinery
    /// sunk into it. Locking the constant is the point (the camera lock does the same), so
    /// the constant-assertion lint is deliberately silenced.
    #[test]
    #[expect(clippy::assertions_on_constants)]
    fn the_station_is_a_flush_ring_not_a_podium() {
        let (vertices, _) = hangar_scene_mesh();
        let assembly: Vec<&SceneVertex> = vertices
            .iter()
            .filter(|v| {
                is_shade_of(v.color, TURNTABLE)
                    || is_shade_of(v.color, TURNTABLE_RIM)
                    || is_shade_of(v.color, TURNTABLE_SEAM)
                    || is_shade_of(v.color, TURNTABLE_HUB)
            })
            .collect();
        assert!(!assembly.is_empty(), "the turntable assembly exists");
        let crest = assembly.iter().map(|v| v.position[1]).fold(f32::MIN, f32::max);
        assert!(crest <= 0.05, "the station must sit flush with the slab, crest {crest}");
        // ...and the deck still leads the assembly: plate above groove, hub above plate.
        assert!(TURNTABLE_TOP_M <= 0.03, "the deck is a plate in the floor, not a stage");
    }

    /// A2: THE STATION WEARS HUMAN SCALE. In the working band around the plate (past the
    /// orbit's air column, inside ~8.5 m) stand the mechanic's cabinet, the jack stands, the
    /// hose coil and the bucket — hand-height geometry that gives the hero its sense of size.
    /// An empty ring in an empty radius reads as a render stage, not a workshop.
    #[test]
    fn the_station_wears_human_scale() {
        let (vertices, _) = hangar_scene_mesh();
        let band = vertices
            .iter()
            .filter(|v| {
                let [x, y, z] = v.position;
                let r = (x * x + z * z).sqrt();
                (5.7..8.6).contains(&r) && y > 0.05 && y < 1.3
            })
            .count();
        // Floor blessed from measurement: the four A2 pieces put 356 vertices in the band
        // (their floor-contact vertices sit under the y > 0.05 cut); 300 catches losing any
        // one piece without flaking on a reshaped drawer.
        assert!(band >= 300, "the station's working band is dressed, got {band} vertices");
    }

    /// A2: the drive lane's wear runs THROUGH the station — `DRIVE_LANE`-toned floor dressing
    /// exists on BOTH sides of the turntable along the axis, so the ring reads as a station
    /// on a route, not a pedestal at a dead end.
    #[test]
    fn the_wear_runs_through_the_station() {
        let (vertices, _) = hangar_scene_mesh();
        let lane_side = |sign: f32| {
            vertices.iter().any(|v| {
                is_shade_of(v.color, DRIVE_LANE)
                    && v.position[2] * sign > TURNTABLE_RADIUS_M
                    && v.position[1] < 0.01
            })
        };
        assert!(lane_side(-1.0), "the lane arrives from the gate");
        assert!(lane_side(1.0), "the wear continues past the station");
    }

    /// A3: EVERY SLOT FRAMING FRAMES A COMPOSED BACKGROUND. For each module shot the camera
    /// flies to, real geometry stands in the frustum BEYOND the subject and BELOW the roof
    /// line — subject matter behind the module (racks, gear stock, the second bay, the gate),
    /// not bare wall and floor. Measured through the same framing constants the live camera
    /// flies (`slot_framings`), so recomposing a shot without restaging its background fails
    /// here by name.
    #[test]
    fn every_slot_framing_frames_a_composed_background() {
        let (vertices, _) = hangar_scene_mesh();
        let half_v = (HERO_FOV_DEGREES.to_radians() / 2.0).tan();
        let half_h = half_v * 16.0 / 9.0;
        for (name, framing) in slot_framings() {
            let (eye, target) = slot_eye(framing);
            let forward = (target - eye).normalize();
            let right = forward.cross(Vec3::Y).normalize();
            let up = right.cross(forward);
            let staged = vertices
                .iter()
                .filter(|vertex| {
                    let p = Vec3::from_array(vertex.position) - eye;
                    let depth = p.dot(forward);
                    // Beyond the subject, in frame, and below the eaves line: composed
                    // BACKGROUND, not the roof and not the vehicle itself.
                    depth > framing.distance + 3.0
                        && vertex.position[1] < WALL_SEAM
                        && p.dot(right).abs() < half_h * depth
                        && p.dot(up).abs() < half_v * depth
                })
                .count();
            assert!(
                staged >= 150,
                "the {name} framing looks at an unstaged background: {staged} vertices in frame"
            );
        }
    }

    /// B2: THE HERO RECEIVES THE ROOM. The station probe carries real bounced light on every
    /// face (the hall surrounds it — no face gathers nothing), stays inside the bake's HDR
    /// envelope, and its strongest vertical face is the one looking DOWN: the sunlit floor
    /// throws more light up at the hull's belly and flanks than the dark shed decks drop on
    /// its roof — which is exactly the bounce character a room lit through its roof has.
    #[test]
    fn the_hero_probe_carries_the_halls_light() {
        let probe = hangar_hero_probe();
        let luma = |c: [f32; 3]| 0.2126 * c[0] + 0.7152 * c[1] + 0.0722 * c[2];
        for face in probe {
            assert!(
                face.iter().all(|c| c.is_finite() && (0.0..=4.0).contains(c)),
                "probe face out of the HDR envelope: {face:?}"
            );
            assert!(luma(face) > 0.0, "the hall surrounds the probe — no face is black");
        }
        assert!(
            luma(probe[3]) > luma(probe[2]),
            "the lit floor must out-bounce the dark roof: down {:?} vs up {:?}",
            probe[3],
            probe[2]
        );
    }

    /// ŚWIATŁO SŁUŻY CZOŁGOWI (user verdict 2026-08-10: the lights played the lead over the
    /// tank — a razor-bar lattice on the floor, a shouting key). Three causes, three bolts:
    /// the hall's penumbra stays real (soft-kernel radius ≥ 8 texels), the roof clutter
    /// stays out of the caster set (the reduced indices genuinely drop a large share), and
    /// the key:ambient ratio stays priced — the sun leads the room without shouting over it.
    /// The re-recorded goldens are the picture's own ratchet on top of these.
    /// Locking constants is the point (the same deliberate silence as
    /// `the_hero_framing_is_the_roomy_cathedral_shot`).
    #[test]
    #[expect(clippy::assertions_on_constants)]
    fn the_light_serves_the_tank() {
        assert!(
            HANGAR_SHADOW_SOFTNESS >= 8.0,
            "the hall's penumbra must stay real: {HANGAR_SHADOW_SOFTNESS} texels"
        );
        let (vertices, full) = hangar_scene_mesh_without_gate();
        let reduced = hangar_shadow_indices();
        assert!(
            reduced.len() < full.len(),
            "the caster cut must actually cut ({} of {})",
            reduced.len(),
            full.len()
        );
        // The CONTRACT, asserted on the output: nothing thin and nothing emissive above the
        // wall band survives in the caster set — thin bars are few triangles (the cut is
        // ~10% of indices) but they were the whole printed lattice.
        for tri in reduced.chunks_exact(3) {
            let p: [Vec3; 3] =
                [0, 1, 2].map(|k| Vec3::from_array(vertices[tri[k] as usize].position));
            if !p.iter().all(|v| v.y > 6.5) {
                continue;
            }
            let emissive =
                tri.iter().any(|&idx| vertices[idx as usize].color.iter().any(|c| *c > 1.0));
            assert!(!emissive, "a pane casts a wall's shadow again at {:?}", p[0]);
            let shortest =
                (p[0] - p[1]).length().min((p[1] - p[2]).length()).min((p[2] - p[0]).length());
            assert!(shortest >= 0.3, "a roof bar is back in the caster set at {:?}", p[0]);
        }
        let luma = |rgb: [f32; 3]| 0.299 * rgb[0] + 0.587 * rgb[1] + 0.114 * rgb[2];
        let rig = SceneLighting::garage_hero();
        let ratio = luma(rig.key_rgb) / luma(rig.ambient_rgb).max(1.0e-3);
        assert!(
            (1.6..=3.4).contains(&ratio),
            "the key leads without shouting: key/ambient luma {ratio:.2} (the lattice-era \
             rig measured ~4.9 and printed a 4.8x stripe swing on the floor)"
        );
    }

    /// H1: THE CANONICAL DAYLIGHT IS THE GOLDEN RIG. Every review artifact and every golden
    /// pins `HangarLight::Day`, and Day must be `garage_hero()` to the bit — a variant system
    /// that nudged the canonical picture would re-record 27 goldens as a side effect.
    #[test]
    fn the_canonical_daylight_is_the_golden_rig() {
        assert!(
            hangar_lighting(HangarLight::Day) == SceneLighting::garage_hero(),
            "Day must be the golden rig, bit for bit"
        );
        assert_eq!(interior_background_for(HangarLight::Day), INTERIOR_BACKGROUND);
        assert_eq!(
            hangar_key_direction(HangarLight::Day).to_array(),
            SceneLighting::garage_hero().key_direction,
            "the shafts' key and the rig's key are one fact"
        );
        // And the flicker holds its review-second identity in every daylight (E2's contract).
        for light in HangarLight::ALL {
            assert!(
                hangar_lighting_at(light, 12.0) == hangar_lighting(light),
                "{light:?}: the frozen review second must carry no flicker dip"
            );
        }
    }

    /// H1: THE VARIANTS ARE DIFFERENT DAYS, NOT DIFFERENT ROOMS. Morning cools and dims the
    /// key with the day's bearing; evening warms it and drops the sun on the SAME azimuth
    /// (the mullion-clear lanes are z-rows — swinging the bearing would put blades on bars);
    /// the three backdrops are three different skies; every grade stays in the moody band.
    #[test]
    fn the_daylights_differ_like_days_do() {
        let day = hangar_lighting(HangarLight::Day);
        let morning = hangar_lighting(HangarLight::Morning);
        let evening = hangar_lighting(HangarLight::Evening);
        // Warmth axis: evening's key is redder than blue, morning's the other way.
        assert!(evening.key_rgb[0] > evening.key_rgb[2], "an evening key is warm");
        assert!(morning.key_rgb[2] > morning.key_rgb[0] * 0.9, "a morning key is not");
        assert!(morning.key_rgb[0] < day.key_rgb[0] * 0.6, "morning light is soft");
        // The evening sun is LOWER on the SAME azimuth.
        let day_key = hangar_key_direction(HangarLight::Day);
        let evening_key = hangar_key_direction(HangarLight::Evening);
        assert!(evening_key.y < day_key.y - 0.05, "the evening sun stands lower");
        let azimuth = |k: Vec3| (k.x / k.z, k.x.signum(), k.z.signum());
        let (day_ratio, dx, dz) = azimuth(day_key);
        let (evening_ratio, ex, ez) = azimuth(evening_key);
        assert!(
            (day_ratio - evening_ratio).abs() < 0.01 && dx == ex && dz == ez,
            "the bearing never swings: {day_ratio} vs {evening_ratio}"
        );
        // Three different skies in the roof openings.
        let backgrounds: Vec<_> = HangarLight::ALL.map(interior_background_for).to_vec();
        assert!(backgrounds[0] != backgrounds[1] && backgrounds[1] != backgrounds[2]);
        // Every variant grades inside the moody band the B1 gate priced.
        for light in HangarLight::ALL {
            let rig = hangar_lighting(light);
            assert!((0.95..=1.10).contains(&rig.exposure), "{light:?} exposure {}", rig.exposure);
            assert!(
                (0.015..=0.032).contains(&rig.black_point),
                "{light:?} black point {}",
                rig.black_point
            );
        }
    }

    /// H1: THE SUN REACHES THE STATION IN EVERY DAYLIGHT THAT HAS ONE, and the shafts stay
    /// honest per variant: morning hangs NO blades (the sun is on the sheds' blind side),
    /// day and evening hang theirs from the same real openings.
    #[test]
    fn every_daylight_reaches_the_station_and_hangs_honest_shafts() {
        let (vertices, indices) = hangar_scene_mesh();
        for light in HangarLight::ALL {
            let key = hangar_key_direction(light).to_array();
            assert!(
                !ray_hits_opaque([0.0, TURNTABLE_TOP_M + 0.6, 0.0], key, &vertices, &indices),
                "{light:?}: the key must reach the turntable centre unobstructed"
            );
            // The same 60% deck fan the day's own sun lock demands — a variant whose sun
            // cannot light the station is a variant that never should have shipped.
            let (mut clear, mut total) = (0, 0);
            for gx in -2i32..=2 {
                for gz in -2i32..=2 {
                    total += 1;
                    let origin = [gx as f32 * 1.5, TURNTABLE_TOP_M + 0.6, gz as f32 * 1.5];
                    if !ray_hits_opaque(origin, key, &vertices, &indices) {
                        clear += 1;
                    }
                }
            }
            assert!(
                clear * 10 >= total * 6,
                "{light:?}: most of the deck stands in the sun: {clear}/{total}"
            );
        }
        assert!(sun_shaft_quads_for(HangarLight::Morning).is_empty(), "morning hangs no beam");
        for light in [HangarLight::Day, HangarLight::Evening] {
            let quads = sun_shaft_quads_for(light);
            assert_eq!(quads.len(), BROKEN_PANES.len(), "{light:?}: one blade per broken pane");
            for quad in &quads {
                let top_y = quad[0][1].max(quad[1][1]);
                let bottom_y = quad[2][1].min(quad[3][1]);
                assert!(top_y > WALL_HEIGHT, "{light:?}: blades hang from the roof band");
                assert!(bottom_y < 1.5, "{light:?}: blades die out near the floor");
            }
        }
        // Evening's lower sun travels farther across the room than the day's.
        let reach = |quads: &Vec<[[f32; 3]; 4]>| {
            quads
                .iter()
                .map(|q| (q[2][0] - q[0][0]).abs() + (q[2][2] - q[0][2]).abs())
                .fold(0.0f32, f32::max)
        };
        assert!(
            reach(&sun_shaft_quads_for(HangarLight::Evening))
                > reach(&sun_shaft_quads_for(HangarLight::Day)),
            "a lower sun throws a longer shaft"
        );
    }

    /// K1: SOMEBODY WORKS THIS HALL. The crane trolley rides its girder — deterministic on
    /// the clock, never off the rail, never parked forever — and the welding arc keeps its
    /// contract with the goldens: the frozen review second falls in the QUIET half, so the
    /// locked picture is the resting hall and the burn is a live moment.
    #[test]
    fn the_trolley_rides_and_the_arc_keeps_the_review_second_quiet() {
        let span = HALF_X - 1.5;
        let mut positions = Vec::new();
        for step in 0..120 {
            let x = crane_trolley_x_at(step as f32 * 1.0);
            assert!(x.abs() <= span - 0.6, "the trolley leaves its girder: {x}");
            positions.push(x);
        }
        let (min, max) =
            positions.iter().fold((f32::MAX, f32::MIN), |(lo, hi), &x| (lo.min(x), hi.max(x)));
        assert!(max - min > 8.0, "the trolley actually travels: {min}..{max}");
        // Determinism, and the same mesh for the same second.
        let (a, ai) = crane_trolley_at(37.5);
        let (b, bi) = crane_trolley_at(37.5);
        assert!(a == b && ai == bi, "the trolley is a pure function of the clock");
        // The goldens' second stays quiet; the arc genuinely burns at other times.
        assert!(!welding_burn_at(12.0), "the frozen review second must be arc-quiet");
        assert!(welding_burn_at(0.5), "the arc does burn on its cycle");
    }

    /// E3: THE CURTAIN RIDES ITS TRACK IN SEGMENTS. Opening raises the bottom edge
    /// monotonically; every slat stays inside [ajar, gate top]; the top slat is parked from
    /// the start and the bottom one moves last into the stack — and at full open the curtain
    /// exactly fills the head track. Same topology at every angle of travel.
    #[test]
    fn the_gate_curtain_opens_in_segments_and_never_leaves_its_track() {
        let (rest_v, rest_i) = bay_gate_slats(GATE_AJAR_M);
        let mut last_bottom = f32::NEG_INFINITY;
        for step in 0..=8 {
            let open = GATE_AJAR_M + (GATE_DRIVE_OPEN_M - GATE_AJAR_M) * step as f32 / 8.0;
            let (v, i) = bay_gate_slats(open);
            assert_eq!(v.len(), rest_v.len(), "travel never changes topology");
            assert_eq!(i, rest_i, "same triangles, moved vertices");
            let bottom = v.iter().map(|s| s.position[1]).fold(f32::INFINITY, f32::min);
            assert!(
                (bottom - open).abs() < 1.0e-4,
                "the clear opening IS the curtain's bottom edge: {bottom} vs {open}"
            );
            assert!(bottom >= last_bottom, "opening only ever raises the curtain");
            last_bottom = bottom;
            let top = v.iter().map(|s| s.position[1]).fold(f32::NEG_INFINITY, f32::max);
            assert!(top <= GATE_TOP + 1.0e-4, "no slat escapes past the head track: {top}");
        }
        // Full open: the stack fills the track — top at the gate top, bottom edge at the
        // drive clearance, and the clearance takes the tallest hull with a margin.
        let (full, _) = bay_gate_slats(GATE_DRIVE_OPEN_M);
        let top = full.iter().map(|s| s.position[1]).fold(f32::NEG_INFINITY, f32::max);
        assert!((top - GATE_TOP).abs() < 1.0e-4, "stacked curtain parks under the lintel");
        let clearance = full.iter().map(|s| s.position[1]).fold(f32::INFINITY, f32::min);
        assert!(clearance > 3.2, "a drive-in clearance, not a crawl space: {clearance}");
    }

    /// E3: THE GATE SPLIT REMOVES THE CURTAIN AND NOTHING ELSE. The render mesh without the
    /// gate stops blocking the eye exactly in the curtain band — the jambs, the lintel and
    /// the walls still stand — and the parked curtain re-emitted dynamically puts back the
    /// same five slats the full mesh carries.
    #[test]
    fn the_gate_split_removes_the_curtain_and_nothing_else() {
        let (full_v, _) = hangar_scene_mesh();
        let (bare_v, bare_i) = hangar_scene_mesh_without_gate();
        let (slat_v, _) = bay_gate_slats(GATE_AJAR_M);
        assert_eq!(
            full_v.len(),
            bare_v.len() + slat_v.len(),
            "the split takes exactly the curtain's vertices"
        );
        let out = [0.0, 0.0, -1.0];
        let mid_curtain = [0.0, (GATE_AJAR_M + GATE_TOP) / 2.0, -10.0];
        assert!(
            !ray_hits_mesh(mid_curtain, out, &bare_v, &bare_i),
            "without the curtain the gate band is open"
        );
        assert!(
            ray_hits_mesh([0.0, GATE_TOP + 0.35, -10.0], out, &bare_v, &bare_i),
            "the lintel band still stands"
        );
        assert!(
            ray_hits_mesh([GATE_HALF_W + 3.0, WALL_SEAM * 0.5, -10.0], out, &bare_v, &bare_i),
            "the flanking wall still stands"
        );
    }

    /// E2: ONLY THE HANGING THINGS SWAY. The hook, the chain and the banner carry a sway
    /// amplitude (they hang free, and the gate is ajar); the shell — walls, floor, roof —
    /// carries exactly zero, because a swaying wall is an earthquake, not a draft.
    #[test]
    fn only_the_hanging_things_sway() {
        let (vertices, _) = hangar_scene_mesh();
        let swaying = vertices.iter().filter(|v| v.sway > 0.0).count();
        assert!(
            (24..=400).contains(&swaying),
            "a handful of hanging pieces sway, got {swaying} vertices"
        );
        for vertex in &vertices {
            if vertex.sway > 0.0 {
                assert!(
                    vertex.position[1] > 1.5,
                    "everything that sways HANGS — sway at floor level {:?}",
                    vertex.position
                );
                assert!(vertex.sway <= 0.06, "a draft, not a storm: {}", vertex.sway);
            }
        }
    }

    /// E2: the fan's blades are deterministic in their angle, live at the fan's own hub, and
    /// spin — two angles give two meshes with identical topology and moved vertices.
    #[test]
    fn the_fan_turns_about_its_own_hub() {
        let (v0, i0) = wall_fan_blades(0.0);
        let (v1, i1) = wall_fan_blades(1.0);
        assert_eq!(v0.len(), v1.len());
        assert_eq!(i0, i1, "rotation moves vertices, never topology");
        assert!(v0 != v1, "two angles are two pictures");
        let hub = Vec3::from_array(FAN_CENTER);
        for vertex in &v0 {
            let d = Vec3::from_array(vertex.position) - hub;
            assert!(d.length() < 0.75, "a blade stays inside its guard: {:?}", vertex.position);
        }
        // Determinism: the same angle is the same mesh, bit for bit.
        let (v0b, _) = wall_fan_blades(0.0);
        assert!(v0 == v0b, "the fan mesh is a pure function of its angle");
    }

    /// E1: THE SHAFTS HANG FROM REAL OPENINGS. Every blade's top edge must let the sky
    /// through — a ray cast UP from just under the top edge escapes the mesh (it starts in a
    /// glazing opening), and the blade reaches the floor. A beam hanging from solid roof
    /// would be the pane-glow lie in a new costume; this holds each blade to the physics the
    /// sun lock holds the key to.
    #[test]
    fn the_shafts_hang_from_real_openings() {
        let (vertices, indices) = hangar_scene_mesh();
        let quads = sun_shaft_quads();
        // Rare and natural (user direction 2026-08-10): a beam per BROKEN pane, nothing else
        // — clean glazing diffuses, only an aperture beams.
        assert_eq!(quads.len(), BROKEN_PANES.len(), "one blade per broken pane, no strays");
        for quad in &quads {
            let top_mid = [
                (quad[0][0] + quad[1][0]) / 2.0,
                (quad[0][1] + quad[1][1]) / 2.0 - 0.25,
                (quad[0][2] + quad[1][2]) / 2.0,
            ];
            // The FULL geometry test, glass included: a blade under an intact pane is the
            // pane-glow lie — the ray may only escape through a genuinely broken bay.
            assert!(
                !ray_hits_mesh(top_mid, [0.0, 1.0, 0.0], &vertices, &indices),
                "a blade top at {top_mid:?} hangs under glass or roof — its hole is a lie"
            );
            assert!(
                quad[2][1] < 1.5,
                "a blade must fall to head height before it dies, bottom at {}",
                quad[2][1]
            );
            assert!(
                quad[0][1] > WALL_HEIGHT - 0.2,
                "a blade hangs from the glazing plane, top at {}",
                quad[0][1]
            );
            // THE BEAM FRAMES THE HERO, NEVER HITS IT (the whole point of the rework): the
            // blade's foot lands outside the turntable's reach in every daylight.
            let foot = [(quad[2][0] + quad[3][0]) / 2.0, (quad[2][2] + quad[3][2]) / 2.0];
            let reach = (foot[0] * foot[0] + foot[1] * foot[1]).sqrt();
            assert!(
                reach > 4.5,
                "a beam lands on the hero's station (foot at {foot:?}, r {reach:.1})"
            );
        }
        // And in the evening too — the lower sun throws the feet farther, never closer.
        for quad in &sun_shaft_quads_for(HangarLight::Evening) {
            let foot = [(quad[2][0] + quad[3][0]) / 2.0, (quad[2][2] + quad[3][2]) / 2.0];
            let reach = (foot[0] * foot[0] + foot[1] * foot[1]).sqrt();
            assert!(reach > 4.5, "an evening beam lands on the station (r {reach:.1})");
        }
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
        // Vertices on the right side-wall inner plane (x == HALF_X - SLAB).
        let on_plane = |c: [f32; 3]| {
            vertices
                .iter()
                .filter(move |v| (v.position[0] - (HALF_X - SLAB)).abs() < 1.0e-4 && v.color == c)
                .map(|v| v.position[1])
        };
        let metal_top = on_plane(WHITEWASH_WALL).fold(f32::MIN, f32::max);
        let upper_bottom = on_plane(UPPER_WALL).fold(f32::MAX, f32::min);
        assert!(metal_top > 0.0, "the whitewashed band should reach the inner wall plane");
        assert!(
            (metal_top - upper_bottom).abs() < 1.0e-4,
            "wall bands must abut at the seam, not overlap: metal top {metal_top}, upper bottom {upper_bottom}"
        );
        // The side wall's upper band runs all the way to the shed ridge — it is the sheds'
        // flank, and a band stopping at the eaves would open a sky sliver over every tooth.
        let upper_top = on_plane(UPPER_WALL).fold(f32::MIN, f32::max);
        assert!((upper_top - SHED_RIDGE).abs() < 1.0e-4, "upper wall must reach the ridge");
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

    /// The nave's volume is EARNED: the band between the wall seam and the truss chords
    /// carries the crane girder and rails, the lamp rig, the catwalk railing and signage — not
    /// empty black air. The 9 m hall's band is thinner than the old 12.6 m cathedral's, so the
    /// floor is re-blessed to the measured furnishing of THIS shell, not inherited.
    #[test]
    fn the_upper_band_is_inhabited() {
        let (vertices, _) = hangar_scene_mesh();
        let inhabited = vertices
            .iter()
            .filter(|v| {
                let [x, y, z] = v.position;
                y > WALL_SEAM
                    && y < WALL_HEIGHT - 0.1
                    && x.abs() < HALF_X - SLAB - 0.01
                    && z.abs() < HALF_Z - SLAB - 0.01
            })
            .count();
        assert!(inhabited >= 600, "the upper band is dressed, got {inhabited} vertices");
    }

    /// Every hot lamp face hangs where the `garage_hero` light rig says a light is: the pools
    /// and the housings are twins, or the room reads as haunted. NO exceptions — a flat HDR
    /// slab is not a window, so every emissive vertex in the hall below the skylights is a
    /// worklamp face with a housing and a rig pool.
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
                vertex.position[2] > -(HALF_Z - 1.0),
                "an emitter on the gate wall is a lightbox, not a window: {:?}",
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
            let by_gate = (x - -5.6).abs() < 0.6 && (z + (HALF_Z - 0.45)).abs() < 0.6;
            let by_bench = (x - (HALF_X - 0.45)).abs() < 0.6 && (z - 3.8).abs() < 0.6;
            assert!(by_gate || by_bench, "stray saturated red at {:?}", vertex.position);
        }
    }

    /// The deep end of the nave reads OCCUPIED: the second bay and its furniture put real
    /// geometry down the long axis past the turntable — across the nave from the hero
    /// framing's eye, so the depth of the shot has work standing in it, not beside the lens.
    #[test]
    fn the_second_bay_reads_occupied() {
        let (vertices, _) = hangar_scene_mesh();
        let occupied = vertices
            .iter()
            .filter(|v| {
                let [x, y, z] = v.position;
                (-7.5..-3.0).contains(&x) && z > 11.0 && y > 0.05 && y < 4.0
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

    /// The hall's SIZE budget — a ceiling, not the cost: vertex count is not what the player
    /// waits for (the `OnceLock` bakes once). The COST is measured next door, in
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
        let (vertices, indices, _, _) = build_hangar_scene_mesh_for(HangarLight::Day);
        let cost = started.elapsed();
        println!(
            "HANGAR COLD BAKE: {:?} for {} vertices / {} triangles",
            cost,
            vertices.len(),
            indices.len() / 3
        );
        // Raised 20 → 60 s with B2 (measured: 44.3 s at opt-level 1 under the full workspace
        // suite, where sibling tests contend for the gather's worker lanes; the SHIPPED cost
        // is the release measurement in `prewarm`'s docs — 0.98 s, priced against the ≤1 s
        // budget). The ceiling still catches an order-of-magnitude regression, which is all
        // it ever measured.
        assert!(
            cost < std::time::Duration::from_secs(60),
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
            // Ceiling of the protected column: under the 9 m chord the high-bay pendants hang
            // at 7.6 m (their faces at 7.585), and the orbit's own eye never climbs past ~5.8
            // inside this radius (the clamp's ceiling binds long before r < 5.7 m).
            assert!(
                !(r < TURNTABLE_RADIUS_M + 0.5 && y > 0.5 && y < 7.4),
                "geometry invades the turntable air column at {:?}",
                vertex.position
            );
            assert!(
                !(x.abs() < 2.7
                    && (-(HALF_Z - 0.5)..-TURNTABLE_RADIUS_M).contains(&z)
                    && y > 0.4
                    && y < 4.4),
                "geometry blocks the drive lane at {:?}",
                vertex.position
            );
            // A2: the through-lane past the station stays drivable too — the wear the floor
            // shows is a route the room could actually use.
            assert!(
                !(x.abs() < 2.7 && (TURNTABLE_RADIUS_M..12.0).contains(&z) && y > 0.4 && y < 4.4),
                "geometry blocks the through-lane at {:?}",
                vertex.position
            );
        }
    }
}
