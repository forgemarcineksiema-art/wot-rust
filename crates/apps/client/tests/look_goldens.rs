//! The GPU half of the look harness (`docs/art-direction-policy.md`): renders the canonical
//! review views (`client::review_views_for` — the same table the human-review examples draw)
//! for EVERY shipped map and locks them. The weather roll is random per battle, so every look a
//! blueprint declares is locked here; a look this file skips is a look the player meets
//! unreviewed.
//!
//! Two layers:
//! - `look_goldens_match_their_recordings` — OPT-IN via `WOT_LOOK_GOLDENS=1` (needs a GPU;
//!   byte-exact per machine, like the studio goldens). Re-record with `WOT_UPDATE_GOLDENS=1`
//!   after a deliberate look change.
//! - the rest — always-on and CPU-only: they decode the committed golden PNGs, so they catch a
//!   policy violation in any committed look on a machine with no GPU at all.
//!   `recorded_goldens_hold_the_value_structure` is rule 1 (three value planes) and rule 3 (the
//!   evening out-warms the overcast, per map). `no_recorded_frame_flattens_into_a_wash` and
//!   `no_recorded_frame_runs_away_with_chroma` are regression guards on detail and chroma.
//!   `the_measured_baseline_of_every_recorded_frame` asserts almost nothing — it PRINTS the
//!   table that `docs/art-direction-program.md` carries, because a number nobody wrote down is
//!   a number nobody can be held to.

use std::fs::File;
use std::io::{BufReader, BufWriter};
use std::path::PathBuf;

use client::{REVIEWED_MAPS, ReviewView, review_views_for};
use game_core::math::srgb_to_linear;
use scene_build::review_views::map_key;
use terrain::MapId;

const WIDTH: u32 = 960;
const HEIGHT: u32 = 540;

fn goldens_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests").join("goldens").join("look")
}

fn golden_path(name: &str) -> PathBuf {
    goldens_dir().join(format!("{name}.png"))
}

/// The harness renders through `client::render_review_views` — the SAME entry the `*_views`
/// examples draw with. They used to hand-roll this setup separately, which is exactly how both
/// of them lost the foliage-atlas bind and started locking white trees.
fn render_views(map: MapId, views: &[ReviewView]) -> Vec<Vec<u8>> {
    client::render_review_views(map, views, WIDTH, HEIGHT).expect("review render")
}

fn write_png(path: &PathBuf, pixels: &[u8]) {
    std::fs::create_dir_all(path.parent().expect("goldens dir")).expect("create goldens dir");
    let file = File::create(path).expect("create golden");
    let mut encoder = png::Encoder::new(BufWriter::new(file), WIDTH, HEIGHT);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.write_header().expect("png header").write_image_data(pixels).expect("png data");
}

fn read_png(path: &PathBuf) -> Vec<u8> {
    let file = File::open(path).unwrap_or_else(|_| {
        panic!("missing golden {} — record with WOT_UPDATE_GOLDENS=1", path.display())
    });
    let decoder = png::Decoder::new(BufReader::new(file));
    let mut reader = decoder.read_info().expect("png info");
    let mut buf = vec![0u8; reader.output_buffer_size().expect("png size")];
    let info = reader.next_frame(&mut buf).expect("png frame");
    assert_eq!(
        (info.width, info.height),
        (WIDTH, HEIGHT),
        "golden {} has stale dimensions — re-record",
        path.display()
    );
    buf.truncate(info.buffer_size());
    buf
}

/// Which half of the review set a re-record covers.
///
/// `WOT_UPDATE_GOLDENS=1` still re-records everything. The scoped forms exist because the two
/// halves rot INDEPENDENTLY and at wildly different rates, and re-recording one should not
/// silently bless the other. Measured 2026-08-09 against the committed set: the twenty
/// battlefield frames were 45-78% different with mean deltas of 5-73/255 (the world was rebuilt
/// under them and nobody re-recorded), while the four garage frames were 8-11% different at a
/// mean delta of ~3. Without a scope, a garage PR that re-records its own four frames folds
/// twenty unreviewed world frames into its diff — which is the opposite of what the byte lock
/// is for.
#[derive(Clone, Copy, PartialEq, Eq)]
enum UpdateScope {
    /// Compare only — the gate's normal mode.
    None,
    Battlefield,
    Garage,
    All,
}

impl UpdateScope {
    fn from_env() -> Self {
        match std::env::var("WOT_UPDATE_GOLDENS").as_deref() {
            Ok("1" | "all") => Self::All,
            Ok("battlefield") => Self::Battlefield,
            Ok("garage") => Self::Garage,
            _ => Self::None,
        }
    }

    fn records_battlefield(self) -> bool {
        matches!(self, Self::All | Self::Battlefield)
    }

    fn records_garage(self) -> bool {
        matches!(self, Self::All | Self::Garage)
    }

    fn is_recording(self) -> bool {
        self != Self::None
    }
}

/// Compare one rendered view against its golden, or record it. A drifted view is APPENDED to
/// `drift` rather than asserted on the spot: an `assert_eq!` inside the loop stops the run at
/// the first bad frame, and because the garage renders last, one stale Prokhorovka frame meant
/// the garage's byte lock never executed at all. Every view is checked; the whole list is
/// reported once.
fn check_or_record(name: &str, pixels: &[u8], record: bool, drift: &mut Vec<String>) {
    let path = golden_path(name);
    if record {
        write_png(&path, pixels);
        eprintln!("recorded {}", path.display());
        return;
    }
    let golden = read_png(&path);
    if golden == pixels {
        return;
    }
    let (mut differing, mut max_delta) = (0u32, 0u32);
    for (a, b) in golden.chunks_exact(4).zip(pixels.chunks_exact(4)) {
        let delta = (0..3).map(|c| a[c].abs_diff(b[c]) as u32).max().unwrap_or(0);
        differing += u32::from(delta > 0);
        max_delta = max_delta.max(delta);
    }
    drift.push(format!(
        "{name}: {:.2}% of pixels differ, max delta {max_delta}",
        differing as f32 / (WIDTH * HEIGHT) as f32 * 100.0
    ));
}

#[test]
fn look_goldens_match_their_recordings() {
    let scope = UpdateScope::from_env();
    if std::env::var("WOT_LOOK_GOLDENS").as_deref() != Ok("1") && !scope.is_recording() {
        eprintln!("skipping look goldens (set WOT_LOOK_GOLDENS=1 to enable)");
        return;
    }
    let mut drift: Vec<String> = Vec::new();

    // A scoped re-record skips the half it is not recording rather than comparing it: the whole
    // point of the scope is to touch one half while the other is knowingly out of date.
    if !scope.is_recording() || scope.records_battlefield() {
        for map in REVIEWED_MAPS {
            let battlefield = map_forge::battlefield(map);
            let views = review_views_for(map, &battlefield);
            let frames = render_views(map, &views);
            for (view, pixels) in views.iter().zip(&frames) {
                check_or_record(&view.name, pixels, scope.records_battlefield(), &mut drift);
            }
        }
    }

    // The garage: an interior studio with its own light rig and its own lens, but the same
    // display transform and the same locks. It had no golden at all before this.
    if !scope.is_recording() || scope.records_garage() {
        let hangar_views = client::hangar_review_views();
        let hangar_frames = client::render_hangar_review_views(&hangar_views, WIDTH, HEIGHT)
            .expect("hangar review render");
        for (view, pixels) in hangar_views.iter().zip(&hangar_frames) {
            check_or_record(&view.name, pixels, scope.records_garage(), &mut drift);
        }
    }

    assert!(
        drift.is_empty(),
        "{} locked frame(s) drifted from their goldens — if the look change is deliberate, \
         re-record with WOT_UPDATE_GOLDENS=1 (or =garage / =battlefield for one half) and say \
         what changed about the PICTURE in the PR:\n  {}",
        drift.len(),
        drift.join("\n  ")
    );

    // The byte-exact contract this harness rests on: the same view renders identically twice
    // on one machine (the render is a pure function of scene + profile + the fixed clock).
    let map = REVIEWED_MAPS[0];
    let battlefield = map_forge::battlefield(map);
    let views = review_views_for(map, &battlefield);
    let once = render_views(map, &views[..1]);
    let again = render_views(map, &views[..1]);
    assert_eq!(once[0], again[0], "the render must be deterministic on one machine");
}

/// What one recorded frame measures. Plane shares answer rule 1's "three separated planes";
/// the percentiles and their spread answer the question the shares cannot — *how far apart* the
/// planes are, which is the difference between a picture with structure and a wash that happens
/// to straddle two thresholds. `band_separation` is rule 1's sky-above-field ordering read off
/// the pixels; `local_contrast` is rule 5's anti-flat clause.
struct FrameStats {
    dark: f32,
    mid: f32,
    bright: f32,
    mean_warmth: f32,
    p05: f32,
    p50: f32,
    p95: f32,
    /// p95 − p05. A wash has a small one no matter where its planes land.
    spread: f32,
    /// Mean per-pixel saturation (max−min over max). A chroma regression measure, NOT rule 2's
    /// albedo bound — see `no_recorded_frame_runs_away_with_chroma`.
    saturation: f32,
    /// Share of pixels at or above 0.97 linear luma — effectively pure white on screen. The sun
    /// disc's hot core is allowed to live here; a washed-out sky band or a blown field is not.
    /// This is the direct pixel-side regression lock on "the picture went white" (D3): the
    /// disc/halo multipliers are shader constants no CPU mirror can see, so the ceiling reads
    /// the recorded photograph instead.
    near_white: f32,
    /// Mean absolute luminance step between horizontally adjacent pixels. Detail, not noise:
    /// a flat wash tends to zero, a shimmering surface runs high.
    local_contrast: f32,
    /// Median luminance of the top 15% of rows minus the bottom 40%. On an outdoor frame at
    /// hull height that is sky-band minus near-field, so rule 1's "the sky out-lumes the field"
    /// becomes a number. Meaningless indoors, where the top of the frame is roof.
    band_separation: f32,
}

fn percentile(sorted: &[f32], q: f32) -> f32 {
    if sorted.is_empty() {
        return 0.0;
    }
    let index = ((sorted.len() - 1) as f32 * q).round() as usize;
    sorted[index]
}

fn median_of(values: &mut [f32]) -> f32 {
    values.sort_by(|a, b| a.partial_cmp(b).expect("luma is finite"));
    percentile(values, 0.5)
}

/// The whole recorded frame.
fn frame_stats(pixels: &[u8]) -> FrameStats {
    frame_stats_sized(pixels, WIDTH as usize, HEIGHT as usize)
}

/// A crop of one. Row width has to be passed in, because local contrast walks rows and band
/// separation splits them — running either against the full frame's stride on a crop would
/// silently measure nonsense.
fn frame_stats_of(pixels: &[u8], width: usize, height: usize) -> FrameStats {
    frame_stats_sized(pixels, width, height)
}

fn frame_stats_sized(pixels: &[u8], width: usize, height: usize) -> FrameStats {
    let (mut dark, mut mid, mut bright, mut near_white) = (0u32, 0u32, 0u32, 0u32);
    let (mut sum_r, mut sum_b, mut sum_sat) = (0.0f64, 0.0f64, 0.0f64);
    let mut lumas = Vec::with_capacity(width * height);

    for px in pixels.chunks_exact(4) {
        let r = srgb_to_linear(px[0]);
        let g = srgb_to_linear(px[1]);
        let b = srgb_to_linear(px[2]);
        let luma = 0.2126 * r + 0.7152 * g + 0.0722 * b;
        if luma < 0.25 {
            dark += 1;
        } else if luma < 0.60 {
            mid += 1;
        } else {
            bright += 1;
        }
        if luma >= 0.97 {
            near_white += 1;
        }
        sum_r += r as f64;
        sum_b += b as f64;
        let max = r.max(g).max(b);
        let min = r.min(g).min(b);
        sum_sat += if max > 1.0e-6 { ((max - min) / max) as f64 } else { 0.0 };
        lumas.push(luma);
    }

    // Local contrast along rows only: a horizontal step is the cheapest honest probe of whether
    // the surface carries detail, and it needs no second pass over the image.
    let mut contrast_sum = 0.0f64;
    let mut contrast_count = 0u32;
    for row in lumas.chunks_exact(width) {
        for pair in row.windows(2) {
            contrast_sum += (pair[1] - pair[0]).abs() as f64;
            contrast_count += 1;
        }
    }

    let top_rows = (height * 15) / 100;
    let bottom_start = height - (height * 40) / 100;
    let band_separation = if top_rows == 0 || bottom_start >= height {
        // A crop can be too short to have bands. Report no separation rather than a lie.
        0.0
    } else {
        let mut top: Vec<f32> = lumas[..top_rows * width].to_vec();
        let mut bottom: Vec<f32> = lumas[bottom_start * width..].to_vec();
        median_of(&mut top) - median_of(&mut bottom)
    };

    let n = lumas.len() as f32;
    let mut sorted = lumas;
    sorted.sort_by(|a, b| a.partial_cmp(b).expect("luma is finite"));
    let p05 = percentile(&sorted, 0.05);
    let p95 = percentile(&sorted, 0.95);

    FrameStats {
        dark: dark as f32 / n,
        mid: mid as f32 / n,
        bright: bright as f32 / n,
        mean_warmth: (sum_r / sum_b.max(1.0e-9)) as f32,
        p05,
        p50: percentile(&sorted, 0.50),
        p95,
        spread: p95 - p05,
        saturation: (sum_sat / n as f64) as f32,
        near_white: near_white as f32 / n,
        local_contrast: (contrast_sum / contrast_count.max(1) as f64) as f32,
        band_separation,
    }
}

// ---------------------------------------------------------------------------------------------
// FLOOR / TARGET. The mechanism the old "their dark floor is symbolic for now — RAISE IT as the
// world fills in" comment needed and did not have: a place to RECORD the gap between what the
// picture is and what it must become. A comment cannot fail a build, so the gap sat there for
// months. A named constant pair can be read, compared and closed.
//
// FLOOR is the recorded worst — asserted, so the picture can never get worse.
// TARGET is what `docs/art-direction-policy.md` demands — reported as a distance, not yet
// asserted, and closed by the wave named beside it.
// ---------------------------------------------------------------------------------------------

/// Recorded worst outdoor dark share: the two overcast frames at 0.68%.
///
/// Re-derived 2026-08-11 with the instrument change, not as a debt: the previous 0.008 was
/// recorded off 4x-multisampled goldens, and the review path now renders the SHIPPED 1x picture
/// — the same scene measures a hair darker when its edge pixels stop being blended fractions.
/// The floor moves WITH the instrument or it asserts that the flattering copy was the truth.
/// The target does not move: the debt to rule 1 is unchanged.
///
/// Re-derived again 2026-08-22 with the flora REPRESENTATION change (Drzewa 3.0 PR8): the
/// steppe's bushes went from solid lobed blobs to card tufts around an interior occlusion
/// hull. Five content levers (Close-rung statics, a 3 m footprint, the scrub palette, deep
/// core shade, a matte rim cap) recovered the dark plane to 0.0057 of the old 0.0066 — the
/// residual is the per-instance FOOTPRINT difference of a changed representation (measured
/// with a dark-pixel diff: thin halos where each old blob's silhouette reached past its
/// tuft), not a lighting regression. The floor records the new representation's worst; the
/// target still does not move, and W1 still owes rule 1 its real shade mass.
const OUTDOOR_DARK_FLOOR: f32 = 0.0055;
/// Rule 1 wants a real shade mass in every frame, not a token one.
const OUTDOOR_DARK_TARGET: f32 = 0.08;
/// Recorded worst outdoor p95−p05 spread: `prokhorovka_overcast` at 0.348.
const OUTDOOR_SPREAD_FLOOR: f32 = 0.34;
/// Three separated planes need range between them, not just presence.
const OUTDOOR_SPREAD_TARGET: f32 = 0.45;

/// Assert the floor, report the distance to the target. The one place the pattern lives, so a
/// new bound cannot quietly forget to state its debt.
fn debt(view: &str, metric: &str, measured: f32, floor: f32, target: f32, wave: &str) {
    assert!(
        measured >= floor,
        "{view}: {metric} {measured:.3} fell below its recorded floor {floor:.3} — this is a \
         REGRESSION, not a debt; the picture got worse",
    );
    if measured < target {
        println!(
            "LOOK DEBT {view}: {metric} {measured:.3}, target {target:.3} \
             (short by {:.3}, {wave})",
            target - measured
        );
    }
}

/// Always-on, CPU-only: the committed goldens must obey the bible's value structure. This is
/// the statistical lock that runs in every `verify` regardless of GPU availability — if a
/// deliberate re-record ships a picture that lost its three value planes, this fails the gate.
#[test]
fn recorded_goldens_hold_the_value_structure() {
    let mut warmth_by_name = std::collections::HashMap::new();
    println!(
        "\nLOOK DEBT is the distance from what the picture achieves to what the policy demands.\n\
         Asserted: the FLOOR. Reported: the gap. See docs/art-direction-program.md.\n"
    );
    for map in REVIEWED_MAPS {
        let battlefield = map_forge::battlefield(map);
        for view in review_views_for(map, &battlefield) {
            // A view through its own lens (the sniper frame, A7) is a CROP of the picture — a
            // few metres of field around one hull at 8° — and three value planes are a claim
            // about a picture, not a crop. It is judged inside its subject box, where the tank
            // is, by `the_vehicle_stays_readable_on_the_side_the_sun_never_touches`.
            if view.vertical_fov_degrees.is_some() {
                continue;
            }
            let pixels = read_png(&golden_path(&view.name));
            let stats = frame_stats(&pixels);
            // RULE 1, in FLOOR/TARGET form. FLOOR is what the recorded picture achieves today
            // and is asserted, so nothing may get worse. TARGET is what the policy demands; the
            // distance is emitted as a LOOK DEBT line instead of hiding in a comment the way the
            // old "symbolic for now" floor did.
            debt(
                &view.name,
                "dark plane",
                stats.dark,
                OUTDOOR_DARK_FLOOR,
                OUTDOOR_DARK_TARGET,
                "W1",
            );
            assert!(
                stats.mid >= 0.05,
                "{}: the mid plane vanished ({:.1}% of pixels)",
                view.name,
                stats.mid * 100.0
            );
            assert!(
                stats.bright >= 0.03,
                "{}: the bright plane vanished ({:.1}% of pixels)",
                view.name,
                stats.bright * 100.0
            );
            // No single plane may swallow the picture. The policy wants 75%; the recorded set
            // clears that outdoors with room to spare, so this bound BITES today rather than
            // recording a debt.
            for (plane, share) in
                [("dark", stats.dark), ("mid", stats.mid), ("bright", stats.bright)]
            {
                assert!(
                    share <= 0.75,
                    "{}: the {plane} plane swallowed the picture ({:.1}%)",
                    view.name,
                    share * 100.0
                );
            }
            // RULE 1's ordering, read off the PHOTOGRAPH rather than off the profile: the sky
            // band must out-lume the near field. This is the lock the analytic checks could
            // never make — a profile can order its planes correctly and still render a frame
            // whose sky and ground meet in the same milk. Every recorded outdoor frame clears
            // it today (the worst is +0.160), so it bites from day one.
            assert!(
                stats.band_separation > 0.05,
                "{}: the sky band no longer out-lumes the field ({:+.3}) — rule 1's ordering \
                 failed on the pixels, whatever the profile says",
                view.name,
                stats.band_separation
            );
            // D3's regression stop, read off the photograph: at most 1.5% of an outdoor frame
            // may sit at effectively pure white (>= 0.97 linear luma). The sun disc's hot core
            // fits comfortably (~0.1% of a 960x540 frame); a milky sky band, a blown cloud
            // deck or a washed field does not. The disc/halo multipliers live in sky.wgsl where
            // no CPU mirror reaches — this ceiling is what keeps them honest.
            assert!(
                stats.near_white <= 0.015,
                "{}: {:.2}% of the frame is pure white — the picture is washing out (D3)",
                view.name,
                stats.near_white * 100.0
            );
            // Rule 1's other half: the planes must be far APART, not merely present. A wash can
            // straddle two thresholds and still read as one flat surface.
            debt(
                &view.name,
                "spread",
                stats.spread,
                OUTDOOR_SPREAD_FLOOR,
                OUTDOOR_SPREAD_TARGET,
                "W1",
            );
            warmth_by_name.insert(view.name.clone(), stats.mean_warmth);
        }
    }

    // The garage under the same value structure. A lit hangar is ALLOWED — required, even — to
    // hold real shade, so its dark floor is the interior one rather than the empty-steppe one.
    //
    // Its BRIGHT plane is the program's first measured FLOOR/TARGET debt (D20). The frame used to
    // hold 0.00% of pixels above the bright threshold, and the reframing that lowered
    // `HERO_ORBIT_PITCH` to bring the room's daylight into shot moved it to 0.3% — the frosted
    // panes over the bay gate, and nothing else, because they are the only emissive surface the
    // hero lens now contains. The floor rises to lock that gain in.
    //
    // 0.3% against a 2% target says the reframing did NOT close D20, and the percentiles say why:
    // p50 sits at 0.119 and p95 at 0.276, so the ENTIRE picture is a narrow band pressed against
    // the 0.25 dark/mid boundary. The floor a player reads as light grey measures 0.238 — a
    // hair's breadth on the dark side. That is a RANGE problem, and a camera cannot fix range:
    // where the lens points decides what is in the picture, the light rig and the grade decide
    // how far apart its values are. D20 closes with light in the room, not with a framing.
    // RAISED TO WHAT THE FRAME MEASURES, which is what a FLOOR is for and what this one had
    // never once had done to it. It sat at 0.0025 while the frame measured 0.010 — four times
    // of slack, so the garage could get four times darker with the gate still green. The policy
    // is explicit: "A wave is finished when its FLOOR has been raised to meet its TARGET."
    //
    // The frame now measures 2.3% bright, so the floor rises TO the 2% target: the garage's
    // bright plane is no longer a debt, and it cannot fall back into being one. The reflection
    // correction and the hall's materials are what carried it there — 1.0% -> 1.5% -> 2.3%.
    const GARAGE_BRIGHT_FLOOR: f32 = 0.02;
    const GARAGE_BRIGHT_TARGET: f32 = 0.02;
    // The dark share barely moved (89.9% -> 90.0%), for the same reason. It does not clear the
    // 75% the outdoor frames answer to, so its ceiling is recorded as a debt rather than asserted
    // away. W4 is about putting light in the room, and this is the number that says by how much.
    // Raised the same way: 0.905 against a frame measuring 0.804 was ten points of slack nobody
    // was using. This one is still a DEBT — 0.804 against a 0.75 target — and it is the garage's
    // last open value bound. Nothing in the reflection or the material work could move it: a
    // specular term and an albedo treatment cannot lift shade, only light can, and the room's
    // shade mass is what the next wave has to argue with.
    const GARAGE_DARK_CEILING_FLOOR: f32 = 0.81;
    const GARAGE_DARK_CEILING_TARGET: f32 = 0.75;
    // B1's second lock: the moody grade may deepen the room, but the bottom of the histogram
    // stays readable on a cheap TN panel — the 5th percentile of the ROOM frame holds a real
    // floor. The first B1 candidate (exposure 1.02, black point 0.028) crushed this to 0.009
    // and read as a black hole on anything but a calibrated display; the shipped grade
    // measures 0.020 against this floor.
    const GARAGE_P05_FLOOR: f32 = 0.015;
    // The screen frame is the room frame plus the overlay and nothing else, so the share of
    // pixels the two disagree on IS the UI's footprint. It is the one measurement that catches a
    // HUD which failed to build, failed to upload, or rendered with no font atlas bound — all
    // three of which produce a perfectly valid-looking picture of an empty hangar that the
    // byte-exact lock would happily re-record. (D13 was this exact failure with a texture.)
    const GARAGE_UI_FOOTPRINT_FLOOR: f32 = 0.15;
    let room_pixels = read_png(&golden_path("garage_hero"));

    for view in client::hangar_review_views() {
        let pixels = read_png(&golden_path(&view.name));
        let stats = frame_stats(&pixels);

        // A screen is not a photograph. The value-structure bounds below describe a lit room —
        // three planes, a shade mass, a bright source — and none of them says anything true about
        // a frame that is half opaque instrument panel. The overlay views answer to their own
        // locks — and every one of them does, not just the hangar screen: the tech tree and the
        // module option list are screens the player reaches with one key or one click.
        if view.screen != client::GarageScreen::Room {
            let differing = room_pixels
                .chunks_exact(4)
                .zip(pixels.chunks_exact(4))
                .filter(|(room, screen)| room != screen)
                .count() as f32
                / (WIDTH * HEIGHT) as f32;
            assert!(
                differing >= GARAGE_UI_FOOTPRINT_FLOOR,
                "{}: the overlay covers {:.1}% of the frame (floor {:.1}%) — the garage UI did \
                 not reach the picture",
                view.name,
                differing * 100.0,
                GARAGE_UI_FOOTPRINT_FLOOR * 100.0
            );
            // Glyphs and plate edges are steps; a UI that lost its text, or drew it in the plate's
            // own colour, collapses toward the flat panel it sits on.
            assert!(
                stats.local_contrast > 0.0,
                "{}: the screen has no edges at all — text and plates both vanished",
                view.name
            );
            println!(
                "GARAGE SCREEN {}: overlay covers {:.1}% of the frame, local contrast {:.4}",
                view.name,
                differing * 100.0,
                stats.local_contrast
            );
            continue;
        }
        // A close-up is a subject photograph (F3): a frame filled by road wheels has no
        // skylight and no lamp in it, so the lit-room plane bounds describe nothing about
        // it — its lock is its SUBJECT_BOUNDS entry, measured inside its own crop.
        if view.close_up {
            println!(
                "GARAGE CLOSE-UP {}: dark {:.3} p50 {:.3} (room planes waived — subject-locked)",
                view.name, stats.dark, stats.p50
            );
            continue;
        }

        assert!(
            stats.dark >= 0.03,
            "{}: the garage lost its shade ({:.2}% of pixels) — a studio without a dark side is \
             a lightbox, not a hangar",
            view.name,
            stats.dark * 100.0
        );
        assert!(
            stats.mid >= 0.05,
            "{}: the garage lost its mid plane ({:.1}% of pixels)",
            view.name,
            stats.mid * 100.0
        );
        assert!(
            stats.bright >= GARAGE_BRIGHT_FLOOR,
            "{}: the bright plane fell below its recorded floor ({:.2}% vs {:.2}%)",
            view.name,
            stats.bright * 100.0,
            GARAGE_BRIGHT_FLOOR * 100.0
        );
        if stats.bright < GARAGE_BRIGHT_TARGET {
            println!(
                "LOOK DEBT {}: bright plane {:.3}, target {:.3} (short by {:.3}, D20, W4)",
                view.name,
                stats.bright,
                GARAGE_BRIGHT_TARGET,
                GARAGE_BRIGHT_TARGET - stats.bright
            );
        }
        // The ceiling runs the other way from a floor: a debt here means TOO MUCH of one plane,
        // so the assert is an upper bound and the target is below the measurement.
        assert!(
            stats.dark <= GARAGE_DARK_CEILING_FLOOR,
            "{}: the dark plane passed its recorded ceiling ({:.3} vs {:.3}) — the room got              darker, not lighter",
            view.name,
            stats.dark,
            GARAGE_DARK_CEILING_FLOOR
        );
        assert!(
            stats.p05 >= GARAGE_P05_FLOOR,
            "{}: the histogram's bottom fell through the TN-readability floor ({:.3} vs {:.3})",
            view.name,
            stats.p05,
            GARAGE_P05_FLOOR
        );
        if stats.dark > GARAGE_DARK_CEILING_TARGET {
            println!(
                "LOOK DEBT {}: dark plane {:.3}, target <= {:.3} (over by {:.3}, D20, W4)",
                view.name,
                stats.dark,
                GARAGE_DARK_CEILING_TARGET,
                stats.dark - GARAGE_DARK_CEILING_TARGET
            );
        }
    }

    // RULE 3, holistically and now on EVERY map that authors both: the golden evening is a
    // genuinely warmer picture than the lead overcast. The light axis has to survive all the way
    // to the final pixels, per map — a warm profile that greys out on one map is a broken look
    // there, whatever the numbers say elsewhere.
    let mut compared = 0;
    for map in REVIEWED_MAPS {
        let (Some(evening), Some(overcast)) = (
            warmth_by_name.get(&format!("{}_golden_evening", map_key(map))),
            warmth_by_name.get(&format!("{}_overcast", map_key(map))),
        ) else {
            continue;
        };
        assert!(
            *evening > *overcast * 1.10,
            "{map:?}: the golden evening must out-warm the overcast day: \
             evening {evening:.3} vs overcast {overcast:.3}"
        );
        compared += 1;
    }
    assert!(compared > 0, "no map authored both a golden evening and an overcast to compare");
}

/// Every recorded frame, measured. Not a pass/fail gate — the BASELINE, printed as the markdown
/// table `docs/art-direction-program.md` carries. The waves that follow move these numbers, and a
/// number nobody wrote down is a number nobody can be held to.
///
/// Run it with output: `cargo test -p client --test look_goldens -- --nocapture measured_baseline`
#[test]
fn the_measured_baseline_of_every_recorded_frame() {
    println!("\n| frame | dark | mid | bright | p05 | p50 | p95 | spread | sat | local | band |");
    println!("|---|---|---|---|---|---|---|---|---|---|---|");

    let mut rows = Vec::new();
    for map in REVIEWED_MAPS {
        let battlefield = map_forge::battlefield(map);
        for view in review_views_for(map, &battlefield) {
            rows.push((view.name.clone(), frame_stats(&read_png(&golden_path(&view.name)))));
        }
    }
    for view in client::hangar_review_views() {
        rows.push((view.name.clone(), frame_stats(&read_png(&golden_path(&view.name)))));
    }

    for (name, s) in &rows {
        println!(
            "| `{name}` | {:.1}% | {:.1}% | {:.1}% | {:.3} | {:.3} | {:.3} | {:.3} | {:.3} | {:.4} | {:+.3} |",
            s.dark * 100.0,
            s.mid * 100.0,
            s.bright * 100.0,
            s.p05,
            s.p50,
            s.p95,
            s.spread,
            s.saturation,
            s.local_contrast,
            s.band_separation
        );
    }

    // The one thing this test DOES assert: every frame was measurable. A view whose golden is
    // missing or truncated must not slip through as a silently absent row.
    assert_eq!(
        rows.len(),
        std::fs::read_dir(goldens_dir()).expect("goldens dir").count(),
        "the baseline table and the golden directory disagree — an orphaned or missing PNG"
    );
}

/// A chroma regression guard, NOT rule 2 restated. Rule 2 bounds the *albedo swatches* at
/// saturation 0.45 and the *profile grade* at 1.30; a graded frame's mean per-pixel saturation
/// is a third quantity and does not answer to either number — the recorded evening frames run
/// to 0.52 and are correct. What this locks is that no change makes the picture gaudy: the
/// ceiling sits above the recorded worst with headroom, and moving it is a deliberate diff.
#[test]
fn no_recorded_frame_runs_away_with_chroma() {
    const CHROMA_CEILING: f32 = 0.60;
    for map in REVIEWED_MAPS {
        let battlefield = map_forge::battlefield(map);
        for view in review_views_for(map, &battlefield) {
            let stats = frame_stats(&read_png(&golden_path(&view.name)));
            assert!(
                stats.saturation <= CHROMA_CEILING,
                "{}: mean frame saturation {:.3} passed the recorded ceiling {CHROMA_CEILING:.2}",
                view.name,
                stats.saturation
            );
        }
    }
}

/// Rule 5 on the pixels: nothing is clean, nothing is noisy. A frame whose local contrast has
/// collapsed is a wash — the "flat reads as cheap" failure the two detail octaves exist to
/// prevent. The floor is the recorded worst; it exists so a change cannot quietly smooth the
/// world out.
#[test]
fn no_recorded_frame_flattens_into_a_wash() {
    for map in REVIEWED_MAPS {
        let battlefield = map_forge::battlefield(map);
        for view in review_views_for(map, &battlefield) {
            let stats = frame_stats(&read_png(&golden_path(&view.name)));
            assert!(
                stats.local_contrast >= 0.0015,
                "{}: local contrast {:.5} — the surface flattened into a wash",
                view.name,
                stats.local_contrast
            );
        }
    }
}

/// Crop a decoded RGBA frame to a normalized `[x0, y0, x1, y1]` box.
fn crop(pixels: &[u8], box_n: [f32; 4]) -> (Vec<u8>, usize, usize) {
    let x0 = (box_n[0] * WIDTH as f32) as usize;
    let y0 = (box_n[1] * HEIGHT as f32) as usize;
    let x1 = ((box_n[2] * WIDTH as f32) as usize).min(WIDTH as usize);
    let y1 = ((box_n[3] * HEIGHT as f32) as usize).min(HEIGHT as usize);
    let mut out = Vec::with_capacity((x1 - x0) * (y1 - y0) * 4);
    for y in y0..y1 {
        let row = y * WIDTH as usize * 4;
        out.extend_from_slice(&pixels[row + x0 * 4..row + x1 * 4]);
    }
    (out, x1 - x0, y1 - y0)
}

/// THE VEHICLE MUST STAY READABLE. Nothing about the light may harm looking at the tank — it is
/// the one object a player stares at for a whole battle, and the frame-wide statistics are blind
/// to it: a tank is a small share of a wide frame, so the picture can lose its entire subject and
/// still report three healthy value planes.
///
/// The failing case is the side the sun never touches. With `dot(n, key) <= 0` the key contributes
/// nothing and the hemispheric ambient alone left hull, tracks and road wheels as one black
/// silhouette — "you cannot see half the tank". This measures INSIDE the authored subject box, so
/// that sentence is a red test rather than a remark on a screenshot.
///
/// Two numbers, because a silhouette fails both ways: `p95` says the brightest part of the
/// vehicle is not crushed, `local_contrast` says the shape still has internal form rather than
/// being one flat mass.
/// Per-view subject bounds. A backlit flank and a sunlit three-quarter are different
/// measurements of different situations, and one global pair of numbers cannot hold both — the
/// attempt is what produced the mis-set bound described below.
struct SubjectBounds {
    view: &'static str,
    /// Recorded medians and dark shares: asserted so the picture cannot regress.
    median_floor: f32,
    dark_ceiling: f32,
    form_floor: f32,
}

const SUBJECT_BOUNDS: &[SubjectBounds] = &[
    SubjectBounds {
        view: "prokhorovka_contact_backlit",
        median_floor: 0.060,
        dark_ceiling: 0.76,
        form_floor: 0.0070,
    },
    // THE SNIPER FRAME (Inny Poziom A7): an enemy T-54 300 m out, backlit, through the scope's
    // 8°. Recorded at first bless 2026-09-02: p50 0.344 / dark 27.9% / form 0.0217 — the crop
    // is a third hull and two thirds field, and the hull carries MORE structure than the chase
    // frame's reference (0.0167), which is the scope doing its job. Floors and ceiling carry the
    // usual ~6-10% slack. Before this entry no locked frame was a scope frame at all.
    // dark_ceiling re-derived 0.32 -> 0.35 at the 2026-09-02 F7 re-bless, openly: neither the
    // subject nor the light changed — the FRAME did. The review harness never drew the
    // instanced tree ladder (every battlefield golden was a map without its oaks; fixed with
    // `battlefield_dressing_objects`), and with the ladder in the frame the fruit tree by the
    // fence draws its near rung inside this crop, so foliage pixels count toward the void
    // (measured 32.6%). The ceiling follows with the usual slack; the metric still cannot
    // tell a leaf from a shadow, which is O2's row.
    SubjectBounds {
        view: "prokhorovka_sniper_contact",
        median_floor: 0.320,
        dark_ceiling: 0.35,
        form_floor: 0.0200,
    },
    // median_floor re-derived 0.110 -> 0.102 at the 2026-08-14 re-bless, openly: the T-54
    // repair + track-tension waves (#563-#570 — fender band raised to the drawing, fittings
    // mirrored to their true sides, taut top run) changed the SUBJECT, not the light. The
    // raised band puts more shaded hull side into this backlit crop (median 0.108), while the
    // corrected shape carries MORE structure, not less: form measures 0.0167 against the
    // 0.0135 floor. The floor follows the intended shape with the usual ~6% slack.
    SubjectBounds {
        view: "prokhorovka_evening_contact",
        median_floor: 0.102,
        dark_ceiling: 0.92,
        form_floor: 0.0135,
    },
    // The showroom subject, and the one the policy makes the strongest claim about. It measures
    // p50 0.161 / dark 64.3% / form 0.0133 — a brighter median than either battlefield subject
    // and the reference frame's structure almost exactly, which is what a studio ought to
    // produce and what nothing checked until now.
    SubjectBounds {
        view: "garage_hero",
        median_floor: 0.145,
        dark_ceiling: 0.68,
        form_floor: 0.0125,
    },
    // F3, the heavy fleet at its own spec-derived boom. Recorded at first bless:
    // tiger2 p50 0.253 / dark 49.6% / form 0.0110; jagdtiger p50 0.283 / dark 43.5% /
    // form 0.0127 — floors and ceilings carry the same ~6-10% slack the hero's do.
    SubjectBounds {
        view: "garage_hero_tiger2",
        median_floor: 0.230,
        dark_ceiling: 0.55,
        form_floor: 0.0100,
    },
    // form_floor re-derived 0.0115 -> 0.0105 at the 2026-08-10 relight (Światło służy
    // czołgowi), openly: the original floor was recorded on a frame where the roof-lattice
    // shadow bars CROSSED the hull and inflated the crop's local contrast — the floor was
    // partly measuring the artifact the relight removed. Measured clean: 0.01146; the new
    // floor carries the same ~9% slack the tiger2 entry does.
    SubjectBounds {
        view: "garage_hero_jagdtiger",
        median_floor: 0.255,
        dark_ceiling: 0.49,
        form_floor: 0.0105,
    },
    // F3's close orbit: the running gear fills the crop, and earth-toned tracks sit almost
    // entirely under the 0.25 luma bar — dark here measures the PAINT (see the note above on
    // why there is no shared void target), so its ceiling is a per-view regression stop, not
    // a readability bar. Recorded: p50 0.086 / dark 92.9% / form 0.0047.
    SubjectBounds {
        view: "garage_susp_close",
        median_floor: 0.078,
        dark_ceiling: 0.95,
        form_floor: 0.0042,
    },
];

/// The reference frame every other subject is judged against: the one `docs/art-direction-program.md`
/// calls golden.
const SUBJECT_REFERENCE_VIEW: &str = "prokhorovka_evening_contact";

#[test]
fn the_vehicle_stays_readable_on_the_side_the_sun_never_touches() {
    // The median is the number that said "you cannot see half the tank": the backlit subject's
    // was 0.016 against this target, with its darkest twentieth at pure 0.000, because the
    // display grade's contrast ran as a straight line and clipped everything below 0.054 to
    // black. With a toe under that line (`display_grade`) and screen AO reconciled against the
    // bakes instead of multiplied into them (`vehicle.wgsl`), it reads 0.070.
    const SUBJECT_MEDIAN_TARGET: f32 = 0.045;

    // WHY THERE IS NO SHARED "VOID" TARGET ANY MORE. There used to be one: dark share <= 0.45.
    // Then the golden frame was given a subject box of its own and scored 89.4% dark — WORSE
    // than the 72.1% of the frame the program calls broken. The bound was not measuring
    // readability at all; `dark` counts pixels under 0.25 linear luma, and a dark-green vehicle
    // is under that almost everywhere it is not in direct sun. It measured how dark the PAINT
    // is. So dark share stays as a per-view regression ceiling, and the readability TARGET moves
    // to the metric that ranked the two frames the way an eye does: local contrast, which reads
    // 0.0145 on the golden frame and read 0.0061 on the broken one.
    //
    // The target is derived from the reference frame rather than invented: two thirds of the
    // structure the golden frame carries. A flank the sun never touches legitimately models less
    // than a sunlit three-quarter — it may not, however, be a flat mass.
    const FORM_TARGET_SHARE_OF_REFERENCE: f32 = 2.0 / 3.0;

    let mut measured = std::collections::HashMap::new();
    for map in REVIEWED_MAPS {
        let battlefield = map_forge::battlefield(map);
        for view in review_views_for(map, &battlefield) {
            let Some(box_n) = view.subject_box else { continue };
            let (cropped, w, h) = crop(&read_png(&golden_path(&view.name)), box_n);
            let stats = frame_stats_of(&cropped, w, h);
            // Always reported, not only when short: the subject's numbers belong in the baseline
            // the same way the frame's do.
            println!(
                "SUBJECT {} ({w}x{h}px): p05 {:.3} p50 {:.3} p95 {:.3} dark {:.1}% form {:.4}",
                view.name,
                stats.p05,
                stats.p50,
                stats.p95,
                stats.dark * 100.0,
                stats.local_contrast
            );
            measured.insert(view.name.clone(), stats);
        }
    }
    // The garage under the same crop. It is the room whose whole job is to sell the vehicle, and
    // it was the one review view with no subject measurement of any kind.
    for view in client::hangar_review_views() {
        let Some(box_n) = view.subject_box else { continue };
        let (cropped, w, h) = crop(&read_png(&golden_path(&view.name)), box_n);
        let stats = frame_stats_of(&cropped, w, h);
        println!(
            "SUBJECT {} ({w}x{h}px): p05 {:.3} p50 {:.3} p95 {:.3} dark {:.1}% form {:.4}",
            view.name,
            stats.p05,
            stats.p50,
            stats.p95,
            stats.dark * 100.0,
            stats.local_contrast
        );
        measured.insert(view.name.clone(), stats);
    }
    assert!(
        !measured.is_empty(),
        "no review view frames a subject — the vehicle is unwatched again"
    );

    let reference_form = measured
        .get(SUBJECT_REFERENCE_VIEW)
        .unwrap_or_else(|| panic!("the reference subject view {SUBJECT_REFERENCE_VIEW} is missing"))
        .local_contrast;
    let form_target = reference_form * FORM_TARGET_SHARE_OF_REFERENCE;

    for bounds in SUBJECT_BOUNDS {
        let stats = measured
            .get(bounds.view)
            .unwrap_or_else(|| panic!("{} lost its subject box", bounds.view));
        debt(
            bounds.view,
            "subject median",
            stats.p50,
            bounds.median_floor,
            SUBJECT_MEDIAN_TARGET,
            "W1",
        );
        debt(
            bounds.view,
            "subject form",
            stats.local_contrast,
            bounds.form_floor,
            form_target,
            "W1",
        );
        assert!(
            stats.dark <= bounds.dark_ceiling,
            "{}: {:.1}% of the vehicle is void, past its recorded ceiling {:.1}% — the light got \
             WORSE at reading the tank",
            bounds.view,
            stats.dark * 100.0,
            bounds.dark_ceiling * 100.0
        );
    }

    // THE STUDIO'S OWN PROMISE, AS A NUMBER. `docs/art-direction-policy.md` says of the garage:
    // "The hero is the brightest, most contrasted, most detailed thing in frame. If the room
    // out-reads the vehicle, the shot has failed no matter how well lit the room is." That
    // sentence had no test — the battlefield had subject crops and the room whose entire job is
    // to sell the vehicle did not.
    //
    // Stated against the frame the subject sits in, which is the only comparison the claim
    // actually makes. It reads 0.161 against 0.089, so the hero is 1.8x the room at the median;
    // the bound bites at 1.4x rather than at the measurement, because this is a floor under a
    // relationship and not a re-recording of one frame's tuning.
    // The floor's own history, because each move was a decision: 1.4 at birth (measured 1.8);
    // raised to 2.0 with B1's moody grade (measured 2.55 — the room darkened faster than the
    // subject); re-derived to 1.7 with E1's sun shafts (measured 1.88 with the MINIMIZED
    // blade set — five narrow beams dying a metre over the floor at the faintest glow that
    // still reads). E1 adds AUTHORED light to the room's air, so the beam-less 2.0 and the
    // plan's beams could not both stand; the floor moved in the open, with the beams named,
    // and it still guards the relationship: a hero under 1.7× has stopped leading its frame.
    const HERO_OVER_ROOM: f32 = 1.7;
    let hero = measured.get("garage_hero").expect("the garage frames its hero");
    let room = frame_stats(&read_png(&golden_path("garage_hero")));
    assert!(
        hero.p50 >= room.p50 * HERO_OVER_ROOM,
        "the garage hero no longer out-reads its own room: subject median {:.3} against frame \
         median {:.3} ({:.2}x, floor {HERO_OVER_ROOM}x)",
        hero.p50,
        room.p50,
        hero.p50 / room.p50.max(1.0e-6)
    );
    println!(
        "GARAGE SUBJECT: hero median {:.3} vs room median {:.3} = {:.2}x",
        hero.p50,
        room.p50,
        hero.p50 / room.p50.max(1.0e-6)
    );
}
