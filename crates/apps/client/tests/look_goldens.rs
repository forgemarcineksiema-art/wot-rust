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

#[test]
fn look_goldens_match_their_recordings() {
    if std::env::var("WOT_LOOK_GOLDENS").as_deref() != Ok("1")
        && std::env::var("WOT_UPDATE_GOLDENS").as_deref() != Ok("1")
    {
        eprintln!("skipping look goldens (set WOT_LOOK_GOLDENS=1 to enable)");
        return;
    }
    let update = std::env::var("WOT_UPDATE_GOLDENS").as_deref() == Ok("1");

    for map in REVIEWED_MAPS {
        let battlefield = map_forge::battlefield(map);
        let views = review_views_for(map, &battlefield);
        let frames = render_views(map, &views);

        for (view, pixels) in views.iter().zip(&frames) {
            let path = golden_path(&view.name);
            if update {
                write_png(&path, pixels);
                eprintln!("recorded {}", path.display());
                continue;
            }
            let golden = read_png(&path);
            assert_eq!(
                &golden, pixels,
                "{} drifted from its golden — if the look change is deliberate, re-record with \
                 WOT_UPDATE_GOLDENS=1 and say why in the PR",
                view.name
            );
        }
    }

    // The garage: an interior studio with its own light rig and its own lens, but the same
    // display transform and the same locks. It had no golden at all before this.
    let hangar_views = client::hangar_review_views();
    let hangar_frames = client::render_hangar_review_views(&hangar_views, WIDTH, HEIGHT)
        .expect("hangar review render");
    for (view, pixels) in hangar_views.iter().zip(&hangar_frames) {
        let path = golden_path(&view.name);
        if update {
            write_png(&path, pixels);
            eprintln!("recorded {}", path.display());
            continue;
        }
        let golden = read_png(&path);
        assert_eq!(
            &golden, pixels,
            "{} drifted from its golden — if the look change is deliberate, re-record with \
             WOT_UPDATE_GOLDENS=1 and say why in the PR",
            view.name
        );
    }

    // The byte-exact contract this harness rests on: the same view renders identically twice
    // on one machine (the render is a pure function of scene + profile + the fixed clock).
    let map = REVIEWED_MAPS[0];
    let battlefield = map_forge::battlefield(map);
    let views = review_views_for(map, &battlefield);
    let once = render_views(map, &views[..1]);
    let again = render_views(map, &views[..1]);
    assert_eq!(once[0], again[0], "the render must be deterministic on one machine");
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

fn frame_stats(pixels: &[u8]) -> FrameStats {
    let (mut dark, mut mid, mut bright) = (0u32, 0u32, 0u32);
    let (mut sum_r, mut sum_b, mut sum_sat) = (0.0f64, 0.0f64, 0.0f64);
    let mut lumas = Vec::with_capacity((WIDTH * HEIGHT) as usize);

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
    for row in lumas.chunks_exact(WIDTH as usize) {
        for pair in row.windows(2) {
            contrast_sum += (pair[1] - pair[0]).abs() as f64;
            contrast_count += 1;
        }
    }

    let rows = HEIGHT as usize;
    let top_rows = (rows * 15) / 100;
    let bottom_start = rows - (rows * 40) / 100;
    let mut top: Vec<f32> = lumas[..top_rows * WIDTH as usize].to_vec();
    let mut bottom: Vec<f32> = lumas[bottom_start * WIDTH as usize..].to_vec();
    let band_separation = median_of(&mut top) - median_of(&mut bottom);

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
    // Its BRIGHT plane is the program's first measured FLOOR/TARGET debt (D20). The hero frame
    // holds 0.00% of pixels above the bright threshold: the room's own skylights and lamp faces
    // are emissive, but the hero framing points at a wall where none of them are in shot, so the
    // picture is entirely mid and shade. FLOOR is what it achieves today and is asserted so it
    // cannot get worse; GARAGE_BRIGHT_TARGET is what rule 1 demands and is closed in W4.
    const GARAGE_BRIGHT_FLOOR: f32 = 0.0;
    const GARAGE_BRIGHT_TARGET: f32 = 0.02;
    // The garage's dark share is 89.9% — it does not clear the 75% the outdoor frames answer to,
    // so its ceiling is recorded as a debt rather than asserted away. W4 is about putting light
    // in the room, and this is the number that says by how much.
    const GARAGE_DARK_CEILING_FLOOR: f32 = 0.92;
    const GARAGE_DARK_CEILING_TARGET: f32 = 0.75;
    for view in client::hangar_review_views() {
        let stats = frame_stats(&read_png(&golden_path(&view.name)));
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
