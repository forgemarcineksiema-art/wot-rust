//! Visible D-10T stowage for the T-54-3. The groups follow the period T-54 layout: the large
//! front-hull skeleton rack, five crosswise turret-rear rounds, two loader-wall rounds, four
//! shin-level hull rounds and the isolated bulkhead cartridge. T-55 wet racks are deliberately
//! not used here. Each racked group hangs off its authoritative `AmmunitionRack` volume, so the
//! rounds a shell can detonate and the rounds the eye sees through a breach cannot drift apart.

use game_core::{DamageComponentId, DamageLayout};
use glam::Vec3;
use vehicle_geometry::{MaterialRole, SubmeshKind};

use super::anchors::obb_anchor_by_id;
use crate::part::{PartKey, PartLod, VehiclePart};
use crate::t54_interior::{box_part, drum_part};

pub(super) fn add_ammunition_parts(parts: &mut Vec<VehiclePart>, layout: &DamageLayout, cy: f32) {
    add_front_hull_rack(parts, layout, cy);
    add_turret_ready_racks(parts, layout, cy);
    add_hull_wall_rounds(parts, layout, cy);
}

fn add_front_hull_rack(parts: &mut Vec<VehiclePart>, layout: &DamageLayout, cy: f32) {
    // Twenty longitudinal rounds in a 4 x 5 skeleton frame, to the loader's side of the driver.
    let rack = obb_anchor_by_id(layout, DamageComponentId(5), cy);
    for row in 0..5 {
        for column in 0..4 {
            let index = row * 4 + column;
            let center = rack.center
                + Vec3::new(-0.27 + column as f32 * 0.18, -0.36 + row as f32 * 0.18, 0.0);
            push_round(parts, "bow_rack_round", index, SubmeshKind::Hull, center, Vec3::Z);
        }
    }

    for (index, dx) in [-0.35_f32, -0.18, 0.0, 0.18, 0.35].into_iter().enumerate() {
        parts.push(box_part(
            PartKey::indexed("bow_rack_upright", index as u16),
            SubmeshKind::Hull,
            rack.center + Vec3::new(dx, 0.0, -0.34),
            Vec3::new(0.012, 0.48, 0.018),
            MaterialRole::InteriorMachinery,
            PartLod::Detail,
        ));
    }
    for (index, dy) in (0..6).map(|row| -0.45 + row as f32 * 0.18).enumerate() {
        parts.push(box_part(
            PartKey::indexed("bow_rack_crossbar", index as u16),
            SubmeshKind::Hull,
            rack.center + Vec3::new(0.0, dy, -0.34),
            Vec3::new(0.36, 0.012, 0.018),
            MaterialRole::InteriorMachinery,
            PartLod::Detail,
        ));
    }
}

fn add_turret_ready_racks(parts: &mut Vec<VehiclePart>, layout: &DamageLayout, cy: f32) {
    // The 1951 egg-shaped turret gained five crosswise rounds in its rear, alternating
    // tip-to-tail. Two staggered rows against the bustle wall (3 + 2) instead of the old
    // 0.6 m vertical ladder, whose bottom rounds hung below the casting skirt and showed
    // brass on the deck from raised rear views (model-logic audit follow-up).
    let ready = obb_anchor_by_id(layout, DamageComponentId(6), cy);
    for index in 0..5 {
        let direction = if index % 2 == 0 { Vec3::X } else { Vec3::NEG_X };
        let (dy, dz) = if index < 3 {
            (-0.15 + index as f32 * 0.15, 0.05)
        } else {
            (-0.075 + (index - 3) as f32 * 0.15, -0.05)
        };
        push_round(
            parts,
            "turret_rear_round",
            index,
            SubmeshKind::Turret,
            ready.center + Vec3::new(0.0, dy, dz),
            direction,
        );
    }

    // Two forward-facing cartridges clipped to the loader's turret wall. Loose wall clips: no
    // free id under the v26 u16 component mask, so they stay visual-only for now.
    for index in 0..2 {
        push_round(
            parts,
            "loader_wall_round",
            index,
            SubmeshKind::Turret,
            Vec3::new(0.67, cy + 0.56 + index as f32 * 0.20, 0.28),
            Vec3::Z,
        );
    }
}

fn add_hull_wall_rounds(parts: &mut Vec<VehiclePart>, layout: &DamageLayout, cy: f32) {
    // Four cartridges sit at shin level on the loader's hull wall.
    let wall = obb_anchor_by_id(layout, DamageComponentId(16), cy);
    for row in 0..2 {
        for column in 0..2 {
            let index = row * 2 + column;
            let center = wall.center
                + Vec3::new(0.0, -0.095 + row as f32 * 0.19, -0.34 + column as f32 * 0.68);
            push_round(parts, "loader_shin_round", index, SubmeshKind::Hull, center, Vec3::Z);
        }
    }

    // The isolated bulkhead cartridge is the other mask-starved loose round — visual-only.
    push_round(
        parts,
        "bulkhead_floor_round",
        0,
        SubmeshKind::Hull,
        Vec3::new(-0.30, cy - 0.48, -0.70),
        Vec3::X,
    );
}

fn push_round(
    parts: &mut Vec<VehiclePart>,
    family: &'static str,
    index: u16,
    submesh: SubmeshKind,
    center: Vec3,
    axis: Vec3,
) {
    let axis = axis.normalize_or_zero();
    parts.push(drum_part(
        PartKey::indexed(family, index),
        submesh,
        center - axis * 0.11,
        axis,
        (0.215, 0.073),
        MaterialRole::Ammunition,
        PartLod::Detail,
    ));
    parts.push(drum_part(
        PartKey::indexed("d10_projectile_nose", round_detail_index(family, index)),
        submesh,
        center + axis * 0.225,
        axis,
        (0.105, 0.048),
        MaterialRole::Ammunition,
        PartLod::Detail,
    ));
    parts.push(drum_part(
        PartKey::indexed("d10_case_rim", round_detail_index(family, index)),
        submesh,
        center - axis * 0.337,
        axis,
        (0.012, 0.078),
        MaterialRole::InteriorMachinery,
        PartLod::Detail,
    ));
}

fn round_detail_index(family: &str, index: u16) -> u16 {
    let family_offset = match family {
        "bow_rack_round" => 0,
        "turret_rear_round" => 32,
        "loader_wall_round" => 40,
        "loader_shin_round" => 48,
        "bulkhead_floor_round" => 56,
        _ => 60,
    };
    family_offset + index
}
