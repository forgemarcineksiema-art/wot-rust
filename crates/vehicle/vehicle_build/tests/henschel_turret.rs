//! The Tiger II as library parts (Forge 2.0 K3, 2026-09-06): the Henschel turret's leaned walls
//! and flat rear plate, the deck furniture the Tiger II wears and the Tiger I does not, the
//! Schürzen on the spaced plane, and the leaned upper hull sides — each lock names what would
//! silently turn the King Tiger back into a Tiger I wearing a longer gun.

use game_core::{VehicleBlueprint, VehicleKind};
use glam::Vec3;
use vehicle_build::{
    PartShape, VehiclePart, german_deck_parts_for_blueprint, skirt_parts_for_blueprint,
    slab_hull_parts_for_blueprint, welded_turret_parts_for_blueprint,
};

fn blueprint(kind: VehicleKind) -> VehicleBlueprint {
    let Some(bp) = VehicleBlueprint::for_vehicle(kind) else {
        panic!("{kind:?}: the German line is blueprint-born")
    };
    bp
}

fn part<'a>(parts: &'a [VehiclePart], key: &str) -> &'a VehiclePart {
    parts.iter().find(|p| p.key.name == key).unwrap_or_else(|| panic!("{key} is built"))
}

fn mesh_bounds(part: &VehiclePart) -> (Vec3, Vec3) {
    let b = part.mesh().bounds().expect("a part has volume");
    (b.min, b.max)
}

#[test]
fn the_henschel_turret_leans_its_walls_and_closes_on_a_flat_rear_plate() {
    let bp = blueprint(VehicleKind::TigerII);
    let parts = welded_turret_parts_for_blueprint(&bp).expect("the Tiger II authors its turret");
    let keys: Vec<&str> = parts.iter().map(|p| p.key.name).collect();
    assert!(!keys.contains(&"turret_bin"), "no stowage bin on the Henschel: {keys:?}");
    for key in ["turret_shell", "cupola_drum", "turret_ring_collar", "mantlet_socket"] {
        assert!(keys.contains(&key), "{key} is built: {keys:?}");
    }
    let shell = part(&parts, "turret_shell");
    let PartShape::Mesh(mesh) = &shell.shape else { panic!("a lofted shell") };
    let t = &bp.turret;
    let rear_plane = t.ring_z - t.plan_half_length;
    // The rear plate stands ON the rear armour plane at the ring seat, spanning the flat width.
    let at_rear: Vec<f32> = mesh
        .vertices()
        .iter()
        .filter(|v| {
            (v.position.y - t.ring_y).abs() < 1.0e-3 && (v.position.z - rear_plane).abs() < 1.0e-3
        })
        .map(|v| v.position.x)
        .collect();
    let rear_half_width = at_rear.iter().copied().fold(0.0_f32, f32::max);
    assert!((rear_half_width - 0.85).abs() < 1.0e-3, "a flat 1.70 m rear plate: {at_rear:?}");
    // Every wall leans: the roof plan is narrower than the ring plan by the side slope's run,
    // and the rear retreats by the rear slope's run.
    let width_at = |y: f32| {
        mesh.vertices()
            .iter()
            .filter(|v| (v.position.y - y).abs() < 1.0e-3)
            .map(|v| v.position.x.abs())
            .fold(0.0_f32, f32::max)
    };
    let side_in = (t.roof_y - t.ring_y) * t.side_slope_deg.to_radians().tan();
    assert!(
        (width_at(t.roof_y) - (t.plan_half_width - side_in)).abs() < 1.0e-3,
        "the side walls lean {}°: {} at the roof vs {} at the ring",
        t.side_slope_deg,
        width_at(t.roof_y),
        width_at(t.ring_y)
    );
    let rear_at_roof = mesh
        .vertices()
        .iter()
        .filter(|v| (v.position.y - t.roof_y).abs() < 1.0e-3)
        .map(|v| v.position.z)
        .fold(f32::INFINITY, f32::min);
    let rear_in = (t.roof_y - t.ring_y) * t.rear_slope_deg.to_radians().tan();
    assert!(
        (rear_at_roof - (rear_plane + rear_in)).abs() < 1.0e-3,
        "the rear leans: {rear_at_roof}"
    );
    // The Turmblende's socket is an oval, wider than tall.
    let (min, max) = mesh_bounds(part(&parts, "mantlet_socket"));
    assert!(max.x - min.x > (max.y - min.y) * 2.0, "an oval socket: {:?}", (max - min));
}

#[test]
fn the_tiger_i_keeps_its_bin_and_vertical_walls_under_the_same_library() {
    let bp = blueprint(VehicleKind::TigerI);
    let parts = welded_turret_parts_for_blueprint(&bp).expect("the Tiger I authors its turret");
    assert!(parts.iter().any(|p| p.key.name == "turret_bin"), "the Rommelkiste stands");
    let PartShape::Mesh(mesh) = &part(&parts, "turret_shell").shape else { panic!("a shell") };
    let t = &bp.turret;
    let width_at = |y: f32| {
        mesh.vertices()
            .iter()
            .filter(|v| (v.position.y - y).abs() < 1.0e-3)
            .map(|v| v.position.x.abs())
            .fold(0.0_f32, f32::max)
    };
    assert!((width_at(t.roof_y) - width_at(t.ring_y)).abs() < 1.0e-4, "vertical side walls");
}

#[test]
fn the_tiger_ii_deck_wears_its_own_furniture() {
    let parts = german_deck_parts_for_blueprint(&blueprint(VehicleKind::TigerII))
        .expect("the Tiger II's deck is the library's");
    let keys: Vec<&str> = parts.iter().map(|p| p.key.name).collect();
    assert!(keys.contains(&"periscope_hood"), "the driver's periscope hood: {keys:?}");
    for absent in ["driver_visor", "exhaust_shield", "spare_track"] {
        assert!(!keys.contains(&absent), "{absent} is the Tiger I's, not the Tiger II's");
    }
    assert_eq!(keys.iter().filter(|k| **k == "fender_flap").count(), 2, "bow flaps only");
    assert_eq!(keys.iter().filter(|k| **k == "exhaust_stack").count(), 2);
    assert!(keys.contains(&"exhaust_mouth"), "open stacks have dark mouths");
    assert!(keys.contains(&"exhaust_bracket"), "the stacks stand off the leaned stern on brackets");
    let tiger_i = german_deck_parts_for_blueprint(&blueprint(VehicleKind::TigerI)).expect("deck");
    let keys: Vec<&str> = tiger_i.iter().map(|p| p.key.name).collect();
    assert!(keys.contains(&"driver_visor") && keys.contains(&"exhaust_shield"));
    assert_eq!(keys.iter().filter(|k| **k == "fender_flap").count(), 4, "flaps at both ends");
}

#[test]
fn the_schuerzen_hang_on_the_spaced_armor_plane() {
    let bp = blueprint(VehicleKind::TigerII);
    let skirt = bp.hull.skirt.expect("the Tiger II hangs skirts");
    let parts = skirt_parts_for_blueprint(&bp).expect("the library hangs them");
    assert_eq!(parts.len(), 2, "one plate a side");
    for part in &parts {
        let (min, max) = mesh_bounds(part);
        let outer = min.x.abs().max(max.x.abs());
        let plane = bp.track.outer_x + skirt.standoff_m + skirt.thickness_m;
        assert!((outer - plane).abs() < 1.0e-3, "on the spaced plane: {outer} vs {plane}");
        assert!(max.z - min.z > 4.0, "over the whole upper run: {}", max.z - min.z);
    }
    assert!(skirt_parts_for_blueprint(&blueprint(VehicleKind::TigerI)).is_none(), "no skirts");
}

#[test]
fn the_upper_hull_sides_lean_the_armour_table_s_degrees() {
    let bp = blueprint(VehicleKind::TigerII);
    let parts = slab_hull_parts_for_blueprint(&bp).expect("a welded prism");
    let upper = part(&parts, "slab_upper_box").mesh();
    let (sin, cos) = bp.armor.hull_side.0.to_radians().sin_cos();
    // The side plane through the sponson fold at the full beam, leaning inward above it.
    let normal = Vec3::new(cos, sin, 0.0);
    let offset = normal.dot(Vec3::new(bp.hull.half_width, bp.hull.sponson_y, 0.0));
    let on_plane = upper
        .vertices()
        .iter()
        .filter(|v| v.position.x > 0.3 && (normal.dot(v.position) - offset).abs() < 1.0e-3)
        .count();
    assert!(on_plane >= 4, "the starboard wall stands on the 25° armour plane: {on_plane}");
    let (min, max) = mesh_bounds(part(&parts, "slab_upper_box"));
    let top_width = upper
        .vertices()
        .iter()
        .filter(|v| (v.position.y - max.y).abs() < 1.0e-3)
        .map(|v| v.position.x.abs())
        .fold(0.0_f32, f32::max);
    let run = (max.y - min.y) * bp.armor.hull_side.0.to_radians().tan();
    assert!((top_width - (bp.hull.half_width - run)).abs() < 1.0e-3, "narrower at the deck");
}

/// The bow furniture stands ON the driver's plate: the MG ball's seat lies on the Tiger II's 50°
/// glacis (which folds at the sponson), not 0.48 m inside the hull where the nose-line rule put
/// it; the Tiger I's ball keeps its seat on the shelf-backed plate.
#[test]
fn the_bow_mg_ball_is_seated_on_the_glacis_plane() {
    for kind in [VehicleKind::TigerII, VehicleKind::TigerI] {
        let bp = blueprint(kind);
        let parts = german_deck_parts_for_blueprint(&bp).expect("a German deck");
        let (min, max) = mesh_bounds(part(&parts, "course_mg_port"));
        let y = (min.y + max.y) * 0.5;
        let glacis = bp.hull.glacis_slope_deg.to_radians().tan();
        let plate_z = match bp.armor.hull_bow_shelf {
            Some((top, setback)) => bp.hull.half_len - setback - (y - top) * glacis,
            None => bp.hull.half_len - (y - bp.hull.sponson_y) * glacis,
        };
        assert!(
            max.z > plate_z + 0.05 && min.z < plate_z,
            "{kind:?}: the ball pokes through the plate at z {plate_z:.3}: {min:?}..{max:?}"
        );
    }
}
