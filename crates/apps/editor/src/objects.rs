//! The object tools (M5): a palette placed by click, selection by ray, and honest deletes
//! — every operation is a document edit through `apply_edit`, grounding happens in the
//! compiler like everywhere else, and a building keeps the ONE-truth rule (its cover box
//! IS the object; mesh, footprint and rubble form all read the same numbers).
//!
//! Scatters stay procedural: deleting a scatter-born instance adds a small
//! `exclusion_circles` entry to its op — the seed keeps walking, that spot now refuses.
//! Fixed scenery is mirror-fair by vocabulary (`push_mirrored`), so a placed tree ALWAYS
//! brings its northern twin; the editor shows both and deletes both through one spot.

use glam::Vec3;
use map_forge::blueprint::{MapBlueprint, ObjectSpec, SceneryOp, XCoord};
use terrain::{BattlefieldMap, SceneryKind, StaticCoverKind};

use crate::pick::ray_aabb;

/// One palette entry: what a click places.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaletteEntry {
    // Buildings: the STYLE is derived from extents + id (church/windmill by name, then
    // elongation/height pick barn/townhouse/cottage) — the palette speaks those rules.
    Cottage,
    Barn,
    Townhouse,
    Church,
    Windmill,
    // Bare cover.
    FenceRun,
    TreeLine,
    Wreck,
    // Render-only flora (a Fixed scenery spot; the vocabulary mirrors it).
    Oak,
    Poplar,
    Willow,
    FruitTree,
    Rock,
    Bush,
    Pine,
    /// Street furniture (urban-map PR-11) — render-only, like all flora.
    Lamppost,
    DebrisHeap,
}

impl PaletteEntry {
    pub const CYCLE: [PaletteEntry; 17] = [
        PaletteEntry::Cottage,
        PaletteEntry::Barn,
        PaletteEntry::Townhouse,
        PaletteEntry::Church,
        PaletteEntry::Windmill,
        PaletteEntry::FenceRun,
        PaletteEntry::TreeLine,
        PaletteEntry::Wreck,
        PaletteEntry::Oak,
        PaletteEntry::Poplar,
        PaletteEntry::Willow,
        PaletteEntry::FruitTree,
        PaletteEntry::Rock,
        PaletteEntry::Bush,
        PaletteEntry::Pine,
        PaletteEntry::Lamppost,
        PaletteEntry::DebrisHeap,
    ];

    pub fn label(self) -> &'static str {
        match self {
            PaletteEntry::Cottage => "cottage",
            PaletteEntry::Barn => "barn",
            PaletteEntry::Townhouse => "townhouse",
            PaletteEntry::Church => "church",
            PaletteEntry::Windmill => "windmill",
            PaletteEntry::FenceRun => "fence run",
            PaletteEntry::TreeLine => "tree line",
            PaletteEntry::Wreck => "wreck",
            PaletteEntry::Oak => "oak",
            PaletteEntry::Poplar => "poplar",
            PaletteEntry::Willow => "willow",
            PaletteEntry::FruitTree => "fruit tree",
            PaletteEntry::Rock => "rock",
            PaletteEntry::Bush => "bush",
            PaletteEntry::Pine => "pine",
            PaletteEntry::Lamppost => "lamppost",
            PaletteEntry::DebrisHeap => "debris heap",
        }
    }

    /// The cover half-extents (and kind) a click places — `None` for flora.
    pub fn cover(self) -> Option<(StaticCoverKind, [f32; 3])> {
        match self {
            // Extents drive the derived building style: barns are elongated and low,
            // townhouses tall, cottages small; church/windmill go by their id.
            PaletteEntry::Cottage => Some((StaticCoverKind::FarmBuilding, [4.0, 2.6, 3.2])),
            PaletteEntry::Barn => Some((StaticCoverKind::FarmBuilding, [7.0, 2.7, 4.2])),
            PaletteEntry::Townhouse => Some((StaticCoverKind::FarmBuilding, [4.5, 3.4, 4.5])),
            PaletteEntry::Church => Some((StaticCoverKind::FarmBuilding, [7.0, 6.0, 10.0])),
            PaletteEntry::Windmill => Some((StaticCoverKind::FarmBuilding, [4.0, 6.0, 4.0])),
            PaletteEntry::FenceRun => Some((StaticCoverKind::WoodenFence, [0.25, 0.65, 6.0])),
            PaletteEntry::TreeLine => Some((StaticCoverKind::TreeLine, [2.5, 3.5, 14.0])),
            PaletteEntry::Wreck => Some((StaticCoverKind::Wreck, [3.4, 1.4, 1.6])),
            _ => None,
        }
    }

    pub fn scenery(self) -> Option<SceneryKind> {
        match self {
            PaletteEntry::Oak => Some(SceneryKind::Oak),
            PaletteEntry::Poplar => Some(SceneryKind::Poplar),
            PaletteEntry::Willow => Some(SceneryKind::Willow),
            PaletteEntry::FruitTree => Some(SceneryKind::FruitTree),
            PaletteEntry::Rock => Some(SceneryKind::Rock),
            PaletteEntry::Bush => Some(SceneryKind::Bush),
            PaletteEntry::Pine => Some(SceneryKind::Pine),
            PaletteEntry::Lamppost => Some(SceneryKind::Lamppost),
            PaletteEntry::DebrisHeap => Some(SceneryKind::DebrisHeap),
            _ => None,
        }
    }

    fn id_prefix(self) -> &'static str {
        match self {
            PaletteEntry::Cottage => "cottage",
            PaletteEntry::Barn => "barn",
            PaletteEntry::Townhouse => "townhouse",
            PaletteEntry::Church => "church",
            PaletteEntry::Windmill => "windmill",
            PaletteEntry::FenceRun => "fence",
            PaletteEntry::TreeLine => "tree_line",
            PaletteEntry::Wreck => "wreck",
            _ => "object",
        }
    }
}

/// Place a palette entry at a world point (snapped to the metre). Returns the status
/// summary. Covers get a unique `prefix_N` id; flora becomes ONE `Fixed` spot (the
/// vocabulary emits its mirror twin — fairness is not optional).
pub fn place_entry(blueprint: &mut MapBlueprint, entry: PaletteEntry, at: [f32; 2]) -> String {
    let at = [at[0].round(), at[1].round()];
    if let Some((kind, half)) = entry.cover() {
        let id = unique_id(blueprint, entry.id_prefix());
        blueprint.objects.push(ObjectSpec::Cover {
            id: id.clone(),
            name: format!("{} (editor)", entry.label()),
            kind,
            at: [XCoord::Fixed(at[0]), XCoord::Fixed(at[1])],
            half_extents_m: half,
        });
        format!("placed {id} at {:.0}, {:.0}", at[0], at[1])
    } else if let Some(kind) = entry.scenery() {
        blueprint.scenery.push(SceneryOp::Fixed {
            kind,
            spots: vec![at],
            yaw_rad: 0.0,
            scale: 1.0,
        });
        format!("planted {} at {:.0}, {:.0} (with its mirror twin)", entry.label(), at[0], at[1])
    } else {
        "nothing to place".to_string()
    }
}

fn unique_id(blueprint: &MapBlueprint, prefix: &str) -> String {
    let mut highest = 0_u32;
    for object in &blueprint.objects {
        if let ObjectSpec::Cover { id, .. } = object
            && let Some(rest) = id.strip_prefix(prefix)
            && let Some(number) = rest.strip_prefix('_').and_then(|n| n.parse::<u32>().ok())
        {
            highest = highest.max(number);
        }
    }
    format!("{prefix}_{}", highest + 1)
}

/// What the navigate click selected.
#[derive(Debug, Clone, PartialEq)]
pub enum Selection {
    /// `objects[index]` — a single authored cover box.
    Cover { object_index: usize, id: String },
    /// A compiled cover that expands from a `TownGrid` — selectable, not editable here.
    TownGridMember { object_index: usize },
    /// One spot of a `Fixed` scenery op (either twin selects the same spot).
    FixedScenery { op_index: usize, spot_index: usize, kind: SceneryKind },
    /// A scatter-born instance: deleting it adds an exclusion circle to its op.
    ScatterInstance { position: [f32; 3], kind: SceneryKind },
}

/// Pick the object under a viewport ray: nearest cover AABB first, then scenery within a
/// 2 m corridor of the ray. Classification maps compiled data back to DOCUMENT entries.
pub fn pick(
    blueprint: &MapBlueprint,
    map: &BattlefieldMap,
    origin: Vec3,
    direction: Vec3,
) -> Option<Selection> {
    let mut best: Option<(f32, Selection)> = None;
    for (cover_index, cover) in map.static_cover.iter().enumerate() {
        if let Some(t) = ray_aabb(
            origin,
            direction,
            Vec3::from_array(cover.center),
            Vec3::from_array(cover.half_extents_m),
        ) && best.as_ref().is_none_or(|(closest, _)| t < *closest)
        {
            let selection = classify_cover(blueprint, cover_index, &cover.id);
            best = Some((t, selection));
        }
    }
    for instance in &map.scenery {
        let position = Vec3::from_array(instance.position) + Vec3::Y * 2.0;
        let along = (position - origin).dot(direction);
        if along <= 0.0 {
            continue;
        }
        let lateral = (position - origin - direction * along).length();
        if lateral < 2.0 && best.as_ref().is_none_or(|(closest, _)| along < *closest) {
            best =
                Some((along, classify_scenery(blueprint, map, instance.position, instance.kind)));
        }
    }
    best.map(|(_, selection)| selection)
}

/// Map a compiled cover index back to its document entry (objects expand in order; a
/// `TownGrid` emits `columns × rows × 2`).
fn classify_cover(blueprint: &MapBlueprint, cover_index: usize, id: &str) -> Selection {
    let mut emitted = 0_usize;
    for (object_index, object) in blueprint.objects.iter().enumerate() {
        let count = match object {
            ObjectSpec::Cover { .. } => 1,
            ObjectSpec::TownGrid { columns_x_m, row_offsets_m, .. } => {
                columns_x_m.len() * row_offsets_m.len() * 2
            }
        };
        if cover_index < emitted + count {
            return match object {
                ObjectSpec::Cover { .. } => Selection::Cover { object_index, id: id.to_string() },
                ObjectSpec::TownGrid { .. } => Selection::TownGridMember { object_index },
            };
        }
        emitted += count;
    }
    Selection::Cover { object_index: blueprint.objects.len().saturating_sub(1), id: id.to_string() }
}

/// A compiled scenery position is a Fixed spot (or its mirror twin) or scatter-born.
fn classify_scenery(
    blueprint: &MapBlueprint,
    map: &BattlefieldMap,
    position: [f32; 3],
    kind: SceneryKind,
) -> Selection {
    let axis_z = map.size_m[1] * 0.5;
    for (op_index, op) in blueprint.scenery.iter().enumerate() {
        if let SceneryOp::Fixed { kind: op_kind, spots, .. } = op
            && *op_kind == kind
        {
            for (spot_index, spot) in spots.iter().enumerate() {
                let twin_z = axis_z * 2.0 - spot[1];
                if (position[0] - spot[0]).abs() < 0.5
                    && ((position[2] - spot[1]).abs() < 0.5 || (position[2] - twin_z).abs() < 0.5)
                {
                    return Selection::FixedScenery { op_index, spot_index, kind };
                }
            }
        }
    }
    Selection::ScatterInstance { position, kind }
}

/// Delete the selection from the document. `Ok(summary)`; `Err(reason)` when the honest
/// answer is "not here" (a town grid is authored as one thing).
pub fn delete(
    blueprint: &mut MapBlueprint,
    map: &BattlefieldMap,
    selection: &Selection,
) -> Result<String, &'static str> {
    match selection {
        Selection::Cover { object_index, id } => {
            if *object_index < blueprint.objects.len() {
                blueprint.objects.remove(*object_index);
                Ok(format!("deleted {id}"))
            } else {
                Err("the selection went stale - reselect")
            }
        }
        Selection::TownGridMember { .. } => {
            Err("a town grid is authored as ONE thing - edit its op in the document")
        }
        Selection::FixedScenery { op_index, spot_index, kind } => {
            let Some(SceneryOp::Fixed { spots, .. }) = blueprint.scenery.get_mut(*op_index) else {
                return Err("the selection went stale - reselect");
            };
            if *spot_index < spots.len() {
                spots.remove(*spot_index);
                if spots.is_empty() {
                    blueprint.scenery.remove(*op_index);
                }
                Ok(format!("removed the {kind:?} (and its mirror twin)"))
            } else {
                Err("the selection went stale - reselect")
            }
        }
        Selection::ScatterInstance { position, kind } => {
            let axis_z = map.size_m[1] * 0.5;
            // Scatters walk the SOUTH half; the exclusion must name the canonical spot.
            let canonical_z =
                if position[2] > axis_z { axis_z * 2.0 - position[2] } else { position[2] };
            for op in blueprint.scenery.iter_mut() {
                if let SceneryOp::Scatter { kind: op_kind, region, exclude, .. } = op
                    && *op_kind == *kind
                    && (region.x[0]..=region.x[1]).contains(&position[0])
                    && (region.z[0]..=region.z[1]).contains(&canonical_z)
                {
                    exclude.exclusion_circles.push([position[0].round(), canonical_z.round(), 2.5]);
                    return Ok(format!(
                        "excluded the {kind:?} (the scatter stays procedural; a 2.5 m circle now refuses that spot)"
                    ));
                }
            }
            Err("no scatter op owns that instance - reselect after a recompile")
        }
    }
}

/// Rotate a selected cover 90 degrees: the boxes are axis-aligned by the collision
/// contract, so rotation IS swapping the x/z half-extents.
pub fn rotate_cover(blueprint: &mut MapBlueprint, selection: &Selection) -> Option<String> {
    if let Selection::Cover { object_index, id } = selection
        && let Some(ObjectSpec::Cover { half_extents_m, .. }) =
            blueprint.objects.get_mut(*object_index)
    {
        half_extents_m.swap(0, 2);
        return Some(format!("rotated {id} (x/z extents swapped)"));
    }
    None
}

/// The inspector rows for a selection (reusing the stamp panel's widget language).
pub fn inspector_lines(
    blueprint: &MapBlueprint,
    selection: &Selection,
    active: usize,
) -> Vec<(String, bool)> {
    match selection {
        Selection::Cover { object_index, id } => {
            let Some(ObjectSpec::Cover { kind, half_extents_m, .. }) =
                blueprint.objects.get(*object_index)
            else {
                return vec![("stale selection".to_string(), false)];
            };
            let mut lines = vec![(format!("{id}  ({kind:?})"), false)];
            for (index, (name, value)) in [
                ("half w", half_extents_m[0]),
                ("half h", half_extents_m[1]),
                ("half d", half_extents_m[2]),
            ]
            .into_iter()
            .enumerate()
            {
                lines.push((format!("{name}: {value:.1} m"), index == active));
            }
            lines.push(("R rotate   Del delete".to_string(), false));
            lines
        }
        Selection::TownGridMember { .. } => {
            vec![("town grid member".to_string(), false), ("authored as ONE op".to_string(), false)]
        }
        Selection::FixedScenery { kind, .. } => vec![
            (format!("{kind:?} (fixed, mirrored)"), false),
            ("Del removes both twins".to_string(), false),
        ],
        Selection::ScatterInstance { kind, .. } => vec![
            (format!("{kind:?} (scatter-born)"), false),
            ("Del excludes the spot".to_string(), false),
        ],
    }
}

/// Adjust the active knob of a selected cover (half extents, half-metre steps).
pub fn adjust_cover(
    blueprint: &mut MapBlueprint,
    selection: &Selection,
    active: usize,
    steps: f32,
) -> Option<String> {
    if let Selection::Cover { object_index, id } = selection
        && let Some(ObjectSpec::Cover { half_extents_m, .. }) =
            blueprint.objects.get_mut(*object_index)
        && active < 3
    {
        half_extents_m[active] = (half_extents_m[active] + steps * 0.5).clamp(0.2, 20.0);
        return Some(format!("{id}: extents {half_extents_m:?}"));
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch() -> crate::EditorDocument {
        crate::EditorDocument::new_scratch()
    }

    #[test]
    fn placement_grounds_compiles_and_ids_stay_unique() {
        let mut document = scratch();
        document.apply_edit(|blueprint| {
            place_entry(blueprint, PaletteEntry::Barn, [140.3, 120.7]);
            place_entry(blueprint, PaletteEntry::Barn, [180.0, 120.0]);
            place_entry(blueprint, PaletteEntry::Oak, [150.0, 100.0]);
        });
        let compiled = document.recompile();
        let barns: Vec<_> = compiled
            .battlefield
            .static_cover
            .iter()
            .filter(|cover| cover.id.starts_with("barn"))
            .collect();
        assert_eq!(barns.len(), 2);
        assert_ne!(barns[0].id, barns[1].id, "ids stay unique");
        assert!(barns[0].center[1] > 0.0, "grounded by the compiler");
        // One planted oak = TWO instances: the vocabulary mirrors fixed scenery.
        assert_eq!(compiled.battlefield.scenery.len(), 2);
        assert_eq!(compiled.battlefield.scenery[0].position[2], 100.0);
        assert_eq!(compiled.battlefield.scenery[1].position[2], 200.0, "the mirror twin");
    }

    #[test]
    fn picking_selects_the_nearest_cover_and_deleting_removes_it() {
        let mut document = scratch();
        document.apply_edit(|blueprint| {
            place_entry(blueprint, PaletteEntry::Cottage, [150.0, 150.0]);
            place_entry(blueprint, PaletteEntry::Wreck, [150.0, 190.0]);
        });
        let compiled = document.recompile();
        // Look at the cottage from the south: it must win over the wreck behind it.
        let origin = Vec3::new(150.0, 8.0, 100.0);
        let direction = (Vec3::new(150.0, 6.0, 150.0) - origin).normalize();
        let picked = pick(document.blueprint(), &compiled.battlefield, origin, direction)
            .expect("something under the ray");
        let Selection::Cover { id, .. } = &picked else { panic!("a cover, got {picked:?}") };
        assert!(id.starts_with("cottage"));

        document.apply_edit(|blueprint| {
            delete(blueprint, &compiled.battlefield, &picked).expect("deletes");
        });
        let after = document.recompile();
        assert!(after.battlefield.static_cover.iter().all(|c| !c.id.starts_with("cottage")));
        assert!(after.battlefield.static_cover.iter().any(|c| c.id.starts_with("wreck")));
    }

    #[test]
    fn deleting_a_scatter_instance_excludes_the_spot_and_the_twin_dies_with_it() {
        use map_forge::blueprint::{Exclusion, ScatterRect, SceneryOp};
        let mut document = scratch();
        document.apply_edit(|blueprint| {
            blueprint.scenery.push(SceneryOp::Scatter {
                seed: 7,
                kind: SceneryKind::Bush,
                pairs: 12,
                region: ScatterRect { x: [40.0, 260.0], z: [40.0, 148.0] },
                exclude: Exclusion::default(),
            });
        });
        let compiled = document.recompile();
        let before = compiled.battlefield.scenery.len();
        assert!(before >= 2, "the scatter grew something");
        let victim = compiled.battlefield.scenery[0];

        let selection = classify_scenery(
            document.blueprint(),
            &compiled.battlefield,
            victim.position,
            victim.kind,
        );
        assert!(matches!(selection, Selection::ScatterInstance { .. }));
        document.apply_edit(|blueprint| {
            delete(blueprint, &compiled.battlefield, &selection).expect("excludes");
        });
        let after = document.recompile();
        assert!(
            !after.battlefield.scenery.iter().any(|instance| {
                (instance.position[0] - victim.position[0]).abs() < 0.6
                    && (instance.position[2] - victim.position[2]).abs() < 0.6
            }),
            "the excluded spot must stay empty after recompile"
        );
    }

    #[test]
    fn fixed_scenery_deletes_through_either_twin_and_covers_rotate() {
        let mut document = scratch();
        document.apply_edit(|blueprint| {
            place_entry(blueprint, PaletteEntry::Willow, [120.0, 110.0]);
            place_entry(blueprint, PaletteEntry::Barn, [200.0, 150.0]);
        });
        let compiled = document.recompile();
        // Select via the NORTHERN twin (z = 190): it must resolve to the same spot.
        let selection = classify_scenery(
            document.blueprint(),
            &compiled.battlefield,
            [120.0, 5.0, 190.0],
            SceneryKind::Willow,
        );
        assert!(matches!(selection, Selection::FixedScenery { .. }));
        document.apply_edit(|blueprint| {
            delete(blueprint, &compiled.battlefield, &selection).expect("deletes");
        });
        assert_eq!(document.recompile().battlefield.scenery.len(), 0, "both twins die");

        let barn = Selection::Cover { object_index: 0, id: "barn_1".into() };
        let before = match &document.blueprint().objects[0] {
            ObjectSpec::Cover { half_extents_m, .. } => *half_extents_m,
            _ => panic!(),
        };
        document.apply_edit(|blueprint| {
            rotate_cover(blueprint, &barn).expect("rotates");
        });
        let after = match &document.blueprint().objects[0] {
            ObjectSpec::Cover { half_extents_m, .. } => *half_extents_m,
            _ => panic!(),
        };
        assert_eq!([after[2], after[1], after[0]], before, "rotation swaps x/z");
    }
}
