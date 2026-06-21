//! The authoritative hybrid visual dimensions for the T-54 benchmark — the single source the mesh
//! generators read. Material intent is a clean, factory-fresh vehicle: crisp cast and machined
//! surfaces with restrained manufactured detailing, and deliberately *no* mud, rust, battle damage,
//! decals or heavy weathering (that finish belongs to the material layer, not the geometry).

use glam::Vec3;

use super::{
    BoxVisual, DetailVisual, FenderVisual, FittingsVisual, GunVisual, HullPlatesVisual, HullVisual,
    HybridVisual, RunningGearVisual, TrackBeltVisual, TurretVisual,
};

pub(super) fn t54_hybrid() -> HybridVisual {
    HybridVisual {
        hull: HullVisual {
            half_width: 1.45,
            belly_y: 0.10,
            roof_y: 1.20,
            half_len: 3.00,
            glacis_offset: 1.85,
            nose_normal: Vec3::new(0.0, -0.5, 1.0),
            nose_offset: 2.55,
        },
        hull_plates: HullPlatesVisual {
            glacis_base_z: 2.85,
            nose_base_z: 2.55,
            deck_bevel: 0.10,
            sponson_overhang: 0.23,
        },
        turret: TurretVisual {
            dome_radius: 0.95,
            dome_front: Vec3::new(0.0, 1.67, 0.06),
            // The rear bustle is narrowed (smaller, pulled-back rear sphere) so the casting tapers to
            // a low, slim tail behind the ring — the front mass reads heavier than the rear.
            dome_rear: Vec3::new(0.0, 1.64, -0.30),
            dome_rear_radius: 0.70,
            dome_blend: 0.45,
            // Fuller, further-forward front cheeks flanking the mantlet: the T-54 turret's signature
            // heavy cast front mass, kept within the turret plan (the dome already fills the width).
            cheek_radius: 0.49,
            cheek_center: Vec3::new(0.48, 1.56, 0.56),
            cheek_blend: 0.40,
            ring_radius: 0.90,
            ring_half_height: 0.18,
            ring_center: Vec3::new(0.0, 1.48, 0.0),
            ring_blend: 0.32,
            roof_plane_y: 1.86,
            ring_plane_y: 1.30,
            cupola_radius: 0.24,
            cupola_half_height: 0.11,
            cupola_center: Vec3::new(-0.34, 1.84, -0.10),
            cupola_blend: 0.05,
            // A deeper, lower mantlet socket: a wider recess seated slightly lower on the front face,
            // so the cast trough the gun mantlet beds into reads as a real cavity, not a dimple.
            socket_radius: 0.38,
            socket_center: Vec3::new(0.0, 1.66, 1.04),
            socket_blend: 0.07,
            bbox_min: Vec3::new(-1.20, 1.25, -1.20),
            bbox_max: Vec3::new(1.20, 2.00, 1.45),
            budget: 12_000,
        },
        gun: GunVisual {
            barrel_radius: 0.09,
            muzzle_radius: 0.085,
            muzzle_taper: 0.20,
            barrel_segments: 20,
            // A substantial, stepped mask: the wide middle shoulder beds into the casting while
            // the narrower forward face carries the barrel sleeve. This must not collapse into a
            // thin, post-scaled oval.
            mantlet_profile: [
                (-0.28, 0.18),
                (-0.20, 0.29),
                (-0.08, 0.37),
                (0.05, 0.39),
                (0.18, 0.31),
                (0.28, 0.16),
            ],
            mantlet_segments: 20,
            mantlet_scale: Vec3::new(1.45, 0.72, 1.0),
            module_delta_scale: 0.65,
        },
        deck: BoxVisual { center: Vec3::new(0.0, 1.25, -1.75), half: Vec3::new(1.30, 0.06, 0.95) },
        fender: FenderVisual { side_x: 1.5, center_y: 1.22, half: Vec3::new(0.28, 0.03, 2.55) },
        running_gear: RunningGearVisual {
            // Wheel radius is sized so five wheels clear each other (even gaps wider than the wheel
            // diameter) and still seat just inside the belt; it is coupled to the belt radius below.
            wheel_radius: 0.34,
            wheel_half_width: 0.12,
            hub_radius: 0.18,
            hub_overhang: 0.02,
            wheel_segments: 20,
            hub_segments: 14,
            wheel_count: 5,
            // The blueprint owns the overall run. The hybrid places the visible wheel train inside
            // it, leaving room for separate idler and sprocket meshes at the belt ends.
            end_inset: 0.58,
            first_z: -2.10,
            // Even rear spacing with a larger characteristic gap before the front wheel; both stay
            // wider than the wheel diameter (0.68) so adjacent road wheels never intersect.
            spacing: 0.72,
            first_gap: 0.86,
            side_x: 1.5,
        },
        track_belt: TrackBeltVisual {
            front_z: 2.3,
            rear_z: -2.3,
            // The belt and the road wheels share this axle line (concentric), set so the bottom run
            // rests on the ground (axle_y - radius - half_thickness = 0) and the top run wraps over
            // the wheel tops under the fender — a track that grounds and wraps, not a buried ribbon.
            // radius - half_thickness (0.35) is the wheel seat; the road wheels (0.34) nest just
            // inside it, and the smaller loop keeps the idler/sprocket end wraps compact.
            axle_y: 0.51,
            radius: 0.43,
            side_x: 1.5,
            half_thickness: 0.08,
            half_width: 0.26,
            straight_segments: 6,
            arc_segments: 8,
            link_count: 56,
        },
        fittings: FittingsVisual {
            cupola_hatch_center: Vec3::new(-0.34, 1.98, -0.10),
            cupola_hatch_radius: 0.20,
            cupola_hatch_half_height: 0.04,
            headlight_center: Vec3::new(0.45, 1.10, 1.98),
            headlight_radius: 0.10,
            headlight_half_height: 0.07,
            tow_hook_center: Vec3::new(1.02, 0.32, 2.42),
            tow_hook_half: Vec3::new(0.12, 0.11, 0.10),
        },
        // Clean factory-fresh detailing only: a louvered rear-deck grille, a boxed left-fender
        // exhaust cover, two low turret-roof periscopes, fender lips and a restrained glacis weld
        // bead. No mud, rust, battle damage, decals or heavy weathering — all sit inside the already
        // validated hull/turret volumes and add nothing to collision, armour, mounts or the snapshot.
        detail: DetailVisual {
            grille_center: Vec3::new(0.0, 1.28, -1.95),
            grille_half: Vec3::new(0.85, 0.03, 0.55),
            grille_slats: 6,
            exhaust_center: Vec3::new(-1.50, 1.30, -1.55),
            exhaust_half: Vec3::new(0.10, 0.10, 0.55),
            periscope_center: Vec3::new(0.34, 1.88, 0.42),
            periscope_half: Vec3::new(0.07, 0.05, 0.07),
            dshk_mount_center: Vec3::new(0.48, 1.93, -0.18),
            dshk_barrel_length: 0.62,
            fender_lip_drop: 0.07,
            fender_lip_thickness: 0.03,
            weld_seam_half_thickness: 0.015,
        },
    }
}
