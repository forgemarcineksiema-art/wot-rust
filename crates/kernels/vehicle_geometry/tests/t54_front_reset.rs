use game_core::{VehicleBlueprint, VehicleKind};
use vehicle_geometry::{GeometryVertex, MaterialRole, SmoothingGroup, SubmeshKind, bake_vehicle};

const SG_CAST: SmoothingGroup = SmoothingGroup(2);
const SG_MANTLET: SmoothingGroup = SmoothingGroup(6);

#[test]
fn t54_front_hull_has_distinct_lower_plate() {
    let bp = VehicleBlueprint::for_vehicle(VehicleKind::T54_1951).expect("T-54 blueprint");
    let hull = t54_hull_vertices();

    let front_y_bands = unique_values(
        hull.iter().filter_map(|v| {
            (v.material == MaterialRole::RolledArmor
                && v.position.z >= bp.hull.half_len - 0.20
                && v.position.y <= bp.track.top_y + 0.08)
                .then_some(v.position.y)
        }),
        0.04,
    );

    assert!(
        front_y_bands.len() >= 3,
        "T-54 lower front should be a visible plate band, not a single nose edge: {front_y_bands:?}"
    );
}

#[test]
fn t54_upper_glacis_does_not_reach_ground() {
    let bp = VehicleBlueprint::for_vehicle(VehicleKind::T54_1951).expect("T-54 blueprint");
    let hull = t54_hull_vertices();
    let upper_front_z = bp.hull.half_len - 1.35;

    let lowest_upper_glacis_y = hull
        .iter()
        .filter(|v| v.material == MaterialRole::RolledArmor)
        .filter(|v| v.position.z >= upper_front_z && v.position.z <= bp.hull.half_len - 0.35)
        .filter(|v| v.normal.z < -0.25)
        .map(|v| v.position.y)
        .fold(f32::INFINITY, f32::min);

    assert!(
        lowest_upper_glacis_y >= bp.track.top_y - 0.04,
        "T-54 upper glacis starts too low at y={lowest_upper_glacis_y:.2}; it reads as a ramp"
    );
}

#[test]
fn t54_fenders_cover_track_top() {
    let bp = VehicleBlueprint::for_vehicle(VehicleKind::T54_1951).expect("T-54 blueprint");
    let hull = t54_hull_vertices();

    let fender_z: Vec<f32> = hull
        .iter()
        .filter_map(|v| {
            (v.material == MaterialRole::RolledArmor
                && v.position.x.abs() >= bp.track.inner_x - 0.02
                && v.position.x.abs() <= bp.track.outer_x + 0.06
                && v.position.y >= bp.track.top_y - 0.02
                && v.position.y <= bp.track.top_y + 0.14)
                .then_some(v.position.z)
        })
        .collect();
    let min_z = fender_z.iter().copied().fold(f32::INFINITY, f32::min);
    let max_z = fender_z.iter().copied().fold(f32::NEG_INFINITY, f32::max);

    assert!(
        min_z <= bp.track.wheel_first_z - bp.track.end_radius * 0.55
            && max_z >= bp.track.wheel_last_z + bp.track.end_radius * 0.55,
        "T-54 fenders should cover the top track run, z span [{min_z:.2}, {max_z:.2}]"
    );
}

#[test]
fn t54_mantlet_socket_is_recessed_between_cheeks() {
    let vehicle = bake_vehicle(VehicleKind::T54_1951).expect("T-54 bakes");
    let trunnion = vehicle.mounts().gun_trunnion.translation;
    let turret = vehicle.submesh(SubmeshKind::Turret).expect("turret submesh");

    let socket = bounds_for(
        turret
            .mesh
            .vertices()
            .iter()
            .filter(|v| v.smoothing == SG_MANTLET && (v.position.y - trunnion.y).abs() <= 0.35),
    );
    let cheeks = bounds_for(
        turret
            .mesh
            .vertices()
            .iter()
            .filter(|v| v.smoothing == SG_CAST)
            .filter(|v| (v.position.y - trunnion.y).abs() <= 0.38)
            .filter(|v| v.position.z >= socket.max_z - 0.16),
    );

    assert!(
        cheeks.max_z >= socket.max_z + 0.12,
        "T-54 turret cheeks must stand proud of recessed socket ({:.2} vs {:.2})",
        cheeks.max_z,
        socket.max_z
    );
    assert!(
        cheeks.width() >= socket.width() * 1.25,
        "T-54 cheeks should frame the socket laterally ({:.2} vs {:.2})",
        cheeks.width(),
        socket.width()
    );
}

#[test]
fn t54_mantlet_socket_has_a_layered_cast_lip() {
    let vehicle = bake_vehicle(VehicleKind::T54_1951).expect("T-54 bakes");
    let trunnion = vehicle.mounts().gun_trunnion.translation;
    let turret = vehicle.submesh(SubmeshKind::Turret).expect("turret submesh");

    let mut z_bands = std::collections::BTreeSet::new();
    for vertex in turret
        .mesh
        .vertices()
        .iter()
        .filter(|v| v.smoothing == SG_MANTLET && (v.position.y - trunnion.y).abs() <= 0.36)
    {
        z_bands.insert((vertex.position.z / 0.03).round() as i32);
    }

    assert!(
        z_bands.len() >= 4,
        "T-54 mantlet socket must have a layered cast lip, not a two-ring cone: {z_bands:?}"
    );
}

#[test]
fn t54_moving_mantlet_is_not_a_front_disc() {
    let vehicle = bake_vehicle(VehicleKind::T54_1951).expect("T-54 bakes");
    let trunnion = vehicle.mounts().gun_trunnion.translation;
    let turret = vehicle.submesh(SubmeshKind::Turret).expect("turret submesh");
    let gun = vehicle.submesh(SubmeshKind::Gun).expect("gun submesh");

    let cheek_front_z = turret
        .mesh
        .vertices()
        .iter()
        .filter(|v| v.smoothing == SG_CAST)
        .filter(|v| (v.position.y - trunnion.y).abs() <= 0.38)
        .map(|v| v.position.z)
        .fold(f32::NEG_INFINITY, f32::max);
    let moving_front_z = gun
        .mesh
        .vertices()
        .iter()
        .filter(|v| v.smoothing == SG_MANTLET)
        .map(|v| v.position.z)
        .fold(f32::NEG_INFINITY, f32::max);

    assert!(
        moving_front_z <= cheek_front_z + 0.02,
        "T-54 moving mantlet protrudes like a front disc ({moving_front_z:.2} vs cheek {cheek_front_z:.2})"
    );
}

#[test]
fn t54_front_review_has_meaningful_hull_plate_bands() {
    let bp = VehicleBlueprint::for_vehicle(VehicleKind::T54_1951).expect("T-54 blueprint");
    let hull = t54_hull_vertices();

    let y_bands = unique_values(
        hull.iter().filter_map(|v| {
            (v.material == MaterialRole::RolledArmor
                && v.position.z >= bp.hull.half_len - 0.70
                && v.position.y <= bp.hull.deck_y + 0.02)
                .then_some(v.position.y)
        }),
        0.08,
    );

    assert!(
        y_bands.len() >= 4,
        "front view should expose lower plate and upper glacis bands, got {y_bands:?}"
    );
}

#[derive(Clone, Copy)]
struct Bounds {
    min_x: f32,
    max_x: f32,
    max_z: f32,
}

impl Bounds {
    fn width(self) -> f32 {
        self.max_x - self.min_x
    }
}

fn t54_hull_vertices() -> Vec<GeometryVertex> {
    bake_vehicle(VehicleKind::T54_1951)
        .expect("T-54 bakes")
        .submesh(SubmeshKind::Hull)
        .expect("hull submesh")
        .mesh
        .vertices()
        .to_vec()
}

fn bounds_for<'a>(vertices: impl Iterator<Item = &'a GeometryVertex>) -> Bounds {
    let mut bounds =
        Bounds { min_x: f32::INFINITY, max_x: f32::NEG_INFINITY, max_z: f32::NEG_INFINITY };
    for v in vertices {
        bounds.min_x = bounds.min_x.min(v.position.x);
        bounds.max_x = bounds.max_x.max(v.position.x);
        bounds.max_z = bounds.max_z.max(v.position.z);
    }
    assert!(bounds.max_z.is_finite(), "expected measured vertices");
    bounds
}

fn unique_values(values: impl Iterator<Item = f32>, tolerance: f32) -> Vec<f32> {
    let mut values: Vec<f32> = values.collect();
    values.sort_by(f32::total_cmp);
    values.dedup_by(|a, b| (*a - *b).abs() <= tolerance);
    values
}
