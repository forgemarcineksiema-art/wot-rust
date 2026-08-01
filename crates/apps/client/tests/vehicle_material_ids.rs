//! The Rust↔WGSL material boundary, bound by a test instead of by hope.
//!
//! History this file exists to not repeat. `material_role_id` emitted ids 0–11 while
//! `material_params` in `vehicle.wgsl` answered for 0–7 and swept the rest into an `else`. So
//! Canvas, Glass and Timber — three roles whose own doc comments argue at length that they must be
//! distinct because "one material for two things is one of them rendered wrong" — all rendered as
//! torn armour. A second miss compounded it: an unbounded `material_id >= 5u` gave them the
//! INTERIOR aperture lighting, so a tarpaulin on the outside of a turret was lit as though it sat
//! in the fighting compartment.
//!
//! Nothing was broken in either language. The two sides simply held the same fact — how many
//! materials exist — as two independent numbers, and only one of them was updated. This test makes
//! it one number and one test, which is the only arrangement that cannot drift.

use renderer_wgpu::vehicle_shader_source;
use vehicle_geometry::MaterialRole;

/// Ids the shader's `material_params` answers for by name, i.e. every `id == Nu` branch.
fn ids_the_shader_names() -> Vec<u32> {
    let source = vehicle_shader_source();
    let body =
        source.split_once("fn material_params(").expect("vehicle.wgsl defines material_params").1;
    let body = body.split_once("\n}").expect("material_params has a body").0;
    body.match_indices("id == ")
        .filter_map(|(at, _)| {
            body[at + "id == ".len()..]
                .split_once('u')
                .and_then(|(digits, _)| digits.parse::<u32>().ok())
        })
        .collect()
}

#[test]
fn the_shader_answers_for_every_material_role_the_renderer_can_emit() {
    let named = ids_the_shader_names();
    assert!(!named.is_empty(), "the branch scan found nothing — has material_params been renamed?");

    let missing: Vec<_> = MaterialRole::ALL
        .into_iter()
        .filter(|role| !named.contains(&client::material_role_id(*role)))
        .collect();

    assert!(
        missing.is_empty(),
        "these roles reach the GPU with no shader branch of their own and fall into the fallback: \
         {missing:?} — give each one its albedo and roughness in vehicle.wgsl"
    );
}

/// Ids must be dense and unique: the shader dispatches on the number, so a gap or a collision is
/// two roles sharing one appearance.
#[test]
fn material_ids_are_dense_and_unique() {
    let mut ids: Vec<u32> = MaterialRole::ALL.into_iter().map(client::material_role_id).collect();
    let count = ids.len() as u32;
    ids.sort_unstable();
    ids.dedup();
    assert_eq!(ids.len() as u32, count, "two roles share one material id");
    assert_eq!(
        ids,
        (0..count).collect::<Vec<_>>(),
        "material ids must be dense from zero — the shader indexes on them"
    );
}

/// The interior lighting lane is for the three INTERIOR roles and nothing else. Canvas, glass and
/// timber sit above them in id order and are exterior fittings; an unbounded `>= 5` lit them from
/// inside the hull.
#[test]
fn only_interior_roles_take_the_aperture_lighting() {
    let source = vehicle_shader_source();
    assert!(
        !source.contains("input.material_id >= 5u)"),
        "an unbounded `material_id >= 5u` catches canvas, glass and timber as interior materials"
    );
    assert!(
        source.contains("input.material_id >= 5u && input.material_id <= 7u"),
        "the interior lane must name its upper bound as well as its lower one"
    );
    for exterior in [MaterialRole::Canvas, MaterialRole::Glass, MaterialRole::Timber] {
        assert!(
            client::material_role_id(exterior) > 7,
            "{exterior:?} sits outside the interior band by id, which is what the bound relies on"
        );
    }
}
