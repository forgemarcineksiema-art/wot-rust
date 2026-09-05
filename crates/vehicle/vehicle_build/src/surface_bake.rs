//! Semantic contact-cavity bands for the bake-time ambient-occlusion pass (R8 / Task 25).
//!
//! The hybrid T-54 carried a flat `surface_shade`; this module darkens the recesses a real casting
//! and welded hull show in ambient light — the turret-ring seam and undercut, the mantlet seat in
//! the embrasure, the running-gear/track recess under the sponson, and the glacis weld line. Bands
//! are derived from the blueprint's [`VisualDetail`] semantics (never from merged-mesh indices), so
//! the shading is reproducible and survives LOD. The renderer already multiplies albedo by
//! `surface_shade`; no renderer change is needed. Mud, rust, decals and camouflage stay runtime
//! overlays — this pass bakes only the geometry's own ambient contact.

use game_core::{CompleteVisual, TrackShape};
use glam::Vec3;
use vehicle_geometry::{CavityBand, SubmeshKind};

/// A contact cavity tagged with the semantic signal it represents (for the Forge manifest summary).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NamedCavity {
    pub signal: &'static str,
    pub band: CavityBand,
    /// The submesh this band shades, or every submesh when `None` (the part library's bands are
    /// positional and reach whatever metal sits in the recess). A recipe's bands are authored per
    /// submesh (`assemble` applies three sets), so a recipe split into pieces scopes each set —
    /// otherwise a hull band would shade turret vertices standing in the same place (K3).
    pub scope: Option<SubmeshKind>,
}

/// The set of contact cavities a vehicle bakes into its `surface_shade`.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SurfaceBake {
    pub cavities: Vec<NamedCavity>,
}

impl SurfaceBake {
    /// The raw bands the geometry pass applies, in author order.
    pub fn bands(&self) -> Vec<CavityBand> {
        self.cavities.iter().map(|c| c.band).collect()
    }

    /// The bands that shade `submesh`: the unscoped ones and those scoped to it.
    pub fn bands_for(&self, submesh: SubmeshKind) -> Vec<CavityBand> {
        self.cavities
            .iter()
            .filter(|c| c.scope.is_none_or(|scope| scope == submesh))
            .map(|c| c.band)
            .collect()
    }

    pub fn is_empty(&self) -> bool {
        self.cavities.is_empty()
    }

    /// The named signals in author order — the human-readable manifest summary.
    pub fn signals(&self) -> Vec<&'static str> {
        self.cavities.iter().map(|c| c.signal).collect()
    }

    /// A stable FNV-1a hash of the band configuration, so a Forge artifact can record exactly which
    /// cavity set it baked and invalidate intentionally when the bands change.
    pub fn config_hash(&self) -> u64 {
        let mut h = 0xcbf2_9ce4_8422_2325_u64;
        let mut mix = |bits: u64| {
            h ^= bits;
            h = h.wrapping_mul(0x0000_0100_0000_01b3);
        };
        for c in &self.cavities {
            for byte in c.signal.bytes() {
                mix(u64::from(byte));
            }
            for v in [c.band.center, c.band.half_extents] {
                mix(u64::from(v.x.to_bits()));
                mix(u64::from(v.y.to_bits()));
                mix(u64::from(v.z.to_bits()));
            }
            mix(u64::from(c.band.falloff.to_bits()));
            mix(u64::from(c.band.amount.to_bits()));
        }
        h
    }
}

/// Derive the T-54's contact cavities from its hybrid blueprint (and the gameplay `TrackShape`,
/// which owns the running gear). Every centre and extent is read from a blueprint dimension, so
/// the shading tracks the geometry it shades.
pub fn t54_surface_bake(v: CompleteVisual<'_>, track: &TrackShape, muzzle: Vec3) -> SurfaceBake {
    let cavities = vec![
        // The cast turret seats on the narrowed ring and overhangs it: the seam and undercut read as
        // a deep ambient shadow around the bottom of the casting and the roof under its skirt.
        NamedCavity {
            signal: "turret_ring_seam",
            scope: None,
            band: CavityBand {
                center: Vec3::new(0.0, v.turret.ring_plane_y, 0.0),
                half_extents: Vec3::new(1.15, 0.06, 1.25),
                falloff: 0.14,
                amount: 0.38,
            },
        },
        // The gun aperture: a pocket cut through the turret wall, with the canvas cover and the
        // tube inside it. Sized to the HOLE (0.42 x 0.38, measured) rather than to the ball
        // mantlet that used to fill it — the band was 0.90 x 0.60, which shaded a swathe of open
        // turret face on either side of an aperture less than half that wide.
        NamedCavity {
            signal: "mantlet_seat",
            scope: None,
            band: CavityBand {
                center: v.turret.socket_center,
                half_extents: Vec3::new(0.23, 0.21, 0.14),
                falloff: 0.14,
                amount: 0.45,
            },
        },
        // The running gear and track sit in the recess under the sponson overhang — the darkest band
        // on the whole hull in ambient light.
        NamedCavity {
            signal: "running_gear_recess",
            scope: None,
            band: CavityBand {
                center: Vec3::new(0.0, (track.top_y + track.bottom_y) * 0.5, 0.0),
                half_extents: Vec3::new(track.outer_x + 0.10, 0.50, track.end_z + 0.35),
                falloff: 0.12,
                amount: 0.42,
            },
        },
        // The engine-deck cooling grille is a louvered intake into the engine bay: the well under
        // the slats sits in deep shadow so the louver gaps read as a dark interior, not bright deck.
        NamedCavity {
            signal: "engine_grille",
            scope: None,
            band: CavityBand {
                center: Vec3::new(
                    v.detail.grille_center.x,
                    (v.deck.center.y + v.deck.half.y) - 0.04,
                    v.detail.grille_center.z,
                ),
                half_extents: Vec3::new(v.detail.grille_half.x, 0.06, v.detail.grille_half.z),
                falloff: 0.06,
                amount: 0.55,
            },
        },
        // THE BORE. A gun muzzle is a hole, and a hole is dark — but the bore was lit exactly
        // like the tube around it, so the deepest recess on the vehicle read as a shallow dimple
        // in a steel disc. The legacy generator had a dark funnel here and the hybrid lost it.
        // This band puts the shadow back where the metal actually is: down the tube, tight to
        // the bore, so the ring of steel at the face stays bright and the hole behind it does
        // not.
        NamedCavity {
            signal: "gun_bore",
            scope: None,
            band: CavityBand {
                center: muzzle - Vec3::Z * (v.gun.bore_radius * 1.6),
                half_extents: Vec3::new(
                    v.gun.bore_radius * 1.05,
                    v.gun.bore_radius * 1.05,
                    v.gun.bore_radius * 1.8,
                ),
                falloff: 0.02,
                amount: 0.80,
            },
        },
        // The glacis-to-roof weld line catches a thin contact shadow across the hull front.
        NamedCavity {
            signal: "glacis_weld",
            scope: None,
            band: CavityBand {
                // On the glacis/roof weld, which moves with the bow.
                center: Vec3::new(0.0, v.hull.roof_y, v.hull.half_len - 1.05),
                // As wide as the hull it runs across — it was a literal 1.05, which was that
                // width until the documented track gauge narrowed the tub to 1.03.
                half_extents: Vec3::new(v.hull.half_width, 0.04, 0.05),
                falloff: 0.08,
                amount: 0.30,
            },
        },
    ];
    SurfaceBake { cavities }
}
