//! Bake the T-54 turret + glacis from SDF, mesh them with Surface Nets *to a triangle budget*, and
//! write comparison PNGs to `target/spike_sdf/`. Run: `cargo run -p sdf_mesh --example spike`.

use sdf_mesh::{glacis_slope_deg, mesh_within_budget, render_png, t54_glacis, t54_turret};
use vehicle_geometry::{MaterialRole, SmoothingGroup};

const TURRET_BUDGET: usize = 9_000;
const GLACIS_BUDGET: usize = 4_000;

fn main() {
    let (turret, tmin, tmax) = t54_turret();
    let (turret_mesh, tgrid) = mesh_within_budget(
        &turret,
        tmin,
        tmax,
        TURRET_BUDGET,
        MaterialRole::CastArmor,
        SmoothingGroup(2),
    );

    let (glacis, gmin, gmax) = t54_glacis();
    let (glacis_mesh, ggrid) = mesh_within_budget(
        &glacis,
        gmin,
        gmax,
        GLACIS_BUDGET,
        MaterialRole::RolledArmor,
        SmoothingGroup::hard_edges(),
    );

    println!("cell size: turret {:.3} m, glacis {:.3} m", tgrid.cell_size(), ggrid.cell_size());
    println!("turret: {} tris (budget {TURRET_BUDGET})", turret_mesh.triangle_count());
    println!("glacis: {} tris (budget {GLACIS_BUDGET})", glacis_mesh.triangle_count());
    let total = turret_mesh.triangle_count() + glacis_mesh.triangle_count();
    println!("combined: {total} tris (LOD0 target ~8000-12000)");
    println!("glacis plate slope carried exactly: {:.1} deg", glacis_slope_deg());

    let out = std::path::Path::new("target/spike_sdf");
    std::fs::create_dir_all(out).expect("create out dir");
    let cast = [118u8, 124, 102];
    let rolled = [108u8, 116, 92];
    let both: &[(&_, [u8; 3])] = &[(&turret_mesh, cast), (&glacis_mesh, rolled)];
    for (name, yaw, pitch) in
        [("oblique", 35.0, -14.0), ("front", 0.0, -6.0), ("profile", 90.0, -4.0)]
    {
        let png = render_png(both, yaw, pitch, 360, 240);
        std::fs::write(out.join(format!("tank_{name}.png")), png).expect("write png");
    }
    std::fs::write(
        out.join("turret.png"),
        render_png(&[(&turret_mesh, cast)], 35.0, -16.0, 360, 240),
    )
    .expect("write turret");
    std::fs::write(
        out.join("glacis.png"),
        render_png(&[(&glacis_mesh, rolled)], 22.0, -10.0, 360, 240),
    )
    .expect("write glacis");
    println!("wrote comparison images to {}", out.display());
}
