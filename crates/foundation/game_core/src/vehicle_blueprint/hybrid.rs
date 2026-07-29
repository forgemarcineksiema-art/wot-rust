//! Authoritative *visual* dimensions for the hybrid generator path (currently the T-54 benchmark).
//!
//! The flat [`VehicleBlueprint`](super::VehicleBlueprint) fields carry the gameplay shape (hitbox,
//! mounts, armour slopes). [`HybridVisual`] carries everything the hybrid mesh generators
//! (`solid`, `sdf_mesh`, `revolve`) need on top of that — the convex hull block, the cast-turret SDF
//! composition, the barrel and mantlet profiles, the engine deck, the fenders, and the running gear.
//!
//! These types live here, in the lowest crate, so the generators read one source of truth rather
//! than each holding its own copy of a dimension. A generator takes the relevant sub-struct by
//! reference; nothing outside this struct is allowed to invent a T-54 dimension.

use glam::Vec3;

use super::{DetailVisual, FittingsVisual};

/// The convex hull block plus its two-plate front. The plate slopes (glacis/side/rear) are *not*
/// stored here — they are read from [`ArmorShape`](super::ArmorShape) so the visible rake is the same
/// number the penetration model uses ("what you see is what you shoot").
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HullVisual {
    pub half_width: f32,
    pub belly_y: f32,
    pub roof_y: f32,
    pub half_len: f32,
    /// Plane offset (distance from origin along the glacis normal) of the upper glacis plate.
    pub glacis_offset: f32,
    /// Lower nose plate: bevels the bottom-front edge into the T-54's two-plate front.
    pub nose_normal: Vec3,
    pub nose_offset: f32,
}

/// Visual parameters for the multi-plate hull (Stage 3). The plate *extents* come from the gameplay
/// [`HullShape`](super::HullShape) — the lower tub width, the sponson step, the deck height, the hull
/// length — and the plate *slopes* from [`ArmorShape`](super::ArmorShape), so the visible hull is the
/// reconciled-to-gameplay form. These fields add only what the shape model does not already carry:
/// where the two-plate front folds, and the small thickness/bevel/seam cues that make the plates read
/// as plates rather than one block.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HullPlatesVisual {
    /// Z where the upper glacis meets the lower nose plate, taken at the sponson step height — the
    /// fold line of the T-54 two-plate front.
    pub glacis_base_z: f32,
    /// Z of the lower nose plate at the belly: tucked back behind the fold, so the nose rakes under.
    pub nose_base_z: f32,
}

/// The cast turret as a Surface-Nets SDF: two offset spheres for the flattened dome, a seating ring,
/// flat roof/ring planes, the commander's cupola, and the recessed mantlet socket. Positions are in
/// vehicle-local space; the gun's moving mantlet belongs to the gun submesh, not the casting.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TurretVisual {
    pub dome_radius: f32,
    pub dome_front: Vec3,
    pub dome_rear: Vec3,
    /// Rear dome radius, smaller than the front, so the casting tapers to a lower narrower bustle.
    pub dome_rear_radius: f32,
    pub dome_blend: f32,
    /// Front cheek bulge flanking the mantlet (right side; mirrored to the left). The cast cheeks are
    /// the T-54 turret's signature front mass, distinct from the rounded rear.
    pub cheek_radius: f32,
    pub cheek_center: Vec3,
    pub cheek_blend: f32,
    pub ring_radius: f32,
    pub ring_half_height: f32,
    pub ring_center: Vec3,
    pub ring_blend: f32,
    /// Flat machined planes capping the casting: roof (upper) and ring seat (lower).
    pub roof_plane_y: f32,
    pub ring_plane_y: f32,
    pub cupola_radius: f32,
    pub cupola_half_height: f32,
    pub cupola_center: Vec3,
    pub cupola_blend: f32,
    pub socket_radius: f32,
    pub socket_center: Vec3,
    pub socket_blend: f32,
    /// Tight world-space meshing box for the casting.
    pub bbox_min: Vec3,
    pub bbox_max: Vec3,
    /// Triangle budget the casting meshes to.
    pub budget: usize,
}

/// One horizontal station of a lofted turret casting: a superellipse outline at height `y`, with
/// separate front (`+Z`) and rear (`-Z`) half-lengths so the casting reads front-heavy with a
/// tapered rear bustle.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LoftStation {
    pub y: f32,
    pub half_width: f32,
    pub half_len_front: f32,
    pub half_len_rear: f32,
    pub z_center: f32,
}

impl LoftStation {
    /// The station's outline point at azimuth `t` (radians; `0` = `+X`, `PI/2` = `+Z` front),
    /// before bumps — the superellipse the `cast_loft` kernel skins.
    ///
    /// This lives here, next to the data, because the ARMOUR has to know the shape too. The
    /// armour volume that a shell is resolved against is built from these same stations
    /// (`armor::vehicle_volumes`), so the steel a player shoots at is the steel they can see.
    /// The two evaluations agreeing is not left to trust: `t54_turret_armor_lock` measures the
    /// finished mesh against the finished volume.
    pub fn outline(&self, azimuth: f32, exponent: f32) -> Vec3 {
        let e = 2.0 / exponent;
        let superlerp = crate::math::superlerp;
        let (sin, cos) = azimuth.sin_cos();
        let half_len = if sin >= 0.0 { self.half_len_front } else { self.half_len_rear };
        Vec3::new(
            self.half_width * superlerp(cos, e),
            self.y,
            self.z_center + half_len * superlerp(sin, e),
        )
    }
}

/// A cast turret built by **lofting** the stations below into one continuous skinned shell, with a
/// symmetric cheek pair and a front gun embrasure as localized radial modulations. This replaces the
/// metaball [`TurretVisual`] composition with a controlled, *designed* surface that reads as one
/// casting from every angle. The cupola and the moving mantlet stay separate bedded parts.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TurretLoftVisual {
    /// Cross-sections, ring seat (bottom) to roof (top).
    pub stations: [LoftStation; 10],
    /// Superellipse fullness (`2.0` = ellipse, `>2.0` = fuller cast shoulders).
    pub exponent: f32,
    /// Azimuth samples per ring.
    pub segments: usize,
    /// Symmetric front cheeks: a swell at the front azimuth `±cheek_azimuth`.
    pub cheek_amount: f32,
    pub cheek_azimuth: f32,
    pub cheek_y: f32,
    pub cheek_az_width: f32,
    pub cheek_y_width: f32,
    /// Front gun embrasure: an inward recess (negative amount) the gun comes through.
    pub embrasure_amount: f32,
    pub embrasure_y: f32,
    pub embrasure_az_width: f32,
    pub embrasure_y_width: f32,
    /// How sharply the embrasure's walls stand up (`cast_loft::CastBump::falloff_exponent`).
    ///
    /// `2.0` is a Gaussian dish. An aperture is not a dish: it has a floor and a rim, and the
    /// difference between the two is the whole reason a viewer reads one as a hole in armour
    /// and the other as a dent in it. The cheeks keep the Gaussian — a cast swell IS soft.
    pub embrasure_falloff: f32,
    /// The WINDOW: the wide, shallow rectangular recess the casting carries between its cheeks,
    /// which the canvas cover is fastened over. The narrow embrasure above is cut through the
    /// window's floor.
    ///
    /// Two steps, because the vehicle has two: the ~0.40 m armour aperture the dossier names is
    /// the inner hole, and what the EYE reads on a T-54's face is the outer rectangle. Built as
    /// one round pocket, the front read as the old ball mantlet shrunk and sunk — the player's
    /// verdict, and correct.
    pub window_amount: f32,
    pub window_az_width: f32,
    pub window_y_width: f32,
    pub window_falloff: f32,
    /// The commander's cupola drum, raised proud of the roof (the hatch lid is a separate fitting).
    /// The metaball turret blended this into the casting; the lofted shell carries it as its own part.
    pub cupola_center: Vec3,
    pub cupola_radius: f32,
    pub cupola_half_height: f32,
}

impl TurretLoftVisual {
    /// How far the casting reaches in direction `normal`: the support function of the lofted
    /// shell, sampled over its stations and around each station's outline (bumps included).
    ///
    /// This is what lets the armour volume BE the casting instead of a circle drawn near it. A
    /// plane with this normal, placed at this offset, touches the metal at its furthest point in
    /// that direction and encloses every other point — which is precisely the two halves of the
    /// honesty doctrine: nothing visible is outside the armour, and no armour stands in air.
    ///
    /// The turret's own frame: `y` as authored (ground-relative), `z` about the ring.
    pub fn support(&self, normal: Vec3) -> f32 {
        /// Azimuth samples per station. The mesh itself uses `segments` (24 on the T-54); this
        /// samples finer so the support never falls short of a mesh vertex sitting between two
        /// of them.
        const SAMPLES: usize = 240;
        let mut support = f32::NEG_INFINITY;
        for station in &self.stations {
            for index in 0..SAMPLES {
                let azimuth = index as f32 / SAMPLES as f32 * std::f32::consts::TAU;
                let mut point = station.outline(azimuth, self.exponent);
                let push = self.radial_push(azimuth, station.y);
                if push != 0.0 {
                    let radial =
                        Vec3::new(point.x, 0.0, point.z - station.z_center).normalize_or_zero();
                    point += radial * push;
                }
                support = support.max(normal.dot(point));
            }
        }
        support
    }

    /// The casting's localized radial modulation at `(azimuth, y)`: the two cheek swells and the
    /// gun embrasure recess. Mirrors `cast_loft::CastBump::push` for the bumps this shell
    /// carries — the loft builder feeds the kernel exactly these three.
    fn radial_push(&self, azimuth: f32, y: f32) -> f32 {
        let front = std::f32::consts::FRAC_PI_2;
        // Super-Gaussian, exactly as `cast_loft::CastBump::push` computes it: `exp(-|t|^n)`.
        // The two evaluations have to agree term for term, because one of them is the mesh the
        // player looks at and the other is the armour they shoot at.
        let bump =
            |center: f32, az_width: f32, center_y: f32, y_width: f32, amount: f32, falloff: f32| {
                let delta = crate::math::wrap_angle(azimuth - center);
                let az = (-(delta / az_width).abs().powf(falloff)).exp();
                let height = (-((y - center_y) / y_width).abs().powf(falloff)).exp();
                amount * az * height
            };
        let gaussian = |center: f32, az_width: f32, center_y: f32, y_width: f32, amount: f32| {
            bump(center, az_width, center_y, y_width, amount, 2.0)
        };
        gaussian(
            front - self.cheek_azimuth,
            self.cheek_az_width,
            self.cheek_y,
            self.cheek_y_width,
            self.cheek_amount,
        ) + gaussian(
            front + self.cheek_azimuth,
            self.cheek_az_width,
            self.cheek_y,
            self.cheek_y_width,
            self.cheek_amount,
        ) + bump(
            front,
            self.embrasure_az_width,
            self.embrasure_y,
            self.embrasure_y_width,
            self.embrasure_amount,
            self.embrasure_falloff,
        ) + bump(
            front,
            self.window_az_width,
            self.embrasure_y,
            self.window_y_width,
            self.window_amount,
            self.window_falloff,
        )
    }
}

/// The gun: a revolved steel barrel (driven by the installed module's length) and the moving cast
/// mantlet mask. The barrel dimensions are the hybrid visual ones — distinct from the legacy-recipe
/// `GunShape`, which feeds the older `vehicle_geometry` path.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GunVisual {
    pub barrel_radius: f32,
    pub muzzle_radius: f32,
    /// Half the gun's CALIBRE — the hole down the middle.
    ///
    /// It used to be a fraction of the outside diameter (`muzzle_radius * 0.55`), which is
    /// backwards: a gun is a bore with steel wrapped round it, not an outside with a dent. On a
    /// 100 mm D-10T this is 0.050 m, full stop, and the wall thickness is then whatever
    /// `muzzle_radius` leaves — which is how you can tell at a glance whether the tube is the
    /// right thickness.
    pub bore_radius: f32,
    pub muzzle_taper: f32,
    pub barrel_segments: usize,
    /// Mantlet side profile as `(z, radius)` points, revolved about Z then scaled to a flat oval.
    ///
    /// Trunnion-relative. Both ends must reach `radius == 0`: the mantlet is a cast BODY, and a
    /// body has a back and a front. Written as a sleeve open at both ends it was neither — a
    /// tube whose rims stood in mid-air where nothing met them.
    pub mantlet_profile: [(f32, f32); 8],
    pub mantlet_segments: usize,
    pub mantlet_scale: Vec3,
    /// The canvas cover's SLEEVE: `(z, radius)` stations on the barrel axis, trunnion-relative,
    /// from inside the panel's mouth out to the strap that grips the tube.
    ///
    /// A vehicle with an INTERNAL mantlet has a hole in its turret face, and something has to
    /// close it or the turret is open to the weather and to the eye. On a T-54 that something is
    /// a proofed canvas COVER: a rectangular panel fastened over the whole window, gathering
    /// through radial folds into this short sleeve. The first build drew only a round boot — and
    /// a round boot in a round pocket reads as the old ball mantlet, shrunk and swallowed, which
    /// is exactly the verdict it got.
    pub mantlet_cover: [(f32, f32); 4],
    /// The panel's frame half-extents `(x, y)` where the fabric is fastened into the window.
    /// Sized to the window at half-depth; `the_cover_frame_matches_the_window` holds the two
    /// together, because a frame wider than its window is fabric bolted to air.
    pub cover_frame_half: (f32, f32),
    /// How far the cover sags between its two clamps, in metres. Fabric is not a tube.
    pub mantlet_cover_sag: f32,
    /// How much of a gun module's length delta the muzzle moves by (visual modularity scale).
    pub module_delta_scale: f32,
}

/// An axis-aligned box part (engine deck), as centre + half-extents.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BoxVisual {
    pub center: Vec3,
    pub half: Vec3,
}

/// A fender (mudguard) plate riding above one track run, mirrored to both sides at `±side_x`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FenderVisual {
    pub side_x: f32,
    pub center_y: f32,
    pub half: Vec3,
}

/// The full hybrid visual description for one vehicle. `Some` only for the hybrid benchmark (T-54);
/// other vehicles bake through the legacy `vehicle_geometry` path and carry `None`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HybridVisual {
    pub hull: HullVisual,
    pub hull_plates: HullPlatesVisual,
    pub turret: TurretVisual,
    /// The lofted cast turret — the controlled-surface replacement for the metaball `turret`. Both
    /// are carried during migration; the bake selects which one feeds the turret submesh.
    pub turret_loft: TurretLoftVisual,
    pub gun: GunVisual,
    pub deck: BoxVisual,
    pub fender: FenderVisual,
    pub fittings: FittingsVisual,
    pub detail: DetailVisual,
}
