//! Authoritative T-54-3 obr. 1951 component placement.

use glam::Vec3;

use super::authoring::{cylinder_shape, hull_component, obb, turret_component};
use super::{
    DamageComponent, DamageComponentId, DamageComponentKind as K, DamageLayout,
    DamageMaterial as M, DamageShape,
};
use crate::{ArmorFrame, ModuleSlot};

/// The bore axis, in the hitbox-centred frame these components are authored in:
/// `blueprint.gun.trunnion_y (1.780) - hull.hitbox_center_y (1.190)`.
///
/// Written out rather than computed because `layout()` is built inside a `OnceLock` that the
/// blueprint's own `OnceLock` must not be re-entered from. `t54_gun_group_rides_the_trunnion`
/// recomputes it from the blueprint and fails if the two ever disagree, so this is a cached
/// derivation and not a remembered number.
const TRUNNION_LOCAL_Y: f32 = 0.590;

pub(super) fn layout() -> DamageLayout {
    let mut components = turret_components();
    components.extend(hull_components());
    components.extend(suspension_components());
    DamageLayout { components }
}

fn turret_components() -> Vec<DamageComponent> {
    vec![
        // THE GUN GROUP HANGS OFF THE TRUNNION, not off a remembered height. The blueprint puts
        // the bore axis at `trunnion_y` 1.78, which is 0.59 in this hitbox-centred frame — and
        // every piece of the D-10T was authored 0.23 to 0.42 m ABOVE that. The breech sat 230 mm
        // over the barrel it closes, with 499 mm of empty air between the two; the cradle rode
        // 420 mm over the tube it carries; and all of it broke out through the casting roof,
        // where the 2026-08-10 audit measured cradle rails standing ~250 mm proud of the metal in
        // the garage hero shot. The recoil guard was the ONE part someone anchored to the gun
        // (0.57), which is how the group's own frame gave the error away.
        //
        // `TRUNNION_LOCAL_Y` is derived, never retyped: `t54_gun_group_rides_the_trunnion` fails
        // if the blueprint's trunnion moves and these do not.
        turret_component(
            1,
            K::Breech,
            ModuleSlot::Gun,
            M::Machinery,
            // Coaxial with the bore: a breech ring is centred ON the barrel, not stacked over it.
            obb([0.0, TRUNNION_LOCAL_Y, 0.58], [0.34, 0.27, 0.43], 0.0),
            40,
            1.15,
        ),
        turret_component(
            2,
            K::RecoilMechanism,
            ModuleSlot::Gun,
            M::Machinery,
            // Brake and recuperator flank the tube in the cradle, a hand's width above the axis —
            // clear of the 0.092 m barrel at x = ±0.25 and clear of the roof.
            cylinder_shape([-0.25, TRUNNION_LOCAL_Y + 0.07, 0.42], Vec3::Z, 0.42, 0.095),
            38,
            1.0,
        ),
        turret_component(
            3,
            K::RecoilMechanism,
            ModuleSlot::Gun,
            M::Machinery,
            cylinder_shape([0.25, TRUNNION_LOCAL_Y + 0.07, 0.42], Vec3::Z, 0.42, 0.095),
            38,
            1.0,
        ),
        turret_component(
            4,
            K::TurretDrive,
            ModuleSlot::Turret,
            M::Driveline,
            cylinder_shape([-0.62, 0.47, 0.02], Vec3::Y, 0.24, 0.20),
            34,
            1.0,
        ),
        turret_component(
            7,
            K::Radio,
            ModuleSlot::Radio,
            M::Electronics,
            // Pulled 18 cm rearward (model-logic audit #13): at z 0.98 the set's forward face
            // reached 1.18 — clear THROUGH the casting front at that azimuth (~1.05), so the
            // museum radio panel with its three dials stood OUTSIDE the tank (the user's
            // "plate with three holes"). The 10-RT lives against the turret wall, inside.
            //
            // Pulled again, this time SIDEWAYS. Rearward cured the front face and left the
            // outboard corner: the control panel reached x −0.91 at z 0.92, where the casting
            // closes at ~1.20 on the diagonal — 61 to 94 mm of museum radio standing outside the
            // tank, sweeping through the hull roof plane at every traverse. The same defect the
            // note above declares fixed, on the other axis, because the fix moved the number that
            // failed instead of measuring the box against the dome.
            obb([0.48, 0.38, 0.68], [0.27, 0.22, 0.20], 0.0),
            28,
            1.4,
        ),
        // Five ready rounds clipped crosswise into the 1951 turret rear. They are turret stowage,
        // not hull stowage: the volume swings with the live turret yaw, so a rear-turret
        // penetration meets ammunition exactly where the period loadout put it.
        // Raised and compacted (model-logic audit follow-up): the old 0.78 m ladder reached
        // 0.23 m BELOW the ring plane — brass visibly poking out from under the casting skirt
        // onto the deck in raised rear views (fits_within is hitbox-blind, the same class as
        // the bow rack #9 and the radio #13). The clips sit against the bustle wall, wholly
        // inside the casting between skirt and roof.
        turret_component(
            6,
            K::AmmunitionRack,
            ModuleSlot::AmmoRack,
            M::Ammunition,
            //
            // AND THEN IT WENT OUT THROUGH THE ROOF. Raising the rack cured the skirt and broke
            // the crown: at y 0.86 the top round's case rim stood ~70 mm PROUD of the rear dome —
            // a 45 cm strip of brass on the casting, which the user spotted in the garage before
            // any test did. Three locks let it through: containment skips the turret entirely,
            // `racked_ammunition_rounds_sit_inside_their_authoritative_rack_volumes` compares the
            // AABB CENTRE (a round half outside the tank still passes), and the rack's own test
            // asserts counts and signs. Lowered and slimmed so BOTH ends are clear — the bottom
            // stays above the skirt, the top stays under the crown.
            obb([0.0, 0.74, -0.55], [0.36, 0.22, 0.12], 0.0),
            32,
            1.35,
        ),
    ]
}

fn hull_components() -> Vec<DamageComponent> {
    vec![
        // The twenty-round 4x5 skeleton rack in the bow, to the loader's side of the driver.
        // T-54 main stowage lives here, on the RIGHT — there is deliberately no rack on the
        // left hull; that side holds the driver and the radio operator's legacy space.
        hull_component(
            5,
            K::AmmunitionRack,
            ModuleSlot::AmmoRack,
            M::Ammunition,
            // Centre lowered 12 cm (model-logic audit #9): at 0.0 the rack topped out at a
            // world 1.67 m — 9 cm PROUD of the 1.58 m foredeck, so the museum rounds hanging
            // off it lay visibly on the glacis. The real 20-round rack stands on the hull
            // floor and stops under the deck; -0.12 puts the top at 1.55, a plate under it.
            // x pulled to 0.58 (2026-07-29 interior audit): at 0.65 the rack's outer face
            // reached 1.02 — through the inner wall of the 80 mm side plate on a 1.03 tub.
            // Rounds stored inside armour are rounds a side penetration cannot reach honestly.
            obb([-0.58, -0.12, 1.20], [0.37, 0.48, 0.36], 0.0),
            32,
            1.35,
        ),
        // Four shin-level rounds clipped low along the loader's hull wall. (The mask that once
        // capped this at id 16 is a u32 as of protocol v27 — ids 17+ own real bits now, so the
        // remaining loader-wall clips and the bulkhead round are free to become damage
        // components whenever the interior program wants them.)
        hull_component(
            16,
            K::AmmunitionRack,
            ModuleSlot::AmmoRack,
            M::Ammunition,
            // x 0.88 -> 0.85 (interior audit): the clips hug the INNER face of the wall
            // (0.95), not the middle of the plate.
            obb([-0.85, -0.335, 0.0], [0.10, 0.18, 0.69], 0.0),
            32,
            1.35,
        ),
        hull_component(
            8,
            K::FuelTank,
            ModuleSlot::Engine,
            M::Fuel,
            // Top pulled onto the deck plane (fleet containment rule, 2026-08-02). At half-y 0.43
            // about centre 0.03 the cell reached 0.46, and the hull interior at this station ends
            // at 0.39: 7 cm of fuel standing in open air above the engine deck. It is the third
            // instance of the fault this file already records fixing twice — the engine block
            // roof and the bow rack — and the first one nothing had to see by eye. The floor
            // stays where the interior audit put it; only the roof moves.
            obb([-0.70, -0.005, -1.43], [0.25, 0.395, 0.65], 0.0),
            25,
            1.1,
        ),
        hull_component(
            9,
            K::FuelTank,
            ModuleSlot::Engine,
            M::Fuel,
            // Both engine-bay tanks pulled from ±0.79 to ±0.70 (interior audit): their outer
            // faces sat at 1.04 — a centimetre PAST the hull side. Fuel outside the tank.
            // Roof pulled to the deck plane with its mirror; see the note on component 8.
            obb([0.70, -0.005, -1.43], [0.25, 0.395, 0.65], 0.0),
            25,
            1.1,
        ),
        hull_component(
            10,
            K::Engine,
            ModuleSlot::Engine,
            M::Machinery,
            // Roof pulled under the deck plane (model-logic follow-up): at half-y 0.48 the
            // block topped 7 cm PROUD of the 1.58 m deck, and the interior parts anchored to
            // it (intake spine, radiators) poked visible primer through the louvres.
            obb([0.0, -0.02, -1.90], [0.63, 0.42, 0.58], 0.0),
            30,
            0.9,
        ),
        hull_component(
            11,
            K::Transmission,
            ModuleSlot::Engine,
            M::Driveline,
            // Reshaped for the authored stern (2026-08-11): the tail is a knuckled wedge now —
            // the undercut below h 1.20 cuts the old box's rear-bottom corner off the vehicle.
            // The gearbox rides higher and a shade forward, inside the wedge, as the real unit
            // sits over the rear floor's rise toward the final drives.
            obb([0.0, 0.02, -2.51], [0.68, 0.36, 0.25], 0.0),
            30,
            0.9,
        ),
        hull_component(
            12,
            K::FinalDrive,
            ModuleSlot::Suspension,
            M::Driveline,
            // z -2.77 -> -2.59 (interior audit): PR-18 moved the end wheels onto the
            // documented belt, and the final drives stayed where the OLD sprocket was — a
            // drive housing 18 cm behind the axle it turns. The housing still crosses the hull
            // side deliberately: the drive must reach the sprocket, and that penetration is the
            // one interior element allowed outside the tub (see the interior containment lock).
            cylinder_shape([-1.08, -0.34, -2.59], Vec3::X, 0.18, 0.24),
            28,
            1.0,
        ),
        hull_component(
            13,
            K::FinalDrive,
            ModuleSlot::Suspension,
            M::Driveline,
            cylinder_shape([1.08, -0.34, -2.59], Vec3::X, 0.18, 0.24),
            28,
            1.0,
        ),
    ]
}

fn suspension_components() -> [DamageComponent; 2] {
    // ±1.24 -> ±1.32 (interior audit): the suspension capsule is the torsion-bar/wheel-arm
    // line, and the wheel plane moved with the documented 2.640 m gauge in PR-18. A capsule 80 mm
    // inboard of the wheels it stands for takes hits the suspension would not.
    [-1.32, 1.32].map(|x| DamageComponent {
        id: DamageComponentId(if x < 0.0 { 14 } else { 15 }),
        frame: ArmorFrame::Hull,
        kind: K::Suspension,
        slot: ModuleSlot::Suspension,
        material: M::SuspensionSteel,
        shape: DamageShape::Capsule {
            a: Vec3::new(x, -0.70, -2.65),
            b: Vec3::new(x, -0.70, 2.65),
            radius: 0.18,
        },
        priority: 20,
        requires_penetration: false,
        vulnerability: 0.8,
    })
}
