//! ONE probe binary for every review render, studio view and capture — 41 link units become
//! one (W4 "probe" wave). Invocation everywhere in the docs:
//! `cargo run -p client --example probe -- <name> [args...]`
//! (release captures unchanged: `cargo run -p client --release --example probe -- perf_capture`).
//!
//! Each former example is a module below, its body untouched apart from `main` becoming `run`
//! and positional args reading through [`sub_arg`]. No subcommand (or an unknown one) prints
//! the roster instead of guessing.

mod battle_age_cost;
mod battle_hud;
mod border_probe;
mod bystra_views;
mod bystra_weather_timeline;
mod centurion_probe;
mod closeup_probe;
mod crater_views;
mod damage_budget_capture;
mod deck_probe;
mod destruction_showcase;
mod detail_cost_probe;
mod factory_probe;
mod farmyard_views;
mod flora_frame_probe;
mod flora_probe;
mod fov_probe;
mod garage_hangar_review;
mod garage_techview;
mod grass_costumes;
mod is3_studio;
mod jagdtiger_profile_probe;
mod muzzle_probe;
mod oak_probe;
mod orliny_views;
mod ostrogorsk_views;
mod panther_ii_profile_probe;
mod perf_capture;
mod prokhorovka_views;
mod reticle_strip;
mod screenshot;
mod shadow_probe;
mod sky_probe;
mod species_probe;
mod t34_85_probe;
mod t54_battle_views;
mod t54_bow_probe;
mod t54_profile_probe;
mod t54_studio;
mod t54_views;
mod tenement_probe;
mod tiger_i_studio;
mod tiger_ii_profile_probe;
mod track_probe;
mod vehicle_lineup;
mod vehicle_lineup_views;

/// The Nth argument AFTER the subcommand — exactly what `env::args().nth(n)` meant back when
/// every probe was its own binary, so the probes' own numbering survives unshifted.
pub(crate) fn sub_arg(n: usize) -> Option<String> {
    std::env::args().nth(n + 1)
}

/// Bind the battle's leaf atlas to an offscreen renderer — EVERY probe that draws a
/// battlefield calls this right after constructing the renderer, exactly as the battle
/// (`app/render.rs`) and the look harness do. Without it every leaf card and every impostor
/// samples the renderer's white no-op texel and renders as a solid rectangle: that is what
/// `perf_capture` and the map-view probes drew from Drzewa 3.0 PR6 to Inny Poziom F1 without
/// anyone noticing, and the review path had lost the same bind once before (`look_harness`).
/// The third time the fix is a rule: `tests/probe_foliage_atlas.rs` refuses a battlefield
/// probe that does not call this.
pub(crate) fn bind_battle_foliage_atlas(
    renderer: &mut renderer_wgpu::SceneRenderer,
    ctx: &renderer_wgpu::GpuContext,
) {
    let (color, normal) = scene_build::foliage_atlas_paint::foliage_atlas_chains();
    renderer.set_foliage_atlas(ctx, &color, Some(&normal));
    renderer.set_bark_textures(ctx, &scene_build::foliage_atlas_paint::bark_texture_layers());
}

type ProbeResult = Result<(), Box<dyn std::error::Error>>;
type ProbeEntry = (&'static str, fn() -> ProbeResult);

/// Every probe, by the name its file (and its output paths) always had.
const PROBES: &[ProbeEntry] = &[
    ("battle_age_cost", || {
        battle_age_cost::run();
        Ok(())
    }),
    ("battle_hud", battle_hud::run),
    ("border_probe", border_probe::run),
    ("bystra_views", bystra_views::run),
    ("bystra_weather_timeline", bystra_weather_timeline::run),
    ("centurion_probe", centurion_probe::run),
    ("closeup_probe", closeup_probe::run),
    ("crater_views", crater_views::run),
    ("damage_budget_capture", || {
        damage_budget_capture::run();
        Ok(())
    }),
    ("deck_probe", deck_probe::run),
    ("destruction_showcase", destruction_showcase::run),
    ("detail_cost_probe", detail_cost_probe::run),
    ("factory_probe", factory_probe::run),
    ("farmyard_views", farmyard_views::run),
    ("flora_frame_probe", flora_frame_probe::run),
    ("flora_probe", flora_probe::run),
    ("fov_probe", fov_probe::run),
    ("garage_hangar_review", garage_hangar_review::run),
    ("garage_techview", garage_techview::run),
    ("grass_costumes", grass_costumes::run),
    ("is3_studio", is3_studio::run),
    ("jagdtiger_profile_probe", jagdtiger_profile_probe::run),
    ("muzzle_probe", muzzle_probe::run),
    ("oak_probe", oak_probe::run),
    ("orliny_views", orliny_views::run),
    ("ostrogorsk_views", ostrogorsk_views::run),
    ("panther_ii_profile_probe", panther_ii_profile_probe::run),
    ("perf_capture", || {
        perf_capture::run();
        Ok(())
    }),
    ("prokhorovka_views", prokhorovka_views::run),
    ("reticle_strip", reticle_strip::run),
    ("screenshot", screenshot::run),
    ("shadow_probe", shadow_probe::run),
    ("species_probe", species_probe::run),
    ("sky_probe", sky_probe::run),
    ("t34_85_probe", t34_85_probe::run),
    ("t54_battle_views", t54_battle_views::run),
    ("t54_bow_probe", t54_bow_probe::run),
    ("t54_profile_probe", t54_profile_probe::run),
    ("t54_studio", t54_studio::run),
    ("t54_views", t54_views::run),
    ("tenement_probe", tenement_probe::run),
    ("tiger_i_studio", tiger_i_studio::run),
    ("tiger_ii_profile_probe", tiger_ii_profile_probe::run),
    ("track_probe", track_probe::run),
    ("vehicle_lineup", vehicle_lineup::run),
    ("vehicle_lineup_views", vehicle_lineup_views::run),
];

fn main() -> ProbeResult {
    // A lost device or an uncaptured GPU error is a `tracing::error!` in the renderer; without a
    // subscriber a probe measures a 0.0 ms frame and says nothing (Q7's first cold sandwich).
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .try_init()
        .ok();
    let name = std::env::args().nth(1).unwrap_or_default();
    if let Some((_, run)) = PROBES.iter().find(|(n, _)| *n == name) {
        return run();
    }
    eprintln!("usage: cargo run -p client --example probe -- <name> [args...]\n\nprobes:");
    for (n, _) in PROBES {
        eprintln!("  {n}");
    }
    std::process::exit(2);
}
