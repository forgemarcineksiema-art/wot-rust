//! THE MATERIAL LAW: a part is rendered as what it is MADE OF.
//!
//! [`MaterialRole`] carries twelve roles, and three of them say in their own declaration why they
//! exist at all — `Canvas`, `Glass` and `Timber` each record the same sentence: *one material for
//! two things is one of them rendered wrong*. That reasoning was applied once, to the T-54, and
//! never became a rule. Measured 2026-08-08 across the shipped bakes:
//!
//! | vehicle | roles | palette |
//! |---|---:|---|
//! | T-54 | 10 | the only vehicle that tags Glass, Canvas or Timber anywhere |
//! | Tiger I, Tiger II, Jagdtiger, Panther II, IS-3, Centurion, T-34-85 | **4** | RolledArmor · CastArmor · BarrelSteel · TrackMetal |
//!
//! Seven of eight vehicles rendered every lens, every prism and every fitting as one of four
//! kinds of steel. The shared `deck_details::headlight` built the whole lamp — glass included —
//! as a solid `BarrelSteel` cylinder at all five of its call sites, and every driver's and
//! commander's periscope was a steel bump on a box.
//!
//! This file is the rule the T-54 never became. It lives in `vehicle_forge`, not in
//! `vehicle_recipes`, for the reason `fleet_draw_cost.rs` gives: only here can each vehicle be
//! resolved through [`authoritative_baked_vehicle`] — the mesh the game actually draws, which for
//! the T-54 is the hybrid and not its unused procedural recipe.

use std::collections::BTreeSet;

use game_core::VehicleKind;
use vehicle_forge::authoritative_baked_vehicle;
use vehicle_geometry::MaterialRole;

/// Triangles a role must carry before it counts as a surface rather than a token. A single
/// sliver tagged `Glass` would satisfy a presence check while rendering as nothing at all.
const REAL_SURFACE_TRIS: usize = 8;

/// Every role used by a vehicle's shipped bake, with the triangle count carrying it.
fn role_tally(kind: VehicleKind) -> Vec<(MaterialRole, usize)> {
    let baked = authoritative_baked_vehicle(kind).expect("shipped bake");
    let mut tally: Vec<(MaterialRole, usize)> = Vec::new();
    for submesh in baked.submeshes() {
        for triangle in submesh.mesh.indices().chunks(3) {
            let role = submesh.mesh.vertices()[triangle[0] as usize].material;
            match tally.iter_mut().find(|(seen, _)| *seen == role) {
                Some((_, count)) => *count += 1,
                None => tally.push((role, 1)),
            }
        }
    }
    tally
}

fn tris_of(kind: VehicleKind, role: MaterialRole) -> usize {
    role_tally(kind).into_iter().find(|(seen, _)| *seen == role).map(|(_, n)| n).unwrap_or(0)
}

/// A crew looks out through glass. Headlights shine through glass. A vehicle whose bake carries
/// no `Glass` at all is one whose lenses and prisms are being drawn as the steel around them —
/// which is a disc and a bump, and a viewer reads them as one.
#[test]
fn every_vehicle_looks_through_glass_and_not_through_steel() {
    for kind in VehicleKind::PLAYABLE {
        let glass = tris_of(kind, MaterialRole::Glass);
        assert!(
            glass >= REAL_SURFACE_TRIS,
            "{kind:?}: {glass} triangles of Glass in the shipped bake. Every vehicle carries \
             headlights and vision devices; without Glass they are rendered as the steel drum \
             and the steel hood, which is the exact mistake MaterialRole::Glass was declared to \
             prevent."
        );
    }
}

/// The floor under the palette. Four roles is what the 2026-08-08 measurement found on seven of
/// eight vehicles: two kinds of armour, a gun tube and a track. That is a vehicle rendered as
/// one flat colour with two dark accents, and no amount of lighting work fixes it.
///
/// Raise this number as roles land — it is a floor that may only travel upward. It is NOT a
/// licence to tag parts falsely to clear it: every role a vehicle claims here must be carried by
/// a real surface, which is what `REAL_SURFACE_TRIS` enforces.
const MIN_MATERIAL_ROLES: usize = 5;

#[test]
fn no_vehicle_falls_back_to_the_four_steel_palette() {
    for kind in VehicleKind::PLAYABLE {
        let roles: BTreeSet<String> = role_tally(kind)
            .into_iter()
            .filter(|(_, tris)| *tris >= REAL_SURFACE_TRIS)
            .map(|(role, _)| format!("{role:?}"))
            .collect();
        assert!(
            roles.len() >= MIN_MATERIAL_ROLES,
            "{kind:?}: {} material roles on real surfaces ({}). The fleet's measured floor is \
             {MIN_MATERIAL_ROLES}; four means every fitting on this vehicle is being rendered as \
             one of four kinds of steel.",
            roles.len(),
            roles.into_iter().collect::<Vec<_>>().join(" · ")
        );
    }
}

/// The T-54 is not allowed to regress to the fleet's level either. It is the one vehicle that
/// ever had the material story written, and it is the reference the rest is being raised toward.
#[test]
fn the_t54_keeps_the_material_story_it_already_had() {
    for role in [MaterialRole::Glass, MaterialRole::Canvas, MaterialRole::Timber] {
        let tris = tris_of(VehicleKind::T54_1951, role);
        assert!(
            tris > 0,
            "T-54 lost {role:?} — the vehicle whose fittings are the fleet's reference for what \
             a material story looks like."
        );
    }
}
