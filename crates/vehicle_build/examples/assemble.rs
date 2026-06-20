//! Assemble the hybrid T-54 (CAD hull plates + SDF cast turret + revolved running gear) through the
//! description layer and render it at every LOD. Run: `cargo run -p vehicle_build --example assemble`.
//!
//! Writes one contact sheet per LOD (front / profile / top / battle-oblique) plus the per-angle LOD0
//! images, to `target/spike_sdf/`.

use vehicle_build::t54_description;
use vehicle_geometry::{BakedVehicle, GeometryMesh, LodLevel, SubmeshKind, reduce_vehicle};

/// The per-LOD asset: drop the parts the LOD policy excludes, then decimate within the tier.
fn lod_asset(lod: LodLevel) -> BakedVehicle {
    let built = t54_description().build_lod(lod);
    match lod {
        LodLevel::Lod0 => built,
        other => reduce_vehicle(&built, other),
    }
}

fn tris(v: &BakedVehicle) -> usize {
    v.submeshes().iter().map(|s| s.mesh.triangle_count()).sum()
}

fn main() {
    let out = std::path::Path::new("target/spike_sdf");
    std::fs::create_dir_all(out).expect("create out dir");

    // The four canonical review cameras: front, profile, top, battle-oblique.
    let cameras: [(f32, f32); 4] = [(0.0, -6.0), (90.0, -4.0), (0.0, -80.0), (35.0, -14.0)];

    for (lod, tag) in [(LodLevel::Lod0, "lod0"), (LodLevel::Lod1, "lod1"), (LodLevel::Lod2, "lod2")]
    {
        let baked = lod_asset(lod);
        println!("{tag}: {} tris", tris(&baked));
        let meshes: Vec<&GeometryMesh> = baked.submeshes().iter().map(|s| &s.mesh).collect();
        let panels: Vec<(&[&GeometryMesh], f32, f32)> =
            cameras.iter().map(|&(yaw, pitch)| (meshes.as_slice(), yaw, pitch)).collect();
        let sheet = sdf_mesh::render_contact_sheet(&panels, 2, 320, 220);
        std::fs::write(out.join(format!("t54_{tag}_sheet.png")), sheet).expect("write sheet");
    }

    // LOD0 per-angle images, by submesh, for close inspection.
    let baked = lod_asset(LodLevel::Lod0);
    let hull = &baked.submesh(SubmeshKind::Hull).expect("hull").mesh;
    let turret = &baked.submesh(SubmeshKind::Turret).expect("turret").mesh;
    let gun = &baked.submesh(SubmeshKind::Gun).expect("gun").mesh;
    let parts: &[&GeometryMesh] = &[hull, turret, gun];
    for (name, yaw, pitch) in [
        ("oblique", 35.0, -14.0),
        ("front", 0.0, -6.0),
        ("profile", 90.0, -4.0),
        ("top", 0.0, -80.0),
    ] {
        let png = sdf_mesh::render_by_material(parts, yaw, pitch, 360, 240);
        std::fs::write(out.join(format!("t54_hybrid_{name}.png")), png).expect("write png");
    }
    println!("wrote t54_lod0/1/2_sheet.png + t54_hybrid_*.png to {}", out.display());
}
