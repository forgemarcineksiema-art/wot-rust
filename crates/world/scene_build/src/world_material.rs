//! The one place a world vertex's MATERIAL becomes a scene vertex's look.
//!
//! `world_forge` bakes structures against its own surface vocabulary (`WorldMaterial`: wall,
//! roof, plinth stone, glass, joinery, timber…), carried through the shared mesh contract. Two
//! consumers now decode it — the building facades and the masonry spill they collapse into — and
//! a decision written twice is a decision about to disagree, so it is written here once.
//!
//! Colour names the palette; the surface-role lane names the MATERIAL the scene shader dresses it
//! in (Materia Świata 3): rendered walls, coursed roofs, plank joinery, ashlar stone. Glass alone
//! keeps the untreated look.

use renderer_api::surface_role;
use world_forge::WorldMaterial;

/// Window glass: near-black with a glazed sheen — the one thing on a wall that answers the sky.
pub(crate) const WINDOW: ([f32; 3], f32) = ([0.07, 0.09, 0.11], 0.45);
/// Plank door: dark weathered timber, matte.
pub(crate) const DOOR: ([f32; 3], f32) = ([0.16, 0.11, 0.07], 0.06);

/// The palettes a caller brings: walls and roofs vary per building instance, dressed stone per
/// map or per id. Everything else is the material's own PBR-lite default.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Palette {
    pub wall: [f32; 3],
    pub roof: [f32; 3],
    pub roof_gloss: f32,
    pub stone: ([f32; 3], f32),
}

/// `(colour, gloss, surface role)` for one baked world vertex.
pub(crate) fn to_scene(material: WorldMaterial, palette: Palette) -> ([f32; 3], f32, f32) {
    match material {
        WorldMaterial::Roof => (palette.roof, palette.roof_gloss, surface_role::SLATE),
        WorldMaterial::PlinthStone => {
            (palette.stone.0, palette.stone.1, surface_role::DRESSED_STONE)
        }
        WorldMaterial::WindowGlass => (WINDOW.0, WINDOW.1, surface_role::LEGACY),
        WorldMaterial::PlankDoor => (DOOR.0, DOOR.1, surface_role::PLANK),
        material @ WorldMaterial::Timber => {
            (material.albedo(), 1.0 - material.roughness(), surface_role::PLANK)
        }
        material @ (WorldMaterial::Straw | WorldMaterial::Bark | WorldMaterial::Canopy) => {
            (material.albedo(), 1.0 - material.roughness(), surface_role::LEGACY)
        }
        WorldMaterial::Wall => (palette.wall, 0.10, surface_role::PLASTER),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every material the world can bake decodes to something the shader knows how to dress —
    /// the match is exhaustive by construction, and this checks it stays sane rather than merely
    /// total (a role outside the table would render as an untreated fill).
    #[test]
    fn every_world_material_lands_on_a_known_surface_role() {
        let palette = Palette {
            wall: [0.5, 0.5, 0.5],
            roof: [0.3, 0.2, 0.2],
            roof_gloss: 0.2,
            stone: ([0.4, 0.4, 0.4], 0.15),
        };
        let known = [
            surface_role::LEGACY,
            surface_role::PLASTER,
            surface_role::PLANK,
            surface_role::SLATE,
            surface_role::DRESSED_STONE,
        ];
        for material in WorldMaterial::ALL {
            let (colour, gloss, role) = to_scene(material, palette);
            assert!(known.contains(&role), "{material:?} decoded to an unknown role {role}");
            assert!((0.0..=1.0).contains(&gloss), "{material:?} gloss {gloss}");
            assert!(colour.iter().all(|c| (0.0..=1.0).contains(c)), "{material:?} {colour:?}");
        }
    }
}
