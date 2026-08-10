//! The turret interior stays inside the CASTING — measured against the dome, not against the hull.
//!
//! `t54_interior_containment` guards the hull, and it guards it against the hull's side plates.
//! Everything above the ring plane it skipped outright (`if b.min.y > 1.58 { continue }`), with a
//! note saying the turret "answers to the casting via its skin and basket tests". Those tests
//! check the skin's own bounds and normals; they never look at what lives inside it. So nothing
//! measured the turret interior against the turret, and four assemblies walked out through the
//! armour while every lock stayed green:
//!
//!   * `d10_cradle_rail`  ~250 mm proud of the roof — visible as two boxes on the turret crown;
//!   * `tsh2_sight_body`   163 mm — a telescope looking out of solid armour;
//!   * `d10_recoil_cylinder` 163 mm;
//!   * `turret_rear_round`/`d10_case_rim` 71-129 mm of BRASS on the rear dome, no breach needed;
//!   * `radio_control_face` 61-94 mm out the side, sweeping the hull roof at every traverse.
//!
//! The user found the brass in the garage before any test did.
//!
//! METHOD. The surface is the FINISHED `turret_shell` mesh, not a re-derived superellipse, so the
//! cheek swells and the gun embrasure are accounted for by construction rather than by
//! re-implementing `cast_loft`'s bumps here and hoping the two agree. For a vertex we compare its
//! radius in the turret's XZ plane against the shell's radius at the nearest (y, azimuth) sample.
//! On a domed roof that OVERSTATES how proud a vertex is, which is the safe direction for a lock:
//! it can nag, it cannot miss.

use std::collections::BTreeMap;

use vehicle_build::t54_description;
use vehicle_geometry::{MaterialRole, SubmeshKind};

/// Interior roles — the paint, the machinery and the ammunition that live inside the armour.
fn is_interior(role: MaterialRole) -> bool {
    matches!(
        role,
        MaterialRole::InteriorPrimer | MaterialRole::InteriorMachinery | MaterialRole::Ammunition
    )
}

/// How far a vertex may read proud of the casting before we call it out. The measure overstates
/// on a curved roof (see METHOD), so this is slack for that bias, not a licence to poke out.
const PROUD_TOLERANCE_M: f32 = 0.010;

/// Below the ring plane the boundary is the HULL, which `t54_interior_containment` owns.
const RING_PLANE_Y: f32 = 1.60;

/// The one part that is SUPPOSED to break the surface: a coaxial machine gun fires through the
/// armour or it does not fire. It is excused here and held to a stricter rule below — it must
/// come out inside the mantlet window, not through bare casting somewhere else on the dome.
const MUZZLE_EXCEPTION: &str = "sg43_coax_barrel";

/// Half-width of the gun window in X, from the blueprint's `window_az_width`. A coax that clears
/// the casting outside this is a gun firing through armour with no port for it — which is exactly
/// what x = 0.39 used to do, 160 mm outside the opening.
const WINDOW_HALF_X_M: f32 = 0.23;

struct Casting {
    samples: Vec<(f32, f32, f32)>,
}

impl Casting {
    fn of(description: &vehicle_build::VehicleDescription) -> Self {
        let shell = description
            .parts
            .iter()
            .find(|p| p.key.name == "turret_shell")
            .expect("the T-54 turret is a lofted casting named turret_shell");
        let samples = shell
            .mesh()
            .vertices()
            .iter()
            .map(|v| {
                let p = v.position;
                (p.y, p.z.atan2(p.x), (p.x * p.x + p.z * p.z).sqrt())
            })
            .collect();
        Self { samples }
    }

    /// The casting's radius nearest to `(y, azimuth)`. Azimuth wraps, so distance is measured on
    /// the circle; `y` is weighted alongside it so a vertex between rings cannot match a far one.
    fn radius_at(&self, y: f32, azimuth: f32) -> f32 {
        let mut best = f32::MAX;
        let mut radius = 0.0;
        for &(sy, saz, sr) in &self.samples {
            let mut daz = (saz - azimuth).abs();
            if daz > std::f32::consts::PI {
                daz = std::f32::consts::TAU - daz;
            }
            let d = (sy - y).abs() * 3.0 + daz;
            if d < best {
                best = d;
                radius = sr;
            }
        }
        radius
    }

    /// How far `p` stands proud of the casting. Negative is inside.
    fn proud(&self, p: glam::Vec3) -> f32 {
        let radius = (p.x * p.x + p.z * p.z).sqrt();
        radius - self.radius_at(p.y, p.z.atan2(p.x))
    }
}

#[test]
fn no_turret_interior_part_breaks_the_casting() {
    let description = t54_description();
    let casting = Casting::of(&description);

    let mut worst: BTreeMap<&str, (f32, glam::Vec3)> = BTreeMap::new();
    for part in &description.parts {
        if part.submesh != SubmeshKind::Turret || part.key.name == "turret_shell" {
            continue;
        }
        if part.key.name == MUZZLE_EXCEPTION {
            continue;
        }
        let mesh = part.mesh();
        if !mesh.vertices().iter().any(|v| is_interior(v.material)) {
            continue;
        }
        for v in mesh.vertices() {
            if v.position.y < RING_PLANE_Y {
                continue;
            }
            let proud = casting.proud(v.position);
            let entry = worst.entry(part.key.name).or_insert((f32::MIN, v.position));
            if proud > entry.0 {
                *entry = (proud, v.position);
            }
        }
    }

    assert!(!worst.is_empty(), "the turret carries interior parts above the ring");
    for (name, (proud, at)) in &worst {
        assert!(
            *proud <= PROUD_TOLERANCE_M,
            "{name} stands {:.0} mm proud of the casting at ({:+.3}, {:.3}, {:+.3}). \
             The gun group hangs off the trunnion and the stowage hangs off its rack; if this \
             part moved, move the volume it belongs to rather than the vertex that failed.",
            proud * 1000.0,
            at.x,
            at.y,
            at.z
        );
    }
}

/// The excused part is excused for a REASON, and the reason is checked: the coax must reach
/// daylight, and it must do it through the gun window.
#[test]
fn the_coaxial_muzzle_reaches_daylight_through_the_gun_window() {
    let description = t54_description();
    let casting = Casting::of(&description);

    let barrel = description
        .parts
        .iter()
        .find(|p| p.key.name == MUZZLE_EXCEPTION)
        .expect("the T-54 mounts a coaxial machine gun");
    let mesh = barrel.mesh();

    let mut clears = false;
    for v in mesh.vertices() {
        if casting.proud(v.position) <= 0.0 {
            continue;
        }
        clears = true;
        assert!(
            v.position.x.abs() <= WINDOW_HALF_X_M,
            "the coax clears the casting at x {:+.3}, outside the {WINDOW_HALF_X_M} m gun window \
             — a machine gun firing through armour that has no port for it",
            v.position.x
        );
        assert!(
            v.position.z > 0.0,
            "the coax clears the casting at z {:+.3}: it fires FORWARD",
            v.position.z
        );
    }
    assert!(
        clears,
        "the coaxial barrel ends inside the casting — a gun with no muzzle. It is on the \
         containment exception list precisely because it is supposed to come out."
    );
}

/// The gun group is one assembly on one axis. Anchoring the pieces to the trunnion is the fix;
/// this is the lock that keeps them there when someone nudges a number.
#[test]
fn the_gun_group_rides_the_trunnion() {
    let blueprint =
        game_core::VehicleBlueprint::for_vehicle(game_core::VehicleKind::T54_1951).expect("bp");
    let axis_y = blueprint.gun.trunnion_y;
    let description = t54_description();

    // Every piece of the D-10T and its sights, and how far each may sit off the bore axis.
    let group: [(&str, f32); 6] = [
        ("d10_breech_ring", 0.05),
        ("d10_cradle_bridge", 0.08),
        ("d10_cradle_rail", 0.08),
        ("d10_recoil_cylinder", 0.16),
        ("sg43_coax_barrel", 0.12),
        ("tsh2_sight_body", 0.14),
    ];
    for (name, allowed) in group {
        let part = description
            .parts
            .iter()
            .find(|p| p.key.name == name)
            .unwrap_or_else(|| panic!("{name} is part of the gun group"));
        let bounds = part.mesh().bounds().expect("part has geometry");
        let center_y = (bounds.min.y + bounds.max.y) * 0.5;
        let off = (center_y - axis_y).abs();
        assert!(
            off <= allowed,
            "{name} sits {:.0} mm off the bore axis ({axis_y:.3}); allowed {:.0} mm. \
             The breech closes the barrel, the cradle carries it and the sight is linked to it — \
             none of them is authored at its own remembered height.",
            off * 1000.0,
            allowed * 1000.0
        );
    }
}
