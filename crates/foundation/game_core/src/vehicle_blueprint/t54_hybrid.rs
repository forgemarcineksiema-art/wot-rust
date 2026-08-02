//! The authoritative hybrid visual dimensions for the T-54 benchmark — the single source the mesh
//! generators read. Material intent is a clean, factory-fresh vehicle: crisp cast and machined
//! surfaces with restrained manufactured detailing, and deliberately *no* mud, rust, battle damage,
//! decals or heavy weathering (that finish belongs to the material layer, not the geometry).
//!
//! Dimensions are the documented T-54 obr. 1951 at 1:1 (see `data.rs`): ground clearance 0.43,
//! track band 1.03..1.61 per side, the LOW hull roof at 1.58, the tall cast dome from the 1.58
//! ring seat up to 2.27, cupola apex ~2.40, gun axis on the documented 1.78 fire line, muzzle
//! z 5.95.

use glam::Vec3;

use super::source::BlueprintFile;
use super::{
    BoxVisual, DetailVisual, FenderVisual, FittingsVisual, GunVisual, HullPlatesVisual, HullVisual,
    TurretVisual, VisualDetail,
};

/// Where the glacis meets the deck, and where the lower nose meets the belly. These two are
/// AUTHORED (the fold positions are a design decision, not a consequence); everything else about
/// the front plates is derived from them below.
///
/// They are stated as SETBACKS from the bow, not as absolute Z. The hull's length is the one
/// number the whole front of the vehicle hangs off, and while these were absolutes the two
/// numbers agreed only by luck: change `half_len` and the two-plate front stayed where it was
/// while the hull moved out from under it.
/// Half-height of the cupola drum itself: enough below the roof to root in the casting. AUTHORED —
/// the blueprint states how proud the drum stands (`cupola_height`), not how deep it roots.
const CUPOLA_HALF_HEIGHT_M: f32 = 0.18;
const GLACIS_SETBACK_M: f32 = 0.05;
const NOSE_SETBACK_M: f32 = 0.48;
/// How far the engine deck's rear edge stops short of the rear plate.
const DECK_REAR_GAP_M: f32 = 0.20;
/// How far each fender run stops short of the hull's end plates.
const FENDER_END_GAP_M: f32 = 0.30;

pub(super) fn t54_hybrid(file: &BlueprintFile) -> VisualDetail {
    let (hull, armor) = (&file.hull, &file.armor);
    // Everything bolted to the turret roof is stated as a DEPTH INTO THE CASTING, not as an
    // absolute height. Raise the dome and the cupola, the hatches, the periscopes and the DShK
    // mount rise with it, each staying the same distance into the metal it is rooted in. They
    // were absolutes, and the whole band would have had to be re-typed by hand.
    //
    // The roof height, the cupola's footprint and how proud it stands are the GAMEPLAY turret's
    // numbers, not a second copy beside it: these used to be consts re-typed from the blueprint
    // (2.40 / 0.312 / 0.131 / −0.34 / −0.10), each one a number about to disagree with itself.
    let roof = file.turret.roof_y;
    let cupola_proud = file
        .turret
        .cupola_height
        .expect("a hybrid vehicle authors its cupola height; the legacy fallback launders errors");
    // The commander's cupola, authored once and handed to everything that draws it.
    let cupola = (
        Vec3::new(
            file.turret.cupola_x,
            roof + cupola_proud - CUPOLA_HALF_HEIGHT_M,
            file.turret.cupola_z,
        ),
        file.turret.cupola_radius,
        CUPOLA_HALF_HEIGHT_M,
    );
    // The glacis plane and the lower-nose plane used to be FROZEN NUMBERS beside the blueprint
    // they are functions of (2.34 / (0,-0.75,1) / 2.20, rounded to 2 dp). Nothing recomputed
    // them, so every part hung off them — the glacis/roof weld seam, the splash board, the bow
    // tow cable — sat ~2 mm behind the plate it claims to lie on, and the moment anyone edited
    // `half_len` or the glacis angle the drift would have become visible instead of merely
    // measurable. They are derived here, from the same armour rake the plate is built at.
    let glacis_base_z = hull.half_len - GLACIS_SETBACK_M;
    let nose_base_z = hull.half_len - NOSE_SETBACK_M;
    let (sin_rake, cos_rake) = armor.hull_front.0.to_radians().sin_cos();
    let glacis_offset = sin_rake * hull.sponson_y + cos_rake * glacis_base_z;
    // The lower nose runs from the fold down to the belly front; its normal is the perpendicular
    // of that run, so the plate follows the fold and the belly rather than a memorised slope.
    let run_z = nose_base_z - glacis_base_z;
    let run_y = hull.belly_y - hull.sponson_y;
    let nose_normal = Vec3::new(0.0, -run_z / run_y, 1.0);
    let nose_offset = nose_normal.dot(Vec3::new(0.0, hull.sponson_y, glacis_base_z));

    VisualDetail {
        hull: Some(HullVisual {
            // The narrow box between fully exposed tracks — no overhanging sponsons. Its width
            // is the GAMEPLAY hull's, not a second copy of it: the tub has to fit the space the
            // documented gauge and track width leave, and a duplicate of that number is a
            // number that is about to disagree with itself.
            half_width: hull.half_width,
            // Length and belly come from the GAMEPLAY hull, not from a second copy beside it.
            // They were duplicated here, and a duplicate of a dimension is a dimension that is
            // about to disagree with itself.
            belly_y: hull.belly_y,
            // The hull roof IS the plane the turret ring seats on — one number, the gameplay
            // turret's, not a 1.58 re-typed beside it.
            roof_y: file.turret.ring_y,
            half_len: hull.half_len,
            glacis_offset,
            nose_normal,
            nose_offset,
        }),
        hull_plates: Some(HullPlatesVisual { glacis_base_z, nose_base_z }),
        turret: Some(TurretVisual {
            // The machined seat, visually narrowed under the loft's overhanging casting (the
            // gameplay turret-ring radius lives in `TurretShape`); the ring-seam AO band and the
            // ring collar hang off these two.
            ring_radius: 0.91,
            ring_plane_y: file.turret.ring_y,
            // The mantlet-seat AO band sits ON the trunnion — the gameplay gun's fire line, not
            // a re-typed 1.78/1.15 that would stay put if the gun moved.
            socket_center: Vec3::new(0.0, file.gun.trunnion_y, file.gun.trunnion_z),
            budget: 12_000,
        }),
        // The lofted cast turret stations and shaping (split into `t54_hybrid_turret` for the file
        // budget): widest LOW for the ring overhang, front-heavy with a rear-pulled bustle, necking
        // into the flat roof, all within the ±1.125 / ±1.17 turret plan.
        turret_loft: Some(super::t54_hybrid_turret::turret_loft(cupola)),
        gun: Some(GunVisual {
            // THE DRIFTED PAIR (W4 F1 finding, left in place deliberately): the gameplay gun says
            // `barrel_radius: 0.092` and the drawn tube says 0.090 — the shell's barrel is 2 mm
            // fatter than the eye's. The visible thickness carries a player verdict (the "barrel
            // too thin" review item was withdrawn), so reconciling is a DECISION about which
            // number is the vehicle, not a mechanical fix; it goes to the Model Idealny session
            // with the plateau/mantlet debt. Until then the drift is on the record here.
            barrel_radius: 0.09,
            // The D-10T is a 100 mm gun with a thin wall: the documented muzzle OD is about
            // 120-126 mm, so a 10-13 mm wall around a 100 mm bore. The tube used to end at
            // ⌀170 with a ⌀192 collar — 40% too fat, which is why the muzzle read as a flat
            // steel disc with a dimple in it rather than as a hole.
            muzzle_radius: 0.062,
            bore_radius: 0.050,
            muzzle_taper: 0.20,
            // A 20-gon tube shows its facets against the sky at any range where the barrel is
            // the thing you are looking down.
            barrel_segments: 28,
            // The T-54 obr. 1951 mantlet is INSIDE the turret. The external "pig's head" ball
            // is a WoT-ism — the dossier is explicit about it — and this profile used to be
            // one: a sleeve reaching 0.40 m PAST the trunnion, out through the casting face
            // and into the air in front of it.
            //
            // What replaces it is a cast body: closed at the back, closed at the front, widest
            // (0.25) well behind the aperture it can therefore never be pushed out through, and
            // stepping down to a 0.155 face that shows through the hole. The barrel passes
            // through it; the two are separate parts and the tube is what covers the axis.
            //
            // The pocket floor sits at z = 1.240 - 0.16 = 1.080, i.e. -0.07 from the trunnion.
            // The face at -0.030 therefore stands 40 mm proud of the floor and stays 0.13 m
            // behind the casting's un-recessed front — inside the aperture, not through it.
            // Pulled 30 mm deeper behind the face after the egg reshape: the casting's nose
            // sits further back than the old squircle plateau, and the internal body must keep
            // clearing it by the contract's 80 mm. It is occluded by the canvas either way.
            mantlet_profile: [
                (-0.330, 0.000),
                (-0.295, 0.135),
                (-0.220, 0.220),
                (-0.145, 0.250),
                (-0.100, 0.243),
                (-0.080, 0.185),
                (-0.070, 0.155),
                (-0.060, 0.000),
            ],
            // Twelve: the mantlet lives BEHIND the canvas now — the cover panel occludes it
            // from every exterior angle, and it only shows through a breach remesh. Twenty-eight
            // was the resolution of a part the player was meant to look at.
            mantlet_segments: 9,
            // Very nearly round. The old 1.15/0.72 squash existed to make a BALL read as a mask;
            // an internal mantlet has no such job — it is a cast cylinder on trunnions, a touch
            // wider than tall because the trunnion bosses are.
            mantlet_scale: Vec3::new(1.06, 0.98, 1.0),
            // The SLEEVE of the canvas cover: from inside the panel's mouth (root radius a
            // shade over the mouth's half-height, so the gather zone is closed by fabric-on-
            // fabric) out to the strap on the tube (0.094 there). Short, as the real one is —
            // the panel does the covering; the sleeve only walks the barrel out of it.
            // The sleeve roots BEHIND the panel's flat front face (the face plane sits at
            // 0.150) so the boot visibly pierces the canvas instead of floating ahead of it.
            // Radii keep the sagged top surface proud of the 0.098 tube everywhere — and
            // it is the ten-gon's INRADIUS (r·cos 18°) that must clear the tube, not the
            // vertex radius: at 0.100 the flats dipped under the steel and the barrel showed
            // through the sleeve as a torn zigzag.
            mantlet_cover: [(0.100, 0.128), (0.230, 0.118), (0.295, 0.116), (0.360, 0.104)],
            mantlet_cover_sag: 0.010,
            // The fabric panel's fastening frame, sized to the window at half-depth (window
            // az 0.26 rad on the 1.125 superellipse is ±0.42 of x; the frame sits 20 mm inside
            // the walls all round).
            // MEASURED off the dead-front reference (module 2): the fastened panel is a compact
            // pillow ~0.45 x 0.37, not the first build's 0.80 letterbox.
            cover_frame_half: (0.225, 0.185),
            module_delta_scale: 0.65,
        }),
        // The engine deck runs from behind the turret ring back to a hand's width off the rear
        // plate: its REAR edge belongs to the stern, its front edge to the fighting compartment.
        deck: Some(BoxVisual {
            center: Vec3::new(0.0, 1.53, (-hull.half_len + DECK_REAR_GAP_M - 0.80) * 0.5),
            half: Vec3::new(0.95, 0.06, (0.80 - hull.half_len + DECK_REAR_GAP_M).abs() * 0.5),
        }),
        // The fender shelf rides over the 1.03..1.61 track band at 1.12 — the primary kit line of
        // the vehicle (stowage, fuel tanks and the exhaust all live on it), with sloping end
        // sections over the idler and sprocket added by the detail pass.
        //
        // It is CENTRED on the belt (the blueprint lint holds it there — one number, one
        // truth) and reaches out to 1.635, which is the documented 3.27 m over the fenders. So
        // it covers the 0.58 m belt and overhangs it by the 25 mm each side that turns 3.220
        // over the tracks into 3.270 over the vehicle. Both numbers are the dossier's; the
        // shelf is what reconciles them, and its inner edge tucks behind the tub side.
        fender: Some(FenderVisual {
            // Stated, not derived — MEASURED decision (W4 F1): `(inner_x + outer_x) * 0.5` lands
            // one ULP off this literal (0x3fa8f5c2 vs 0x3fa8f5c3) and re-blessing a golden over
            // a rounding ghost is not worth the precedent. The blueprint lint still holds the
            // shelf centred on the belt, so the pair cannot drift.
            side_x: 1.32,
            center_y: 1.12,
            // The shelf runs the length of the hull, short of each end plate.
            half: Vec3::new(0.315, 0.02, hull.half_len - FENDER_END_GAP_M),
        }),
        // The running gear (wheels, idler, sprocket, links) has no hybrid-visual copy: the animated
        // path reads the blueprint's `TrackShape` directly (`vehicle_geometry::RunningGearKinematics`).
        fittings: Some(FittingsVisual {
            // The hatch rides ITS cupola: same x/z as the drum, never a second copy of them.
            cupola_hatch_center: Vec3::new(file.turret.cupola_x, roof + 0.12, file.turret.cupola_z),
            cupola_hatch_radius: 0.20,
            cupola_hatch_half_height: 0.04,
            // Driver's hatch on the hull roof, front-left: ahead of the turret ring, on the flat roof
            // that the 60deg glacis cuts off at z ~= 1.95, so the lid stays clear of the slope.
            driver_hatch_center: Vec3::new(-0.45, 1.62, hull.half_len - 1.55),
            driver_hatch_radius: 0.17,
            driver_hatch_half_height: 0.05,
            // Loader's hatch ring on the turret roof, loader (right) side. Like the cupola it is
            // rooted deep into the curved dome (base ~2.04) so the lid never levitates.
            // Deep enough to root in the casting, shallow enough that the LID is not what a
            // tape measure finds when it asks how tall the bare turret roof is.
            loader_hatch_center: Vec3::new(0.36, roof - 0.085, 0.05),
            loader_hatch_radius: 0.19,
            loader_hatch_half_height: 0.12,
            // On the left fender front, outboard of the glacis, as every reference view shows.
            headlight_center: Vec3::new(-1.25, 1.225, hull.half_len - 0.45),
            headlight_radius: 0.10,
            headlight_half_height: 0.09,
            // Bow hooks at the lower corners of the narrow nose plate.
            tow_hook_center: Vec3::new(0.80, 0.55, hull.half_len - 0.38),
            tow_hook_half: Vec3::new(0.12, 0.11, 0.10),
        }),
        // Clean factory-fresh detailing only: a louvered rear-deck grille, a boxed left-fender
        // exhaust cover, two low turret-roof periscopes, fender lips and a restrained glacis weld
        // bead. No mud, rust, battle damage, decals or heavy weathering — all sit inside the already
        // validated hull/turret volumes and add nothing to collision, armour, mounts or the snapshot.
        detail: Some(DetailVisual {
            // The grille rides proud of the engine deck (deck top is y=1.59): at center_y 1.60 its
            // frame and slats top out at 1.63, clear of the deck plane. Earlier the grille top
            // was coplanar with the deck top, so the slats z-fought the deck into a flickering mess.
            grille_center: Vec3::new(0.0, 1.60, -hull.half_len + 1.00),
            grille_half: Vec3::new(0.80, 0.03, 0.55),
            grille_slats: 6,
            // The louvered exhaust box sits ON the left fender at the engine bay, as the top view
            // shows — the dark armoured cover among the left-fender stowage line.
            exhaust_center: Vec3::new(-1.34, 1.25, -hull.half_len + 2.10),
            exhaust_half: Vec3::new(0.26, 0.11, 0.45),
            // Turret-roof periscopes root into the curved dome (tall heads, bases ~2.02).
            // Model-logic audit #10: a Mk.4 head is a low fist-sized housing, not a chimney.
            // The centre sits ON the dome surface so only ~7 cm of head stands proud; the old
            // 0.24 m slab read as a floating plate with holes from the bow.
            // On the roof PLATE rather than out on the slope past its front edge: at z 0.55 the
            // casting has already fallen away, so the head sat in mid-air beside the metal it is
            // supposed to be set into.
            periscope_center: Vec3::new(0.34, roof - 0.02, 0.42),
            periscope_half: Vec3::new(0.055, 0.055, 0.055),
            // The pedestal drum centre; the DShK barrel rides near its top (+0.16).
            dshk_mount_center: Vec3::new(0.48, roof - 0.07, -0.18),
            // The DShK's barrel is a documented 1070 mm. It was 0.62 — sized to look plausible
            // beside a gun that was lying flat on the roof.
            dshk_barrel_length: 1.07,
            // Shallow folded edge: the belt's top run carries its link bodies up to ~1.02, so a
            // deeper lip curtain has the scrolling shoes cutting through it (lip bottom 1.05).
            fender_lip_drop: 0.05,
            fender_lip_thickness: 0.03,
            weld_seam_half_thickness: 0.015,
        }),
    }
}
