//! Assemble the hybrid T-54 (CAD hull plates + SDF cast turret) through the description layer and
//! render it. Run: `cargo run -p vehicle_build --example assemble`.

use sdf_mesh::{render_by_material, render_contact_sheet};
use vehicle_build::t54_description;
use vehicle_geometry::{GeometryMesh, LodLevel, SubmeshKind, reduce_vehicle};

fn main() {
    let baked = t54_description().build();
    let mut total = 0;
    for submesh in baked.submeshes() {
        total += submesh.mesh.triangle_count();
        println!("{:?}: {} tris", submesh.kind, submesh.mesh.triangle_count());
    }
    println!("hybrid T-54: {total} tris (LOD0 target ~8000-12000)");

    let out = std::path::Path::new("target/spike_sdf");
    std::fs::create_dir_all(out).expect("create out dir");

    // The hybrid BakedVehicle flows straight through the existing LOD pipeline — render each tier so
    // the silhouette can be checked as it decimates.
    for lod in [LodLevel::Lod1, LodLevel::Lod2] {
        let reduced = reduce_vehicle(&baked, lod);
        let t: usize = reduced.submeshes().iter().map(|s| s.mesh.triangle_count()).sum();
        println!("  {lod:?}: {t} tris");
        let meshes: Vec<&GeometryMesh> = reduced.submeshes().iter().map(|s| &s.mesh).collect();
        let png = render_by_material(&meshes, 35.0, -14.0, 360, 240);
        std::fs::write(out.join(format!("t54_{lod:?}_oblique.png")), png).expect("write lod png");
    }

    let hull = &baked.submesh(SubmeshKind::Hull).expect("hull").mesh;
    let turret = &baked.submesh(SubmeshKind::Turret).expect("turret").mesh;
    let gun = &baked.submesh(SubmeshKind::Gun).expect("gun").mesh;
    let parts: &[&_] = &[hull, turret, gun];

    for (name, yaw, pitch) in
        [("oblique", 35.0, -14.0), ("front", 0.0, -6.0), ("profile", 90.0, -4.0)]
    {
        let png = render_by_material(parts, yaw, pitch, 360, 240);
        std::fs::write(out.join(format!("t54_hybrid_{name}.png")), png).expect("write png");
    }

    // One review contact sheet: four LOD0 angles plus the LOD2 oblique, so the silhouette and the
    // decimation can be checked at a glance.
    let lod2 = reduce_vehicle(&baked, LodLevel::Lod2);
    let lod2_meshes: Vec<&GeometryMesh> = lod2.submeshes().iter().map(|s| &s.mesh).collect();
    let panels: &[(&[&GeometryMesh], f32, f32)] = &[
        (parts, 35.0, -14.0),
        (parts, 90.0, -4.0),
        (parts, 0.0, -6.0),
        (parts, 0.0, -80.0),
        (lod2_meshes.as_slice(), 35.0, -14.0),
    ];
    std::fs::write(out.join("t54_contact_sheet.png"), render_contact_sheet(panels, 3, 300, 200))
        .expect("write contact sheet");
    println!("wrote t54_hybrid_*.png + t54_contact_sheet.png to {}", out.display());
}
