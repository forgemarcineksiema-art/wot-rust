//! Fleet-wide locks on component layouts.
//!
//! Every promise here used to be a hand-written test naming one vehicle, or a comment in
//! `t54.rs` recording that somebody had checked it by eye. Both are the same failure: the T-54
//! got audited repeatedly and thoroughly, and nothing carried a single one of those findings to
//! the tank parked next to it.

use game_core::{
    ArmorFrame, ArmorVolume, DamageComponent, DamageComponentKind, DamageLayout, DamageShape,
    HullEnvelope, VehicleArmorVolumes, VehicleKind, vehicle_armor_volumes,
};
use glam::Vec3;

/// A component id above this loses its bit and reports damage to nobody.
///
/// `sim::combat::component_bit` is `1 << (id - 1)` over a `u32`, with `checked_shl(..).
/// unwrap_or(0)` — so id 33 does not overflow loudly, it silently becomes the empty mask. The
/// fuel-fire check reads that mask (`damaged_components_mask & fuel_bits`), which means a
/// thirty-third component would be a fuel tank that can be hit and can never catch fire.
const MAX_COMPONENT_ID: u16 = 32;

/// Components this rule cannot judge, with the reason. Not a list of things to fix later.
const ARMOUR_CONTAINMENT_EXEMPT: &[(VehicleKind, u16)] = &[
    // The T-54's breech box reaches forward and up into the gun aperture, and its two top-front
    // corners land outside the CASTING at y 1.09. MEASURED 2026-08-02 before attempting the
    // "mantlet as a volume" fix, and the measurement killed that plan: the mantlet patch has
    // radius 0.20 while the breech corners sit 0.60 off the gun axis — no volume built from the
    // blueprint's existing numbers covers them. The real causes are two, and both are geometry
    // authoring rather than mechanics: the armour volume models the whole turret front as a 35
    // degree sector, where the real casting stands nearly upright around the gun window (the
    // plateau band the visual loft already carries and the armour volume does not); and the
    // breech box front (±0.34 m) is wider than the internal mount it bolts into. Closing this
    // routes through a Model Idealny session — photograph, dossier row, then the volume — not
    // through a mechanics PR inventing a mount size.
    //
    // It stays a single named exemption rather than a blanket pass for every Breech, so the
    // mechanisms this rule CAN judge — the seven authored against `turret_group` — stay judged.
    (VehicleKind::T54_1951, 1),
    // The list ends here on purpose. It briefly carried the T-54's two engine-bay fuel cells,
    // which this rule caught standing 7 cm above the engine deck — a real defect, exempted only
    // long enough to decide whether correcting it was worth re-baking the reference model. It
    // was: the cells now stop on the deck plane and the rule judges them like everybody else.
];

#[test]
fn every_blueprint_born_vehicle_has_an_interior() {
    let empty: Vec<_> = VehicleKind::PLAYABLE
        .into_iter()
        .filter(|kind| DamageLayout::for_vehicle(*kind).is_empty())
        .collect();

    assert!(
        empty.is_empty(),
        "these hulls have nothing inside them — a penetration damages no module, starts no fire \
         and detonates no rack: {empty:?}"
    );
}

#[test]
fn no_component_id_falls_off_the_end_of_the_damage_mask() {
    let mut offenders = Vec::new();
    for kind in VehicleKind::ALL {
        for component in DamageLayout::for_vehicle(kind).components() {
            if component.id.0 == 0 || component.id.0 > MAX_COMPONENT_ID {
                offenders.push(format!("{kind:?}: {:?} has id {}", component.kind, component.id.0));
            }
        }
    }
    assert!(
        offenders.is_empty(),
        "component ids must be 1..={MAX_COMPONENT_ID}; outside that range the damage mask bit is \
         silently zero and the component can never be reported:\n  {}",
        offenders.join("\n  ")
    );
}

#[test]
fn no_vehicle_gives_two_components_the_same_id() {
    let mut offenders = Vec::new();
    for kind in VehicleKind::ALL {
        let layout = DamageLayout::for_vehicle(kind);
        let mut seen = Vec::new();
        for component in layout.components() {
            if seen.contains(&component.id.0) {
                offenders.push(format!("{kind:?}: id {} used twice", component.id.0));
            }
            seen.push(component.id.0);
        }
    }
    assert!(
        offenders.is_empty(),
        "an id is a component's identity in the damage mask; two components sharing one report as \
         each other:\n  {}",
        offenders.join("\n  ")
    );
}

/// The parts without which the battle model has nothing to bite on.
///
/// Written after this rule's absence let a Jagdtiger ship with no ammunition at all: its casemate
/// wall racks were dropped in a refactor, `every_blueprint_born_vehicle_has_an_interior` still
/// passed on the thirteen components that remained, and a tank that cannot detonate looked exactly
/// like a tank that can.
#[test]
fn every_hull_carries_the_components_a_battle_needs() {
    const REQUIRED: &[(DamageComponentKind, usize)] = &[
        (DamageComponentKind::Breech, 1),
        (DamageComponentKind::AmmunitionRack, 1),
        (DamageComponentKind::Engine, 1),
        (DamageComponentKind::Transmission, 1),
        (DamageComponentKind::FuelTank, 2),
        (DamageComponentKind::FinalDrive, 2),
        (DamageComponentKind::Suspension, 2),
        (DamageComponentKind::Radio, 1),
    ];

    let mut offenders = Vec::new();
    for kind in VehicleKind::PLAYABLE {
        let layout = DamageLayout::for_vehicle(kind);
        for (needed, count) in REQUIRED {
            let have = layout.components().iter().filter(|c| c.kind == *needed).count();
            if have < *count {
                offenders.push(format!("{kind:?}: {have} {needed:?}, needs {count}"));
            }
        }
    }
    assert!(
        offenders.is_empty(),
        "a hull missing one of these is a hull that cannot burn, cannot be immobilised, or cannot          be detonated — whatever the shell does to it:
  {}",
        offenders.join("
  ")
    );
}

/// Was `t54_damage_layout_fits_the_current_blueprint_hitbox`, for one vehicle.
#[test]
fn every_layout_fits_its_own_hitbox() {
    let offenders: Vec<_> = VehicleKind::PLAYABLE
        .into_iter()
        .filter(|kind| !DamageLayout::for_vehicle(*kind).fits_within(kind.spec().hitbox))
        .collect();

    assert!(offenders.is_empty(), "components reach outside the collision box: {offenders:?}");
}

/// The sharper version of the test above, and the one `t54.rs` kept asking for in its comments:
/// "fits_within is hitbox-blind". The hitbox is a box with margin; the ARMOUR is the shape a shell
/// actually meets. A component outside it is stowage floating in the air beside the tank, or fuel
/// stored in the plate.
#[test]
fn every_component_sits_inside_the_armour_that_protects_it() {
    let mut offenders = Vec::new();
    let mut hulls_measured = 0;

    for kind in VehicleKind::PLAYABLE {
        let Some(volumes) = vehicle_armor_volumes(kind) else { continue };
        hulls_measured += 1;
        let ring_y =
            HullEnvelope::of(kind).expect("a blueprint-born vehicle has an envelope").ring_y;
        for component in DamageLayout::for_vehicle(kind).components() {
            // A final drive has to reach the sprocket, so it crosses the hull side on purpose —
            // the one interior element allowed outside the tub. The suspension line stands on the
            // belt centreline for the same reason: it represents the arms and wheels, not
            // anything inside the armour.
            if matches!(
                component.kind,
                DamageComponentKind::FinalDrive | DamageComponentKind::Suspension
            ) || ARMOUR_CONTAINMENT_EXEMPT.contains(&(kind, component.id.0))
            {
                continue;
            }
            let escaped: Vec<Vec3> = corners(&component.shape)
                .into_iter()
                .filter(|corner| !inside_armour(component, *corner, volumes, ring_y))
                .collect();
            if !escaped.is_empty() {
                offenders.push(format!(
                    "{kind:?}: {:?} (id {}) has {} of {} sampled points outside the {:?} armour, first at {:?}",
                    component.kind,
                    component.id.0,
                    escaped.len(),
                    corners(&component.shape).len(),
                    component.frame,
                    escaped[0],
                ));
            }
        }
    }

    assert!(
        offenders.is_empty(),
        "components outside the armour they are supposed to sit behind:\n  {}",
        offenders.join("\n  ")
    );
    assert_eq!(
        hulls_measured,
        VehicleKind::PLAYABLE.len(),
        "containment has to be measured against every playable hull — a vehicle whose armour \
         volumes went missing would otherwise leave this test green and unarmoured"
    );
}

/// The anatomy that separates the two schools, stated as a promise rather than left in prose.
///
/// A German hull drives from the bow, so its gearbox is ahead of the turret ring and a bow
/// penetration takes the transmission. A Soviet or British hull drives from the tail, so its
/// gearbox is behind the engine and a bow penetration does not.
#[test]
fn the_gearbox_sits_at_the_driven_end_of_each_hull() {
    const FRONT_DRIVE: [VehicleKind; 4] =
        [VehicleKind::TigerI, VehicleKind::TigerII, VehicleKind::PantherII, VehicleKind::Jagdtiger];

    let mut offenders = Vec::new();
    for kind in VehicleKind::PLAYABLE {
        let Some(env) = HullEnvelope::of(kind) else { continue };
        let layout = DamageLayout::for_vehicle(kind);
        // The gearbox proper is the transmission volume nearest an end of the hull; a German
        // driveshaft is also a Transmission and deliberately spans the middle.
        let Some(gearbox) = layout
            .components()
            .iter()
            .filter(|component| component.kind == DamageComponentKind::Transmission)
            .max_by(|a, b| center_z(&a.shape).abs().total_cmp(&center_z(&b.shape).abs()))
        else {
            offenders.push(format!("{kind:?}: no transmission at all"));
            continue;
        };
        let forward = center_z(&gearbox.shape) > env.ring_z;
        if forward != FRONT_DRIVE.contains(&kind) {
            offenders.push(format!(
                "{kind:?}: gearbox at z {:+.2} is {} the ring, which is the wrong end for this drive",
                center_z(&gearbox.shape),
                if forward { "ahead of" } else { "behind" },
            ));
        }
    }
    assert!(offenders.is_empty(), "{}", offenders.join("\n  "));
}

/// A hull that overhangs its tracks has room out there, and what a designer put in it is a real
/// choice with a real consequence. The German hulls stowed ammunition in the sponsons; the T-54,
/// which has no sponson at all, could not.
#[test]
fn sponson_stowage_matches_the_hull_that_has_a_sponson() {
    let mut offenders = Vec::new();
    let mut hulls_measured = 0;
    for kind in VehicleKind::PLAYABLE {
        let Some(env) = HullEnvelope::of(kind) else { continue };
        hulls_measured += 1;
        let outboard = DamageLayout::for_vehicle(kind).components().iter().any(|component| {
            component.frame == ArmorFrame::Hull
                && matches!(
                    component.kind,
                    DamageComponentKind::AmmunitionRack | DamageComponentKind::FuelTank
                )
                && corners(&component.shape).iter().any(|c| c.x.abs() > env.tub_half_width)
        });
        if outboard && !env.has_sponson() {
            offenders
                .push(format!("{kind:?}: stowage out past a tub wall with no sponson to hold it"));
        }
    }
    assert!(offenders.is_empty(), "{}", offenders.join("\n  "));
    assert_eq!(
        hulls_measured,
        VehicleKind::PLAYABLE.len(),
        "every playable hull answers whether it has a sponson; a missing envelope must fail here \
         rather than quietly excuse a vehicle from the question"
    );
}

#[test]
fn the_casemate_has_no_turret_drive() {
    let drives = DamageLayout::for_vehicle(VehicleKind::Jagdtiger)
        .components()
        .iter()
        .filter(|component| component.kind == DamageComponentKind::TurretDrive)
        .count();

    assert_eq!(drives, 0, "a fixed casemate has nothing to traverse");
}

/// Which armour a point has to be inside of.
///
/// Split at the ring plane rather than by frame, because a real turret does not stop there: the
/// traverse gearbox reaches DOWN through the ring into the hull, and the basket hangs below it.
/// Checking every turret-frame point against the casting alone would report that mechanism as
/// escaping, and the fix would be to lift a gearbox off the ring it turns.
///
/// At neutral traverse the two frames coincide, which is the pose these shapes are authored in.
fn inside_armour(
    component: &DamageComponent,
    point: Vec3,
    volumes: &VehicleArmorVolumes,
    ring_y: f32,
) -> bool {
    let in_hull = volumes.hull.iter().any(|volume| within(volume, point));
    match component.frame {
        ArmorFrame::Hull => in_hull,
        ArmorFrame::Turret | ArmorFrame::Mantlet => {
            if point.y < ring_y {
                in_hull
            } else {
                within(&volumes.turret, point)
            }
        }
    }
}

/// A convex volume is the intersection of its half-spaces; interior is `normal · p <= offset`.
/// The margin forgives a component resting exactly against the inside face of a plate.
fn within(volume: &ArmorVolume, point: Vec3) -> bool {
    const MARGIN_M: f32 = 0.02;
    volume.planes.iter().all(|plane| plane.normal.dot(point) <= plane.offset + MARGIN_M)
}

fn center_z(shape: &DamageShape) -> f32 {
    let (min, max) = bounds(shape);
    (min.z + max.z) * 0.5
}

/// Points ON the shape's surface — not the corners of its bounding box.
///
/// The distinction is the whole test: the AABB corners of a vertical cylinder stick out past its
/// wall by `radius * (sqrt(2) - 1)`, so sampling them reports every round component as escaping
/// its armour. A rule that cannot be trusted is worse than no rule, because the fix is to move
/// correct components until it goes quiet.
fn corners(shape: &DamageShape) -> Vec<Vec3> {
    const RING: usize = 8;
    match shape {
        DamageShape::Obb { center, half_extents, yaw_rad } => {
            let rotation = glam::Mat3::from_rotation_y(*yaw_rad);
            let mut points = Vec::with_capacity(8);
            for x in [-half_extents.x, half_extents.x] {
                for y in [-half_extents.y, half_extents.y] {
                    for z in [-half_extents.z, half_extents.z] {
                        points.push(*center + rotation * Vec3::new(x, y, z));
                    }
                }
            }
            points
        }
        DamageShape::Cylinder { center, axis, half_length, radius } => {
            let along = axis.normalize_or_zero();
            let helper = if along.y.abs() < 0.9 { Vec3::Y } else { Vec3::X };
            let u = along.cross(helper).normalize_or_zero();
            let v = along.cross(u);
            (0..RING)
                .flat_map(|step| {
                    let angle = std::f32::consts::TAU * step as f32 / RING as f32;
                    let rim = (u * angle.cos() + v * angle.sin()) * *radius;
                    [*center + along * *half_length + rim, *center - along * *half_length + rim]
                })
                .collect()
        }
        DamageShape::Capsule { a, b, radius } => [*a, *b]
            .into_iter()
            .flat_map(|end| {
                (0..RING).map(move |step| {
                    let angle = std::f32::consts::TAU * step as f32 / RING as f32;
                    end + Vec3::new(angle.cos(), angle.sin(), 0.0) * *radius
                })
            })
            .collect(),
        DamageShape::Convex { bounds_min, bounds_max, .. } => {
            let mut points = Vec::with_capacity(8);
            for x in [bounds_min.x, bounds_max.x] {
                for y in [bounds_min.y, bounds_max.y] {
                    for z in [bounds_min.z, bounds_max.z] {
                        points.push(Vec3::new(x, y, z));
                    }
                }
            }
            points
        }
    }
}

fn bounds(shape: &DamageShape) -> (Vec3, Vec3) {
    match shape {
        DamageShape::Obb { center, half_extents, .. } => {
            (*center - *half_extents, *center + *half_extents)
        }
        DamageShape::Cylinder { center, axis, half_length, radius } => {
            let along = axis.normalize_or_zero() * *half_length;
            let radial = Vec3::splat(*radius);
            (*center - along.abs() - radial, *center + along.abs() + radial)
        }
        DamageShape::Capsule { a, b, radius } => {
            let radial = Vec3::splat(*radius);
            (a.min(*b) - radial, a.max(*b) + radial)
        }
        DamageShape::Convex { bounds_min, bounds_max, .. } => (*bounds_min, *bounds_max),
    }
}
