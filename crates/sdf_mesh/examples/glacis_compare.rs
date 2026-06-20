//! Side-by-side sharpness comparison of the T-54 glacis: SDF + Surface Nets vs exact convex CAD.
//! Renders both from identical cameras. Run: `cargo run -p sdf_mesh --example glacis_compare`.

use game_core::{VehicleBlueprint, VehicleKind};
use sdf_mesh::{mesh_within_budget, render_png, t54_glacis};
use solid::t54_glacis_solid;
use vehicle_geometry::{MaterialRole, SmoothingGroup};

fn main() {
    let bp = VehicleBlueprint::for_vehicle(VehicleKind::T54_1951).expect("T-54 blueprint");
    let hybrid = bp.hybrid().expect("T-54 hybrid visual");
    let (sdf_glacis, gmin, gmax) = t54_glacis(bp.armor.hull_front.0);
    let (sdf_mesh, grid) = mesh_within_budget(
        &sdf_glacis,
        gmin,
        gmax,
        4_000,
        MaterialRole::RolledArmor,
        SmoothingGroup::hard_edges(),
    );
    let cad_mesh = t54_glacis_solid(&hybrid.hull, bp.armor.hull_front.0)
        .to_mesh(MaterialRole::RolledArmor, SmoothingGroup::hard_edges());

    println!("SDF glacis: {} tris (cell {:.3} m)", sdf_mesh.triangle_count(), grid.cell_size());
    println!("CAD glacis: {} tris (exact convex)", cad_mesh.triangle_count());

    let out = std::path::Path::new("target/spike_sdf");
    std::fs::create_dir_all(out).expect("create out dir");
    let rolled = [108u8, 116, 92];
    for (suffix, yaw, pitch) in [("top", 22.0, -10.0), ("grazing", -35.0, -4.0)] {
        let sdf_png = render_png(&[(&sdf_mesh, rolled)], yaw, pitch, 360, 240);
        std::fs::write(out.join(format!("glacis_sdf_{suffix}.png")), sdf_png).expect("write sdf");
        let cad_png = render_png(&[(&cad_mesh, rolled)], yaw, pitch, 360, 240);
        std::fs::write(out.join(format!("glacis_cad_{suffix}.png")), cad_png).expect("write cad");
    }
    println!("wrote glacis_sdf_*.png and glacis_cad_*.png to {}", out.display());
}
