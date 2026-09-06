//! The T-34-85 as library parts (Forge 2.0 K3, 2026-09-06): the first cast dome the library
//! builds outside the benchmark, the slit cupola built to the authored height, and the Soviet
//! deck's glacis furniture — each lock names what would silently turn the -85 into a T-54 with a
//! shorter gun.

use game_core::{VehicleBlueprint, VehicleKind};
use glam::Vec3;
use vehicle_build::{
    PartShape, VehiclePart, british_deck_parts_for_blueprint, cast_dome_parts_for_blueprint,
    fitting_parts_for_blueprint, slab_hull_parts_for_blueprint, soviet_deck_parts_for_blueprint,
};

fn t34_85() -> VehicleBlueprint {
    VehicleBlueprint::for_vehicle(VehicleKind::T34_85).expect("the T-34-85 is blueprint-born")
}

fn extent(part: &VehiclePart) -> (Vec3, Vec3) {
    let aabb = part.mesh().bounds().expect("a Soviet part has volume");
    (aabb.min, aabb.max)
}

#[test]
fn the_cast_dome_carries_the_slit_cupola_to_the_authored_height() {
    let bp = t34_85();
    let parts = cast_dome_parts_for_blueprint(&bp).expect("the T-34-85 authors its dome");
    let keys: Vec<&str> = parts.iter().map(|p| p.key.name).collect();
    for key in ["turret_shell", "cupola_drum", "roof_hatch", "turret_ring_collar", "mantlet_socket"]
    {
        assert!(keys.contains(&key), "{key} is built: {keys:?}");
    }
    assert!(!keys.contains(&"turret_bin"), "no bustle bin on the -85");
    let t = &bp.turret;
    let proud = t.cupola_height.expect("the cupola's height is authored");
    // The drum's cast top (the split lid's crown) stands at the authored height over the roof.
    let cupola = parts.iter().find(|p| p.key.name == "cupola_drum").expect("cupola");
    let PartShape::Mesh(mesh) = &cupola.shape else { panic!("a revolved cupola") };
    let cast_top = mesh
        .vertices()
        .iter()
        .filter(|v| v.material == vehicle_geometry::MaterialRole::CastArmor)
        .map(|v| v.position.y)
        .fold(f32::NEG_INFINITY, f32::max);
    assert!(
        (cast_top - (t.roof_y + proud)).abs() < 0.02,
        "the lid's crown at roof {} + {proud}: {cast_top}",
        t.roof_y
    );
    // The dome itself keeps the blueprint's ring and roof.
    let (min, max) = extent(parts.iter().find(|p| p.key.name == "turret_shell").unwrap());
    assert!((min.y - t.ring_y).abs() < 1.0e-3 && (max.y - t.roof_y).abs() < 1.0e-3);
    assert!((max.x - t.base_radius).abs() < 1.0e-2, "the beam at the shoulder: {}", max.x);
}

#[test]
fn the_t34_deck_puts_the_driver_s_hatch_and_the_ball_in_the_glacis() {
    let bp = t34_85();
    let parts = soviet_deck_parts_for_blueprint(&bp).expect("the T-34-85 authors its deck");
    let keys: Vec<&str> = parts.iter().map(|p| p.key.name).collect();
    assert_eq!(keys.iter().filter(|k| **k == "engine_deck_louvre").count(), 4);
    assert_eq!(keys.iter().filter(|k| **k == "exhaust_port").count(), 2);
    assert_eq!(keys.iter().filter(|k| **k == "periscope_hood").count(), 4, "two hoods, two panes");
    assert!(keys.contains(&"glacis_hatch") && keys.contains(&"course_mg_port"));
    assert!(!keys.iter().any(|k| k.starts_with("fender") || k.starts_with("fuel_tank")));
    // The hatch plate lies ON the 60° glacis: its centre sits 2 cm off the plane.
    let glacis = bp.hull.glacis_slope_deg.to_radians();
    // The plate leans `glacis` from vertical: its normal is (0, sin g, cos g).
    let normal = Vec3::new(0.0, glacis.sin(), glacis.cos());
    let fold = Vec3::new(0.0, bp.hull.sponson_y, bp.hull.half_len);
    let (min, max) = extent(parts.iter().find(|p| p.key.name == "glacis_hatch").unwrap());
    let centre = (min + max) * 0.5;
    let off = normal.dot(centre - fold);
    assert!((off - 0.02).abs() < 0.03, "the hatch rides the glacis plane: {off:.3} off it");
    // The ball pokes through the plate: metal on both sides of the plane.
    let (min, max) = extent(parts.iter().find(|p| p.key.name == "course_mg_port").unwrap());
    let front = normal.dot(Vec3::new(0.0, max.y, max.z) - fold);
    let back = normal.dot(Vec3::new(0.0, min.y, min.z) - fold);
    assert!(front > 0.05 && back < 0.0, "the ball seats in the plate: {back:.3}..{front:.3}");
    // The exhaust ports sit ON the sloped stern.
    let rear = bp.hull.rear_slope_deg.to_radians().tan();
    for port in parts.iter().filter(|p| p.key.name == "exhaust_port") {
        let (min, max) = extent(port);
        let y = (min.y + max.y) * 0.5;
        let plate_z = -bp.hull.half_len + (y - bp.hull.sponson_y).abs() * rear;
        assert!(min.z < plate_z && max.z > plate_z, "a port in the stern plate at {plate_z:.3}");
    }
}

#[test]
fn the_fittings_build_only_the_lamp_and_the_hooks_on_the_t34() {
    let parts = fitting_parts_for_blueprint(&t34_85()).expect("the bow fittings");
    let keys: Vec<&str> = parts.iter().map(|p| p.key.name).collect();
    for absent in ["cupola_hatch", "loader_hatch", "driver_hatch", "radio_hatch"] {
        assert!(
            !keys.contains(&absent),
            "{absent}: the T-34's lids are the dome's and the glacis's"
        );
    }
    assert!(
        keys.iter().any(|k| k.starts_with("headlight"))
            && keys.iter().any(|k| k.starts_with("tow_hook"))
    );
}

/// The IS-3's pike hull: the tub and the upper box end where the pike takes over, and the four
/// bow faces lie ON the armour's pike planes (the fold ridge at the sponson step, the 56° slope,
/// the ±38° sweep) — the honesty lock the whole fleet copied, now on the library's parts.
#[test]
fn the_pike_bow_lies_on_the_armour_s_pike_planes() {
    let bp = VehicleBlueprint::for_vehicle(VehicleKind::IS3).expect("the IS-3 is blueprint-born");
    let parts = slab_hull_parts_for_blueprint(&bp).expect("a welded pike");
    let keys: Vec<&str> = parts.iter().map(|p| p.key.name).collect();
    assert_eq!(keys, ["slab_tub", "slab_upper_box", "slab_bow_pike"]);
    let hull = &bp.hull;
    let sweep = hull.pike_sweep_deg.to_radians();
    let glacis = hull.glacis_slope_deg.to_radians();
    let fold = Vec3::new(0.0, hull.sponson_y, hull.half_len);
    let pike = parts.iter().find(|p| p.key.name == "slab_bow_pike").unwrap().mesh();
    for side in [1.0_f32, -1.0] {
        let normal =
            Vec3::new(side * sweep.sin() * glacis.cos(), glacis.sin(), sweep.cos() * glacis.cos());
        let on_plane = pike
            .vertices()
            .iter()
            .filter(|v| v.position.x * side >= -1.0e-3 && v.position.y >= hull.sponson_y - 1.0e-3)
            .filter(|v| (normal.dot(v.position - fold)).abs() < 1.0e-3)
            .count();
        assert!(on_plane >= 4, "the upper pike face on its plane (side {side}): {on_plane}");
    }
    // The bow's forwardmost point is the fold ridge tip at the sponson step, on the centreline.
    let tip = pike
        .vertices()
        .iter()
        .map(|v| v.position)
        .fold(Vec3::ZERO, |best, p| if p.z > best.z { p } else { best });
    assert!((tip.z - hull.half_len).abs() < 1.0e-3 && tip.x.abs() < 1.0e-3);
    assert!(
        (tip.y - hull.sponson_y).abs() < 1.0e-3,
        "the ridge tip at the sponson step: {}",
        tip.y
    );
}

/// The IS-3's dome carries no cupola: two flush hatches at the mirrored cupola stations and the
/// commander's periscope; the deck hangs the fender line and the drums along the belts.
#[test]
fn the_is3_dome_and_deck_wear_the_heavy_s_furniture() {
    let bp = VehicleBlueprint::for_vehicle(VehicleKind::IS3).expect("the IS-3 is blueprint-born");
    let dome = cast_dome_parts_for_blueprint(&bp).expect("the IS-3 authors its dome");
    let keys: Vec<&str> = dome.iter().map(|p| p.key.name).collect();
    assert!(!keys.contains(&"cupola_drum"), "no cupola: {keys:?}");
    assert_eq!(keys.iter().filter(|k| **k == "roof_hatch").count(), 2);
    assert!(keys.contains(&"turret_periscope"));
    let deck = soviet_deck_parts_for_blueprint(&bp).expect("the IS-3 authors its deck");
    let keys: Vec<&str> = deck.iter().map(|p| p.key.name).collect();
    for key in ["fender_shelf", "fender_front", "fender_rear", "fuel_tank"] {
        assert_eq!(keys.iter().filter(|k| **k == key).count(), 2, "{key} both sides: {keys:?}");
    }
    assert!(
        !keys.contains(&"glacis_hatch") && !keys.contains(&"course_mg_port"),
        "no glacis furniture"
    );
    // The drums lie along the rear shelves, over the belts, inside the hitbox width.
    for drum in deck.iter().filter(|p| p.key.name == "fuel_tank") {
        let (min, max) = extent(drum);
        assert!(max.z < 0.0, "a drum along the REAR shelf: {}", max.z);
        assert!(max.x.abs().max(min.x.abs()) <= bp.hull.hitbox_half_width + 1.0e-3);
    }
}

/// The Centurion's Mk 3 dome closes its plan with the bustle bin ON the rear armour plane, wears
/// the British cupola, and its deck carries the cowls, the rectangular roof hatch and the fender
/// boxes over the belts.
#[test]
fn the_centurion_dome_and_deck_wear_the_british_furniture() {
    let bp = VehicleBlueprint::for_vehicle(VehicleKind::Centurion).expect("blueprint-born");
    let dome = cast_dome_parts_for_blueprint(&bp).expect("the Centurion authors its dome");
    let keys: Vec<&str> = dome.iter().map(|p| p.key.name).collect();
    assert!(
        keys.contains(&"cupola_drum")
            && keys.contains(&"turret_bin")
            && keys.contains(&"roof_hatch")
    );
    let (min, _) = extent(dome.iter().find(|p| p.key.name == "turret_bin").unwrap());
    let plan_rear = bp.turret.ring_z - bp.turret.plan_half_length;
    assert!(
        (min.z - (plan_rear + 0.01)).abs() < 1.0e-2,
        "the bin's back at the plan's rear: {}",
        min.z
    );
    let deck = british_deck_parts_for_blueprint(&bp).expect("the Centurion authors its deck");
    let keys: Vec<&str> = deck.iter().map(|p| p.key.name).collect();
    assert_eq!(keys.iter().filter(|k| **k == "exhaust_cowl").count(), 2);
    assert_eq!(keys.iter().filter(|k| **k == "stowage_bin").count(), 4, "two boxes a side");
    assert!(keys.contains(&"driver_roof_hatch") && keys.contains(&"engine_deck_panel"));
    // The hatch lies on the roof, starboard, ahead of the ring.
    let (min, max) = extent(deck.iter().find(|p| p.key.name == "driver_roof_hatch").unwrap());
    assert!(max.x < 0.0, "starboard: {}", max.x);
    assert!((min.y - bp.hull.deck_y).abs() < 0.03, "on the roof: {}", min.y);
    assert!(min.z > bp.turret.ring_z + bp.turret.ring_radius, "ahead of the ring: {}", min.z);
    // The boxes sit on the fender line over the belts, never past the belt's outer face.
    for bin in deck.iter().filter(|p| p.key.name == "stowage_bin") {
        let (min, max) = extent(bin);
        assert!(max.x.abs().max(min.x.abs()) <= bp.track.outer_x + 1.0e-3);
        assert!(min.y >= bp.hull.sponson_y, "over the sponson line: {}", min.y);
    }
}
