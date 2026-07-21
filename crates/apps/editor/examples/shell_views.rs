//! Offscreen proof of the editor shell's look: the viewport through the game's render path,
//! the editor's 3D annotations (spawn rings, nav posts, problem pylons, the cursor probe)
//! and the instrument-direction overlay, composited exactly as the live window draws them.
//! `cargo run -p editor --example shell_views` → PNGs under `target/`.

use std::fs::File;
use std::io::BufWriter;

use editor::app::layer_lines;
use editor::brush::{BrushMode, BrushSettings, Stroke};
use editor::overlay::{OverlayModel, overlay};
use editor::{EditorDocument, markers};
use glam::Vec3;
use renderer_api::{Camera, CameraProjectionPolicy, RenderFrame, view_projection_matrix};
use renderer_wgpu::{GpuContext, OffscreenTarget, SceneRenderer};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (width, height) = (1600u32, 900u32);
    let aspect = width as f32 / height as f32;

    // View 1: the shipped valley, freshly opened — overview camera, document panel on.
    let valley = EditorDocument::open(std::path::Path::new(
        "crates/world/map_forge/blueprints/bystra-valley.map.ron",
    ))?;
    // View 2: a broken document — a cliff dropped beside a spawn (steep-approach warning
    // with a world position), the jump cursor on it.
    let mut broken = EditorDocument::new_scratch();
    broken.apply_edit(|blueprint| {
        blueprint.meta.name = "Scratch (broken on purpose)".into();
        blueprint.terrain.ops.push(map_forge::blueprint::TerrainOp::Gauss2 {
            apply: map_forge::blueprint::Apply::Add,
            terms: vec![map_forge::blueprint::Gauss2Term {
                x: 150.0,
                z: 82.0,
                sx: 9.0,
                sz: 9.0,
                amp: 11.0,
            }],
        });
    });

    // View 3: a document sculpted with the M4 brushes - a raised ridge and a flattened
    // pad painted programmatically through the same Stroke API the live window drives,
    // committed into the sculpt layer and compiled like any other data.
    let mut sculpted = EditorDocument::new_scratch();
    {
        // Stroke 1: the ridge. Stroke 2 (a NEW stroke, like every LMB press) carves a
        // hull-down notch into its flank — strokes stack on the committed layer.
        let compiled = sculpted.recompile();
        let mut stroke = Stroke::begin(sculpted.blueprint(), &compiled.battlefield.heightmap);
        let raise = BrushSettings { mode: BrushMode::Raise, radius_m: 22.0, rate_m_s: 6.0 };
        for step in 0..18 {
            let t = step as f32 / 17.0;
            stroke.dab([90.0 + t * 120.0, 130.0 + (t * 6.3).sin() * 18.0], &raise, 0.4);
        }
        let ridge = stroke.committed(None).expect("the ridge sculpts");
        sculpted.apply_edit(|blueprint| blueprint.sculpt = Some(ridge));
        let compiled = sculpted.recompile();
        let mut stroke = Stroke::begin(sculpted.blueprint(), &compiled.battlefield.heightmap);
        let lower = BrushSettings { mode: BrushMode::Lower, radius_m: 14.0, rate_m_s: 6.0 };
        for _ in 0..10 {
            stroke.dab([150.0, 158.0], &lower, 0.14);
        }
        let committed =
            stroke.committed(sculpted.blueprint().sculpt.as_ref()).expect("the notch sculpts");
        sculpted.apply_edit(|blueprint| {
            blueprint.sculpt = Some(committed);
            // A raking golden-evening look so the sculpted relief reads in the proof shot.
            blueprint.environment = Some(map_forge::blueprint::EnvironmentSpec {
                looks: vec![map_forge::blueprint::LookSpec {
                    variant: game_core::WeatherVariant::GoldenEvening,
                    preset: map_forge::blueprint::LightingPreset::GoldenEvening,
                    sky_rgb: [0.8, 0.62, 0.45],
                    rain_intensity: 0.0,
                    wetness: 0.0,
                    overrides: Default::default(),
                }],
            });
        });
    }

    let ctx = GpuContext::headless()?;
    let target = OffscreenTarget::new(&ctx, width, height)?;

    for (document, camera, probe, selected, name) in [
        (
            &valley,
            Camera {
                eye: [500.0, 120.0, -60.0],
                target: [520.0, 10.0, 400.0],
                vertical_fov_degrees: 55.0,
            },
            Some(Vec3::new(585.0, 5.0, 320.0)),
            None,
            "editor_shell_valley",
        ),
        (
            &broken,
            Camera {
                eye: [120.0, 34.0, 52.0],
                target: [150.0, 8.0, 95.0],
                vertical_fov_degrees: 55.0,
            },
            Some(Vec3::new(150.0, 5.0, 120.0)),
            Some(0),
            "editor_shell_problems",
        ),
        (
            &sculpted,
            Camera {
                eye: [150.0, 70.0, 40.0],
                target: [150.0, 8.0, 160.0],
                vertical_fov_degrees: 55.0,
            },
            Some(Vec3::new(150.0, 5.0, 200.0)),
            None,
            "editor_shell_sculpt",
        ),
    ] {
        let compiled = document.recompile();
        let battlefield = &compiled.battlefield;

        let (ground, statics) =
            scene_build::battlefield::battlefield_ground_and_statics_meshes(battlefield, &[]);
        let maps = scene_build::terrain_maps::bake_terrain_ground_maps(battlefield);
        let materials = match &document.blueprint().materials {
            Some(spec) => scene_build::terrain_maps::material_set_from(spec),
            None => renderer_api::TerrainMaterialSet::default(),
        };
        let mut renderer = SceneRenderer::for_offscreen(&ctx, &statics.0, &statics.1)?;
        let (font_w, font_h, coverage) = client::hud_font_atlas();
        renderer.set_hud_font_atlas(&ctx, font_w, font_h, coverage);
        renderer.set_battlefield_ground(&ctx, &ground.0, &ground.1, &maps, &materials);
        let (water_vertices, water_indices) =
            scene_build::water::battlefield_water_mesh(battlefield);
        renderer.set_water(&ctx, &water_vertices, &water_indices);
        let (dressing_vertices, dressing_indices) =
            scene_build::grass_cards::grass_card_dressing_mesh(battlefield, &maps, &materials);
        renderer.set_dressing(&ctx, &dressing_vertices, &dressing_indices);

        let look = match document.blueprint().environment.as_ref().and_then(|env| env.looks.first())
        {
            Some(look) => scene_build::weather::realize_look(look),
            None => scene_build::weather::hazy_noon_fallback(),
        };
        renderer.scene_lighting = look.lighting;
        renderer.set_outdoor_sky(look.sky.0, look.sky.1, look.sky.2);
        renderer.rain_intensity = look.rain_intensity;
        renderer.wetness = look.wetness;
        renderer.scene_time_s = 9.0;
        renderer.shadow_focus = Some(camera.target);

        let (mut marker_vertices, mut marker_indices) =
            markers::map_markers(battlefield, &compiled.report, selected);
        let brush_armed = name == "editor_shell_sculpt";
        if let Some(at) = probe {
            let ground_h = battlefield.heightmap.sample_height(at.x, at.z).unwrap_or(at.y);
            if brush_armed {
                markers::brush_ring(
                    &mut marker_vertices,
                    &mut marker_indices,
                    &battlefield.heightmap,
                    [at.x, at.z],
                    16.0,
                    markers::brush_color("flatten"),
                );
            } else {
                markers::probe_marker(
                    &mut marker_vertices,
                    &mut marker_indices,
                    Vec3::new(at.x, ground_h, at.z),
                    1.5,
                );
            }
        }
        renderer.set_dynamic_mesh(&ctx, &marker_vertices, &marker_indices);
        renderer.set_render_frame(&ctx, &RenderFrame { camera, ..RenderFrame::default() });

        let problems = editor::overlay::problem_rows(&compiled.report);
        let probe_line = probe
            .map(|at| format!("cursor {:.0}, {:.0}  h {:.1}", at.x, at.z, at.y))
            .unwrap_or_default();
        let model = OverlayModel {
            brush_line: if brush_armed { "brush flatten  r 16 m".to_string() } else { String::new() },
            document_label: document
                .path()
                .map_or_else(|| document.blueprint().meta.id.clone(), |p| p.display().to_string()),
            dirty: document.dirty(),
            compile_ms: compiled.compile_time.as_secs_f32() * 1000.0,
            problems,
            selected_problem: selected,
            map_size_m: battlefield.size_m[0],
            layer_lines: layer_lines(document.blueprint(), &compiled),
            show_overview: true,
            camera_line: format!(
                "cam {:.0} {:.0}  h {:.0}",
                camera.eye[0], camera.eye[2], camera.eye[1]
            ),
            probe_line,
            status:
                "F1 overview   F5 recompile   N problem   Ctrl+S save   Ctrl+Z/Y undo   Ctrl+P playtest"
                    .to_string(),
        };
        renderer.set_hud(&ctx, &overlay(&model, aspect));

        let projection = CameraProjectionPolicy::webgpu_default();
        let view_proj = view_projection_matrix(
            &camera,
            aspect,
            projection.near_plane_m(),
            projection.far_plane_m(),
        );
        renderer.render(&ctx, target.render_target(), view_proj, camera.eye)?;
        let pixels = target.read_rgba8(&ctx)?;
        let path = format!("target/{name}.png");
        let file = File::create(&path)?;
        let mut encoder = png::Encoder::new(BufWriter::new(file), width, height);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        encoder.write_header()?.write_image_data(&pixels)?;
        println!("wrote {path} ({width}x{height})");
    }
    Ok(())
}
