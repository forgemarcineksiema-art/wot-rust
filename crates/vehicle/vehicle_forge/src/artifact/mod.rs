mod asset;
mod bake_profile;
mod default_materials;
mod load;
mod manifest;
mod material_synthesis;
mod mesh_optimize;
mod mesh_payload;
mod obj;
mod review;
mod review_images;
mod review_raster;
mod review_text;
mod slug;
mod studio;
mod surface_bake;
mod texture_maps;

pub use asset::ForgeArtifact;
pub use bake_profile::BakeProfile;
pub use default_materials::{
    DEFAULT_MATERIAL_MAP_SIZE, DefaultMaterialFamily, DefaultMaterialMap, default_material_families,
};
pub use manifest::{ArtifactError, ForgeArtifactManifest, ForgeSubmeshManifest};
pub use obj::{ObjExport, export_obj};
pub use review::{ReviewCamera, ReviewCameraSet, ReviewCameraSpec};
pub use slug::forge_vehicle_slug;
pub use studio::{StudioBundle, bake_studio_bundle, bake_studio_bundle_from_blueprint};
pub use surface_bake::SurfaceBakeManifest;
pub use texture_maps::{ForgeTextureManifest, MaterialFamily};
