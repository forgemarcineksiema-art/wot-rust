//! Every authored species on its own (route 2, trees as data): one tree per species on
//! Prokhorovka's ground, the whole silhouette from a tank's eye at a distance that frames
//! the species (22 m for a tree, 8 m for the bush), and a GROVE of every variant of one
//! species side by side — young, mature, old, sparse — so the "generator of sizes" is judged
//! on one frame. PNGs: `target/species_<name>.png`, `target/species_variants_<name>.png`.
//! `cargo run -p client --example probe -- species_probe`

use std::fs::File;
use std::io::BufWriter;

use client::{
    bake_terrain_ground_maps, battlefield_ground_and_statics_meshes, terrain_material_set_for,
};
use renderer_api::{Camera, CameraProjectionPolicy, SceneLighting, view_projection_matrix};
use renderer_wgpu::{GpuContext, OffscreenTarget, SceneRenderer};
use terrain::{SceneryInstance, SceneryKind};

pub(crate) fn run() -> Result<(), Box<dyn std::error::Error>> {
    let (width, height) = (1600u32, 900u32);
    let mut battlefield = map_forge::battlefield(terrain::MapId::ProkhorovkaHill252_2);
    battlefield.static_cover.clear();
    battlefield.scenery.clear();
    let ground = |x: f32, z: f32| battlefield.heightmap.sample_height(x, z).unwrap_or(0.0);
    let ((ground_v, ground_i), (statics_v, statics_i)) =
        battlefield_ground_and_statics_meshes(&battlefield, &[]);
    let ground_maps = bake_terrain_ground_maps(&battlefield);

    let ctx = GpuContext::headless()?;
    let target = OffscreenTarget::new(&ctx, width, height)?;
    let mut renderer = SceneRenderer::for_offscreen(&ctx, &statics_v, &statics_i)?;
    renderer.set_battlefield_ground(
        &ctx,
        &ground_v,
        &ground_i,
        &ground_maps,
        &terrain_material_set_for(terrain::MapId::ProkhorovkaHill252_2),
    );
    renderer.scene_lighting = SceneLighting::battlefield_default();
    renderer.scene_time_s = 12.0;
    for (handle, mesh) in scene_build::tree_lod::tree_lod_meshes() {
        renderer.register_mesh(&ctx, handle, &mesh);
    }
    crate::bind_battle_foliage_atlas(&mut renderer, &ctx);
    let projection = CameraProjectionPolicy::webgpu_default();

    // (kind, name, eye distance, aim height as a fraction of the tree)
    let species = [
        (SceneryKind::Oak, "oak", 24.0_f32, 8.5_f32),
        (SceneryKind::Poplar, "poplar", 34.0, 12.0),
        (SceneryKind::Willow, "willow", 22.0, 7.5),
        (SceneryKind::FruitTree, "fruit", 12.0, 3.5),
        (SceneryKind::Pine, "pine", 30.0, 10.0),
        (SceneryKind::Bush, "bush", 7.0, 1.4),
    ];
    let (tx, tz) = (500.0_f32, 500.0_f32);
    let mut shoot = |scenery: Vec<SceneryInstance>,
                     distance: f32,
                     aim_up: f32,
                     path: String|
     -> Result<(), Box<dyn std::error::Error>> {
        let ex = tx - 0.35 * distance;
        let ez = tz - 0.94 * distance;
        let eye = [ex, ground(ex, ez) + 2.2, ez];
        let look = [tx, ground(tx, tz) + aim_up, tz];
        renderer.shadow_focus = Some(look);
        let mut lod_state = scene_build::tree_lod::TreeLodState::default();
        let frame = renderer_api::RenderFrame {
            objects: scene_build::tree_lod::tree_frame_objects(
                &scenery,
                &[],
                &[],
                glam::Vec3::from_array(eye),
                &mut lod_state,
            ),
            ..renderer_api::RenderFrame::default()
        };
        renderer.set_render_frame(&ctx, &frame);
        let camera = Camera { eye, target: look, vertical_fov_degrees: 45.0 };
        let view_proj = view_projection_matrix(
            &camera,
            width as f32 / height as f32,
            projection.near_plane_m(),
            projection.far_plane_m(),
        );
        renderer.render(&ctx, target.render_target(), view_proj, camera.eye)?;
        let pixels = target.read_rgba8(&ctx)?;
        let file = File::create(&path)?;
        let mut encoder = png::Encoder::new(BufWriter::new(file), width, height);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        encoder.write_header()?.write_image_data(&pixels)?;
        println!("wrote {path}");
        Ok(())
    };

    for (kind, name, distance, aim_up) in species {
        // One tree, the position chosen so the seed names the MATURE variant.
        let position = mature_position(kind, tx, tz, &ground);
        shoot(
            vec![SceneryInstance { kind, position, yaw_rad: 0.6, scale: 1.0 }],
            distance,
            aim_up,
            format!("target/species_{name}.png"),
        )?;
        // The grove: every variant in a row, spaced by the species' size.
        let spacing = distance * 0.42;
        let mut row = Vec::new();
        for variant in 0..scene_build::tree_lod::VARIANTS {
            let x = tx + (variant as f32 - 1.5) * spacing;
            let position = variant_position(kind, variant, x, tz, &ground);
            row.push(SceneryInstance { kind, position, yaw_rad: 0.4 * variant as f32, scale: 1.0 });
        }
        shoot(row, distance * 1.9, aim_up * 0.9, format!("target/species_variants_{name}.png"))?;
    }
    Ok(())
}

/// A position near (x, z) whose seed names variant `variant`, unmirrored or not.
fn variant_position(
    kind: SceneryKind,
    variant: u32,
    x: f32,
    z: f32,
    ground: &impl Fn(f32, f32) -> f32,
) -> [f32; 3] {
    let _ = kind;
    for step in 0..4096 {
        let candidate_x = x + (step % 64) as f32 * 0.01;
        let candidate_z = z + (step / 64) as f32 * 0.01;
        let instance = SceneryInstance {
            kind,
            position: [candidate_x, 0.0, candidate_z],
            yaw_rad: 0.0,
            scale: 1.0,
        };
        if scene_build::tree_lod::instance_variant(&instance) == variant {
            return [candidate_x, ground(candidate_x, candidate_z), candidate_z];
        }
    }
    [x, ground(x, z), z]
}

fn mature_position(
    kind: SceneryKind,
    x: f32,
    z: f32,
    ground: &impl Fn(f32, f32) -> f32,
) -> [f32; 3] {
    variant_position(kind, world_forge::tree::authored::REFERENCE_VARIANT, x, z, ground)
}
