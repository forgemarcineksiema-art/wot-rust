//! The fabricated-panel contract: a convex outline becomes a closed plate of the right size with the
//! requested edge treatment, and malformed specs are typed errors.

use glam::{Vec2, Vec3};
use panel::{PanelEdge, PanelError, PanelSpec, try_panel};
use vehicle_geometry::{
    CLOSED_SMOOTH_MESH, GeometryMesh, MaterialRole, OPEN_OR_CLOSED_MESH, SmoothingGroup,
};

fn rect() -> Vec<Vec2> {
    vec![Vec2::new(-1.0, -0.6), Vec2::new(1.0, -0.6), Vec2::new(1.0, 0.6), Vec2::new(-1.0, 0.6)]
}

fn panel(edge: PanelEdge, smoothing: SmoothingGroup) -> Result<GeometryMesh, PanelError> {
    try_panel(&PanelSpec {
        outline: rect(),
        origin: Vec3::ZERO,
        normal: Vec3::Y,
        thickness: 0.2,
        edge,
        material: MaterialRole::RolledArmor,
        smoothing,
    })
}

#[test]
fn a_sharp_panel_is_a_closed_plate_of_the_right_size() {
    // Built with a smooth group the coincident corners weld, so it is a watertight manifold; the
    // hard-edge build below keeps the crisp faces.
    let mesh = panel(PanelEdge::Sharp, SmoothingGroup(1)).expect("sharp panel");
    mesh.validate_quality(CLOSED_SMOOTH_MESH).expect("a welded panel is a closed manifold");
    let b = mesh.bounds().expect("bounds");
    assert!((b.max.x - 1.0).abs() < 1.0e-4 && (b.min.x + 1.0).abs() < 1.0e-4, "spans its width");
    assert!((b.max.z - 0.6).abs() < 1.0e-4, "spans its depth");
    // Extruded down -normal (=-Y) by the thickness.
    assert!(
        (b.max.y - 0.0).abs() < 1.0e-4 && (b.min.y + 0.2).abs() < 1.0e-4,
        "thickness 0.2 in -Y"
    );
}

#[test]
fn a_sharp_panel_keeps_crisp_planar_faces_by_default() {
    let mesh = panel(PanelEdge::Sharp, SmoothingGroup::hard_edges()).expect("sharp panel");
    mesh.validate_quality(OPEN_OR_CLOSED_MESH).expect("clean");
    // The top face vertices carry the exact +Y plate normal — a flat planar face.
    let top = mesh.vertices().iter().filter(|v| v.position.y > -1.0e-3);
    assert!(top.clone().count() > 0);
    assert!(
        top.into_iter().any(|v| (v.normal - Vec3::Y).length() < 1.0e-3),
        "the top face carries the exact +Y plate normal"
    );
}

#[test]
fn a_chamfer_insets_the_top_and_drops_it() {
    let mesh = panel(PanelEdge::Chamfer { width: 0.08 }, SmoothingGroup(1)).expect("chamfer");
    mesh.validate_quality(CLOSED_SMOOTH_MESH).expect("a chamfered panel is closed");
    // The flat top face is inset: no top-face vertex reaches the full ±1.0 / ±0.6 outline rim.
    let top_at_rim = mesh
        .vertices()
        .iter()
        .filter(|v| v.position.y > -1.0e-3)
        .any(|v| v.position.x.abs() > 0.999 || v.position.z.abs() > 0.599);
    assert!(!top_at_rim, "the chamfered top face is inset from the rim");
}

#[test]
fn a_hem_adds_a_return_skirt_below_the_thickness() {
    let mesh =
        panel(PanelEdge::Hem { width: 0.05, return_depth: 0.1 }, SmoothingGroup(1)).expect("hem");
    mesh.validate_quality(CLOSED_SMOOTH_MESH).expect("a hemmed panel is closed");
    let b = mesh.bounds().expect("bounds");
    // Thickness 0.2 plus a 0.1 return = 0.3 of depth below the top plane.
    assert!((b.min.y + 0.3).abs() < 1.0e-4, "hem extends the skirt to 0.3, got {}", b.min.y);
}

#[test]
fn too_few_outline_points_is_rejected() {
    let spec = PanelSpec {
        outline: vec![Vec2::ZERO, Vec2::X],
        origin: Vec3::ZERO,
        normal: Vec3::Y,
        thickness: 0.2,
        edge: PanelEdge::Sharp,
        material: MaterialRole::RolledArmor,
        smoothing: SmoothingGroup::hard_edges(),
    };
    assert_eq!(try_panel(&spec), Err(PanelError::TooFewOutlinePoints));
}

#[test]
fn a_non_positive_thickness_is_rejected() {
    let spec = PanelSpec {
        outline: rect(),
        origin: Vec3::ZERO,
        normal: Vec3::Y,
        thickness: 0.0,
        edge: PanelEdge::Sharp,
        material: MaterialRole::RolledArmor,
        smoothing: SmoothingGroup::hard_edges(),
    };
    assert_eq!(try_panel(&spec), Err(PanelError::NonPositiveThickness));
}

#[test]
fn a_zero_normal_is_rejected() {
    let spec = PanelSpec {
        outline: rect(),
        origin: Vec3::ZERO,
        normal: Vec3::ZERO,
        thickness: 0.2,
        edge: PanelEdge::Sharp,
        material: MaterialRole::RolledArmor,
        smoothing: SmoothingGroup::hard_edges(),
    };
    assert_eq!(try_panel(&spec), Err(PanelError::InvalidNormal));
}

#[test]
fn a_concave_outline_is_rejected() {
    let concave = vec![
        Vec2::new(-1.0, -1.0),
        Vec2::new(1.0, -1.0),
        Vec2::new(1.0, 1.0),
        Vec2::new(0.0, 0.1),
        Vec2::new(-1.0, 1.0),
    ];
    let spec = PanelSpec {
        outline: concave,
        origin: Vec3::ZERO,
        normal: Vec3::Y,
        thickness: 0.2,
        edge: PanelEdge::Sharp,
        material: MaterialRole::RolledArmor,
        smoothing: SmoothingGroup::hard_edges(),
    };
    assert_eq!(try_panel(&spec), Err(PanelError::NonConvexOutline));
}

#[test]
fn a_chamfer_wider_than_the_thickness_is_rejected() {
    assert_eq!(
        panel(PanelEdge::Chamfer { width: 0.5 }, SmoothingGroup(1)),
        Err(PanelError::InvalidEdge)
    );
}
