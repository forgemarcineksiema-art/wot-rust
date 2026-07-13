use game_core::math::world_to_tank_local;
use game_core::{
    ApertureLobe, ArmorBreach, ArmorBreachDescriptor, ArmorFrame, ArmorMaterial, ArmorSurfaceId,
    ArmorZone, BreachContour, BreachFace, ShellId, ShellType, segment_volume_interval_with_margin,
    vehicle_armor_volumes,
};
use glam::{Mat3, Vec3};

use crate::TankState;

pub(crate) fn frame_for_zone(zone: ArmorZone) -> ArmorFrame {
    match zone {
        ArmorZone::Mantlet => ArmorFrame::Mantlet,
        ArmorZone::TurretFront
        | ArmorZone::TurretSide
        | ArmorZone::TurretRear
        | ArmorZone::Roof => ArmorFrame::Turret,
        _ => ArmorFrame::Hull,
    }
}

pub(crate) fn world_to_breach_frame(world: Vec3, tank: &TankState, frame: ArmorFrame) -> Vec3 {
    let mut local =
        world_to_tank_local(world, tank.position, tank.spec.hitbox.center_y_m, tank.hull_pose());
    local += Vec3::Y * tank.spec.hitbox.center_y_m;
    if matches!(frame, ArmorFrame::Turret | ArmorFrame::Mantlet) {
        let pivot = tank.spec.mounts.turret_ring.translation;
        local = pivot + Mat3::from_rotation_y(-tank.turret_yaw_rad) * (local - pivot);
    }
    if frame == ArmorFrame::Mantlet {
        let pivot = tank.spec.mounts.gun_trunnion.translation;
        local = pivot + Mat3::from_rotation_x(-tank.gun_pitch_rad) * (local - pivot);
    }
    local
}

pub(crate) fn vector_to_breach_frame(world: Vec3, tank: &TankState, frame: ArmorFrame) -> Vec3 {
    let mut local = tank.hull_pose().basis().transpose() * world;
    if matches!(frame, ArmorFrame::Turret | ArmorFrame::Mantlet) {
        local = Mat3::from_rotation_y(-tank.turret_yaw_rad) * local;
    }
    if frame == ArmorFrame::Mantlet {
        local = Mat3::from_rotation_x(-tank.gun_pitch_rad) * local;
    }
    local.normalize_or_zero()
}

pub(crate) struct BreachImpact {
    pub zone: ArmorZone,
    pub shell_id: ShellId,
    pub shell_type: ShellType,
    pub created_tick: u64,
    pub hit_position: Vec3,
    pub plate_normal: Vec3,
    pub direction: Vec3,
    pub caliber_mm: f32,
    pub impact_angle_degrees: f32,
    pub impact_speed_mps: f32,
    pub effective_armor_mm: f32,
    pub residual_penetration_mm: f32,
}

pub(crate) fn make_breach(tank: &TankState, impact: BreachImpact) -> ArmorBreach {
    let frame = frame_for_zone(impact.zone);
    let surface = ArmorSurfaceId::new(frame, impact.zone);
    let entry_local = world_to_breach_frame(impact.hit_position, tank, frame);
    let direction_local = vector_to_breach_frame(impact.direction, tank, frame);
    let normal_local = vector_to_breach_frame(impact.plate_normal, tank, frame);
    let thickness_m = (impact.effective_armor_mm / 1000.0).clamp(0.01, 0.40);
    let projectile_diameter_m = impact.caliber_mm * 0.001;
    let core_scale = match impact.shell_type {
        ShellType::ArmorPiercing => 1.0,
        ShellType::Apcr => 0.55,
        ShellType::Heat => 0.22,
        ShellType::HighExplosive => 1.25,
    };
    let base_radius = (projectile_diameter_m * 0.5 * core_scale).clamp(0.008, 0.19);
    let obliquity = impact.impact_angle_degrees.to_radians().cos().abs().max(0.38);
    let seed = game_core::math::splitmix64(impact.shell_id.0 ^ u64::from(surface.0));
    let rotation = ((seed >> 40) as f32 / ((1_u64 << 24) as f32)) * std::f32::consts::TAU;
    let outer = BreachContour::new(
        (base_radius / obliquity).min(0.38),
        base_radius,
        rotation,
        shell_irregularity(impact.shell_type),
    );
    let inner_scale = inner_scale(impact.shell_type);
    let lobe = ApertureLobe {
        entry_local,
        exit_local: entry_local + direction_local * thickness_m,
        entry_normal_local: normal_local,
        exit_normal_local: -normal_local,
        direction_local,
        thickness_m,
        outer,
        inner: BreachContour::new(
            (outer.major_radius_m * inner_scale).min(0.42),
            (outer.minor_radius_m * inner_scale).min(0.42),
            rotation + 0.17,
            (outer.irregularity + 0.04).min(0.22),
        ),
        fracture_seed: seed,
    };
    ArmorBreach::new(
        ArmorBreachDescriptor {
            breach_id: impact.shell_id.0,
            surface,
            frame,
            zone: impact.zone,
            material: material_for_frame(frame),
            face: BreachFace::Ingress,
            shell_type: impact.shell_type,
            created_tick: impact.created_tick,
            impact_angle_degrees: impact.impact_angle_degrees,
            impact_energy_kj: estimated_impact_energy_kj(
                impact.shell_type,
                projectile_diameter_m,
                impact.impact_speed_mps,
            ),
            projectile_diameter_m,
            residual_penetration_mm: impact.residual_penetration_mm,
        },
        lobe,
    )
}

pub(crate) fn admits_existing_channel(
    tank: &TankState,
    zone: ArmorZone,
    hit_position: Vec3,
    projectile_radius_m: f32,
) -> bool {
    let frame = frame_for_zone(zone);
    let local = world_to_breach_frame(hit_position, tank, frame);
    tank.armor_breaches.passage_at(frame, local, projectile_radius_m).is_some()
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct EgressContact {
    pub position: Vec3,
    pub normal: Vec3,
    pub zone: ArmorZone,
}

/// Find the opposite physical plate of the same convex armor body entered at `hit_position`.
pub(crate) fn find_egress_contact(
    tank: &TankState,
    entry_zone: ArmorZone,
    hit_position: Vec3,
    direction: Vec3,
) -> Option<EgressContact> {
    let volumes = vehicle_armor_volumes(tank.spec.kind)?;
    let direction = direction.normalize_or_zero();
    if direction == Vec3::ZERO {
        return None;
    }
    let world_start = hit_position - direction * 0.08;
    let world_end = hit_position + direction * 8.0;
    let hull_start = world_to_tank_local(
        world_start,
        tank.position,
        tank.spec.hitbox.center_y_m,
        tank.hull_pose(),
    );
    let hull_end = world_to_tank_local(
        world_end,
        tank.position,
        tank.spec.hitbox.center_y_m,
        tank.hull_pose(),
    );
    let mut best: Option<(f32, Vec3, Vec3, ArmorZone)> = None;
    let hit_t = 0.08 / 8.08;
    if frame_for_zone(entry_zone) == ArmorFrame::Hull {
        for volume in &volumes.hull {
            let Some(interval) =
                segment_volume_interval_with_margin(hull_start, hull_end, volume, 0.0)
            else {
                continue;
            };
            if interval.enter_t > hit_t + 0.03 || interval.exit_t <= hit_t + 1.0e-4 {
                continue;
            }
            let exit_point = hull_start.lerp(hull_end, interval.exit_t);
            let exit_plane = &volume.planes[interval.exit_plane];
            select_egress(
                &mut best,
                interval.exit_t,
                exit_point,
                exit_plane.normal,
                exit_plane.zone_at(exit_point),
            );
        }
    }
    let pivot = Vec3::new(0.0, 0.0, volumes.turret_ring_z);
    let to_turret = Mat3::from_rotation_y(-tank.turret_yaw_rad);
    let turret_start = pivot + to_turret * (hull_start - pivot);
    let turret_end = pivot + to_turret * (hull_end - pivot);
    if frame_for_zone(entry_zone) != ArmorFrame::Hull
        && let Some(interval) =
            segment_volume_interval_with_margin(turret_start, turret_end, &volumes.turret, 0.0)
        && interval.enter_t <= hit_t + 0.03
        && interval.exit_t > hit_t + 1.0e-4
    {
        let exit_turret = turret_start.lerp(turret_end, interval.exit_t);
        let exit_plane = &volumes.turret.planes[interval.exit_plane];
        let from_turret = Mat3::from_rotation_y(tank.turret_yaw_rad);
        let exit_hull = pivot + from_turret * (exit_turret - pivot);
        let normal_hull = from_turret * exit_plane.normal;
        select_egress(
            &mut best,
            interval.exit_t,
            exit_hull,
            normal_hull,
            exit_plane.zone_at(exit_turret),
        );
    }
    let (_, exit_hull, normal_hull, zone) = best?;
    let basis = tank.hull_pose().basis();
    let position = tank.position + basis * (exit_hull + Vec3::Y * tank.spec.hitbox.center_y_m);
    Some(EgressContact { position, normal: (basis * normal_hull).normalize_or_zero(), zone })
}

pub(crate) fn make_egress_breach(tank: &TankState, mut impact: BreachImpact) -> ArmorBreach {
    // The exterior aperture's tunnel points back into the tank, opposite the shell's travel.
    impact.direction = -impact.direction;
    let mut breach = make_breach(tank, impact);
    breach.reshape_as_egress();
    breach
}

fn select_egress(
    best: &mut Option<(f32, Vec3, Vec3, ArmorZone)>,
    exit_t: f32,
    exit_point: Vec3,
    exit_normal: Vec3,
    exit_zone: ArmorZone,
) {
    if best.as_ref().is_none_or(|candidate| exit_t < candidate.0) {
        *best = Some((exit_t, exit_point, exit_normal, exit_zone));
    }
}

fn material_for_frame(frame: ArmorFrame) -> ArmorMaterial {
    match frame {
        ArmorFrame::Hull => ArmorMaterial::RolledSteel,
        ArmorFrame::Turret | ArmorFrame::Mantlet => ArmorMaterial::CastSteel,
    }
}

fn shell_irregularity(shell_type: ShellType) -> f32 {
    match shell_type {
        ShellType::Heat => 0.05,
        ShellType::Apcr => 0.08,
        ShellType::ArmorPiercing => 0.12,
        ShellType::HighExplosive => 0.18,
    }
}

fn inner_scale(shell_type: ShellType) -> f32 {
    match shell_type {
        ShellType::ArmorPiercing => 1.55,
        ShellType::Apcr => 1.28,
        ShellType::Heat => 1.18,
        ShellType::HighExplosive => 1.75,
    }
}

fn estimated_impact_energy_kj(shell_type: ShellType, caliber_m: f32, speed_mps: f32) -> f32 {
    let mass_scale = match shell_type {
        ShellType::ArmorPiercing => 0.65,
        ShellType::Apcr => 0.30,
        ShellType::Heat | ShellType::HighExplosive => 0.45,
    };
    let diameter = caliber_m.max(0.01);
    let estimated_mass_kg =
        std::f32::consts::PI * (diameter * 0.5).powi(2) * (diameter * 4.0) * 7_850.0 * mass_scale;
    estimated_mass_kg * speed_mps.powi(2) * 0.0005
}
