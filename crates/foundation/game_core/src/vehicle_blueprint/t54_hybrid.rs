//! The authoritative hybrid visual dimensions for the T-54 benchmark — the single source the mesh
//! generators read. Material intent is a clean, factory-fresh vehicle: crisp cast and machined
//! surfaces with restrained manufactured detailing, and deliberately *no* mud, rust, battle damage,
//! decals or heavy weathering (that finish belongs to the material layer, not the geometry).
//!
//! Dimensions are the documented T-54 obr. 1951 at 1:1 (see `data.rs`): ground clearance 0.43,
//! track band 1.06..1.63 per side, the LOW hull roof at 1.58, the tall cast dome from the 1.58
//! ring seat up to 2.27, cupola apex ~2.40, gun axis on the documented 1.78 fire line, muzzle
//! z 5.95.

use glam::Vec3;

use super::{
    ArmorShape, BoxVisual, DetailVisual, FenderVisual, FittingsVisual, GunVisual, HullPlatesVisual,
    HullShape, HullVisual, HybridVisual, TurretVisual,
};

/// Where the glacis meets the deck, and where the lower nose meets the belly. These two are
/// AUTHORED (the fold positions are a design decision, not a consequence); everything else about
/// the front plates is derived from them below.
const GLACIS_BASE_Z: f32 = 2.95;
const NOSE_BASE_Z: f32 = 2.52;

pub(super) fn t54_hybrid(hull: &HullShape, armor: &ArmorShape) -> HybridVisual {
    // The glacis plane and the lower-nose plane used to be FROZEN NUMBERS beside the blueprint
    // they are functions of (2.34 / (0,-0.75,1) / 2.20, rounded to 2 dp). Nothing recomputed
    // them, so every part hung off them — the glacis/roof weld seam, the splash board, the bow
    // tow cable — sat ~2 mm behind the plate it claims to lie on, and the moment anyone edited
    // `half_len` or the glacis angle the drift would have become visible instead of merely
    // measurable. They are derived here, from the same armour rake the plate is built at.
    let (sin_rake, cos_rake) = armor.hull_front.0.to_radians().sin_cos();
    let glacis_offset = sin_rake * hull.sponson_y + cos_rake * GLACIS_BASE_Z;
    // The lower nose runs from the fold down to the belly front; its normal is the perpendicular
    // of that run, so the plate follows the fold and the belly rather than a memorised slope.
    let run_z = NOSE_BASE_Z - GLACIS_BASE_Z;
    let run_y = hull.belly_y - hull.sponson_y;
    let nose_normal = Vec3::new(0.0, -run_z / run_y, 1.0);
    let nose_offset = nose_normal.dot(Vec3::new(0.0, hull.sponson_y, GLACIS_BASE_Z));

    HybridVisual {
        hull: HullVisual {
            // The narrow ~2.1 m box between fully exposed tracks — no overhanging sponsons.
            half_width: 1.05,
            belly_y: 0.43,
            roof_y: 1.58,
            half_len: 3.00,
            glacis_offset,
            nose_normal,
            nose_offset,
        },
        hull_plates: HullPlatesVisual { glacis_base_z: GLACIS_BASE_Z, nose_base_z: NOSE_BASE_Z },
        turret: TurretVisual {
            // The cast dome bulges LOW and wide: the sphere centre sits just above the ring (not high
            // in the band), so the widest cross-section overhangs the ring and the casting necks back
            // in toward the flat roof — the flattened "river-stone" T-54 pancake, not a tall round
            // pot. The wider radius (>ring_radius) is what gives the signature ring overhang/undercut.
            // The sphere centres sit BELOW the machined ring seat (the seat plane clips them), so
            // the widest cross-section is near the bottom of the casting and the tall shell
            // rounds continuously up to the roof — the 2.25 m-wide, ~0.7 m-tall hemispherical
            // dome the references show, not a shallow saucer.
            dome_radius: 1.12,
            dome_front: Vec3::new(0.0, 1.45, 0.10),
            // The rear bustle is a low, broad lobe reaching back behind the ring: a sphere set low
            // gives the casting a heavy overhanging tail rather than a narrow tapered tip.
            dome_rear: Vec3::new(0.0, 1.43, -0.28),
            dome_rear_radius: 0.92,
            dome_blend: 0.55,
            // Fuller, further-forward front cheeks flanking the mantlet at gun height: the T-54
            // turret's signature heavy cast front mass, fused into the dome with a wide blend so the
            // front reads as one continuous casting, not a separate ball stuck on the face.
            cheek_radius: 0.56,
            cheek_center: Vec3::new(0.47, 1.76, 0.58),
            cheek_blend: 0.58,
            // The ring is narrowed below the dome so the wider low cast dome visibly overhangs it —
            // the signature Soviet undercut — while the dome itself stays inside the ±1.125 turret
            // plan. (Visual ring only; the gameplay turret-ring radius lives in `TurretShape`.)
            ring_radius: 0.91,
            ring_half_height: 0.20,
            ring_center: Vec3::new(0.0, 1.76, 0.0),
            ring_blend: 0.33,
            roof_plane_y: 2.27,
            ring_plane_y: 1.58,
            cupola_radius: 0.24,
            // The drum roots DEEP into the curved dome (base ~2.02, well under the local shell
            // surface) so it grows out of the casting instead of levitating over the slope.
            cupola_half_height: 0.18,
            cupola_center: Vec3::new(-0.34, 2.20, -0.10),
            cupola_blend: 0.05,
            // A deeper mantlet socket on the fire line, so the cast trough the gun mantlet beds
            // into reads as a real cavity, not a dimple.
            socket_radius: 0.42,
            socket_center: Vec3::new(0.0, 1.78, 1.15),
            socket_blend: 0.07,
            bbox_min: Vec3::new(-1.30, 1.53, -1.35),
            bbox_max: Vec3::new(1.30, 2.45, 1.60),
            budget: 12_000,
        },
        // The lofted cast turret stations and shaping (split into `t54_hybrid_turret` for the file
        // budget): widest LOW for the ring overhang, front-heavy with a rear-pulled bustle, necking
        // into the flat roof, all within the ±1.125 / ±1.17 turret plan.
        turret_loft: super::t54_hybrid_turret::turret_loft(),
        gun: GunVisual {
            barrel_radius: 0.09,
            muzzle_radius: 0.085,
            muzzle_taper: 0.20,
            barrel_segments: 20,
            // The 1951 mask sits ON the turret face: its wide rounded shoulder rides at the
            // embrasure mouth (the turret front lip is at z ≈ 1.05, trunnion-relative ≈ -0.10)
            // and the sleeve tapers forward of it — a visible cast "pig's head", not a collar
            // swallowed inside the casting with a bare barrel poking out.
            mantlet_profile: [
                (-0.32, 0.13),
                (-0.22, 0.24),
                (-0.14, 0.295),
                (-0.04, 0.30),
                (0.06, 0.26),
                (0.16, 0.19),
                (0.26, 0.13),
                (0.36, 0.10),
            ],
            mantlet_segments: 28,
            // The rounded "pig's head" mask: wider than tall, but full enough to READ as the cast
            // mask on the turret face (the references show a substantial rounded jarzmo, not a
            // bare barrel root). Tall enough in Y to overlap the embrasure mouth through the whole
            // elevation arc — no exposed socket above/below the gun.
            mantlet_scale: Vec3::new(1.15, 0.72, 1.0),
            module_delta_scale: 0.65,
        },
        deck: BoxVisual { center: Vec3::new(0.0, 1.53, -1.80), half: Vec3::new(0.95, 0.06, 1.00) },
        // The fender shelf rides over the 1.06..1.63 track band at 1.12 — the primary kit line of
        // the vehicle (stowage, fuel tanks and the exhaust all live on it), with sloping end
        // sections over the idler and sprocket added by the detail pass.
        fender: FenderVisual { side_x: 1.345, center_y: 1.12, half: Vec3::new(0.29, 0.02, 2.70) },
        // The running gear (wheels, idler, sprocket, links) has no hybrid-visual copy: the animated
        // path reads the blueprint's `TrackShape` directly (`vehicle_geometry::RunningGearKinematics`).
        fittings: FittingsVisual {
            cupola_hatch_center: Vec3::new(-0.34, 2.39, -0.10),
            cupola_hatch_radius: 0.20,
            cupola_hatch_half_height: 0.04,
            // Driver's hatch on the hull roof, front-left: ahead of the turret ring, on the flat roof
            // that the 60deg glacis cuts off at z ~= 1.95, so the lid stays clear of the slope.
            driver_hatch_center: Vec3::new(-0.45, 1.62, 1.45),
            driver_hatch_radius: 0.17,
            driver_hatch_half_height: 0.05,
            // Loader's hatch ring on the turret roof, loader (right) side. Like the cupola it is
            // rooted deep into the curved dome (base ~2.04) so the lid never levitates.
            loader_hatch_center: Vec3::new(0.36, 2.16, 0.05),
            loader_hatch_radius: 0.19,
            loader_hatch_half_height: 0.12,
            // On the left fender front, outboard of the glacis, as every reference view shows.
            headlight_center: Vec3::new(-1.25, 1.225, 2.55),
            headlight_radius: 0.10,
            headlight_half_height: 0.09,
            // Bow hooks at the lower corners of the narrow nose plate.
            tow_hook_center: Vec3::new(0.80, 0.55, 2.62),
            tow_hook_half: Vec3::new(0.12, 0.11, 0.10),
        },
        // Clean factory-fresh detailing only: a louvered rear-deck grille, a boxed left-fender
        // exhaust cover, two low turret-roof periscopes, fender lips and a restrained glacis weld
        // bead. No mud, rust, battle damage, decals or heavy weathering — all sit inside the already
        // validated hull/turret volumes and add nothing to collision, armour, mounts or the snapshot.
        detail: DetailVisual {
            // The grille rides proud of the engine deck (deck top is y=1.59): at center_y 1.60 its
            // frame and slats top out at 1.63, clear of the deck plane. Earlier the grille top
            // was coplanar with the deck top, so the slats z-fought the deck into a flickering mess.
            grille_center: Vec3::new(0.0, 1.60, -2.00),
            grille_half: Vec3::new(0.80, 0.03, 0.55),
            grille_slats: 6,
            // The louvered exhaust box sits ON the left fender at the engine bay, as the top view
            // shows — the dark armoured cover among the left-fender stowage line.
            exhaust_center: Vec3::new(-1.34, 1.25, -0.90),
            exhaust_half: Vec3::new(0.26, 0.11, 0.45),
            // Turret-roof periscopes root into the curved dome (tall heads, bases ~2.02).
            // Model-logic audit #10: a Mk.4 head is a low fist-sized housing, not a chimney.
            // The centre sits ON the dome surface so only ~7 cm of head stands proud; the old
            // 0.24 m slab read as a floating plate with holes from the bow.
            periscope_center: Vec3::new(0.34, 2.045, 0.55),
            periscope_half: Vec3::new(0.055, 0.055, 0.055),
            // The pedestal drum centre; the DShK barrel rides near its top (+0.16).
            dshk_mount_center: Vec3::new(0.48, 2.20, -0.18),
            dshk_barrel_length: 0.62,
            // Shallow folded edge: the belt's top run carries its link bodies up to ~1.02, so a
            // deeper lip curtain has the scrolling shoes cutting through it (lip bottom 1.05).
            fender_lip_drop: 0.05,
            fender_lip_thickness: 0.03,
            weld_seam_half_thickness: 0.015,
        },
    }
}
