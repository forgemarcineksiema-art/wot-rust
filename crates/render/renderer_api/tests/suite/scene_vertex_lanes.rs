//! The SceneVertex lane contract (Imported Flora 2.0, FL-1): the UV lane exists, defaults
//! to [0, 0] through EVERY constructor — which is what licenses "pixel-identical until the
//! textured path opts in" — and the layout grew by appending, never reordering.

use renderer_api::SceneVertex;

#[test]
fn the_uv_lane_defaults_to_zero_through_every_constructor() {
    let p = [1.0, 2.0, 3.0];
    let n = [0.0, 1.0, 0.0];
    let c = [0.5, 0.5, 0.5];
    for vertex in [
        SceneVertex::new(p, n, c),
        SceneVertex::surfaced(p, n, c, 0.3),
        SceneVertex::tinted(p, n, c, 1.0),
        SceneVertex::new(p, n, c).with_surface(2.0),
        SceneVertex::new(p, n, c).with_sway(0.7),
    ] {
        assert_eq!(vertex.uv, [0.0, 0.0], "procedural content never samples - uv stays zero");
        assert_eq!(vertex.bounce, [0.0; 3], "unbaked content carries no indirect light");
    }
    let textured = SceneVertex::new(p, n, c).with_uv([0.25, 0.75]);
    assert_eq!(textured.uv, [0.25, 0.75], "the textured path names its coordinates");
    let baked = SceneVertex::new(p, n, c).with_bounce([0.1, 0.2, 0.3]);
    assert_eq!(baked.bounce, [0.1, 0.2, 0.3], "the GI bake names its radiance");
}

#[test]
fn the_layout_grew_by_appending() {
    // 18 floats, 72 bytes: the earlier lanes keep their offsets, bounce is the tail. A reorder
    // would corrupt every pipeline at once - this pins the append discipline.
    assert_eq!(std::mem::size_of::<SceneVertex>(), 72);
    assert_eq!(std::mem::offset_of!(SceneVertex, bounce), 60, "bounce appended after uv");
    assert_eq!(std::mem::offset_of!(SceneVertex, uv), 52, "uv appended after sway");
    assert_eq!(std::mem::offset_of!(SceneVertex, sway), 48);
    assert_eq!(std::mem::offset_of!(SceneVertex, surface), 44);
}
