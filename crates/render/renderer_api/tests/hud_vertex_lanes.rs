//! The `HudVertex` lane contract (interface program F2): the layout grew by APPENDING — the
//! three legacy lanes keep their offsets, the four new ones follow — and both legacy constructors
//! leave the new lanes at zero, which is what licenses "the look goldens do not move by a byte"
//! while the plate, sheet and glass styles opt in vertex by vertex.

use renderer_api::{HUD_SOLID_UV, HudVertex, hud_style};

#[test]
fn the_hud_vertex_grew_by_appending() {
    assert_eq!(std::mem::size_of::<HudVertex>(), 64, "sixteen lanes of four bytes");
    assert_eq!(std::mem::offset_of!(HudVertex, position), 0);
    assert_eq!(std::mem::offset_of!(HudVertex, uv), 8);
    assert_eq!(std::mem::offset_of!(HudVertex, color), 16);
    assert_eq!(std::mem::offset_of!(HudVertex, local), 32, "local appended after colour");
    assert_eq!(std::mem::offset_of!(HudVertex, extent), 40);
    assert_eq!(std::mem::offset_of!(HudVertex, params), 48);
    assert_eq!(std::mem::offset_of!(HudVertex, style), 56);
    assert_eq!(std::mem::offset_of!(HudVertex, reserved), 60);
}

#[test]
fn the_legacy_constructors_leave_the_new_lanes_at_zero() {
    let solid = HudVertex::new([0.1, -0.2], [1.0, 0.5, 0.25, 0.9]);
    assert_eq!(solid.uv, HUD_SOLID_UV);
    assert_eq!(solid.local, [0.0, 0.0]);
    assert_eq!(solid.extent, [0.0, 0.0]);
    assert_eq!(solid.params, [0.0, 0.0]);
    assert_eq!(solid.style, hud_style::SOLID);
    assert_eq!(solid.reserved, 0);

    let glyph = HudVertex::textured([0.1, -0.2], [0.3, 0.7], [1.0, 1.0, 1.0, 1.0]);
    assert_eq!(glyph.uv, [0.3, 0.7]);
    assert_eq!(glyph.local, [0.0, 0.0]);
    assert_eq!(glyph.extent, [0.0, 0.0]);
    assert_eq!(glyph.params, [0.0, 0.0]);
    assert_eq!(glyph.style, hud_style::GLYPH);
    assert_eq!(glyph.reserved, 0);
}

#[test]
fn a_style_packs_its_kind_and_its_tile() {
    let style = hud_style::with_tile(hud_style::PLATE, 5);
    assert_eq!(hud_style::kind(style), hud_style::PLATE);
    assert_eq!(hud_style::tile(style), 5);
    assert_eq!(hud_style::kind(hud_style::GLASS), hud_style::GLASS);
    assert_eq!(hud_style::tile(hud_style::GLASS), 0);
    // The kinds are append-only and distinct: a renumbering re-keys every plate ever drawn.
    let kinds =
        [hud_style::SOLID, hud_style::GLYPH, hud_style::PLATE, hud_style::SHEET, hud_style::GLASS];
    assert_eq!(kinds, [0, 1, 2, 3, 4]);
}

#[test]
fn the_new_constructors_name_their_style() {
    let plate = HudVertex::plate([0.0, 0.0], [3.0, 4.0], [40.0, 12.0], 6.0, -1.5, 5, [0.5; 4]);
    assert_eq!(hud_style::kind(plate.style), hud_style::PLATE);
    assert_eq!(hud_style::tile(plate.style), 5);
    assert_eq!(plate.local, [3.0, 4.0]);
    assert_eq!(plate.extent, [40.0, 12.0]);
    assert_eq!(plate.params, [6.0, -1.5], "corner radius, then bevel — negative is inset");
    assert_eq!(plate.uv, [0.0, 0.0], "a plate never touches the atlas");

    let sheet = HudVertex::sheet([0.0, 0.0], [0.25, 0.75], [1.0; 4]);
    assert_eq!(sheet.style, hud_style::SHEET);
    assert_eq!(sheet.uv, [0.25, 0.75]);

    let glass =
        HudVertex::glass([0.0, 0.0], [1.0, 1.0], [20.0, 8.0], 4.0, 0.3, [0.2, 0.2, 0.3, 0.5]);
    assert_eq!(glass.style, hud_style::GLASS);
    assert_eq!(glass.params, [4.0, 0.3], "corner radius, then the reflection's phase");
}
