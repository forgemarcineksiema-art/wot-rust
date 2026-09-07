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
use std::path::{Path, PathBuf};

use client::{REVIEWED_MAPS, ReviewView, review_views_for};
use game_core::math::srgb_to_linear;
use scene_build::review_views::map_key;
use terrain::MapId;

const WIDTH: u32 = 960;
const HEIGHT: u32 = 540;

fn goldens_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests").join("goldens").join("look")
}

/// The HUD goldens live beside the look goldens, in their own directory, so the look set's
/// counting locks (`the_measured_baseline_of_every_recorded_frame`) and the garage's screen
/// lock never see them (interface program F8).
fn hud_goldens_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests").join("goldens").join("hud")
}

fn golden_path(name: &str) -> PathBuf {
    goldens_dir().join(format!("{name}.png"))
}

fn hud_golden_path(name: &str) -> PathBuf {
    hud_goldens_dir().join(format!("{name}.png"))
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
    /// The HUD golden instrument's frames (interface program F8).
    Hud,
    All,
}

impl UpdateScope {
    fn from_env() -> Self {
        match std::env::var("WOT_UPDATE_GOLDENS").as_deref() {
            Ok("1" | "all") => Self::All,
            Ok("battlefield") => Self::Battlefield,
            Ok("garage") => Self::Garage,
            Ok("hud") => Self::Hud,
            _ => Self::None,
        }
    }

    fn records_hud(self) -> bool {
        matches!(self, Self::All | Self::Hud)
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
    check_or_record_at(&golden_path(name), name, pixels, record, drift);
}

fn check_or_record_at(
    path: &Path,
    name: &str,
    pixels: &[u8],
    record: bool,
    drift: &mut Vec<String>,
) {
    let path = path.to_path_buf();
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

    // The HUD (interface program F8): every state in every size class over its frozen frame,
    // byte-exact, in its own directory and its own re-record scope.
    if !scope.is_recording() || scope.records_hud() {
        let hud_views = client::hud_review_views();
        let hud_frames =
            client::render_hud_review_views(&hud_views, WIDTH, HEIGHT).expect("hud review render");
        for (view, pixels) in hud_views.iter().zip(&hud_frames) {
            check_or_record_at(
                &hud_golden_path(&view.name),
                &view.name,
                pixels,
                scope.records_hud(),
                &mut drift,
            );
        }
    }

    assert!(
        drift.is_empty(),
        "{} locked frame(s) drifted from their goldens — if the look change is deliberate, \
         re-record with WOT_UPDATE_GOLDENS=1 (or =garage / =battlefield / =hud for one part) and say \
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
    /// Share of pixels below 0.07 linear luma — DEEP shade, the cast-shadow core rule 1 calls a
    /// mass. The dark plane (< 0.25) also counts a shaded field; this counts only what reads as
    /// shadow (D35).
    deep: f32,
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
    /// Mean R over mean B of the top 15 % of rows — the SKY band's warmth, read off the
    /// photograph (D37). A lavender sky measures below 1; a straw evening sky well above.
    sky_warmth: f32,
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
    let mut deep = 0u32;
    let (mut sum_r, mut sum_b, mut sum_sat) = (0.0f64, 0.0f64, 0.0f64);
    let (mut sky_r, mut sky_b) = (0.0f64, 0.0f64);
    let sky_rows_end = ((height * 15) / 100) * width;
    let mut lumas = Vec::with_capacity(width * height);

    for px in pixels.chunks_exact(4) {
        let r = srgb_to_linear(px[0]);
        let g = srgb_to_linear(px[1]);
        let b = srgb_to_linear(px[2]);
        let luma = 0.2126 * r + 0.7152 * g + 0.0722 * b;
        if luma < 0.07 {
            deep += 1;
        }
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
        if lumas.len() < sky_rows_end {
            sky_r += r as f64;
            sky_b += b as f64;
        }
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
        deep: deep as f32 / n,
        mid: mid as f32 / n,
        bright: bright as f32 / n,
        mean_warmth: (sum_r / sum_b.max(1.0e-9)) as f32,
        p05,
        p50: percentile(&sorted, 0.50),
        p95,
        spread: p95 - p05,
        saturation: (sum_sat / n as f64) as f32,
        near_white: near_white as f32 / n,
        sky_warmth: (sky_r / sky_b.max(1.0e-9)) as f32,
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
/// Rule 1 wants a real shade mass in every frame, not a token one. ASSERTED on the sunward
/// frame since D35 (`the_sunward_frame_carries_rule_ones_shade_mass`); every antisolar frame
/// still reports its distance as a debt — looking away from the sun, a frame cannot see the
/// shade the sun casts, which is exactly why the fleet floor sat at 0.0055 for a year.
const OUTDOOR_DARK_TARGET: f32 = 0.08;
/// D35: the sunward frame's recorded DEEP shade (< 0.07 linear) — 15.8 % on the record, against
/// 1.8 % on the antisolar contact frame and 0.1 % on the reference frame. The floor sits under
/// the recording with room for the grain, and the target is the recording itself.
/// D37: the least warm sky band of any recorded golden-evening frame, with room for grain.
const GOLDEN_SKY_WARMTH_FLOOR: f32 = 1.05;
const SUNWARD_DEEP_FLOOR: f32 = 0.10;
const SUNWARD_DEEP_TARGET: f32 = 0.15;
/// The view that looks INTO the evening sun from the player's seat (`review_views.rs`, D35).
const SUNWARD_VIEW: &str = "prokhorovka_evening_into_sun";
/// The antisolar reference frame the whole art direction aims at — the one whose shadows all
/// hid behind their casters until the sunward frame was added.
const ANTISOLAR_REFERENCE_VIEW: &str = "prokhorovka_golden_evening";
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

/// D37: the golden evening's sky is straw, not lavender — measured on the PHOTOGRAPH. The
/// two-stop dome mixed a blue zenith and an orange horizon linearly and the played band came
/// out (0.50, 0.44, 0.46): magenta-grey under an amber sun. Every recorded frame lit by the
/// golden evening profile (selected by its data, not by name) must carry a sky band whose mean
/// R exceeds its mean B — the top 15 % of rows, which on every chase frame is sky.
#[test]
fn the_golden_evening_sky_is_straw_not_lavender_on_the_record() {
    let evening = renderer_api::SceneLighting::prokhorovka_golden_evening();
    let mut judged = 0;
    for map in REVIEWED_MAPS {
        let battlefield = map_forge::battlefield(map);
        for view in review_views_for(map, &battlefield) {
            if view.vertical_fov_degrees.is_some()
                || view.lighting.sky_band_rgb != evening.sky_band_rgb
            {
                continue;
            }
            let stats = frame_stats(&read_png(&golden_path(&view.name)));
            assert!(
                stats.sky_warmth >= GOLDEN_SKY_WARMTH_FLOOR,
                "{}: the evening sky band reads lavender on the record (R/B {:.3} < {:.2})",
                view.name,
                stats.sky_warmth,
                GOLDEN_SKY_WARMTH_FLOOR
            );
            judged += 1;
        }
    }
    assert!(judged >= 4, "the golden evening must be on the record in at least four frames");
}

/// D41, on the record: the evening contact frame's LOWER HULL — the band the mud climbs and
/// the running gear — reads darker and warmer than its sunlit deck. Two crops authored against
/// the rendered frame (the deck: turret roof and hull top; the lower band: hull side under the
/// fender and the running gear). Before D41 the two crops' warmth ratio was 1.21 (one paint
/// tone, top to bottom); with the mud band it measures 1.40. The floor sits between.
#[test]
fn the_lower_hull_is_darker_and_warmer_than_the_deck_on_the_record() {
    const DECK: [f32; 4] = [0.44, 0.61, 0.58, 0.67];
    const LOWER_HULL: [f32; 4] = [0.40, 0.77, 0.60, 0.84];
    // 1.40 on the D41 record; 1.30 after D42 gave the evening's sky ambient the sky's blue
    // (the deck takes the sky, the lower hull does not). The floor keeps its slack.
    const MIN_WARMTH_RATIO: f32 = 1.25;
    let pixels = read_png(&golden_path(SUBJECT_REFERENCE_VIEW));
    let (deck, w, h) = crop(&pixels, DECK);
    let deck = frame_stats_of(&deck, w, h);
    let (lower, w, h) = crop(&pixels, LOWER_HULL);
    let lower = frame_stats_of(&lower, w, h);
    assert!(
        lower.p50 < deck.p50,
        "the lower hull ({:.3}) must read darker than the deck ({:.3})",
        lower.p50,
        deck.p50
    );
    let ratio = lower.mean_warmth / deck.mean_warmth.max(1.0e-6);
    assert!(
        ratio >= MIN_WARMTH_RATIO,
        "the lower hull must read warmer than the deck by {MIN_WARMTH_RATIO:.2}x (the mud band):          {ratio:.2}x"
    );
}

/// D42, on the record: the SAME hull under the hangar's rig and the field's reads as the same
/// paint. The hangar adds its own ambient, a GI probe, a showroom exposure and a dust film; the
/// field lights the hull with an amber sun — the LIGHT may differ, the MATERIAL may not. So the
/// lock is on chroma, not on warmth or luminance: the hero's subject crop in the garage and the
/// reference subject crop in the field carry a mean saturation within `PAINT_CHROMA_BAND` of
/// each other, and neither crop's median luminance is more than `PAINT_LUMA_RATIO_CEILING`
/// times the other's. The bible (`docs/vehicle-presentation-bible.md`) quotes this lock and the
/// hangar constants it reasons about; until D42 it asserted parity with nothing measuring it.
///
/// The crops are the TURRET ROOF in both frames (authored against the rendered goldens) — the
/// subject boxes hold grass in one and the hall's floor in the other, and a crop's mean
/// saturation is mostly its background. FLOOR/TARGET, like the rest of this file: the band is
/// a ceiling on the recorded gap (0.255 on 2026-09-07: field 0.787, hangar 0.532 — the amber
/// key on olive, after the grade's saturation came down from 1.25 and the sky ambient took the
/// sky's blue), and the distance to `PAINT_CHROMA_TARGET` is printed as a debt.
#[test]
fn the_hull_wears_the_same_paint_in_the_hangar_and_on_the_field() {
    const FIELD_TURRET_ROOF: [f32; 4] = [0.46, 0.62, 0.54, 0.66];
    const HANGAR_TURRET_ROOF: [f32; 4] = [0.45, 0.38, 0.55, 0.44];
    const PAINT_CHROMA_BAND: f32 = 0.30;
    const PAINT_CHROMA_TARGET: f32 = 0.15;
    const PAINT_LUMA_RATIO_CEILING: f32 = 1.8;
    let crop_stats = |name: &str, box_n: [f32; 4]| {
        let (cropped, w, h) = crop(&read_png(&golden_path(name)), box_n);
        frame_stats_of(&cropped, w, h)
    };
    let field = crop_stats(SUBJECT_REFERENCE_VIEW, FIELD_TURRET_ROOF);
    let hangar = crop_stats("garage_hero", HANGAR_TURRET_ROOF);
    let gap = (field.saturation - hangar.saturation).abs();
    if gap > PAINT_CHROMA_TARGET {
        println!(
            "LOOK DEBT paint parity: turret chroma gap {gap:.3}, target {PAINT_CHROMA_TARGET:.2}              (short by {:.3}, D42)",
            gap - PAINT_CHROMA_TARGET
        );
    }
    println!(
        "PAINT PARITY field sat {:.3} p50 {:.3} | hangar sat {:.3} p50 {:.3}",
        field.saturation, field.p50, hangar.saturation, hangar.p50
    );
    assert!(
        gap <= PAINT_CHROMA_BAND,
        "the hull's chroma differs between the field ({:.3}) and the hangar ({:.3}) by more than          {PAINT_CHROMA_BAND}: two paints for one tank",
        field.saturation,
        hangar.saturation
    );
    let ratio = field.p50.max(hangar.p50) / field.p50.min(hangar.p50).max(1.0e-4);
    assert!(
        ratio <= PAINT_LUMA_RATIO_CEILING,
        "the hangar and the field light the same hull {ratio:.2}x apart in median luminance          (field {:.3}, hangar {:.3}) — the showroom is lying about the paint",
        field.p50,
        hangar.p50
    );
}

/// D35, the one program: the reference set looked +X with the sun at -X, so every cast shadow
/// hid behind its caster and rule 1's shade mass was certified from frames that could not see
/// it (the dark plane's fleet floor: 0.55 %). The sunward frame looks into the sun from the
/// player's seat, and on it the policy's TARGET is asserted, not reported: a real shade mass
/// (>= 8 % dark) and a real cast-shadow core (>= 10 % deep). The antisolar reference frame
/// must carry LESS deep shade than the sunward one — if it ever carries more, the frames have
/// been swapped or the sun has moved, and the lock is looking at the wrong picture.
#[test]
fn the_sunward_frame_carries_rule_ones_shade_mass() {
    let sunward = frame_stats(&read_png(&golden_path(SUNWARD_VIEW)));
    assert!(
        sunward.dark >= OUTDOOR_DARK_TARGET,
        "{SUNWARD_VIEW}: {:.1}% dark against rule 1's target of {:.0}% — looking into the sun          the picture has no shade mass",
        sunward.dark * 100.0,
        OUTDOOR_DARK_TARGET * 100.0
    );
    debt(SUNWARD_VIEW, "deep shade", sunward.deep, SUNWARD_DEEP_FLOOR, SUNWARD_DEEP_TARGET, "D35");
    assert!(
        sunward.near_white <= 0.015,
        "{SUNWARD_VIEW}: {:.2}% pure white — the sun-side haze is milk again (D3)",
        sunward.near_white * 100.0
    );
    let antisolar = frame_stats(&read_png(&golden_path(ANTISOLAR_REFERENCE_VIEW)));
    assert!(
        antisolar.deep < sunward.deep,
        "the antisolar reference frame ({:.1}% deep) out-shades the sunward one ({:.1}%) — the          sun has moved or the frames have swapped",
        antisolar.deep * 100.0,
        sunward.deep * 100.0
    );
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
    // G13 (2026-09-06): the garage's own program re-measured every room frame after the
    // interface rebuilt the screens over it — the hero 0.766, the inspector 0.717, the Tiger II
    // 0.734, the Jagdtiger 0.711 — so the ceiling comes down to 0.78 with the number, and the
    // 0.75 target stays a debt of the hero frame alone (0.016).
    const GARAGE_DARK_CEILING_FLOOR: f32 = 0.78;
    const GARAGE_DARK_CEILING_TARGET: f32 = 0.75;
    // B1's second lock: the moody grade may deepen the room, but the bottom of the histogram
    // stays readable on a cheap TN panel — the 5th percentile of the ROOM frame holds a real
    // floor. The first B1 candidate (exposure 1.02, black point 0.028) crushed this to 0.009
    // and read as a black hole on anything but a calibrated display; the shipped grade
    // measures 0.020 against this floor.
    // G13 (2026-09-06): the lowest room frame measures 0.024 (the inspector views), so the
    // floor rises to 0.02 with the number.
    const GARAGE_P05_FLOOR: f32 = 0.02;
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
///
/// Moved 0.60 -> 0.63 on 2026-09-07 (D37): the golden evening's sky band went from lavender
/// grey to straw and its key from 1.32 to a warm 1.72 (D34), and `prokhorovka_evening_contact`
/// measured 0.563 -> 0.603 — the chroma is in the sky and the light, where rule 2 puts it; the
/// ground swatches did not move.
#[test]
fn no_recorded_frame_runs_away_with_chroma() {
    const CHROMA_CEILING: f32 = 0.63;
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
    // dark_ceiling 0.35 -> 0.42 at the species-and-variants re-bless (route 2, 2026-09-02,
    // late): the fruit tree by the fence is an authored, dense one now and its variant is
    // the position's; more foliage pixels in the crop, the same tank under the same light.
    SubjectBounds {
        view: "prokhorovka_sniper_contact",
        median_floor: 0.320,
        dark_ceiling: 0.42,
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
    //
    // Re-derived 2026-09-07 (K24-1), openly: those numbers were recorded off the pale
    // SHOWROOM tint (luma 0.74) that no vehicle wore in battle; the heroes now wear their
    // nation's coat — dunkelgelb, luma 0.55 — so the subject's median falls with the PAINT,
    // not the light — and the hall itself darkened in the same PR (D33 decided: concrete
    // 0.30 → 0.18, whitewash 0.38 → 0.25) so the hero keeps leading its room in a real coat.
    // Measured in the coat, in the darker hall: tiger2 p50 0.183 / dark 63.5% / form 0.0090;
    // jagdtiger p50 0.198 / dark 60.6% / form 0.0103. Same ~8% slack on every bound.
    SubjectBounds {
        view: "garage_hero_tiger2",
        median_floor: 0.168,
        dark_ceiling: 0.69,
        form_floor: 0.0083,
    },
    // form_floor re-derived 0.0115 -> 0.0105 at the 2026-08-10 relight (Światło służy
    // czołgowi), openly: the original floor was recorded on a frame where the roof-lattice
    // shadow bars CROSSED the hull and inflated the crop's local contrast — the floor was
    // partly measuring the artifact the relight removed. Measured clean: 0.01146; the new
    // floor carries the same ~9% slack the tiger2 entry does.
    SubjectBounds {
        view: "garage_hero_jagdtiger",
        median_floor: 0.182,
        dark_ceiling: 0.66,
        // 0.0095 -> 0.0082 at D40/D42 (2026-09-07), openly: the vehicle maps upload a
        // filtered mip chain now, and the per-texel hash the old floor was partly measuring
        // as "form" is gone (rule 5). Measured clean: 0.0090; the same ~9 % slack.
        form_floor: 0.0082,
    },
    // F3's close orbit: the running gear fills the crop, and earth-toned tracks sit almost
    // entirely under the 0.25 luma bar — dark here measures the PAINT (see the note above on
    // why there is no shared void target), so its ceiling is a per-view regression stop, not
    // a readability bar. Recorded: p50 0.086 / dark 92.9% / form 0.0047.
    SubjectBounds {
        view: "garage_susp_close",
        median_floor: 0.078,
        dark_ceiling: 0.95,
        // 0.0042 -> 0.0037 at D40/D42 (2026-09-07), the same reason as the Jagdtiger's: the
        // filtered mip chain took the texel noise out of "form". Measured clean: 0.0040.
        form_floor: 0.0037,
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
    // Re-derived to 1.5 on 2026-09-05 (measured 1.56): T3's tiled ground material (`51b8f224`)
    // brightened the hall's floor and walls — the room's median rose 0.162 → 0.193 with the hero
    // unchanged at 0.300 — and blessed the battlefield only, so the change surfaced at the next
    // bless, the interface program's F1. The floor follows the measurement; whether the hall
    // should darken again is the light lane's row (art-direction D33), not this bless's call.
    const HERO_OVER_ROOM: f32 = 1.5;
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

/// The HUD frames carry the interface (interface program F8): each one differs from the look
/// golden of the frame it sits on by at least the footprint floor — the one measurement that
/// catches a HUD which failed to build, upload, or bind its atlas — and none of them blows out
/// to white, which is what a lamp glow gone wrong would do. Always-on, over the committed PNGs.
/// H23: every battle readout on a plate reads at three to one, MEASURED on the golden — the
/// ink of each text element against the mean of the frame's pixels on the plate around it
/// (the plate under the text, whatever the scene behind it did), in the frame the game draws.
#[test]
fn every_battle_readout_sits_on_glass_at_three_to_one() {
    use ui_kit::draw_list::Payload;
    const FLOOR: f32 = 3.0;
    let mut checked = 0usize;
    let mut offenders: Vec<String> = Vec::new();
    for view in client::hud_review_views() {
        // The editor's panes veil the instruments on purpose (H21): the crew arranges there,
        // it does not read; the floor is for the battle's own frames.
        if view.state == client::HudState::HudEditorOpen {
            continue;
        }
        let pixels = read_png(&hud_golden_path(&view.name));
        let list = client::hud_state_list(view.state, view.size, WIDTH, HEIGHT);
        let elements: Vec<_> = list.iter().collect();
        for text in &elements {
            let Payload::Text { color, text: word, .. } = &text.payload else { continue };
            // What is dimmed by design — a dead or withheld row (the emitter dims a disabled
            // element), a dim token, a unit tag — is not a readout the crew acts on; the floor
            // is for the ink at full light.
            if text.state == ui_kit::draw_list::WidgetState::Disabled
                || color[3] <= 0.85
                || word.trim().is_empty()
            {
                continue;
            }
            // The plate the text sits on: the largest plate or glass whose rectangle holds it.
            let plate = elements
                .iter()
                .filter(|e| {
                    matches!(e.payload, Payload::Plate { .. } | Payload::Glass { .. })
                        && e.rect.encloses(&text.rect)
                        && e.rect.w * e.rect.h > text.rect.w * text.rect.h * 1.2
                })
                .max_by(|a, b| (a.rect.w * a.rect.h).total_cmp(&(b.rect.w * b.rect.h)));
            let Some(plate) = plate else { continue };
            // The frame's pixels on the plate in a ring around the text — never the text's own
            // ink, never the far side of the plate.
            let (mut sum, mut n) = ([0.0f32; 3], 0usize);
            let ring = text.rect.inset(-6.0);
            let (px0, py0) = (
                ring.x.max(plate.rect.x).max(0.0) as usize,
                ring.y.max(plate.rect.y).max(0.0) as usize,
            );
            let (px1, py1) = (
                ring.right().min(plate.rect.right()).min(WIDTH as f32) as usize,
                ring.bottom().min(plate.rect.bottom()).min(HEIGHT as f32) as usize,
            );
            for y in py0..py1 {
                for x in px0..px1 {
                    if text.rect.contains([x as f32 + 0.5, y as f32 + 0.5]) {
                        continue;
                    }
                    let at = (y * WIDTH as usize + x) * 4;
                    for c in 0..3 {
                        sum[c] += pixels[at + c] as f32 / 255.0;
                    }
                    n += 1;
                }
            }
            if n < 16 {
                continue;
            }
            let ground = [sum[0] / n as f32, sum[1] / n as f32, sum[2] / n as f32, 1.0];
            // The ink as drawn: its own alpha over the ground.
            let ink = [
                ground[0] + (color[0] - ground[0]) * color[3],
                ground[1] + (color[1] - ground[1]) * color[3],
                ground[2] + (color[2] - ground[2]) * color[3],
                1.0,
            ];
            let ratio = ui_kit::theme::contrast_ratio(ink, ground);
            if ratio < FLOOR {
                offenders.push(format!(
                    "{}: {:?} „{word}\" reads {ratio:.2}:1 on its plate",
                    view.name, text.id
                ));
            }
            checked += 1;
        }
    }
    assert!(checked > 100, "the floor walked {checked} readouts — the instrument is blind");
    assert!(
        offenders.is_empty(),
        "readouts under the {FLOOR}:1 floor:
  {}",
        offenders.join(
            "
  "
        )
    );
}

#[test]
fn every_hud_frame_carries_the_interface_and_none_blows_out() {
    const HUD_UI_FOOTPRINT_FLOOR: f32 = 0.02;
    const HUD_NEAR_WHITE_CEILING: f32 = 0.03;
    for view in client::hud_review_views() {
        let base_name = if view.state.sniper() {
            client::HUD_REVIEW_SNIPER_VIEW
        } else {
            client::HUD_REVIEW_THIRD_PERSON_VIEW
        };
        let base = read_png(&golden_path(base_name));
        let pixels = read_png(&hud_golden_path(&view.name));
        let differing =
            base.chunks_exact(4).zip(pixels.chunks_exact(4)).filter(|(a, b)| a != b).count() as f32
                / (WIDTH * HEIGHT) as f32;
        let stats = frame_stats(&pixels);
        println!(
            "HUD FRAME {}: interface covers {:.2}% of the frame, near white {:.4}, local contrast {:.4}",
            view.name,
            differing * 100.0,
            stats.near_white,
            stats.local_contrast
        );
        assert!(
            differing >= HUD_UI_FOOTPRINT_FLOOR,
            "{}: the interface covers {:.2}% of the frame (floor {:.1}%) — the HUD did not reach the picture",
            view.name,
            differing * 100.0,
            HUD_UI_FOOTPRINT_FLOOR * 100.0
        );
        assert!(
            stats.near_white <= HUD_NEAR_WHITE_CEILING,
            "{}: {:.3} of the frame is near white — a glow or a plate blew out",
            view.name,
            stats.near_white
        );
    }
}
