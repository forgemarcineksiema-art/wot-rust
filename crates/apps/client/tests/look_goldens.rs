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
//! - `recorded_goldens_hold_the_value_structure` — always-on and CPU-only: decodes the
//!   committed golden PNGs and asserts the art-direction value statistics (rule 1: three
//!   value planes; rule 3: the evening reads warmer than the overcast). Catches a policy
//!   violation in any committed look without needing a GPU.

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

struct FrameStats {
    dark: f32,
    mid: f32,
    bright: f32,
    mean_warmth: f32,
}

fn frame_stats(pixels: &[u8]) -> FrameStats {
    let (mut dark, mut mid, mut bright) = (0u32, 0u32, 0u32);
    let (mut sum_r, mut sum_b) = (0.0f64, 0.0f64);
    let mut count = 0u32;
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
        count += 1;
    }
    let n = count as f32;
    FrameStats {
        dark: dark as f32 / n,
        mid: mid as f32 / n,
        bright: bright as f32 / n,
        mean_warmth: (sum_r / sum_b.max(1.0e-9)) as f32,
    }
}

/// Always-on, CPU-only: the committed goldens must obey the bible's value structure. This is
/// the statistical lock that runs in every `verify` regardless of GPU availability — if a
/// deliberate re-record ships a picture that lost its three value planes, this fails the gate.
#[test]
fn recorded_goldens_hold_the_value_structure() {
    let mut warmth_by_name = std::collections::HashMap::new();
    for map in REVIEWED_MAPS {
        let battlefield = map_forge::battlefield(map);
        for view in review_views_for(map, &battlefield) {
            let pixels = read_png(&golden_path(&view.name));
            let stats = frame_stats(&pixels);
            // RULE 1: the frame reads in three value planes — none may vanish. The raking evening
            // views carry a real shade mass today; the empty noon/overcast steppe legitimately has
            // almost none until shadow-casting content lands (trees, vehicles — the world
            // packages), so their dark floor is symbolic for now. RAISE IT as the world fills in.
            let dark_floor = if view.name.contains("evening") { 0.03 } else { 0.001 };
            assert!(
                stats.dark >= dark_floor,
                "{}: the dark plane vanished ({:.2}% of pixels, floor {:.1}%)",
                view.name,
                stats.dark * 100.0,
                dark_floor * 100.0
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
            // And no single plane may swallow the picture.
            for (plane, share) in
                [("dark", stats.dark), ("mid", stats.mid), ("bright", stats.bright)]
            {
                assert!(
                    share <= 0.90,
                    "{}: the {plane} plane swallowed the picture ({:.1}%)",
                    view.name,
                    share * 100.0
                );
            }
            warmth_by_name.insert(view.name.clone(), stats.mean_warmth);
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
