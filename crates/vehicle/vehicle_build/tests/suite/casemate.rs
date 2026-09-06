//! The Jagdtiger as library parts (Forge 2.0 K3, 2026-09-06): the first casemate — its own
//! class, not a turret with a cupola. Each lock names what would silently turn the welded-in
//! fighting compartment back into a Tiger with a fixed turret.

use game_core::{VehicleBlueprint, VehicleKind};
use glam::Vec3;
use vehicle_build::{
    PartShape, VehiclePart, casemate_parts_for_blueprint, fitting_parts_for_blueprint,
    german_deck_parts_for_blueprint,
};

fn jagdtiger() -> VehicleBlueprint {
    VehicleBlueprint::for_vehicle(VehicleKind::Jagdtiger).expect("the Jagdtiger is blueprint-born")
}

fn bounds(part: &VehiclePart) -> (Vec3, Vec3) {
    part.mesh().bounds().map(|aabb| (aabb.min, aabb.max)).expect("a casemate part has volume")
}

#[test]
fn the_casemate_walls_lean_and_the_collar_seats_on_the_face() {
    let bp = jagdtiger();
    let parts = casemate_parts_for_blueprint(&bp).expect("the Jagdtiger authors its casemate");
    let keys: Vec<&str> = parts.iter().map(|p| p.key.name).collect();
    for key in ["turret_shell", "periscope_housing", "turret_ventilator", "mantlet_socket"] {
        assert!(keys.contains(&key), "{key} is built: {keys:?}");
    }
    assert_eq!(keys.iter().filter(|k| **k == "casemate_hatch").count(), 2, "two flush hatches");
    assert_eq!(keys.iter().filter(|k| **k == "spare_track").count(), 12, "six shoes a side");
    assert!(!keys.contains(&"cupola_drum"), "a casemate carries no cupola");
    let t = &bp.turret;
    let shell = parts.iter().find(|p| p.key.name == "turret_shell").expect("shell");
    let PartShape::Mesh(mesh) = &shell.shape else { panic!("a lofted shell") };
    let width_at = |y: f32| {
        mesh.vertices()
            .iter()
            .filter(|v| (v.position.y - y).abs() < 1.0e-3)
            .map(|v| v.position.x.abs())
            .fold(0.0_f32, f32::max)
    };
    assert!((width_at(t.ring_y) - t.plan_half_width).abs() < 1.0e-3, "full width at the seat");
    let side_in = (t.roof_y - t.ring_y) * t.side_slope_deg.to_radians().tan();
    assert!((width_at(t.roof_y) - (t.plan_half_width - side_in)).abs() < 1.0e-3, "25° walls");
    // The collar's root ring lies on the leaning face at every height it reaches.
    let collar = parts.iter().find(|p| p.key.name == "mantlet_socket").expect("collar");
    let PartShape::Mesh(collar) = &collar.shape else { panic!("a collar mesh") };
    let lean = t.front_slope_deg.to_radians().tan();
    let face_z = |y: f32| t.ring_z + t.plan_half_length - (y - t.ring_y) * lean;
    let on_face = collar
        .vertices()
        .iter()
        .filter(|v| (v.position.z - face_z(v.position.y)).abs() < 2.0e-3)
        .count();
    assert!(on_face >= 12, "the root ring sits on the 15° face: {on_face} vertices");
    let (min, max) = bounds(parts.iter().find(|p| p.key.name == "mantlet_socket").unwrap());
    assert!(max.x - min.x > 1.1, "a collar over a metre across: {}", max.x - min.x);
    assert!(max.x - min.x > max.y - min.y, "an oval, wider than tall");
    // The periscope housing tops the roof by 7 cm, and the silhouette by nothing more.
    let (_, top) = bounds(parts.iter().find(|p| p.key.name == "periscope_housing").unwrap());
    assert!((top.y - (t.roof_y + 0.07)).abs() < 1.0e-3, "a low housing: {}", top.y);
}

#[test]
fn the_shoe_racks_lie_on_the_leaning_side_wall() {
    let bp = jagdtiger();
    let parts = casemate_parts_for_blueprint(&bp).expect("casemate");
    let t = &bp.turret;
    let lean = t.side_slope_deg.to_radians().tan();
    for shoe in parts.iter().filter(|p| p.key.name == "spare_track") {
        // The pad's inner face follows the wall, so its innermost x is at its TOP edge.
        let (min, max) = bounds(shoe);
        let wall = t.plan_half_width - (max.y - t.ring_y) * lean;
        let inner = min.x.abs().min(max.x.abs());
        assert!(
            (inner - wall).abs() < 0.03,
            "the shoe's inner face on the wall: {inner} vs {wall}"
        );
    }
}

#[test]
fn the_fittings_build_no_cupola_and_no_loader_lid_on_a_casemate() {
    let bp = jagdtiger();
    let parts = fitting_parts_for_blueprint(&bp).expect("the bow fittings");
    let keys: Vec<&str> = parts.iter().map(|p| p.key.name).collect();
    assert!(!keys.contains(&"cupola_hatch") && !keys.contains(&"loader_hatch"), "{keys:?}");
    for key in ["driver_hatch", "radio_hatch", "headlight", "tow_hook"] {
        assert!(keys.iter().any(|k| k.starts_with(key)), "{key}: {keys:?}");
    }
    let tiger = VehicleBlueprint::for_vehicle(VehicleKind::TigerI).expect("Tiger I");
    let keys: Vec<&str> =
        fitting_parts_for_blueprint(&tiger).expect("fittings").iter().map(|p| p.key.name).collect();
    assert!(
        keys.contains(&"cupola_hatch") && keys.contains(&"loader_hatch"),
        "the Tiger keeps both"
    );
}

#[test]
fn the_jagdtiger_deck_wears_flat_guards_and_flank_stowage() {
    let bp = jagdtiger();
    let parts = german_deck_parts_for_blueprint(&bp).expect("the Jagdtiger's deck");
    let keys: Vec<&str> = parts.iter().map(|p| p.key.name).collect();
    assert_eq!(keys.iter().filter(|k| **k == "fender_guard").count(), 2, "one flat guard a side");
    assert!(!keys.contains(&"fender_flap") && !keys.contains(&"fender_sweep"));
    for key in ["tow_cable", "tool_jack", "tool_block", "tool_box"] {
        assert_eq!(keys.iter().filter(|k| **k == key).count(), 2, "{key} both flanks");
    }
    // Every stowage piece lies ON the leaned sponson plate, proud of it, never inside.
    let lean = bp.armor.hull_side.0.to_radians().tan();
    for item in
        parts.iter().filter(|p| p.key.name.starts_with("tow_") || p.key.name.starts_with("tool_"))
    {
        let (min, max) = bounds(item);
        let wall = bp.hull.half_width - (max.y - bp.hull.sponson_y) * lean;
        let inner = min.x.abs().min(max.x.abs());
        assert!((inner - wall).abs() < 0.03, "{}: on the plate {wall}: {inner}", item.key.name);
    }
    // The MG ball at its station in the 50° glacis that folds at the sponson.
    let ball = parts.iter().find(|p| p.key.name == "course_mg_port").expect("ball");
    let (min, max) = bounds(ball);
    let y = (min.y + max.y) * 0.5;
    let plate_z =
        bp.hull.half_len - (y - bp.hull.sponson_y) * bp.hull.glacis_slope_deg.to_radians().tan();
    assert!(max.z > plate_z + 0.05 && min.z < plate_z, "in the plate at {plate_z:.3}");
}
