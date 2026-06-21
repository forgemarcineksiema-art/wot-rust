//! The authoritative hybrid visual dimensions for the T-54 benchmark — the single source the mesh
//! generators read. Material intent is a clean, factory-fresh vehicle: crisp cast and machined
//! surfaces with restrained manufactured detailing, and deliberately *no* mud, rust, battle damage,
//! decals or heavy weathering (that finish belongs to the material layer, not the geometry).

use glam::Vec3;

use super::{
    BoxVisual, DetailVisual, FenderVisual, FittingsVisual, GunVisual, HullPlatesVisual, HullVisual,
    HybridVisual, LoftStation, RunningGearVisual, TrackBeltVisual, TurretLoftVisual, TurretVisual,
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
            // The cast dome bulges LOW and wide: the sphere centre sits just above the ring (not high
            // in the band), so the widest cross-section overhangs the ring and the casting necks back
            // in toward the flat roof — the flattened "river-stone" T-54 pancake, not a tall round
            // pot. The wider radius (>ring_radius) is what gives the signature ring overhang/undercut.
            dome_radius: 0.91,
            dome_front: Vec3::new(0.0, 1.44, 0.10),
            // The rear bustle is a low, broad lobe reaching back behind the ring: a sphere set low
            // gives the casting a heavy overhanging tail rather than a narrow tapered tip.
            dome_rear: Vec3::new(0.0, 1.42, -0.27),
            dome_rear_radius: 0.74,
            dome_blend: 0.50,
            // Fuller, further-forward front cheeks flanking the mantlet at gun height: the T-54
            // turret's signature heavy cast front mass, fused into the dome with a wide blend so the
            // front reads as one continuous casting, not a separate ball stuck on the face.
            cheek_radius: 0.50,
            cheek_center: Vec3::new(0.42, 1.50, 0.52),
            cheek_blend: 0.52,
            // The ring is narrowed below the dome so the wider low cast dome visibly overhangs it —
            // the signature Soviet undercut — while the dome itself stays inside the ±1.00 turret
            // plan. (Visual ring only; the gameplay turret-ring radius lives in `TurretShape`.)
            ring_radius: 0.85,
            ring_half_height: 0.18,
            ring_center: Vec3::new(0.0, 1.48, 0.0),
            ring_blend: 0.30,
            roof_plane_y: 1.86,
            ring_plane_y: 1.30,
            cupola_radius: 0.24,
            cupola_half_height: 0.11,
            cupola_center: Vec3::new(-0.34, 1.84, -0.10),
            cupola_blend: 0.05,
            // A deeper, lower mantlet socket: a wider recess seated slightly lower on the front face,
            // so the cast trough the gun mantlet beds into reads as a real cavity, not a dimple.
            socket_radius: 0.38,
            socket_center: Vec3::new(0.0, 1.52, 1.04),
            socket_blend: 0.07,
            bbox_min: Vec3::new(-1.20, 1.25, -1.20),
            bbox_max: Vec3::new(1.20, 2.00, 1.45),
            budget: 12_000,
        },
        // The lofted cast turret: stations from the ring seat (1.30) up to the flat roof (1.86),
        // widest LOW (station 2) for the ring overhang, front-heavy (front > rear half-length) with a
        // rear-pulled bustle (negative z_center climbing with height), necking into the flat roof.
        // All within the ±1.00 / ±1.04 turret plan. Cheeks and the front gun embrasure ride as
        // localized radial modulations of the one surface.
        turret_loft: TurretLoftVisual {
            stations: [
                LoftStation {
                    y: 1.30,
                    half_width: 0.84,
                    half_len_front: 0.90,
                    half_len_rear: 0.90,
                    z_center: 0.00,
                },
                LoftStation {
                    y: 1.40,
                    half_width: 0.91,
                    half_len_front: 0.98,
                    half_len_rear: 0.94,
                    z_center: -0.02,
                },
                LoftStation {
                    y: 1.50,
                    half_width: 0.90,
                    half_len_front: 0.98,
                    half_len_rear: 0.88,
                    z_center: -0.03,
                },
                LoftStation {
                    y: 1.60,
                    half_width: 0.84,
                    half_len_front: 0.92,
                    half_len_rear: 0.80,
                    z_center: -0.05,
                },
                LoftStation {
                    y: 1.70,
                    half_width: 0.74,
                    half_len_front: 0.84,
                    half_len_rear: 0.66,
                    z_center: -0.08,
                },
                LoftStation {
                    y: 1.80,
                    half_width: 0.60,
                    half_len_front: 0.72,
                    half_len_rear: 0.52,
                    z_center: -0.11,
                },
                LoftStation {
                    y: 1.86,
                    half_width: 0.48,
                    half_len_front: 0.60,
                    half_len_rear: 0.42,
                    z_center: -0.12,
                },
            ],
            exponent: 2.8,
            segments: 64,
            roof_apex: Vec3::new(0.0, 1.865, -0.06),
            floor_apex: Vec3::new(0.0, 1.30, 0.0),
            cheek_amount: 0.10,
            cheek_azimuth: 0.70,
            cheek_y: 1.48,
            cheek_az_width: 0.42,
            cheek_y_width: 0.18,
            embrasure_amount: -0.10,
            embrasure_y: 1.54,
            embrasure_az_width: 0.50,
            embrasure_y_width: 0.18,
            cupola_center: Vec3::new(-0.34, 1.86, -0.10),
            cupola_radius: 0.24,
            cupola_half_height: 0.11,
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
            // Taller in Y than before so the mask fully overlaps the embrasure mouth through the
            // whole elevation arc (no exposed socket above/below the gun), while staying clearly
            // wider than tall — the flat cast oval, not a round ball.
            mantlet_scale: Vec3::new(1.45, 0.86, 1.0),
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
            // Driver's hatch on the hull roof, front-left: ahead of the turret ring, on the flat roof
            // that the 60deg glacis cuts off at z ~= 1.62, so the lid stays clear of the slope.
            driver_hatch_center: Vec3::new(-0.45, 1.20, 1.40),
            driver_hatch_radius: 0.17,
            driver_hatch_half_height: 0.05,
            // Loader's hatch on the turret roof, loader (right) side: between the right periscope and
            // the cupola, clear of the DShK pedestal, seated on the 1.86 roof plane.
            loader_hatch_center: Vec3::new(0.36, 1.88, 0.05),
            loader_hatch_radius: 0.19,
            loader_hatch_half_height: 0.04,
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
