//! Static scene builders (client cut #2, W2a/B4): everything that turns a `BattlefieldMap`
//! or the hangar into renderer-ready meshes — terrain, cover, foliage, grass, water, backdrop,
//! weather looks. Renderer-free by construction (depends on `renderer_api` types only), so the
//! world's LOOK is testable without a GPU and reusable by every offscreen example.

pub mod backdrop;
pub mod battlefield;
pub mod clutter;
pub mod foliage;
pub mod grass;
pub mod grass_cards;
pub mod hangar;
pub(crate) mod hangar_bake;
pub(crate) mod hangar_gallery;
pub(crate) mod hangar_props;
pub mod review_views;
pub mod tank_mesh;
pub mod terrain_maps;
pub mod tree_lod;
pub mod water;
pub mod weather;
pub mod weather_timeline;
pub(crate) mod world_material;
