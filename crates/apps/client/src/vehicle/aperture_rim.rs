//! Exposed-steel meshes around analytical apertures. They add plate thickness and a broken
//! silhouette without cutting the shared vehicle mesh; the shader owns the open center.

use game_core::{
    ApertureLobe, ArmorBreachSet, ArmorFrame, ArmorMaterial, BreachFace, ShellType,
    armor_surface_basis,
};
use glam::Vec3;
use vehicle_geometry::{GeometryMesh, GeometryVertex, MaterialRole, SmoothingGroup};

const SEGMENTS: usize = 24;
/// How many tongues a kinetic tear folds back around the hole (Z6): photographs show a torn
/// plate breaking into many narrow curled strips, never four wide flaps.
const PETALS_MIN: usize = 8;
const PETALS_MAX: usize = 14;
pub(crate) const DAMAGE_BAKE_VERSION: u64 = 4;

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
    // Petaling (Fizyczny Świat P7, reshaped by Z6): a full-calibre KINETIC punch does not
    // drill — it TEARS, folding the plate back into narrow curled tongues around the hole.
    // A HEAT jet melts and an HE blast slaps, so neither petals. Deterministic from
    // the replicated fracture seed — every client bends the same steel the same way.
    if matches!(shell, ShellType::ArmorPiercing | ShellType::Apcr) {
        append_petals(vertices, indices, lobe, face, normal, u, v);
    }
}

fn append_petals(
    vertices: &mut Vec<GeometryVertex>,
    indices: &mut Vec<u32>,
    lobe: ApertureLobe,
    face: BreachFace,
    normal: Vec3,
    u: Vec3,
    v: Vec3,
) {
    let mut seed = lobe.fracture_seed.rotate_left(41) | 1;
    let mut next = move || {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        (seed >> 40) as f32 / ((1u64 << 24) - 1) as f32
    };
    // Steel tears along MANY short lines (Z6): eight to fourteen narrow tongues around the
    // hole, not four wide flaps. Each tongue is a three-station strip that narrows toward a
    // torn tip and CURLS — the lift grows with the square of the distance along it, so the
    // root lies on the plate and the tip stands off it. Exit tongues flare hard with the shell;
    // entry tongues fold back shallower.
    let count = PETALS_MIN + (next() * (PETALS_MAX - PETALS_MIN + 1) as f32 * 0.999) as usize;
    let flare = if face == BreachFace::Egress { 0.85 } else { 0.55 };
    let reach = lobe.outer.major_radius_m.max(lobe.outer.minor_radius_m);
    const STATIONS: usize = 3;
    // The tear lane the shader reads: a scorched root, dark section, a bright torn tip.
    const SHADES: [f32; STATIONS + 1] = [0.30, 0.55, 0.78, 0.97];
    for petal in 0..count {
        let angle = petal as f32 / count as f32 * std::f32::consts::TAU
            + next() * 0.35
            + lobe.outer.rotation_rad;
        let half_width = 0.10 + next() * 0.09;
        let root_a2 = lobe.outer.point_at(angle - half_width, lobe.fracture_seed);
        let root_b2 = lobe.outer.point_at(angle + half_width, lobe.fracture_seed);
        let root_a = lobe.entry_local + u * root_a2.x + v * root_a2.y;
        let root_b = lobe.entry_local + u * root_b2.x + v * root_b2.y;
        let radial = (u * angle.cos() + v * angle.sin()).normalize_or_zero();
        let length = reach * (0.22 + next() * 0.22);
        let curl = flare * (0.7 + next() * 0.6);
        let mut stations: Vec<(Vec3, Vec3)> = Vec::with_capacity(STATIONS + 1);
        stations.push((root_a, root_b));
        let root_mid = (root_a + root_b) * 0.5;
        let half = (root_b - root_a) * 0.5;
        for station in 1..=STATIONS {
            let t = station as f32 / STATIONS as f32;
            let width = 1.0 - 0.55 * t;
            let along = radial * (length * t * (1.0 - curl * 0.35 * t));
            let lift = normal * (length * curl * t * t * 2.0);
            let center = root_mid + along + lift;
            stations.push((center - half * width, center + half * width));
        }
        // Per-station normals follow the curl; the underside is the same strip turned over.
        let mut station_normals = Vec::with_capacity(STATIONS + 1);
        for station in 0..=STATIONS {
            let (a, b) = stations[station];
            let (na, nb) =
                if station < STATIONS { stations[station + 1] } else { stations[station - 1] };
            let dir = if station < STATIONS {
                (na + nb) * 0.5 - (a + b) * 0.5
            } else {
                (a + b) * 0.5 - (na + nb) * 0.5
            };
            station_normals.push((b - a).cross(dir).normalize_or(normal));
        }
        for side in [1.0_f32, -1.0] {
            let base = vertices.len() as u32;
            for station in 0..=STATIONS {
                let (a, b) = stations[station];
                let n = station_normals[station] * side;
                let shade = SHADES[station] * if side > 0.0 { 1.0 } else { 0.8 };
                vertices.push(vertex(a, n, MaterialRole::ExposedSteel, shade));
                vertices.push(vertex(b, n, MaterialRole::ExposedSteel, shade));
            }
            for station in 0..STATIONS as u32 {
                let a0 = base + station * 2;
                let b0 = a0 + 1;
                let a1 = a0 + 2;
                let b1 = a0 + 3;
                if side > 0.0 {
                    indices.extend_from_slice(&[a0, b0, b1, a0, b1, a1]);
                } else {
                    indices.extend_from_slice(&[a0, b1, b0, a0, a1, b1]);
                }
            }
        }
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
        // The ring plus 8..=14 kinetic tongues of three two-sided quads each (Z6).
        assert!(
            (SEGMENTS * 6 + PETALS_MIN * 12..=SEGMENTS * 6 + PETALS_MAX * 12)
                .contains(&mesh.triangle_count()),
            "ring + petals, got {} triangles",
            mesh.triangle_count()
        );
        assert!(mesh.vertices().iter().any(|vertex| vertex.position.z < -0.09));
        assert!(mesh.vertices().iter().any(|vertex| vertex.material == MaterialRole::RolledArmor));
        assert!(mesh.vertices().iter().any(|vertex| vertex.material == MaterialRole::ExposedSteel));
        assert!(mesh.indices().iter().all(|index| *index < mesh.vertex_count() as u32));
    }

    /// Fizyczny Świat P7: only a KINETIC punch tears petals — a HEAT jet melts a clean-edged
    /// hole and gets the bare ring, and every petal reaches past the contour (bent steel has
    /// length, it is not a sticker on the rim).
    #[test]
    fn only_kinetic_holes_grow_petals_and_they_stand_off_the_plate() {
        let lobe = ApertureLobe {
            entry_local: Vec3::ZERO,
            exit_local: Vec3::new(0.0, 0.0, -0.1),
            entry_normal_local: Vec3::Z,
            exit_normal_local: Vec3::NEG_Z,
            direction_local: Vec3::NEG_Z,
            thickness_m: 0.1,
            outer: BreachContour::new(0.06, 0.05, 0.2, 0.1),
            inner: BreachContour::new(0.09, 0.075, 0.3, 0.14),
            fracture_seed: 99,
        };
        let build = |shell: ShellType| {
            let mut set = ArmorBreachSet::default();
            set.add(ArmorBreach::new(
                game_core::ArmorBreachDescriptor {
                    breach_id: 9,
                    surface: ArmorSurfaceId::new(ArmorFrame::Hull, ArmorZone::HullSide),
                    frame: ArmorFrame::Hull,
                    zone: ArmorZone::HullSide,
                    material: ArmorMaterial::RolledSteel,
                    face: BreachFace::Ingress,
                    shell_type: shell,
                    created_tick: 1,
                    impact_angle_degrees: 0.0,
                    impact_energy_kj: 500.0,
                    projectile_diameter_m: 0.1,
                    residual_penetration_mm: 50.0,
                },
                lobe,
            ));
            build_rim_mesh(&set, ArmorFrame::Hull).expect("rim")
        };

        let kinetic = build(ShellType::ArmorPiercing);
        let heat = build(ShellType::Heat);
        assert_eq!(heat.triangle_count(), SEGMENTS * 6, "a HEAT melt has no petals");
        assert!(kinetic.triangle_count() > heat.triangle_count(), "the AP tear grows petals");
        // Tongues stand OFF the plate: some vertex rises clearly above the lip's ~9 mm raise.
        let peak = kinetic.vertices().iter().map(|v| v.position.z).fold(f32::MIN, f32::max);
        assert!(peak > 0.014, "bent steel has height: peak z {peak}");
        // Determinism: the same seed bends the same steel.
        let twin = build(ShellType::ArmorPiercing);
        assert_eq!(kinetic.vertices().len(), twin.vertices().len());
    }

    /// Z6: a tongue is torn steel — its root scorched (a low tear lane), its tip BRIGHT bare
    /// metal (a high lane), and it CURLS: the tip stands further off the plate than the middle
    /// station, by more than the linear share.
    #[test]
    fn tongues_curl_and_end_in_bright_torn_steel() {
        let lobe = ApertureLobe {
            entry_local: Vec3::ZERO,
            exit_local: Vec3::new(0.0, 0.0, -0.1),
            entry_normal_local: Vec3::Z,
            exit_normal_local: Vec3::NEG_Z,
            direction_local: Vec3::NEG_Z,
            thickness_m: 0.1,
            outer: BreachContour::new(0.06, 0.05, 0.2, 0.1),
            inner: BreachContour::new(0.09, 0.075, 0.3, 0.14),
            fracture_seed: 4242,
        };
        let mut set = ArmorBreachSet::default();
        set.add(ArmorBreach::new(
            game_core::ArmorBreachDescriptor {
                breach_id: 3,
                surface: ArmorSurfaceId::new(ArmorFrame::Hull, ArmorZone::HullSide),
                frame: ArmorFrame::Hull,
                zone: ArmorZone::HullSide,
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
        let steel: Vec<_> =
            mesh.vertices().iter().filter(|v| v.material == MaterialRole::ExposedSteel).collect();
        let tips: Vec<_> = steel.iter().filter(|v| v.surface_shade >= 0.95).collect();
        let roots: Vec<_> = steel.iter().filter(|v| v.surface_shade <= 0.31).collect();
        let mids: Vec<_> = steel.iter().filter(|v| (v.surface_shade - 0.55).abs() < 0.02).collect();
        assert!(tips.len() >= PETALS_MIN * 2, "every tongue ends in bright torn steel");
        assert!(roots.len() >= PETALS_MIN * 2, "every tongue roots in scorched steel");
        let peak =
            |set: &[&&GeometryVertex]| set.iter().map(|v| v.position.z).fold(f32::MIN, f32::max);
        assert!(
            peak(&tips) > peak(&mids) * 2.0,
            "the tongue curls: tip {} vs middle {}",
            peak(&tips),
            peak(&mids)
        );
    }
}
