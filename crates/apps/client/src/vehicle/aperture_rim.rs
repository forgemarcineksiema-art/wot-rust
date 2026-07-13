//! Exposed-steel meshes around analytical apertures. They add plate thickness and a broken
//! silhouette without cutting the shared vehicle mesh; the shader owns the open center.

use game_core::{
    ApertureLobe, ArmorBreachSet, ArmorFrame, ArmorMaterial, BreachFace, ShellType,
    armor_surface_basis,
};
use glam::Vec3;
use vehicle_geometry::{GeometryMesh, GeometryVertex, MaterialRole, SmoothingGroup};

const SEGMENTS: usize = 24;
pub(crate) const DAMAGE_BAKE_VERSION: u64 = 2;

pub(crate) fn build_rim_mesh(breaches: &ArmorBreachSet, frame: ArmorFrame) -> Option<GeometryMesh> {
    let mut vertices = Vec::new();
    let mut indices = Vec::new();
    for breach in breaches.breaches().iter().filter(|breach| breach.frame == frame) {
        for lobe in breach.lobes() {
            append_lobe(
                &mut vertices,
                &mut indices,
                *lobe,
                breach.material,
                breach.shell_type,
                breach.face,
            );
        }
    }
    (!indices.is_empty()).then(|| GeometryMesh::new(vertices, indices))
}

pub(crate) fn frame_hash(breaches: &ArmorBreachSet, frame: ArmorFrame) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    hash_word(&mut hash, DAMAGE_BAKE_VERSION);
    for breach in breaches.breaches().iter().filter(|breach| breach.frame == frame) {
        hash_word(&mut hash, breach.breach_id);
        hash_word(&mut hash, breach.material as u64);
        hash_word(&mut hash, breach.shell_type as u64);
        hash_word(&mut hash, breach.face as u64);
        for lobe in breach.lobes() {
            for bits in [
                lobe.entry_local.x.to_bits(),
                lobe.entry_local.y.to_bits(),
                lobe.entry_local.z.to_bits(),
                lobe.exit_local.x.to_bits(),
                lobe.exit_local.y.to_bits(),
                lobe.exit_local.z.to_bits(),
                lobe.entry_normal_local.x.to_bits(),
                lobe.entry_normal_local.y.to_bits(),
                lobe.entry_normal_local.z.to_bits(),
                lobe.exit_normal_local.x.to_bits(),
                lobe.exit_normal_local.y.to_bits(),
                lobe.exit_normal_local.z.to_bits(),
                lobe.direction_local.x.to_bits(),
                lobe.direction_local.y.to_bits(),
                lobe.direction_local.z.to_bits(),
                lobe.outer.major_radius_m.to_bits(),
                lobe.outer.minor_radius_m.to_bits(),
                lobe.outer.rotation_rad.to_bits(),
                lobe.outer.irregularity.to_bits(),
                lobe.inner.major_radius_m.to_bits(),
                lobe.inner.minor_radius_m.to_bits(),
                lobe.inner.rotation_rad.to_bits(),
                lobe.inner.irregularity.to_bits(),
                lobe.thickness_m.to_bits(),
            ] {
                hash_word(&mut hash, u64::from(bits));
            }
            hash_word(&mut hash, lobe.fracture_seed);
        }
    }
    hash
}

fn append_lobe(
    vertices: &mut Vec<GeometryVertex>,
    indices: &mut Vec<u32>,
    lobe: ApertureLobe,
    armor: ArmorMaterial,
    shell: ShellType,
    face: BreachFace,
) {
    let normal = lobe.entry_normal_local.normalize_or_zero();
    let (u, v) = armor_surface_basis(normal, lobe.direction_local);
    if normal == Vec3::ZERO || u == Vec3::ZERO || v == Vec3::ZERO {
        return;
    }
    let paint = match armor {
        ArmorMaterial::RolledSteel => MaterialRole::RolledArmor,
        ArmorMaterial::CastSteel => MaterialRole::CastArmor,
    };
    let (lip_scale, lip_raise) = fracture_profile(shell, face);
    let base = vertices.len() as u32;
    for segment in 0..SEGMENTS {
        let angle = segment as f32 / SEGMENTS as f32 * std::f32::consts::TAU;
        let outer_2d = lobe.outer.point_at(angle, lobe.fracture_seed);
        let inner_2d = lobe.inner.point_at(angle, lobe.fracture_seed.rotate_left(17));
        let edge = lobe.entry_local + u * outer_2d.x + v * outer_2d.y;
        let lip = lobe.entry_local
            + u * outer_2d.x * lip_scale
            + v * outer_2d.y * lip_scale
            + normal * lip_raise * fracture_wave(angle, lobe.fracture_seed);
        let tunnel = lobe.exit_local + u * inner_2d.x + v * inner_2d.y;
        let mid = edge.lerp(tunnel, 0.42)
            + normal
                * lip_raise
                * 0.35
                * fracture_wave(angle + 1.7, lobe.fracture_seed.rotate_left(11));
        let radial = (u * outer_2d.x + v * outer_2d.y).normalize_or_zero();
        vertices.push(vertex(lip, normal, paint, 0.90));
        vertices.push(vertex(
            edge,
            (normal - radial).normalize_or_zero(),
            MaterialRole::ExposedSteel,
            0.78,
        ));
        vertices.push(vertex(mid, -radial, MaterialRole::ExposedSteel, 0.64));
        vertices.push(vertex(tunnel, -radial, MaterialRole::ExposedSteel, 0.54));
    }
    for segment in 0..SEGMENTS as u32 {
        let next = (segment + 1) % SEGMENTS as u32;
        let lip_a = base + segment * 4;
        let edge_a = lip_a + 1;
        let mid_a = lip_a + 2;
        let tunnel_a = lip_a + 3;
        let lip_b = base + next * 4;
        let edge_b = lip_b + 1;
        let mid_b = lip_b + 2;
        let tunnel_b = lip_b + 3;
        indices.extend_from_slice(&[lip_a, edge_a, edge_b, lip_a, edge_b, lip_b]);
        indices.extend_from_slice(&[edge_a, mid_a, mid_b, edge_a, mid_b, edge_b]);
        indices.extend_from_slice(&[mid_a, tunnel_a, tunnel_b, mid_a, tunnel_b, mid_b]);
    }
}

fn fracture_profile(shell: ShellType, face: BreachFace) -> (f32, f32) {
    let profile = match shell {
        ShellType::ArmorPiercing => (1.09, 0.009),
        ShellType::Apcr => (1.05, 0.005),
        ShellType::Heat => (1.04, 0.004),
        ShellType::HighExplosive => (1.18, 0.018),
    };
    if face == BreachFace::Egress { (profile.0 + 0.08, profile.1 * 1.7) } else { profile }
}

fn fracture_wave(angle: f32, seed: u64) -> f32 {
    let phase = (seed as u32 as f32) * (1.0 / u32::MAX as f32) * std::f32::consts::TAU;
    0.72 + 0.28 * (angle * 4.0 + phase).sin()
}

fn vertex(position: Vec3, normal: Vec3, material: MaterialRole, shade: f32) -> GeometryVertex {
    GeometryVertex::new(position, normal, material, SmoothingGroup::hard_edges())
        .with_surface_shade(shade)
}

fn hash_word(hash: &mut u64, word: u64) {
    *hash ^= word;
    *hash = hash.wrapping_mul(0x100_0000_01b3);
}

#[cfg(test)]
mod tests {
    use game_core::{
        ArmorBreach, ArmorMaterial, ArmorSurfaceId, ArmorZone, BreachContour, BreachFace, ShellType,
    };

    use super::*;

    #[test]
    fn rim_has_an_open_center_and_real_plate_wall() {
        let lobe = ApertureLobe {
            entry_local: Vec3::ZERO,
            exit_local: Vec3::new(0.0, 0.0, -0.1),
            entry_normal_local: Vec3::Z,
            exit_normal_local: Vec3::NEG_Z,
            direction_local: Vec3::NEG_Z,
            thickness_m: 0.1,
            outer: BreachContour::new(0.06, 0.05, 0.2, 0.1),
            inner: BreachContour::new(0.09, 0.075, 0.3, 0.14),
            fracture_seed: 7,
        };
        let mut set = ArmorBreachSet::default();
        set.add(ArmorBreach::new(
            game_core::ArmorBreachDescriptor {
                breach_id: 7,
                surface: ArmorSurfaceId::new(ArmorFrame::Hull, ArmorZone::UpperGlacis),
                frame: ArmorFrame::Hull,
                zone: ArmorZone::UpperGlacis,
                material: ArmorMaterial::RolledSteel,
                face: BreachFace::Ingress,
                shell_type: ShellType::ArmorPiercing,
                created_tick: 1,
                impact_angle_degrees: 0.0,
                impact_energy_kj: 500.0,
                projectile_diameter_m: 0.1,
                residual_penetration_mm: 50.0,
            },
            lobe,
        ));
        let mesh = build_rim_mesh(&set, ArmorFrame::Hull).expect("rim");
        assert_eq!(mesh.triangle_count(), SEGMENTS * 6);
        assert!(mesh.vertices().iter().any(|vertex| vertex.position.z < -0.09));
        assert!(mesh.vertices().iter().any(|vertex| vertex.material == MaterialRole::RolledArmor));
        assert!(mesh.vertices().iter().any(|vertex| vertex.material == MaterialRole::ExposedSteel));
        assert!(mesh.indices().iter().all(|index| *index < mesh.vertex_count() as u32));
    }
}
