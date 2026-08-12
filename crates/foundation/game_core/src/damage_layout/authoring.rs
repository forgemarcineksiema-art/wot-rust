//! Shared vocabulary for authoring a vehicle's component layout.
//!
//! Component positions are stated RELATIVE to the hull the blueprint already describes — floor,
//! deck, tub wall, sponson wall, ring — instead of as free-standing metres. Two reasons, both
//! learned from the T-54 file next door:
//!
//!   * every correction recorded in `t54.rs` is the same mistake ("the rack topped out 9 cm proud
//!     of the foredeck", "the tanks' outer faces sat a centimetre past the hull side"). Those are
//!     arithmetic against hull anchors, and arithmetic belongs in code;
//!   * when a hull is re-measured — and this fleet's hulls have been, repeatedly — components
//!     authored against anchors move with it, while components authored as literals silently end
//!     up outside the tank.
//!
//! Everything here is in the HITBOX-CENTERED hull frame that damage components and the shell trace
//! share: blueprint heights are ground-relative, so `hitbox_center_y` is subtracted once, here.

use glam::Vec3;

use super::{DamageComponent, DamageComponentId, DamageComponentKind, DamageMaterial, DamageShape};
use crate::{ArmorFrame, ModuleSlot, VehicleBlueprint, VehicleKind};

/// How far inside a hull wall a component's face sits: plate thickness plus the hand's width of
/// clearance real stowage leaves. Rounds pressed into the middle of the armour are rounds a side
/// penetration reaches before it has finished penetrating — the T-54 bow rack shipped that way
/// once (`t54.rs`, interior audit) and it read as ammunition stored inside the plate.
pub const WALL_INSET_M: f32 = 0.08;

/// The interior of one hull, in the frame components are authored in.
///
/// Read off the blueprint, never retyped: `HullEnvelope::of(kind)` is the only way to build one.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HullEnvelope {
    /// Half the hull length; the bow is `+half_len`, the tail `-half_len`.
    pub half_len: f32,
    /// Half-width of the lower tub, between the tracks.
    pub tub_half_width: f32,
    /// Half-width of the upper hull where it overhangs the running gear. Equal to `tub_half_width`
    /// on a hull with no sponson (the T-54's narrow box).
    pub sponson_half_width: f32,
    /// Belly plate — the floor a rack or an engine block stands on.
    pub floor_y: f32,
    /// Where the tub steps out into the sponson, roughly axle height.
    pub sponson_y: f32,
    /// Deck plate — the ceiling nothing inside the hull may poke through.
    pub deck_y: f32,
    /// Turret ring centre and height; turret-frame components hang off these.
    pub ring_z: f32,
    pub ring_y: f32,
    /// Turret roof — the ceiling for turret-frame components.
    pub roof_y: f32,
    /// Belt centreline: where the suspension arms and their wheels actually are.
    pub track_center_x: f32,
    /// Inner face of the belt — the plane a final drive has to reach across.
    pub track_inner_x: f32,
    /// Road-wheel axle height.
    pub axle_y: f32,
    /// Sprocket/idler centre; the drive end sits at `±track_end_z`.
    pub track_end_z: f32,
    /// Glacis lean from vertical; the bow interior slopes back by this much per metre of height.
    pub glacis_slope_deg: f32,
    /// Casting half-span at the ring and at the roof — the taper a turret component must respect.
    pub ring_radius: f32,
    pub roof_radius: f32,
    /// The vehicle this envelope describes, so the armour it is wrapped in can be queried.
    pub kind: VehicleKind,
}

/// Which end of the hull the sprocket is on — the single fact that decides where the driveline
/// lives, and the sharpest anatomical difference in this fleet.
///
/// A rear-drive tank (Soviet, British) puts the gearbox behind the engine, both in the tail. A
/// front-drive tank (German) runs a propeller shaft the length of the hull, under the turret
/// basket, to a gearbox sitting between the driver's knees — which is why a German bow
/// penetration wrecks the transmission and a Soviet one does not.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DriveEnd {
    Front,
    Rear,
}

impl DriveEnd {
    /// The sign of the driven end along z.
    pub fn sign(self) -> f32 {
        match self {
            DriveEnd::Front => 1.0,
            DriveEnd::Rear => -1.0,
        }
    }
}

impl HullEnvelope {
    /// The envelope of a blueprint-born vehicle, or `None` for one with no blueprint.
    pub fn of(kind: VehicleKind) -> Option<Self> {
        let blueprint = VehicleBlueprint::for_vehicle(kind)?;
        let hull = blueprint.hull;
        let center_y = hull.hitbox_center_y;
        Some(Self {
            half_len: hull.half_len,
            tub_half_width: hull.lower_half_width,
            sponson_half_width: hull.half_width,
            floor_y: hull.belly_y - center_y,
            sponson_y: hull.sponson_y - center_y,
            deck_y: hull.deck_y - center_y,
            ring_z: blueprint.turret.ring_z,
            ring_y: blueprint.turret.ring_y - center_y,
            roof_y: blueprint.turret.roof_y - center_y,
            track_center_x: blueprint.track.center_x,
            track_inner_x: blueprint.track.inner_x,
            axle_y: blueprint.track.bottom_y + blueprint.track.wheel_radius - center_y,
            track_end_z: blueprint.track.end_z,
            glacis_slope_deg: hull.glacis_slope_deg,
            ring_radius: blueprint.turret.base_radius,
            roof_radius: blueprint.turret.roof_radius,
            kind,
        })
    }

    /// Does this hull overhang its tracks at all? A sponson is interior volume OUTSIDE the tub
    /// walls, and what a designer puts there is a real choice with a real consequence: German
    /// mediums and heavies stowed ammunition in it, which is why their flanks brew up.
    pub fn has_sponson(&self) -> bool {
        self.sponson_half_width > self.tub_half_width + 1.0e-3
    }

    /// The centreline offset a component of `half_width` gets when pushed against the tub wall.
    pub fn against_tub_wall(&self, half_width: f32) -> f32 {
        self.tub_half_width - WALL_INSET_M - half_width
    }

    /// The same against the sponson wall, for stowage that lives out over the tracks.
    pub fn against_sponson_wall(&self, half_width: f32) -> f32 {
        self.sponson_half_width - WALL_INSET_M - half_width
    }

    /// A component of `half_height` standing on the floor.
    pub fn standing_on_floor(&self, half_height: f32) -> f32 {
        self.floor_y + half_height
    }

    /// `fraction` of the way from the tail (0.0) to the bow (1.0).
    pub fn along_hull(&self, fraction: f32) -> f32 {
        -self.half_len + fraction * 2.0 * self.half_len
    }

    /// Is this point inside the hull's armour?
    ///
    /// Asked of the baked volumes rather than reasoned about, because hulls in this fleet are not
    /// boxes: the T-34's upper sides lean inward, the IS-3 wears a pike nose, every tail plate
    /// slopes. Arithmetic against `half_width` says a component fits when it does not.
    pub fn inside_hull(&self, point: Vec3) -> bool {
        let Some(volumes) = crate::vehicle_armor_volumes(self.kind) else { return true };
        volumes
            .hull
            .iter()
            .any(|volume| volume.planes.iter().all(|p| p.normal.dot(point) <= p.offset))
    }

    /// The furthest the hull interior reaches sideways at this height and station.
    ///
    /// Bisected against the real armour, so a sloped side wall gives back the width it actually
    /// leaves rather than the width the widest plane suggests.
    pub fn hull_half_width_at(&self, y: f32, z: f32) -> f32 {
        self.reach(|x| Vec3::new(x, y, z), self.sponson_half_width + 0.2, |p| self.inside_hull(p))
    }

    /// How far forward the hull interior reaches on the centreline at this height.
    ///
    /// A glacis leans back, so the bow is a ramp, not a wall: the higher a component stands the
    /// further back its front face must sit. A German gearbox is the case that matters — it
    /// belongs as far forward as it can go, and this is how far that is.
    pub fn bow_limit_at(&self, y: f32) -> f32 {
        self.reach(|z| Vec3::new(0.0, y, z), self.half_len + 0.2, |p| self.inside_hull(p))
    }

    /// The same at the tail, returned as a NEGATIVE station.
    pub fn tail_limit_at(&self, y: f32) -> f32 {
        -self.reach(|z| Vec3::new(0.0, y, -z), self.half_len + 0.2, |p| self.inside_hull(p))
    }

    /// Centreline offset for a component of `half_width` set against the real side wall at this
    /// height and station.
    pub fn against_wall_at(&self, y: f32, z: f32, half_width: f32) -> f32 {
        (self.hull_half_width_at(y, z) - WALL_INSET_M - half_width).max(0.0)
    }

    /// Slide a box of half-depth `half_z` along z until both of its faces are inside the hull at
    /// every height it occupies, starting from `wanted`.
    ///
    /// The bow and the tail are both ramps, and they lean opposite ways; a box pushed against
    /// either by arithmetic on `half_len` ends up with two corners in the open air.
    pub fn fit_station(&self, wanted: f32, half_z: f32, y_lo: f32, y_hi: f32) -> f32 {
        let bow = self.bow_limit_at(y_lo).min(self.bow_limit_at(y_hi)) - WALL_INSET_M;
        let tail = self.tail_limit_at(y_lo).max(self.tail_limit_at(y_hi)) + WALL_INSET_M;
        let (lo, hi) = (tail + half_z, bow - half_z);
        if lo > hi { 0.5 * (lo + hi) } else { wanted.clamp(lo, hi) }
    }

    /// Largest non-negative `t` with `point(t)` still satisfying `inside`, to the nearest 5 mm.
    fn reach(
        &self,
        point: impl Fn(f32) -> Vec3,
        ceiling: f32,
        inside_test: impl Fn(Vec3) -> bool,
    ) -> f32 {
        if !inside_test(point(0.0)) {
            return 0.0;
        }
        let (mut inside, mut outside) = (0.0_f32, ceiling);
        while outside - inside > 0.005 {
            let middle = 0.5 * (inside + outside);
            if inside_test(point(middle)) {
                inside = middle;
            } else {
                outside = middle;
            }
        }
        inside
    }

    /// Half-span of the turret casting at height `y`, measured in every azimuth and reported as
    /// the tightest.
    ///
    /// This began as a straight cone from ring radius to roof radius, and the IS-3 showed why that
    /// is not good enough: its casting is flat and wide, and the cone over-estimated the room at
    /// breech height by enough to push the gun mechanism out through the front. Castings in this
    /// fleet taper at genuinely different rates, so the taper gets measured, not modelled.
    pub fn casting_half_span_at(&self, y: f32) -> f32 {
        const AZIMUTHS: usize = 16;
        let Some(volumes) = crate::vehicle_armor_volumes(self.kind) else {
            return self.ring_radius;
        };
        let turret = &volumes.turret;
        (0..AZIMUTHS)
            .map(|step| {
                let angle = std::f32::consts::TAU * step as f32 / AZIMUTHS as f32;
                let direction = Vec3::new(angle.cos(), 0.0, angle.sin());
                self.reach(
                    |r| Vec3::new(0.0, y, self.ring_z) + direction * r,
                    self.ring_radius + 0.5,
                    |point| turret.planes.iter().all(|p| p.normal.dot(point) <= p.offset),
                )
            })
            .fold(f32::INFINITY, f32::min)
    }
}

/// A component authored in the hull frame.
pub fn hull_component(
    id: u16,
    kind: DamageComponentKind,
    slot: ModuleSlot,
    material: DamageMaterial,
    shape: DamageShape,
    priority: u8,
    vulnerability: f32,
) -> DamageComponent {
    component(id, ArmorFrame::Hull, kind, slot, material, shape, priority, vulnerability)
}

/// A component authored in the neutral turret frame; it swings with the live turret at trace time.
pub fn turret_component(
    id: u16,
    kind: DamageComponentKind,
    slot: ModuleSlot,
    material: DamageMaterial,
    shape: DamageShape,
    priority: u8,
    vulnerability: f32,
) -> DamageComponent {
    component(id, ArmorFrame::Turret, kind, slot, material, shape, priority, vulnerability)
}

#[allow(clippy::too_many_arguments)]
pub fn component(
    id: u16,
    frame: ArmorFrame,
    kind: DamageComponentKind,
    slot: ModuleSlot,
    material: DamageMaterial,
    shape: DamageShape,
    priority: u8,
    vulnerability: f32,
) -> DamageComponent {
    DamageComponent {
        id: DamageComponentId(id),
        frame,
        kind,
        slot,
        material,
        shape,
        priority,
        requires_penetration: true,
        vulnerability,
    }
}

pub fn obb(center: [f32; 3], half_extents: [f32; 3], yaw_rad: f32) -> DamageShape {
    DamageShape::Obb {
        center: Vec3::from_array(center),
        half_extents: Vec3::from_array(half_extents),
        yaw_rad,
    }
}

pub fn cylinder_shape(center: [f32; 3], axis: Vec3, half_length: f32, radius: f32) -> DamageShape {
    DamageShape::Cylinder { center: Vec3::from_array(center), axis, half_length, radius }
}

/// The torsion-bar / wheel-arm line down each flank, on the belt centreline at axle height.
///
/// Unlike everything else here it does NOT require penetration: a track-line hit reaches the
/// running gear without entering the tank. The line stands where the WHEELS stand — a capsule
/// inboard of the gauge it represents takes hits the suspension would not, which is exactly the
/// error the T-54 file records fixing by hand.
pub fn suspension_pair(env: &HullEnvelope, ids: [u16; 2], radius: f32) -> [DamageComponent; 2] {
    [(-env.track_center_x, ids[0]), (env.track_center_x, ids[1])].map(|(x, id)| DamageComponent {
        id: DamageComponentId(id),
        frame: ArmorFrame::Hull,
        kind: DamageComponentKind::Suspension,
        slot: ModuleSlot::Suspension,
        material: DamageMaterial::SuspensionSteel,
        shape: DamageShape::Capsule {
            a: Vec3::new(x, env.axle_y + radius * 0.25, -env.track_end_z + 0.2),
            b: Vec3::new(x, env.axle_y + radius * 0.25, env.track_end_z - 0.2),
            radius,
        },
        priority: 20,
        requires_penetration: false,
        vulnerability: 0.8,
    })
}

/// The two final-drive housings at the driven end. They deliberately cross the hull side: a drive
/// has to reach the sprocket, and that is the one interior element allowed outside the tub.
pub fn final_drive_pair(
    env: &HullEnvelope,
    ids: [u16; 2],
    drive: DriveEnd,
    radius: f32,
) -> [DamageComponent; 2] {
    let x = env.track_inner_x + radius * 0.2;
    let z = drive.sign() * env.track_end_z;
    [(-x, ids[0]), (x, ids[1])].map(|(x, id)| {
        hull_component(
            id,
            DamageComponentKind::FinalDrive,
            ModuleSlot::Suspension,
            DamageMaterial::Driveline,
            cylinder_shape([x, env.axle_y + radius * 0.5, z], Vec3::X, radius * 0.75, radius),
            28,
            1.0,
        )
    })
}

/// What a turret holds besides the gun that defines it.
#[derive(Debug, Clone, Copy)]
pub struct TurretFit {
    /// How much of the casting the gun mechanism fills, 0..1. A 128 mm PaK in a casemate is near
    /// the top of that range; a 76 mm in a roomy dome near the bottom.
    pub breech_fill: f32,
    /// A traverse gearbox at the ring. A fixed casemate has nothing to traverse and gets none.
    pub turret_drive: bool,
    /// Ready rounds against the bustle wall, sized as a fraction of the casting. `None` for a
    /// turret with no bustle worth stowing in — the Tiger I is the case that matters.
    pub bustle_rack: Option<f32>,
}

/// The gun mechanism, its traverse gear, the wireless and any bustle rounds, placed so that every
/// one of them fits inside THIS casting's taper.
///
/// Sized from the envelope rather than retyped per vehicle, because the failure mode is specific
/// and was reproducible: breech dimensions copied from the T-54's tall dome poke straight out of
/// the IS-3's flatter one, and nothing but a containment test notices.
pub fn turret_group(env: &HullEnvelope, fit: TurretFit) -> Vec<DamageComponent> {
    let height = (env.roof_y - env.ring_y).max(0.20);
    let breech_half_y = height * 0.20;
    let breech_y = env.ring_y + breech_half_y + height * 0.10;
    let span = env.casting_half_span_at(breech_y + breech_half_y);
    let breech_half_x = span * 0.34 * fit.breech_fill;
    let breech_half_z = span * 0.44 * fit.breech_fill;

    let mut components = vec![turret_component(
        1,
        DamageComponentKind::Breech,
        ModuleSlot::Gun,
        DamageMaterial::Machinery,
        obb(
            [0.0, breech_y, env.ring_z + forward_within(span, breech_half_x, breech_half_z)],
            [breech_half_x, breech_half_y, breech_half_z],
            0.0,
        ),
        40,
        1.15,
    )];

    // Recoil cylinders flanking the barrel, above the breech.
    let recoil_radius = breech_half_x * 0.30;
    let recoil_y = breech_y + breech_half_y * 0.55;
    let recoil_span = env.casting_half_span_at(recoil_y + recoil_radius);
    let recoil_x = breech_half_x * 0.72;
    let recoil_half_len = breech_half_z * 0.95;
    let recoil_z =
        env.ring_z + forward_within(recoil_span, recoil_x + recoil_radius, recoil_half_len);
    for (index, side) in [-1.0_f32, 1.0].into_iter().enumerate() {
        components.push(turret_component(
            2 + index as u16,
            DamageComponentKind::RecoilMechanism,
            ModuleSlot::Gun,
            DamageMaterial::Machinery,
            cylinder_shape(
                [side * recoil_x, recoil_y, recoil_z],
                Vec3::Z,
                recoil_half_len,
                recoil_radius,
            ),
            38,
            1.0,
        ));
    }

    if fit.turret_drive {
        // Straddles the ring: the top half turns inside the casting, the gearbox below it reaches
        // into the hull, which is where a traverse drive actually lives.
        let drive_radius = height * 0.18;
        let drive_half_len = height * 0.22;
        let drive_top = env.ring_y + drive_half_len;
        let drive_span = env.casting_half_span_at(drive_top);
        components.push(turret_component(
            4,
            DamageComponentKind::TurretDrive,
            ModuleSlot::Turret,
            DamageMaterial::Driveline,
            cylinder_shape(
                [(drive_span * 0.95 - drive_radius).max(0.0), env.ring_y, env.ring_z + 0.02],
                Vec3::Y,
                drive_half_len,
                drive_radius,
            ),
            34,
            1.0,
        ));
    }

    // The wireless against the rear wall, on the commander's side — the CUPOLA's side, read
    // from the blueprint rather than assumed (the 2026-08-12 mirror flip moved every cupola;
    // a literal here would have stranded the set on the loader's side).
    let commander_sign = crate::VehicleBlueprint::for_vehicle(env.kind)
        .map(|bp| bp.turret.cupola_x.signum())
        .filter(|sign| *sign != 0.0)
        .unwrap_or(1.0);
    let radio_half = [span * 0.22, height * 0.16, span * 0.14];
    let radio_y = env.ring_y + height * 0.28;
    let radio_span = env.casting_half_span_at(radio_y + radio_half[1]);
    components.push(turret_component(
        7,
        DamageComponentKind::Radio,
        ModuleSlot::Radio,
        DamageMaterial::Electronics,
        obb(
            [
                commander_sign * radio_half[0] * 0.9,
                radio_y,
                env.ring_z - forward_within(radio_span, radio_half[0] * 1.9, radio_half[2]),
            ],
            radio_half,
            0.0,
        ),
        28,
        1.4,
    ));

    if let Some(fill) = fit.bustle_rack {
        let rack_half = [span * 0.30 * fill, height * 0.16, span * 0.11 * fill];
        let rack_y = env.ring_y + height * 0.26;
        let rack_span = env.casting_half_span_at(rack_y + rack_half[1]);
        components.push(turret_component(
            6,
            DamageComponentKind::AmmunitionRack,
            ModuleSlot::AmmoRack,
            DamageMaterial::Ammunition,
            obb(
                [
                    rack_half[0] * 0.5,
                    rack_y,
                    env.ring_z - forward_within(rack_span, rack_half[0] * 1.5, rack_half[2]),
                ],
                rack_half,
                0.0,
            ),
            32,
            1.35,
        ));
    }

    components
}

/// How far from the ring axis a box of these half-extents can sit before a corner leaves a casting
/// of half-span `span`. Solves the circle for the corner, then backs off by the box's own depth.
fn forward_within(span: f32, half_x: f32, half_z: f32) -> f32 {
    let usable = span * 0.95;
    ((usable * usable - half_x * half_x).max(0.0)).sqrt() - half_z
}

/// A fuel cell against each side wall at a station along the hull, sized and seated so neither
/// cell reaches through the plate beside it.
///
/// Three hulls had written this out privately before it was hoisted — the IS-3, the Centurion and
/// the shared German bay — with the same shape and different numbers. The numbers are the
/// vehicle's; the arithmetic that keeps fuel inside the tank is not.
pub fn flank_fuel_pair(
    env: &HullEnvelope,
    ids: [u16; 2],
    half: [f32; 3],
    station_fraction: f32,
) -> Vec<DamageComponent> {
    let center_y = env.standing_on_floor(half[1]);
    let station =
        env.fit_station(env.along_hull(station_fraction), half[2], env.floor_y, center_y + half[1]);
    [(-1.0_f32, ids[0]), (1.0, ids[1])]
        .into_iter()
        .map(|(side, id)| {
            hull_component(
                id,
                DamageComponentKind::FuelTank,
                ModuleSlot::Engine,
                DamageMaterial::Fuel,
                obb(
                    [
                        side * env.against_wall_at(center_y + half[1], station, half[0]),
                        center_y,
                        station,
                    ],
                    half,
                    0.0,
                ),
                25,
                1.1,
            )
        })
        .collect()
}
