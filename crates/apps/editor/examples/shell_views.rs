//! Offscreen proof of the editor shell's look: the viewport through the game's render path,
//! the editor's 3D annotations (spawn rings, nav posts, problem pylons, the cursor probe)
//! and the instrument-direction overlay, composited exactly as the live window draws them.
//! `cargo run -p editor --example shell_views` → PNGs under `target/`.

use std::fs::File;
use std::io::BufWriter;

use editor::app::layer_lines;
use editor::brush::{BrushMode, BrushSettings, Stroke};
use editor::objects::{self, PaletteEntry};
use editor::overlay::{OverlayModel, overlay};
use editor::roads::{self, PendingRoad};
use editor::stamp::{self, StampKind};
use editor::stroke::{self, PendingStroke, StrokeKind};
use editor::{EditorDocument, grab, markers};
use editor::{gameplay, visibility};
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
        let raise = BrushSettings {
            mode: BrushMode::Raise,
            radius_m: 22.0,
            rate_m_s: 6.0,
            terrace_step_m: 2.0,
        };
        for step in 0..18 {
            let t = step as f32 / 17.0;
            stroke.dab([90.0 + t * 120.0, 130.0 + (t * 6.3).sin() * 18.0], &raise, 0.4);
        }
        let ridge = stroke.committed(None).expect("the ridge sculpts");
        sculpted.apply_edit(|blueprint| blueprint.sculpt = Some(ridge));
        let compiled = sculpted.recompile();
        let mut stroke = Stroke::begin(sculpted.blueprint(), &compiled.battlefield.heightmap);
        let lower = BrushSettings {
            mode: BrushMode::Lower,
            radius_m: 14.0,
            rate_m_s: 6.0,
            terrace_step_m: 2.0,
        };
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

    // View 4: structural stamps (M4b) - a two-click hill and a hull-down crest placed as
    // quantized TerrainOps through the same build/insert path the live gestures drive.
    let mut stamped = EditorDocument::new_scratch();
    {
        let blueprint = stamped.blueprint().clone();
        let (hill, _) = stamp::build_stamp(
            StampKind::Hill,
            &StampKind::Hill.parameters(),
            &blueprint,
            [110.0, 170.0],
            [150.0, 170.0],
        )
        .expect("the hill lands");
        let (crest, _) = stamp::build_stamp(
            StampKind::Crest,
            &StampKind::Crest.parameters(),
            &blueprint,
            [190.0, 120.0],
            [165.0, 150.0],
        )
        .expect("the crest lands");
        stamped.apply_edit(|blueprint| {
            stamp::insert(blueprint, hill);
            stamp::insert(blueprint, crest);
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

    // View 5: the object palette (M5) - a farmyard placed by clicks, the barn selected
    // (outline + OBJECT inspector), an oak planted with its mirror twin.
    let mut farmyard = EditorDocument::new_scratch();
    farmyard.apply_edit(|blueprint| {
        objects::place_entry(blueprint, PaletteEntry::Cottage, [138.0, 128.0]);
        objects::place_entry(blueprint, PaletteEntry::Barn, [162.0, 118.0]);
        objects::place_entry(blueprint, PaletteEntry::FenceRun, [148.0, 136.0]);
        objects::place_entry(blueprint, PaletteEntry::Wreck, [128.0, 106.0]);
        objects::place_entry(blueprint, PaletteEntry::Oak, [176.0, 132.0]);
        objects::place_entry(blueprint, PaletteEntry::Bush, [132.0, 118.0]);
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

    // View 6: roads & water (M6) - a stamped bowl flooded into a lake, the depth tint
    // speaking the gameplay thresholds (ford green / marginal amber / drowning red), and a
    // committed dirt road painted into the splat by the recompile.
    let mut lakeside = EditorDocument::new_scratch();
    {
        let blueprint = lakeside.blueprint().clone();
        let mut bowl_params = StampKind::Bowl.parameters();
        bowl_params[0].adjust(2.0); // 7 m deep
        let (bowl, _) = stamp::build_stamp(
            StampKind::Bowl,
            &bowl_params,
            &blueprint,
            [150.0, 150.0],
            [190.0, 150.0],
        )
        .expect("the bowl lands");
        let mut road = PendingRoad::default();
        for point in [[60.0, 96.0], [110.0, 104.0], [150.0, 96.0], [205.0, 118.0], [235.0, 150.0]] {
            road.add_point(point);
        }
        lakeside.apply_edit(|blueprint| {
            stamp::insert(blueprint, bowl);
            roads::set_water_level(blueprint, 3.0);
            if let Some((spec, _)) = road.committed(blueprint) {
                blueprint.roads.push(spec);
            }
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

    // View 7: the gameplay layer (M7) - moved spawns, nav points with roles, a capture
    // zone, and the turret-eye viewshed from a stamped hill (amber = a turret there works
    // a hull-height target).
    let mut battle = EditorDocument::new_scratch();
    {
        let blueprint = battle.blueprint().clone();
        let (hill, _) = stamp::build_stamp(
            StampKind::Hill,
            &StampKind::Hill.parameters(),
            &blueprint,
            [150.0, 150.0],
            [185.0, 150.0],
        )
        .expect("the hill lands");
        battle.apply_edit(|blueprint| {
            stamp::insert(blueprint, hill);
            gameplay::move_spawn(blueprint, 1, [150.0, 50.0]);
            gameplay::move_spawn(blueprint, 2, [150.0, 250.0]);
            gameplay::add_point(blueprint, terrain::StrategicRole::HullDown, [150.0, 120.0]);
            gameplay::add_point(blueprint, terrain::StrategicRole::FlankRoute, [60.0, 150.0]);
            gameplay::add_zone(blueprint, [150.0, 150.0]);
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

    // View 8: Rece do terenu (W1-W4) - one document worked by all three hands. A ridge
    // DRAWN as a fitted stroke op, a stamped hill GRABBED (moved + raised through the
    // same snapped transform the live drag commits), and the macro brushes finishing the
    // ground: a terrace pass on the hill's flank and an erode pass over the crest.
    let mut hands = EditorDocument::new_scratch();
    {
        let blueprint = hands.blueprint().clone();
        let (hill, _) = stamp::build_stamp(
            StampKind::Hill,
            &StampKind::Hill.parameters(),
            &blueprint,
            [190.0, 120.0],
            [220.0, 120.0],
        )
        .expect("the hill lands");
        let mut drawn = PendingStroke::new(StrokeKind::Ridge);
        for at in [[70.0, 200.0], [110.0, 175.0], [150.0, 168.0], [200.0, 180.0], [235.0, 205.0]] {
            drawn.add_point(at, 5.0);
        }
        let (ridge_ops, _) = stroke::fit_stroke(&drawn, &blueprint).expect("the ridge fits");
        hands.apply_edit(|blueprint| {
            stamp::insert(blueprint, hill);
            stamp::insert(blueprint, ridge_ops);
        });
        // Grab the hill and set it: 14 m east, 6 m north, 1.5 m taller - the exact door
        // the live release walks through.
        let compiled = hands.recompile();
        let forms = grab::enumerate_forms(hands.blueprint(), &compiled.battlefield.heightmap);
        let held = forms.iter().find(|form| form.label == "hill").expect("the hill is a handle");
        let transform = grab::Transform { move_m: [14.0, 6.0], raise_m: 1.5, widen: 1.0 };
        let held_form = held.clone();
        hands.apply_edit(|blueprint| {
            grab::apply_transform(blueprint, &held_form, &transform);
        });
        // The macro brushes finish the ground: terraces up the hill flank, erosion over
        // the drawn crest - committed into the sculpt layer like any hand stroke.
        let compiled = hands.recompile();
        let mut finish = Stroke::begin(hands.blueprint(), &compiled.battlefield.heightmap);
        let terrace = BrushSettings {
            mode: BrushMode::Terrace,
            radius_m: 18.0,
            rate_m_s: 6.0,
            terrace_step_m: 2.0,
        };
        for _ in 0..24 {
            finish.dab([190.0, 132.0], &terrace, 0.2);
        }
        let erode = BrushSettings {
            mode: BrushMode::Erode,
            radius_m: 16.0,
            rate_m_s: 6.0,
            terrace_step_m: 2.0,
        };
        for step in 0..14 {
            let t = step as f32 / 13.0;
            finish.dab([90.0 + t * 120.0, 190.0 - (t * 3.1).sin() * 14.0], &erode, 0.2);
        }
        let sculpted = finish.committed(None).expect("the finish sculpts");
        hands.apply_edit(|blueprint| {
            blueprint.sculpt = Some(sculpted);
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
        (
            &stamped,
            Camera {
                eye: [120.0, 55.0, 40.0],
                target: [170.0, 8.0, 150.0],
                vertical_fov_degrees: 55.0,
            },
            Some(Vec3::new(190.0, 5.0, 120.0)),
            None,
            "editor_shell_stamps",
        ),
        (
            &farmyard,
            Camera {
                eye: [120.0, 32.0, 78.0],
                target: [155.0, 8.0, 125.0],
                vertical_fov_degrees: 55.0,
            },
            Some(Vec3::new(148.0, 5.0, 136.0)),
            None,
            "editor_shell_objects",
        ),
        (
            &lakeside,
            Camera {
                eye: [110.0, 60.0, 60.0],
                target: [160.0, 2.0, 150.0],
                vertical_fov_degrees: 55.0,
            },
            Some(Vec3::new(172.0, 2.0, 150.0)),
            None,
            "editor_shell_water",
        ),
        (
            &battle,
            Camera {
                eye: [70.0, 75.0, 55.0],
                target: [150.0, 10.0, 155.0],
                vertical_fov_degrees: 55.0,
            },
            Some(Vec3::new(150.0, 5.0, 85.0)),
            None,
            "editor_shell_gameplay",
        ),
        (
            &hands,
            Camera {
                eye: [110.0, 70.0, 70.0],
                target: [170.0, 8.0, 175.0],
                vertical_fov_degrees: 55.0,
            },
            Some(Vec3::new(150.0, 5.0, 230.0)),
            None,
            "editor_shell_hands",
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
        let stamp_armed = name == "editor_shell_stamps";
        let farmyard_view = name == "editor_shell_objects";
        let water_view = name == "editor_shell_water";
        let gameplay_view = name == "editor_shell_gameplay";
        let hands_view = name == "editor_shell_hands";
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
        if gameplay_view {
            // The observer stands LOW on the southern approach: the stamped hill throws
            // its dead-ground shadow north - exactly what a hull-down defender exploits.
            let (shed_vertices, shed_indices) =
                visibility::viewshed_mesh(&battlefield.heightmap, [150.0, 85.0], 400.0);
            let base = marker_vertices.len() as u32;
            marker_vertices.extend_from_slice(&shed_vertices);
            marker_indices.extend(shed_indices.iter().map(|index| index + base));
        }
        if water_view && let Some(water) = battlefield.water {
            let (tint_vertices, tint_indices) = roads::depth_tint_mesh(
                &battlefield.heightmap,
                water.surface_level_m,
                &map_forge::WaterThresholds::default(),
            );
            let base = marker_vertices.len() as u32;
            marker_vertices.extend_from_slice(&tint_vertices);
            marker_indices.extend(tint_indices.iter().map(|index| index + base));
        }
        if hands_view {
            // A valley stroke MID-GESTURE: the chalk ribbon rides the clicked waypoints
            // toward the cursor, exactly as the C tool draws it live.
            let pending_valley = [[110.0, 240.0], [140.0, 232.0], [150.0, 230.0]];
            stroke::chalk_mesh(
                &mut marker_vertices,
                &mut marker_indices,
                &battlefield.heightmap,
                &pending_valley,
                5.0,
            );
            // The grabbed hill's amber foot ring - the H tool's handle marker.
            let forms = grab::enumerate_forms(document.blueprint(), &battlefield.heightmap);
            if let Some(held) = forms.iter().find(|form| form.label == "hill") {
                markers::brush_ring(
                    &mut marker_vertices,
                    &mut marker_indices,
                    &battlefield.heightmap,
                    [held.center.x, held.center.z],
                    held.footprint_m,
                    [0.95, 0.65, 0.15],
                );
            }
        }
        if farmyard_view
            && let Some(barn) =
                battlefield.static_cover.iter().find(|cover| cover.id.starts_with("barn"))
        {
            markers::aabb_outline(
                &mut marker_vertices,
                &mut marker_indices,
                Vec3::from_array(barn.center),
                Vec3::from_array(barn.half_extents_m),
                [0.95, 0.65, 0.15],
            );
        }
        renderer.set_dynamic_mesh(&ctx, &marker_vertices, &marker_indices);
        renderer.set_render_frame(&ctx, &RenderFrame { camera, ..RenderFrame::default() });

        let problems = editor::overlay::problem_rows(&compiled.report);
        let probe_line = probe
            .map(|at| format!("cursor {:.0}, {:.0}  h {:.1}", at.x, at.z, at.y))
            .unwrap_or_default();
        let model = OverlayModel {
            brush_line: if brush_armed { "brush flatten  r 16 m".to_string() } else { String::new() },
            inspector_title: if gameplay_view {
                "GAMEPLAY".to_string()
            } else if water_view {
                "WATER".to_string()
            } else if farmyard_view {
                "OBJECT".to_string()
            } else if hands_view {
                "STROKE".to_string()
            } else {
                "STAMP".to_string()
            },
            stamp_lines: if hands_view {
                vec![
                    ("valley - LMB draws the line".to_string(), false),
                    ("depth: 4.0 m".to_string(), true),
                    ("width: 5.0 m".to_string(), false),
                    ("3 points".to_string(), false),
                    ("Enter commits   Backspace pops   Esc clears".to_string(), false),
                ]
            } else if gameplay_view {
                vec![
                    ("nav point - LMB places".to_string(), false),
                    ("HighGround".to_string(), false),
                    ("Crossing".to_string(), false),
                    ("Observation".to_string(), false),
                    ("HullDown".to_string(), true),
                    ("FlankRoute".to_string(), false),
                    ("Tab cycles the role".to_string(), false),
                ]
            } else if water_view {
                vec![
                    ("level: 3.00 m".to_string(), true),
                    ("green: ford  amber: marginal".to_string(), false),
                    ("red: the current drowns".to_string(), false),
                    ("X removes standing water".to_string(), false),
                ]
            } else if farmyard_view {
                objects::inspector_lines(
                    document.blueprint(),
                    &editor::objects::Selection::Cover { object_index: 1, id: "barn_1".into() },
                    0,
                )
            } else if stamp_armed {
                vec![
                    ("crest - click 1: crest, click 2: shelf side + window".to_string(), false),
                    ("crest h: 1.5 m".to_string(), true),
                    ("shelf cut: 1.0 m".to_string(), false),
                    ("sigma x: 9.0 m".to_string(), false),
                    ("anchor: click LMB".to_string(), false),
                ]
            } else {
                Vec::new()
            },
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
