use game_core::{TrackShape, VehicleBlueprint, VehicleKind};
use vehicle_geometry::{MaterialRole, SmoothingGroup, SubmeshKind, bake_vehicle};

const SG_CAST: SmoothingGroup = SmoothingGroup(2);
const SG_MANTLET: SmoothingGroup = SmoothingGroup(6);

#[test]
fn t54_1951_d10t_has_no_bore_evacuator() {
    let bp = VehicleBlueprint::for_vehicle(VehicleKind::T54_1951).expect("T-54 blueprint");

    assert!(bp.gun.evacuator.is_none(), "T-54-3 obr. 1951 D-10T must not carry a bore evacuator");
}

#[test]
fn t54_running_gear_has_no_return_rollers() {
    let bp = VehicleBlueprint::for_vehicle(VehicleKind::T54_1951).expect("T-54 blueprint");
    let vehicle = bake_vehicle(VehicleKind::T54_1951).expect("T-54 bakes");
    let hull = vehicle.submesh(SubmeshKind::Hull).expect("hull submesh");

    let rubber_above_track = hull
        .mesh
        .vertices()
        .iter()
        .filter(|v| v.material == MaterialRole::Rubber)
        .filter(|v| v.position.y > bp.track.top_y + 0.03)
        .count();

    assert_eq!(
        rubber_above_track, 0,
        "T-54 running gear should not add return rollers above the road-wheel/track band"
    );
}

#[test]
fn t54_track_belt_wraps_the_ends_instead_of_two_straight_slabs() {
    let bp = VehicleBlueprint::for_vehicle(VehicleKind::T54_1951).expect("T-54 blueprint");
    let vehicle = bake_vehicle(VehicleKind::T54_1951).expect("T-54 bakes");
    let hull = vehicle.submesh(SubmeshKind::Hull).expect("hull submesh");
    let mid_y_min = bp.track.bottom_y + 0.22;
    let mid_y_max = bp.track.top_y - 0.22;
    let end_z = bp.track.wheel_last_z - 0.10;

    let wrapped_end_vertices = hull
        .mesh
        .vertices()
        .iter()
        .filter(|v| v.material == MaterialRole::TrackMetal)
        .filter(|v| v.position.y >= mid_y_min && v.position.y <= mid_y_max)
        .filter(|v| v.position.z.abs() >= end_z)
        .count();

    assert!(
        wrapped_end_vertices >= 24,
        "track metal needs mid-height vertices around idler/sprocket ends, not only top/bottom slabs"
    );
}

#[test]
fn t54_mantlet_sits_low_in_the_turret_front() {
    let bp = VehicleBlueprint::for_vehicle(VehicleKind::T54_1951).expect("T-54 blueprint");
    let vertical_ratio =
        (bp.gun.trunnion_y - bp.turret.ring_y) / (bp.turret.roof_y - bp.turret.ring_y);

    assert!(
        vertical_ratio <= 0.55,
        "T-54 mantlet/trunnion sits too high in turret face: ratio {vertical_ratio:.2}"
    );
}

#[test]
fn t54_moving_mantlet_is_wide_oval_not_round_ball() {
    let vehicle = bake_vehicle(VehicleKind::T54_1951).expect("T-54 bakes");
    let gun = vehicle.submesh(SubmeshKind::Gun).expect("gun submesh");

    let mut min_x = f32::INFINITY;
    let mut max_x = f32::NEG_INFINITY;
    let mut min_y = f32::INFINITY;
    let mut max_y = f32::NEG_INFINITY;
    for v in gun.mesh.vertices().iter().filter(|v| v.smoothing == SG_MANTLET) {
        min_x = min_x.min(v.position.x);
        max_x = max_x.max(v.position.x);
        min_y = min_y.min(v.position.y);
        max_y = max_y.max(v.position.y);
    }

    let width = max_x - min_x;
    let height = max_y - min_y;
    assert!(
        width >= height * 1.35,
        "T-54 moving mantlet should be a flattened oval embedded in the turret ({width:.2} x {height:.2})"
    );
}

#[test]
fn t54_road_wheels_keep_the_first_gap_visible() {
    let bp = VehicleBlueprint::for_vehicle(VehicleKind::T54_1951).expect("T-54 blueprint");
    let vehicle = bake_vehicle(VehicleKind::T54_1951).expect("T-54 bakes");
    let hull = vehicle.submesh(SubmeshKind::Hull).expect("hull submesh");
    let wheel_tops = road_wheel_top_centers(&bp.track, hull.mesh.vertices().iter());

    assert_eq!(wheel_tops.len(), 5, "T-54 should expose five road-wheel centers");
    let first_gap = wheel_tops[1] - wheel_tops[0];
    let middle_gap = wheel_tops[3] - wheel_tops[2];
    assert!(
        first_gap >= middle_gap * 1.18,
        "T-54 first-second wheel gap should read wider than the middle spacing \
         ({first_gap:.2} vs {middle_gap:.2})"
    );
}

#[test]
fn t54_track_links_are_dense_enough_to_read_as_a_belt() {
    let bp = VehicleBlueprint::for_vehicle(VehicleKind::T54_1951).expect("T-54 blueprint");
    let vehicle = bake_vehicle(VehicleKind::T54_1951).expect("T-54 bakes");
    let hull = vehicle.submesh(SubmeshKind::Hull).expect("hull submesh");
    let outer_x = bp.track.center_x + bp.track.belt_half_thickness * 0.35;

    let top_edges = unique_z_values(
        hull.mesh.vertices().iter().filter_map(|v| {
            (v.material == MaterialRole::TrackMetal
                && v.position.x > outer_x
                && (v.position.y - bp.track.top_y).abs() <= 0.12)
                .then_some(v.position.z)
        }),
        0.03,
    );

    assert!(
        top_edges.len() >= 48,
        "T-54 top track run needs many repeated shoe edges, not ten boxes ({})",
        top_edges.len()
    );
}

#[test]
fn t54_cast_turret_has_enough_plan_segments_for_closeup_review() {
    let vehicle = bake_vehicle(VehicleKind::T54_1951).expect("T-54 bakes");
    let turret = vehicle.submesh(SubmeshKind::Turret).expect("turret submesh");

    let plan_angles = unique_z_values(
        turret
            .mesh
            .vertices()
            .iter()
            .filter(|v| v.smoothing == SG_CAST)
            .map(|v| v.position.z.atan2(v.position.x)),
        0.03,
    );

    assert!(
        plan_angles.len() >= 24,
        "T-54 cast turret is too faceted for close-up review: {} plan segments",
        plan_angles.len()
    );
}

fn road_wheel_top_centers<'a>(
    track: &TrackShape,
    vertices: impl Iterator<Item = &'a vehicle_geometry::GeometryVertex>,
) -> Vec<f32> {
    unique_z_values(
        vertices.filter_map(|v| {
            (v.material == MaterialRole::Rubber
                && (v.position.y - (track.top_y + track.bottom_y) * 0.5 - track.wheel_radius).abs()
                    <= 0.025
                && v.position.x > track.inner_x - 0.02)
                .then_some(v.position.z)
        }),
        0.04,
    )
}

fn unique_z_values(values: impl Iterator<Item = f32>, tolerance: f32) -> Vec<f32> {
    let mut values: Vec<f32> = values.collect();
    values.sort_by(f32::total_cmp);
    values.dedup_by(|a, b| (*a - *b).abs() <= tolerance);
    values
}
