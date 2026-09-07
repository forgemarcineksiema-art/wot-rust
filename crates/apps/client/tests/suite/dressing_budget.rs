//! The dressing pass's instance budget (B3): the frame the battle submits — the near grass
//! ring at its peak, every tree station and hull, and the densest town's dwellings with
//! nothing culled — fits the scene instance buffer. The buffer truncates with a warning
//! beyond its budget, and a truncated frame is a street with plinths and no walls.

use glam::Vec3;

#[test]
fn the_densest_dressing_frame_fits_the_scene_instance_budget() {
    let battlefield = map_forge::battlefield(terrain::MapId::Ostrogorsk);
    let ground_maps = client::bake_terrain_ground_maps(&battlefield);
    let materials = client::terrain_material_set_for(terrain::MapId::Ostrogorsk);
    // The instruments' eye: no cone, so every tree and every building is submitted — the
    // worst case the battle can ask for from the town's centre.
    let mut state = scene_build::tree_lod::TreeLodState::default();
    let dressing = client::battlefield_dressing_objects(
        &battlefield,
        &ground_maps,
        &materials,
        &[],
        scene_build::tree_lod::TreeEye::at(Vec3::new(500.0, 3.0, 500.0)),
        &mut state,
    );
    let kit = scene_build::building_kit::kit_object_count(&dressing);
    let budget = renderer_wgpu::scene_instance_budget();
    println!(
        "DRESSING BUDGET: {} objects ({kit} kit parts, {} grass at most) of {budget}",
        dressing.len(),
        scene_build::grass::MAX_GRASS_INSTANCES
    );
    assert!(kit > 1_000, "the kit is in the dressing frame: {kit}");
    // The grass ring is eye-dependent; count it at its ceiling.
    // The grass is the only shadowless dressing (its handles sit above the shadowless base).
    let grass = dressing
        .iter()
        .filter(|object| object.mesh.0 >= renderer_api::SHADOWLESS_DRESSING_MESH_BASE)
        .count();
    let worst = dressing.len() - grass + scene_build::grass::MAX_GRASS_INSTANCES;
    assert!(
        worst * 5 <= budget * 4,
        "the worst dressing frame ({worst}) must keep a fifth of the budget ({budget}) free"
    );
}
