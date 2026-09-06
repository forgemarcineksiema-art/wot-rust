//! Authoritative *visual* dimensions for the hybrid generator path (currently the T-54 benchmark).
//!
//! The flat [`VehicleBlueprint`](super::VehicleBlueprint) fields carry the gameplay shape (hitbox,
//! mounts, armour slopes). [`VisualDetail`] carries everything the hybrid mesh generators
//! (`solid`, `cast_loft`, `panel`, `revolve`, `sweep`) need on top of that — the convex hull block,
//! the cast-turret loft stations and the machined seat they sit on, the barrel and mantlet profiles,
//! the engine deck, the fenders, and the running gear. (The metaball `sdf_mesh` composition was
//! deleted 2026-08-02; `TurretVisual` keeps the four numbers that survived it.)
//!
//! These types live here, in the lowest crate, so the generators read one source of truth rather
//! than each holding its own copy of a dimension. A generator takes the relevant sub-struct by
//! reference; nothing outside this struct is allowed to invent a T-54 dimension.

use glam::Vec3;

use super::{DetailVisual, FittingsVisual};

/// The convex hull block plus its two-plate front. The plate slopes (glacis/side/rear) are *not*
/// stored here — they are read from [`ArmorShape`](super::ArmorShape) so the visible rake is the same
/// number the penetration model uses ("what you see is what you shoot").
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
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
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct HullPlatesVisual {
    /// Z where the upper glacis meets the lower nose plate, taken at the sponson step height — the
    /// fold line of the T-54 two-plate front.
    pub glacis_base_z: f32,
    /// Z of the lower nose plate at the belly: tucked back behind the fold, so the nose rakes under.
    pub nose_base_z: f32,
}

/// Machined reference planes of the turret seat, plus the casting's triangle budget — what
/// SURVIVES of the old metaball composition (deleted 2026-08-02). The shipped casting is the
/// loft below; these four numbers are the fields production code still reads: the ring-seam
/// AO band and the ring collar sit on `ring_plane_y`/`ring_radius`, the mantlet-seat AO band
/// on `socket_center`, and the bake holds the casting to `budget`.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct TurretVisual {
    pub ring_radius: f32,
    /// The flat machined ring seat the casting sits on.
    pub ring_plane_y: f32,
    /// Centre of the recessed mantlet socket on the fire line.
    pub socket_center: Vec3,
    /// Triangle budget the casting meshes to.
    pub budget: usize,
}

/// One horizontal station of a lofted turret casting: a superellipse outline at height `y`, with
/// separate front (`+Z`) and rear (`-Z`) half-lengths so the casting reads front-heavy with a
/// tapered rear bustle.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
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
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct TurretLoftVisual {
    /// Cross-sections, skirt lip (bottom, below the ring plane — the casting overhangs its
    /// race) to roof (top).
    pub stations: [LoftStation; 11],
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

    /// A point on (or `standoff` metres proud of) the loft's superellipse family at height `y`
    /// and azimuth `phi` (0 = forward +Z, positive toward +X) — WITHOUT the bumps. This is the
    /// bare ring the kit lines used to duplicate privately in `vehicle_build`; it lives here so
    /// the shell, the armour and every fitting that follows the casting read ONE family.
    pub fn ring_point(&self, y: f32, phi: f32, standoff: f32) -> Vec3 {
        let s = &self.stations;
        let above = s.iter().position(|st| st.y >= y).unwrap_or(s.len() - 1).max(1);
        let (a, b) = (&s[above - 1], &s[above]);
        let t = ((y - a.y) / (b.y - a.y).max(1.0e-4)).clamp(0.0, 1.0);
        let lerp = |p: f32, q: f32| p + (q - p) * t;
        let half_width = lerp(a.half_width, b.half_width);
        let z_center = lerp(a.z_center, b.z_center);
        let (dx, dz) = (phi.sin(), phi.cos());
        let half_len = if dz >= 0.0 {
            lerp(a.half_len_front, b.half_len_front)
        } else {
            lerp(a.half_len_rear, b.half_len_rear)
        };
        let n = self.exponent;
        let scale =
            ((dx.abs() / half_width).powf(n) + (dz.abs() / half_len).powf(n)).powf(-1.0 / n);
        Vec3::new(dx * (scale + standoff), y, z_center + dz * (scale + standoff))
    }

    /// [`Self::ring_point`] WITH the casting's bumps: where the surface actually is, cheeks and
    /// embrasure included. A fitting that follows the casting has to follow this, not the bare
    /// ring — the mould line drawn off the bare family sat ~50 mm inside the cheek plateau and
    /// hung ~70 mm off the gun window, paying 480 triangles for a feature that read as an error.
    ///
    /// With every bump amount at zero this IS `ring_point`, term for term — the equivalence the
    /// rails rely on, pinned by `surface_point_without_bumps_is_the_bare_ring`.
    pub fn surface_point(&self, y: f32, phi: f32, standoff: f32) -> Vec3 {
        let bare = self.ring_point(y, phi, standoff);
        // The bump family speaks the outline's PARAMETER azimuth — the angle the kernel walks
        // when it skins the stations — not the geometric direction angle `phi`. On a circle the
        // two coincide; on the superellipse they diverge hardest just off the axes, and that
        // divergence is where this used to lie: the bumps were evaluated at the geometric
        // angle, so the analytic surface disagreed with the skinned mesh by up to ~40 mm at
        // the gun window's flank (the old blunt-front table kept it under the seam lock's
        // tolerance; the forward-registered egg pushed it out into the open). The parameter is
        // recovered exactly from the bare point through the superellipse identity:
        // |x/w|^n + |z'/l|^n = 1 with x = w·|cos t|^(2/n) gives cos t = (|x|/w)^(n/2),
        // sin t = (|z'|/l)^(n/2), and cos²+sin² = 1 lands for free.
        let s = &self.stations;
        let above = s.iter().position(|st| st.y >= y).unwrap_or(s.len() - 1).max(1);
        let (a, b) = (&s[above - 1], &s[above]);
        let t = ((y - a.y) / (b.y - a.y).max(1.0e-4)).clamp(0.0, 1.0);
        let lerp = |p: f32, q: f32| p + (q - p) * t;
        let half_width = lerp(a.half_width, b.half_width);
        let z_local = bare.z - self.station_z_center(y);
        let half_len = if z_local >= 0.0 {
            lerp(a.half_len_front, b.half_len_front)
        } else {
            lerp(a.half_len_rear, b.half_len_rear)
        };
        let half = self.exponent * 0.5;
        let cos_t = (bare.x.abs() / half_width).clamp(0.0, 1.0).powf(half).copysign(bare.x);
        let sin_t = (z_local.abs() / half_len).clamp(0.0, 1.0).powf(half).copysign(z_local);
        let push = self.radial_push(sin_t.atan2(cos_t), y);
        if push == 0.0 {
            return bare;
        }
        let radial = Vec3::new(bare.x, 0.0, z_local).normalize_or_zero();
        bare + radial * push
    }

    /// The interpolated station centreline at height `y` — the axis `surface_point` pushes from.
    fn station_z_center(&self, y: f32) -> f32 {
        let s = &self.stations;
        let above = s.iter().position(|st| st.y >= y).unwrap_or(s.len() - 1).max(1);
        let (a, b) = (&s[above - 1], &s[above]);
        let t = ((y - a.y) / (b.y - a.y).max(1.0e-4)).clamp(0.0, 1.0);
        a.z_center + (b.z_center - a.z_center) * t
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
        // ONE face plateau, falloff 6 — exactly what the mesh builder feeds `cast_loft`
        // (`t54_turret_loft.rs`: a single `CastBump::plateau` at the front). This used to be a
        // PAIR of gaussians at `front ± cheek_azimuth`, and with `cheek_azimuth` authored at 0
        // the pair collapsed onto one spot and DOUBLE-COUNTED the amount: the armour volume
        // carried a +0.100 face where the metal carries +0.050 — four to five centimetres of
        // phantom armour across the whole face plateau, hidden exactly under the 0.05 m
        // tolerance of `the_armour_dome_does_not_stand_proud_of_the_casting`. The mould-line
        // lock caught it the day the seam started following the surface: the seam landed on the
        // armour's face and hung 45 mm off the metal's.
        let _ = gaussian;
        let _ = self.cheek_azimuth;
        bump(front, self.cheek_az_width, self.cheek_y, self.cheek_y_width, self.cheek_amount, 6.0)
            + bump(
                front,
                self.embrasure_az_width,
                self.embrasure_y,
                self.embrasure_y_width,
                self.embrasure_amount,
                self.embrasure_falloff,
            )
            + bump(
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
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
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
    /// The canvas COVER over the gun window — an INTERNAL-mantlet vehicle's part (F5.iii-a:
    /// optional, because a vehicle with an external mantlet has no window to cover, and a
    /// mandatory field would force it to author fabric it does not have).
    #[serde(default)]
    pub canvas: Option<CanvasCoverVisual>,
    /// The muzzle BRAKE, when the gun wears one (the Tiger's double-baffle KwK 36; the D-10T
    /// carries none). `None` is a statement, not an omission — the bore-honest muzzle face is
    /// then the end of the tube.
    #[serde(default)]
    pub muzzle_brake: Option<MuzzleBrakeVisual>,
    /// How much of a gun module's length delta the muzzle moves by (visual modularity scale).
    pub module_delta_scale: f32,
}

/// The proofed canvas cover fastened over an internal mantlet's gun window.
///
/// A vehicle with an INTERNAL mantlet has a hole in its turret face, and something has to
/// close it or the turret is open to the weather and to the eye. On a T-54 that something is
/// a canvas COVER: a rectangular panel fastened over the whole window, gathering through
/// radial folds into a short sleeve. The first build drew only a round boot — and a round boot
/// in a round pocket reads as the old ball mantlet, shrunk and swallowed, which is exactly the
/// verdict it got.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct CanvasCoverVisual {
    /// The cover's SLEEVE: `(z, radius)` stations on the barrel axis, trunnion-relative, from
    /// inside the panel's mouth out to the strap that grips the tube.
    pub sleeve: [(f32, f32); 4],
    /// The panel's frame half-extents `(x, y)` where the fabric is fastened into the window.
    /// Sized to the window at half-depth; `the_cover_frame_matches_the_window` holds the two
    /// together, because a frame wider than its window is fabric bolted to air.
    pub frame_half: (f32, f32),
    /// How far the cover sags between its two clamps, in metres. Fabric is not a tube.
    pub sag: f32,
}

/// A revolved muzzle brake at the end of the tube: the chamber body, its baffle count, and the
/// front plate the bore pierces. One radius and one length, because that is what the eye reads
/// at combat range — the baffles are rings the profile dips between.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct MuzzleBrakeVisual {
    /// Outer radius of the brake body (proud of the tube).
    pub radius: f32,
    /// Length along the barrel axis; the tube ends where the brake begins.
    pub length: f32,
    /// Baffle chambers (the KwK 36 wears two).
    pub baffles: usize,
}

/// An axis-aligned box part (engine deck), as centre + half-extents.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct BoxVisual {
    pub center: Vec3,
    pub half: Vec3,
}

/// A fender (mudguard) plate riding above one track run, mirrored to both sides at `±side_x`.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct FenderVisual {
    pub side_x: f32,
    pub center_y: f32,
    pub half: Vec3,
}

/// The visual-detail description for one vehicle — the FLEET slot (W4): any vehicle may carry
/// one, and a vehicle carries only the PARTS it has authored (F5.i). The slot is a sum of
/// optional part vocabularies, not a bundle: a welded-turret vehicle authors its gun group and
/// plate cues without pretending to be a casting, and a missing part simply falls back to that
/// vehicle's recipe geometry. Today the benchmark (T-54) authors every part; `#[serde(default)]`
/// on each field means a RON author states only what exists.
#[derive(Debug, Clone, Copy, PartialEq, Default, serde::Serialize, serde::Deserialize)]
pub struct VisualDetail {
    #[serde(default)]
    pub hull: Option<HullVisual>,
    #[serde(default)]
    pub hull_plates: Option<HullPlatesVisual>,
    #[serde(default)]
    pub turret: Option<TurretVisual>,
    /// The lofted cast turret — the controlled-surface replacement for the metaball `turret`. Both
    /// are carried during migration; the bake selects which one feeds the turret submesh.
    #[serde(default)]
    pub turret_loft: Option<TurretLoftVisual>,
    #[serde(default)]
    pub gun: Option<GunVisual>,
    #[serde(default)]
    pub deck: Option<BoxVisual>,
    #[serde(default)]
    pub fender: Option<FenderVisual>,
    #[serde(default)]
    pub fittings: Option<FittingsVisual>,
    #[serde(default)]
    pub detail: Option<DetailVisual>,
    /// How the hull is BUILT, when the visual file says: a welded slab hull takes its plates
    /// from the library straight off the blueprint (Forge 2.0 K3, step 4a) instead of the
    /// recipe's extrusions. `None` keeps whatever the vehicle's path draws. Appended 2026-09-05.
    #[serde(default)]
    pub construction: Option<HullConstruction>,
    /// The welded box turret's proportions, when the library builds it (Forge 2.0 K3, step 4b):
    /// the armour numbers (`TurretShape`: plan half-width/length, ring, roof, slopes, cupola)
    /// stay in the blueprint; this is only where the horseshoe bends. Appended 2026-09-05.
    #[serde(default)]
    pub welded_turret: Option<WeldedTurretVisual>,
    /// Which of the German family's deck furniture this vehicle wears when the library builds
    /// its deck (Forge 2.0 K3, the Tiger II); `None` builds no German deck (a Soviet hull wears
    /// `soviet_deck`). Appended 2026-09-06.
    #[serde(default)]
    pub german_deck: Option<GermanDeckVisual>,
    /// The welded-in casemate's furniture, when the library builds a casemate vehicle (Forge
    /// 2.0 K3, the Jagdtiger): the prism itself is the blueprint's (`TurretForm::Casemate`);
    /// this is what stands on it. A vehicle authors a casemate OR a welded turret. Appended
    /// 2026-09-06.
    #[serde(default)]
    pub casemate: Option<CasemateVisual>,
    /// The cast dome turret's roof and furniture, when the library builds a cast-dome vehicle
    /// outside the benchmark (Forge 2.0 K3, the Soviet family and the Centurion): the dome
    /// itself is the blueprint's ring/roof loft; this is what stands on it. Appended 2026-09-06.
    #[serde(default)]
    pub cast_dome: Option<CastDomeVisual>,
    /// The Soviet deck's furniture — louvres, the V-2's exhaust ports, the glacis furniture, the
    /// IS fender line and drums — when the library builds it. Appended 2026-09-06.
    #[serde(default)]
    pub soviet_deck: Option<SovietDeckVisual>,
}

/// What stands on a cast dome (`vehicle_build::cast_dome_parts`): the dome is the blueprint's
/// (`TurretShape` ring, base radius, plan length, roof); the roof furniture is per vehicle.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct CastDomeVisual {
    /// Azimuth samples of the dome loft.
    pub segments: u8,
    /// Which roof furniture the dome wears.
    pub roof: CastRoofKind,
    pub ring_height: f32,
    pub ring_segments: u8,
    pub socket_segments: u8,
    /// A stowage bin closing the rear of the turret plan (the Centurion's bustle bin), or none.
    pub bustle_bin: Option<BustleBinVisual>,
}

/// The roof furniture a cast dome wears — per vehicle, from the photos (one cloned cupola drum
/// used to top every cast turret across three nations). Append-only.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum CastRoofKind {
    /// No cupola: two flush hatches and the commander's periscope (the IS-3).
    Is3,
    /// The slit-ring cupola with a split lid, built to the authored `cupola_height`, and the
    /// loader's flush hatch (the T-34-85).
    T3485,
    /// The wide British cupola with sight hoods and the loader's flush hatch (the Centurion).
    Centurion,
}

/// A stowage bin closing the rear of a turret plan: standing `rise` over the ring seat, its
/// centre `back_inset` ahead of the plan's rear, with the given half-extents.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct BustleBinVisual {
    pub rise: f32,
    pub back_inset: f32,
    pub half: (f32, f32, f32),
}

/// Which of the Soviet family's deck furniture a vehicle wears (`vehicle_build::soviet_deck_parts`).
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct SovietDeckVisual {
    /// Transverse louvre strips over the engine bay.
    pub louvres: u8,
    /// The height of the twin round exhaust ports on the sloped stern.
    pub exhaust_port_y: f32,
    /// The driver's hatch IN the glacis with its twin periscope hoods: `(x, y)` on the plate.
    pub glacis_hatch: Option<(f32, f32)>,
    /// The hull MG ball in the glacis: `(x, y)` on the plate.
    pub glacis_mg_ball: Option<(f32, f32)>,
    /// The IS family's fender line: shelf, front mudguard, rear flap.
    pub is_fenders: bool,
    /// External cylindrical fuel drums along the rear fender shelves.
    pub fuel_drums: bool,
}

/// The horseshoe's plan, relative to the blueprint's turret: the flat front plate spans
/// `±cheek_x`, the cheeks open to `plan_half_width` over `cheek_setback`, the side walls run to
/// `wall_end_behind_ring` behind the ring centre, the bustle bends through two facets (x, z above
/// the bin's front face) to the centreline, and the stowage bin claims the last `bin_depth` of
/// the armour prism, its back face ON the rear armour plane.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct WeldedTurretVisual {
    pub cheek_x: f32,
    pub cheek_setback: f32,
    pub wall_end_behind_ring: f32,
    pub bustle: [(f32, f32); 2],
    pub bin_depth: f32,
    pub bin_half_width: f32,
    pub bin_half_height: f32,
    pub bin_rise: f32,
    pub ring_height: f32,
    pub ring_segments: u8,
    pub socket_segments: u8,
    /// A FLAT rear plate of this half-width ON the rear armour plane (the Henschel turret's)
    /// instead of the two-facet bustle to the centreline; `bustle` is then unread. Appended
    /// 2026-09-06.
    #[serde(default)]
    pub flat_rear_half_width: Option<f32>,
    /// The side and rear walls retreat at the roof by the blueprint's own `side_slope_deg` and
    /// `rear_slope_deg` — the armour's leaned planes — instead of standing vertical on the plan.
    /// Appended 2026-09-06.
    #[serde(default)]
    pub leaned_walls: bool,
    /// The mantlet socket flattened to an oval `(x, y)` scale — the Turmblende's wide band seat —
    /// instead of the round collar. Appended 2026-09-06.
    #[serde(default)]
    pub socket_scale: Option<(f32, f32)>,
}

/// Hull constructions the part library builds from a blueprint alone. Append-only.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum HullConstruction {
    /// Welded rolled plates: a tub between the belts, an upper box on the sponson whose sides
    /// lean the armour table's `hull_side` degrees (vertical on the Tiger I, 25° on the Tiger
    /// II), and the bow shelf's wedge when `ArmorShape::hull_bow_shelf` is authored — the
    /// German line.
    WeldedSlab,
}

/// Which of the German family's deck furniture a welded slab vehicle wears
/// (`vehicle_build::german_deck_parts`). The defaults are the late Tiger I's layout — the
/// family's first library deck — for the fields a file leaves out.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct GermanDeckVisual {
    /// Three-sided armoured shields round the exhaust stacks on a near-vertical stern (the late
    /// Tiger I); without them the stacks stand open, dark-mouthed, off the leaned stern on a
    /// top bracket (the Tiger II).
    pub exhaust_shields: bool,
    /// Spare links racked across the lower bow plate (the Tiger I carries four; the Tiger II
    /// hangs its spares on the turret walls — none here).
    pub spare_links: u8,
    /// The driver's slot visor on a near-vertical driver's plate (the Tiger I).
    pub driver_visor: bool,
    /// The driver's periscope hood riding the glacis line at the deck's front edge (the Tiger
    /// II, whose 50° glacis has no visor).
    pub periscope_hood: bool,
    /// Hinged fender flaps over the stern wrap as well as the bow (the Tiger I's rear mudflaps).
    pub rear_flaps: bool,
    /// The driver's TWIN periscope hoods at the roof's front edge, left, each with its glass
    /// (the Panther). Appended 2026-09-06.
    #[serde(default)]
    pub twin_periscopes: bool,
    /// The CURVED fender sweep over the front sprockets — three chained slanted segments — in
    /// place of the family's single drooping flap (the Panther). Appended 2026-09-06.
    #[serde(default)]
    pub curved_sweep: bool,
    /// Where the bow MG ball sits on the driver's plate, `(x, y)`; `None` is the Tiger's station
    /// (x -0.62, y 1.58). Appended 2026-09-06.
    #[serde(default)]
    pub mg_ball: Option<(f32, f32)>,
    /// LARGE FLAT bow guards over the front sprockets with a downturned front lip (the
    /// Jagdtiger's read) in place of the flap or the sweep. Appended 2026-09-06.
    #[serde(default)]
    pub flat_bow_guards: bool,
    /// Hull-flank stowage on the leaned sponson plates — the tow cable, the jack and its timber
    /// block, the tool box (the Jagdtiger's enormous bare flank). Appended 2026-09-06.
    #[serde(default)]
    pub flank_stowage: bool,
}

/// What stands on a welded-in casemate (`vehicle_build::casemate_parts`): the prism is the
/// blueprint's `TurretShape` (a `Casemate` form — plan, slopes, the cupola station reused for
/// the commander's periscope housing); the furniture and the cast gun collar are here.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct CasemateVisual {
    /// Half the height of the commander's low periscope housing at the cupola station.
    pub periscope_housing_half_height: f32,
    /// The ventilator dome on the roof: `(x, z ahead of the ring centre, half-width)`.
    pub ventilator: Option<(f32, f32, f32)>,
    /// The two flush crew hatches on the roof, mirrored: `(x, z behind the ring centre, radius)`.
    pub hatches: Option<(f32, f32, f32)>,
    /// The cast collar's rings, root first: `(radius, stand-off ahead of the face plane)`; the
    /// root ring sits ON the plate.
    pub collar: [(f32, f32); 4],
    /// The collar squashed in Y (an oval wider than tall, so it stays on the face).
    pub collar_y_scale: f32,
    pub collar_segments: u8,
    /// Spare-shoe rows racked on the side walls, or none.
    pub racks: Option<ShoeRackVisual>,
}

/// A row of spare shoes on a carrier rail, hung on a leaning wall so each shoe's outer face
/// lies ON the armour plane.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ShoeRackVisual {
    pub row_y_above_ring: f32,
    pub half_y: f32,
    pub thickness: f32,
    pub shoes: u8,
    pub pitch: f32,
    /// The first shoe's z relative to the ring centre (+Z forward).
    pub first_z: f32,
}

impl Default for GermanDeckVisual {
    fn default() -> Self {
        Self {
            exhaust_shields: true,
            spare_links: 4,
            driver_visor: true,
            periscope_hood: false,
            rear_flaps: true,
            twin_periscopes: false,
            curved_sweep: false,
            mg_ball: None,
            flat_bow_guards: false,
            flank_stowage: false,
        }
    }
}

/// Every part of a [`VisualDetail`], unwrapped — the view a FULLY-authored consumer stack
/// reads. The benchmark's construction modules each take their sub-struct by reference; this
/// view is the one place the "every part is present" claim is made, so the modules stay free
/// of per-field unwrapping and a partially-authored vehicle simply never yields the view.
#[derive(Debug, Clone, Copy)]
pub struct CompleteVisual<'a> {
    pub hull: &'a HullVisual,
    pub hull_plates: &'a HullPlatesVisual,
    pub turret: &'a TurretVisual,
    pub turret_loft: &'a TurretLoftVisual,
    pub gun: &'a GunVisual,
    pub deck: &'a BoxVisual,
    pub fender: &'a FenderVisual,
    pub fittings: &'a FittingsVisual,
    pub detail: &'a DetailVisual,
}

impl VisualDetail {
    /// Whether the part library can build EVERY piece of this vehicle from its authored parts
    /// (Forge 2.0 K3, step 4e): the benchmark's complete tree, or a welded slab vehicle whose
    /// hull construction, welded turret and gun are authored — then no recipe piece stands
    /// and the vehicle ships at Benchmark fidelity with part-aware LODs.
    pub fn is_library_complete(&self) -> bool {
        self.is_complete()
            || (self.construction.is_some()
                && (self.welded_turret.is_some()
                    || self.casemate.is_some()
                    || self.cast_dome.is_some())
                && self.gun.is_some())
    }

    /// The FULL truth-aligned view — `Some` only when every part is authored, so the analytic
    /// breach path may really OPEN the vehicle (the client's cut-truth gate reads this, not
    /// mere presence of the slot). A partial block (a gun group alone, F5.iii) improves the
    /// LOOK without claiming the skin/armour alignment it does not have.
    pub fn complete(&self) -> Option<CompleteVisual<'_>> {
        Some(CompleteVisual {
            hull: self.hull.as_ref()?,
            hull_plates: self.hull_plates.as_ref()?,
            turret: self.turret.as_ref()?,
            turret_loft: self.turret_loft.as_ref()?,
            gun: self.gun.as_ref()?,
            deck: self.deck.as_ref()?,
            fender: self.fender.as_ref()?,
            fittings: self.fittings.as_ref()?,
            detail: self.detail.as_ref()?,
        })
    }

    /// Whether every part is authored — see [`Self::complete`].
    pub fn is_complete(&self) -> bool {
        self.complete().is_some()
    }
}
