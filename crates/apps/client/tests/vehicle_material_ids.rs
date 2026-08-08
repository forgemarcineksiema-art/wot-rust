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

/// Ids the shader's `material_layer` answers for, paired with the layer it returns.
fn layers_the_shader_maps() -> Vec<(u32, u32)> {
    let source = vehicle_shader_source();
    let body =
        source.split_once("fn material_layer(").expect("vehicle.wgsl defines material_layer").1;
    let body = body.split_once("\n}").expect("material_layer has a body").0;
    body.lines()
        .filter_map(|line| {
            let (_, rest) = line.split_once("id == ")?;
            let (id, rest) = rest.split_once('u')?;
            let (_, rest) = rest.split_once("return ")?;
            let (layer, _) = rest.split_once(';')?;
            Some((id.trim().parse().ok()?, layer.trim().parse().ok()?))
        })
        .collect()
}

/// THE LAYER TABLE, held in both languages at once.
///
/// The shader used to pick its texture layer with `min(material_id, 4u)`. That is a clamp, not a
/// mapping: with five layers and twelve roles it sent the interior three, torn steel, canvas,
/// glass and timber all to the RUBBER layer — a headlight lens wearing a tyre, a tarpaulin wearing
/// a tyre, an unditching beam wearing a tyre. Nothing failed; it simply looked wrong.
#[test]
fn the_shader_maps_every_role_to_the_layer_the_renderer_says_it_has() {
    let mapped = layers_the_shader_maps();
    assert!(!mapped.is_empty(), "the branch scan found nothing — has material_layer been renamed?");

    for role in MaterialRole::ALL {
        let id = client::material_role_id(role);
        let expected = client::material_layer_id(role);
        let found = mapped.iter().find(|(shader_id, _)| *shader_id == id);
        let Some((_, shader_layer)) = found else {
            panic!("{role:?} (id {id}) has no branch in the shader's material_layer");
        };
        assert_eq!(
            *shader_layer, expected,
            "{role:?} (id {id}): Rust says layer {expected}, the shader says {shader_layer}"
        );
    }
}

/// Every layer the table names must actually exist in the uploaded texture array.
#[test]
fn no_role_points_past_the_end_of_the_texture_array() {
    for role in MaterialRole::ALL {
        let layer = client::material_layer_id(role) as usize;
        assert!(
            layer < renderer_api::VehicleMaterialFamilies::LAYERS,
            "{role:?} maps to layer {layer}, past the {} the renderer uploads",
            renderer_api::VehicleMaterialFamilies::LAYERS
        );
    }
}

/// The three roles that argue for their own existence must not share a layer with anything.
///
/// `Canvas`, `Glass` and `Timber` each carry the same sentence at their declaration — *one
/// material for two things is one of them rendered wrong* — and all three were the same wrong
/// thing until they had layers of their own.
#[test]
fn canvas_glass_and_timber_do_not_share_a_layer_with_any_other_role() {
    for distinct in [MaterialRole::Canvas, MaterialRole::Glass, MaterialRole::Timber] {
        let layer = client::material_layer_id(distinct);
        let mut compared = 0;
        for other in MaterialRole::ALL {
            if other == distinct {
                continue;
            }
            compared += 1;
            assert_ne!(
                client::material_layer_id(other),
                layer,
                "{distinct:?} shares layer {layer} with {other:?} — the exact mistake its own doc \
                 comment exists to prevent"
            );
        }
        // The floor the ratchet asks for, and it is not ceremony: a walk that `continue`s past
        // its own subject would pass having compared nothing at all.
        assert_eq!(
            compared,
            MaterialRole::ALL.len() - 1,
            "{distinct:?} was compared against {compared} roles, not the whole enum"
        );
    }
}
