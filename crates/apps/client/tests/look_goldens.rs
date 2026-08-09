//! The GPU half of the look harness (`docs/art-direction-policy.md`): renders the canonical
//! review views (`client::review_views_for` — the same table the human-review examples draw)
//! for EVERY shipped map and locks them. The weather roll is random per battle, so every look a
//! blueprint declares is locked here; a look this file skips is a look the player meets
//! unreviewed.
//!
//! History this file exists to not repeat — the same lesson `studio_goldens.rs` learned first,
//! which this harness never got: the compare loop was one `assert_eq!` per frame, so the FIRST
//! recording that moved ended the run. A drift report named one frame out of twenty-four and
//! left the rest unmeasured while reading as green, and it carried no picture of what moved, so
//! the only way to review a re-record was to bless it and look afterwards. Now every mismatch is
//! collected and measured (share of pixels, largest and mean level step), the biggest mover is
//! reported first, and each one leaves `recorded` / `fresh` / `delta` pictures in
//! `target/look-diff/` to be looked at BEFORE deciding the change was intended.
//!
//! Two layers:
//! - `look_goldens_match_their_recordings` — OPT-IN via `WOT_LOOK_GOLDENS=1` (needs a GPU;
//!   byte-exact per machine, like the studio goldens). Re-record with `WOT_UPDATE_GOLDENS=1`
//!   after a deliberate look change, in its own commit that says why.
//! - the drift-scan locks (`the_drift_scan_names_every_frame_that_moved_not_just_the_first` and
//!   its three neighbours) — always-on and CPU-only, so the promise above survives on a machine
//!   with no GPU at all.
//! - the rest — always-on and CPU-only: they decode the committed golden PNGs, so they catch a
//!   policy violation in any committed look on a machine with no GPU at all.
//!   `recorded_goldens_hold_the_value_structure` is rule 1 (three value planes) and rule 3 (the
//!   evening out-warms the overcast, per map). `no_recorded_frame_flattens_into_a_wash` and
//!   `no_recorded_frame_runs_away_with_chroma` are regression guards on detail and chroma.
//!   `the_measured_baseline_of_every_recorded_frame` asserts almost nothing — it PRINTS the
//!   table that `docs/art-direction-program.md` carries, because a number nobody wrote down is
//!   a number nobody can be held to.

use std::cmp::Reverse;
use std::fs::File;
use std::io::{BufReader, BufWriter};
use std::path::{Path, PathBuf};

use client::{REVIEWED_MAPS, ReviewView, review_views_for};
use terrain::MapId;

const WIDTH: u32 = 960;
const HEIGHT: u32 = 540;

/// Every look a shipped blueprint declares, plus the four garage views: the number of frames the
/// harness must actually reach. A floor, not a ceiling — adding a view is fine, quietly losing
/// one is the failure this catches, because a harness that compares nothing also reports green.
const LOCKED_FRAME_COUNT: usize = 24;

/// The delta picture is AMPLIFIED by this factor. At 1:1 a tone-curve move of two levels is an
/// all-black image, and an invisible diff is the same as no diff for the person who has to
/// approve the re-record. Sixteen makes a 1/255 step visible without saturating a real change.
const DELTA_AMPLIFY: u16 = 16;

fn goldens_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests").join("goldens").join("look")
}

fn golden_path(name: &str) -> PathBuf {
    goldens_dir().join(format!("{name}.png"))
}

/// Where a failing run leaves its pictures. Outside the source tree on purpose: these are
/// evidence for one review, not artefacts anyone commits.
///
/// Walked up rather than joined with `../../..`, because this path ends up in a failure message
/// somebody has to paste into a file browser, and `crates/apps/client/../../../target` is not an
/// address — it is a puzzle.
fn diff_dir() -> PathBuf {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let root = manifest.ancestors().nth(3).unwrap_or(manifest.as_path());
    root.join("target").join("look-diff")
}

/// The harness renders through `client::render_review_views` — the SAME entry the `*_views`
/// examples draw with. They used to hand-roll this setup separately, which is exactly how both
/// of them lost the foliage-atlas bind and started locking white trees.
fn render_views(map: MapId, views: &[ReviewView]) -> Vec<Vec<u8>> {
    client::render_review_views(map, views, WIDTH, HEIGHT).expect("review render")
}

/// Best-effort write, for the diff pictures. A full disk must not turn a drift report into a
/// panic about the evidence: the list of what moved is worth more than the pictures of it.
fn try_write_png(path: &Path, pixels: &[u8]) -> bool {
    let Some(parent) = path.parent() else { return false };
    if std::fs::create_dir_all(parent).is_err() {
        return false;
    }
    let Ok(file) = File::create(path) else { return false };
    let mut encoder = png::Encoder::new(BufWriter::new(file), WIDTH, HEIGHT);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let Ok(mut writer) = encoder.write_header() else { return false };
    writer.write_image_data(pixels).is_ok()
}

/// Recording a golden, where failing to write IS the failure.
fn write_png(path: &Path, pixels: &[u8]) {
    assert!(try_write_png(path, pixels), "could not write {}", path.display());
}

/// Decode a golden for COMPARISON: a missing file, a corrupt one or a stale size is a finding to
/// collect and report beside the others, never a panic. A panic in the compare loop is exactly
/// what let one drifted frame hide the twenty-three behind it.
fn try_read_png(path: &Path) -> Result<Vec<u8>, String> {
    let file =
        File::open(path).map_err(|_| "missing — record with WOT_UPDATE_GOLDENS=1".to_string())?;
    let decoder = png::Decoder::new(BufReader::new(file));
    let mut reader = decoder.read_info().map_err(|err| format!("unreadable png ({err})"))?;
    let size = reader.output_buffer_size().ok_or_else(|| "png size overflows".to_string())?;
    let mut buf = vec![0u8; size];
    let info = reader.next_frame(&mut buf).map_err(|err| format!("undecodable png ({err})"))?;
    if (info.width, info.height) != (WIDTH, HEIGHT) {
        return Err(format!(
            "stale dimensions {}×{}, the harness renders {WIDTH}×{HEIGHT} — re-record",
            info.width, info.height
        ));
    }
    buf.truncate(info.buffer_size());
    Ok(buf)
}

/// Reading a golden the CPU-only gates measure, where a missing recording is a hard stop.
fn read_png(path: &Path) -> Vec<u8> {
    try_read_png(path).unwrap_or_else(|reason| panic!("golden {}: {reason}", path.display()))
}

/// One frame's disagreement with its recording, MEASURED rather than hashed.
///
/// A digest answers "did it move", which is all the studio tiles need — they are line art on a
/// flat field, and any move there is a shape change. A graded photograph is different: nearly
/// every pixel changes when an exposure knob turns, and almost none do when a hatch moves. The
/// review question is *how much, and everywhere or in one corner*, and only numbers answer it.
struct Drift {
    view: String,
    /// Pixels differing in any channel.
    differing: usize,
    total: usize,
    /// Largest single-channel step, 0–255. Separates a re-tuned curve (small, everywhere) from
    /// moved geometry (large, local).
    max_delta: u8,
    /// Mean channel step across the channels that moved. Averaging over the whole frame instead
    /// would report "nothing happened" for a change confined to the tank.
    mean_delta: f32,
}

impl Drift {
    fn share(&self) -> f32 {
        if self.total == 0 { 0.0 } else { self.differing as f32 / self.total as f32 }
    }

    fn line(&self) -> String {
        format!(
            "{}: {:.1}% of pixels moved ({} of {}), max step {}/255, mean step {:.1}",
            self.view,
            self.share() * 100.0,
            self.differing,
            self.total,
            self.max_delta,
            self.mean_delta,
        )
    }
}

/// `None` when the frame is byte-identical to its recording — the same verdict the old
/// `assert_eq!` reached, without ending the run for the frames behind it.
fn frame_drift(view: &str, golden: &[u8], fresh: &[u8]) -> Option<Drift> {
    assert_eq!(
        golden.len(),
        fresh.len(),
        "{view}: comparing frames of different sizes — try_read_png should have caught this"
    );
    if golden == fresh {
        return None;
    }

    let mut differing = 0usize;
    let mut max_delta = 0u8;
    let mut delta_sum = 0u64;
    let mut moved_channels = 0u64;
    for (recorded, current) in golden.chunks_exact(4).zip(fresh.chunks_exact(4)) {
        if recorded == current {
            continue;
        }
        differing += 1;
        for (a, b) in recorded.iter().zip(current) {
            let step = a.abs_diff(*b);
            if step > 0 {
                max_delta = max_delta.max(step);
                delta_sum += u64::from(step);
                moved_channels += 1;
            }
        }
    }

    Some(Drift {
        view: view.to_string(),
        differing,
        total: golden.len() / 4,
        max_delta,
        mean_delta: if moved_channels == 0 {
            0.0
        } else {
            delta_sum as f32 / moved_channels as f32
        },
    })
}

/// The three pictures a bless needs: what was recorded, what renders today, and where they
/// disagree. Side-by-side alone is not enough — two frames that differ by a few levels look
/// identical to the eye, which is how a real regression gets waved through as "looks the same".
fn write_diff(view: &str, golden: &[u8], fresh: &[u8]) {
    let dir = diff_dir();
    try_write_png(&dir.join(format!("{view}.recorded.png")), golden);
    try_write_png(&dir.join(format!("{view}.fresh.png")), fresh);

    let delta: Vec<u8> = golden
        .chunks_exact(4)
        .zip(fresh.chunks_exact(4))
        .flat_map(|(recorded, current)| {
            let step = recorded.iter().zip(current).map(|(a, b)| a.abs_diff(*b)).max().unwrap_or(0);
            let lit = u8::try_from(u16::from(step) * DELTA_AMPLIFY).unwrap_or(u8::MAX);
            [lit, lit, lit, 255]
        })
        .collect();
    try_write_png(&dir.join(format!("{view}.delta.png")), &delta);
}

/// What one pass over every view accumulates. A struct rather than four locals because the two
/// loops below — the maps and the garage — must feed the SAME totals; the previous shape had the
/// comparison written out twice, which is how the two halves drift apart in the first place.
#[derive(Default)]
struct Scan {
    drifted: Vec<Drift>,
    /// Goldens that could not be compared at all, with why. Counted separately from drift: a
    /// missing recording is a hole in the gate, not a change in the picture.
    unusable: Vec<String>,
    recorded: usize,
    compared: usize,
    write_pictures: bool,
}

impl Scan {
    /// The gate's scan: leaves the three review pictures for every frame that moved.
    fn reviewing() -> Self {
        Self { write_pictures: true, ..Self::default() }
    }

    /// The scan the CPU locks below use: identical collection, no pictures. A passing test must
    /// not leave evidence of a drift that never happened — the next person to open
    /// `target/look-diff/` has to be able to trust that everything in it is real.
    fn measuring_only() -> Self {
        Self::default()
    }

    fn visit(&mut self, update: bool, name: &str, pixels: &[u8]) {
        let path = golden_path(name);
        if update {
            write_png(&path, pixels);
            eprintln!("recorded {}", path.display());
            self.recorded += 1;
            return;
        }
        match try_read_png(&path) {
            Err(reason) => self.unusable.push(format!("{name}: {reason}")),
            Ok(golden) => {
                self.compared += 1;
                if let Some(drift) = frame_drift(name, &golden, pixels) {
                    if self.write_pictures {
                        write_diff(name, &golden, pixels);
                    }
                    self.drifted.push(drift);
                }
            }
        }
    }
}

#[test]
fn look_goldens_match_their_recordings() {
    if std::env::var("WOT_LOOK_GOLDENS").as_deref() != Ok("1")
        && std::env::var("WOT_UPDATE_GOLDENS").as_deref() != Ok("1")
    {
        eprintln!("skipping look goldens (set WOT_LOOK_GOLDENS=1 to enable)");
        return;
    }
    let update = std::env::var("WOT_UPDATE_GOLDENS").as_deref() == Ok("1");
    let mut scan = Scan::reviewing();

    for map in REVIEWED_MAPS {
        let battlefield = map_forge::battlefield(map);
        let views = review_views_for(map, &battlefield);
        let frames = render_views(map, &views);

        for (view, pixels) in views.iter().zip(&frames) {
            scan.visit(update, &view.name, pixels);
        }
    }

    // The garage: an interior studio with its own light rig and its own lens, but the same
    // display transform and the same locks. It had no golden at all before this.
    let hangar_views = client::hangar_review_views();
    let hangar_frames = client::render_hangar_review_views(&hangar_views, WIDTH, HEIGHT)
        .expect("hangar review render");
    for (view, pixels) in hangar_views.iter().zip(&hangar_frames) {
        scan.visit(update, &view.name, pixels);
    }

    if update {
        // A bless is a deliberate act: say exactly what was rewritten so the commit can too.
        println!("re-recorded {} look frames", scan.recorded);
        assert!(scan.recorded > 0, "update mode must record something");
        return;
    }

    // The byte-exact contract this harness rests on: the same view renders identically twice
    // on one machine (the render is a pure function of scene + profile + the fixed clock).
    // Measured BEFORE the verdict rather than after it, because it is the one fact that decides
    // how to read a drift list — and asserting it last meant it never ran on the runs that
    // produced one.
    let map = REVIEWED_MAPS[0];
    let battlefield = map_forge::battlefield(map);
    let views = review_views_for(map, &battlefield);
    let once = render_views(map, &views[..1]);
    let again = render_views(map, &views[..1]);
    let deterministic = once[0] == again[0];

    assert!(
        scan.unusable.is_empty(),
        "{} look golden(s) could not be compared — the gate measured nothing for them:\n  {}",
        scan.unusable.len(),
        scan.unusable.join("\n  ")
    );

    scan.drifted.sort_by_key(|drift| Reverse(drift.differing));
    assert!(
        scan.drifted.is_empty(),
        "{} of {} look frames drifted from their recordings (biggest mover first). Look at the \
         pictures in {} FIRST — <view>.recorded.png, <view>.fresh.png and <view>.delta.png (the \
         absolute difference, amplified ×{DELTA_AMPLIFY} so a two-level move is visible). \
         Re-record with WOT_UPDATE_GOLDENS=1 only once the change is understood and intended, in \
         its own commit that says why. Renderer determinism on this machine: {}.\n  {}",
        scan.drifted.len(),
        scan.compared,
        diff_dir().display(),
        if deterministic {
            "holds, so every line below is a real change"
        } else {
            "BROKEN — fix that first, the list below cannot be trusted"
        },
        scan.drifted.iter().map(Drift::line).collect::<Vec<_>>().join("\n  ")
    );

    assert!(deterministic, "the render must be deterministic on one machine");
    assert!(
        scan.compared >= LOCKED_FRAME_COUNT,
        "the look gate must actually compare every declared frame (got {}, expected at least \
         {LOCKED_FRAME_COUNT})",
        scan.compared
    );
}

/// THE REGRESSION THIS HARNESS WAS REBUILT FOR, owned on the CPU so it holds on a machine with
/// no GPU to render with.
///
/// The compare loop used to be a bare `assert_eq!` per frame. The first recording that moved
/// ended the run, so a drift report named exactly one frame out of twenty-four and said nothing
/// about the other twenty-three — they were not green, they were unmeasured. Two days of render
/// work could land, `prokhorovka_clear_afternoon` would fail, and nobody could tell whether one
/// frame had changed or all of them.
///
/// Two real recordings, each handed a frame one level brighter. A harness that stops early can
/// only ever name the first.
#[test]
fn the_drift_scan_names_every_frame_that_moved_not_just_the_first() {
    let moved = ["prokhorovka_overcast", "bystra_rain"];
    let mut scan = Scan::measuring_only();
    for name in moved {
        let mut brighter = read_png(&golden_path(name));
        for byte in brighter.iter_mut() {
            *byte = byte.saturating_add(1);
        }
        scan.visit(false, name, &brighter);
    }

    assert_eq!(scan.compared, 2, "both recordings must be reached");
    let named: Vec<&str> = scan.drifted.iter().map(|drift| drift.view.as_str()).collect();
    assert_eq!(
        named,
        moved,
        "the scan named {} of 2 moved frames — a harness that stops at the first one turns the \
         rest into unmeasured frames that read as green",
        named.len()
    );
}

/// The other half of the same promise: collecting must not invent drift. A recording compared
/// against itself is silence, not a line in the report.
#[test]
fn a_frame_that_matches_its_recording_reports_no_drift() {
    let name = "prokhorovka_overcast";
    let recorded = read_png(&golden_path(name));
    let mut scan = Scan::measuring_only();
    scan.visit(false, name, &recorded);

    assert_eq!(scan.compared, 1);
    assert!(scan.drifted.is_empty(), "an identical frame must not be reported as drift");
    assert!(scan.unusable.is_empty(), "a present, decodable golden is not unusable");
    assert!(frame_drift(name, &recorded, &recorded).is_none());
}

/// What the report says, not just that it says something. A digest can only answer "it moved";
/// these four numbers are what tells a reviewer whether an exposure knob turned or a hatch moved,
/// which is the difference between blessing a re-record and refusing it.
#[test]
fn drift_is_measured_in_pixels_and_levels_not_in_bytes() {
    // Four pixels, one channel of one pixel five levels off.
    let recorded = vec![10, 20, 30, 255, 0, 0, 0, 255, 40, 40, 40, 255, 90, 90, 90, 255];
    let mut current = recorded.clone();
    current[1] = 25;

    let drift = frame_drift("synthetic", &recorded, &current).expect("a moved frame drifts");
    assert_eq!(drift.differing, 1, "one pixel moved");
    assert_eq!(drift.total, 4, "out of four");
    assert_eq!(drift.max_delta, 5, "by five levels");
    assert!((drift.mean_delta - 5.0).abs() < 1.0e-6, "mean is over moved channels only");
    assert!((drift.share() - 0.25).abs() < 1.0e-6);
    assert!(drift.line().contains("25.0% of pixels moved"), "line was: {}", drift.line());
}

/// A golden that cannot be read is a hole in the gate, and a hole is not a pass. It also must not
/// end the run: the frames behind it still have to be measured.
#[test]
fn an_unreadable_golden_is_collected_rather_than_thrown() {
    let mut scan = Scan::measuring_only();
    scan.visit(false, "no_such_view_was_ever_recorded", &[0u8; 4]);

    assert_eq!(scan.compared, 0, "nothing was compared");
    assert_eq!(scan.unusable.len(), 1, "and the hole is named");
    assert!(scan.unusable[0].contains("missing"), "reason was: {}", scan.unusable[0]);
}

/// sRGB byte -> display-linear channel.
fn srgb_to_linear(byte: u8) -> f32 {
    let c = byte as f32 / 255.0;
    if c <= 0.04045 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) }
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
    let (mut dark, mut mid, mut bright) = (0u32, 0u32, 0u32);
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

/// Recorded worst outdoor dark share: `prokhorovka_overcast` at 0.9%.
const OUTDOOR_DARK_FLOOR: f32 = 0.008;
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
    const GARAGE_BRIGHT_FLOOR: f32 = 0.0025;
    const GARAGE_BRIGHT_TARGET: f32 = 0.02;
    // The dark share barely moved (89.9% -> 90.0%), for the same reason. It does not clear the
    // 75% the outdoor frames answer to, so its ceiling is recorded as a debt rather than asserted
    // away. W4 is about putting light in the room, and this is the number that says by how much.
    const GARAGE_DARK_CEILING_FLOOR: f32 = 0.905;
    const GARAGE_DARK_CEILING_TARGET: f32 = 0.75;
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
    SubjectBounds {
        view: "prokhorovka_evening_contact",
        median_floor: 0.110,
        dark_ceiling: 0.92,
        form_floor: 0.0135,
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
}

/// Mirrors `scene_build::review_views`'s naming so the warmth lookup above can address a map's
/// frames. Kept here rather than exported: the golden filename convention is this harness's
/// business, and a second copy that drifts would fail loudly on the first missing key.
fn map_key(map: MapId) -> &'static str {
    match map {
        MapId::ProkhorovkaHill252_2 => "prokhorovka",
        MapId::BystraValley => "bystra",
        MapId::OrlinyPereval => "orliny",
        MapId::Ostrogorsk => "ostrogorsk",
        _ => "scratch",
    }
}
