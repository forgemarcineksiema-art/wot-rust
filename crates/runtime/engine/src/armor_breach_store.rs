//! Every hull's perforations, accumulated client-side from the replicated stream of additions.
//!
//! Perforations used to ride the world snapshot as a whole `ArmorBreachSet` per tank, re-sent at
//! snapshot cadence forever — which made a battle's wire cost grow monotonically with the
//! shooting until the snapshot no longer fit one transport message (see `net`'s v39 note). They
//! are permanent and append-only, so protocol v39 replicates them as a stream of ADDITIONS on
//! the reliable lane instead, and this is where a client puts them back together.
//!
//! The convergence argument is that the client runs the SAME [`ArmorBreachSet::add`] the
//! authoritative simulation ran, on the same values, in the same order. Both the overlap merge
//! and the bounded-capacity eviction depend on that order, and the reliable lane is what
//! guarantees it.

use std::collections::HashMap;

use game_core::{ArmorBreach, ArmorBreachSet, TankId};

#[derive(Debug, Default, Clone, PartialEq)]
pub struct ArmorBreachStore {
    sets: HashMap<TankId, ArmorBreachSet>,
}

impl ArmorBreachStore {
    /// Apply one replicated addition. Idempotence is the LANE's job, not this call's: replaying
    /// the same breach twice would genuinely carve it twice, which is why the sequence cursor
    /// (`RemoteCombatEventInbox`) drops duplicates before they reach here.
    pub fn apply(&mut self, tank: TankId, breach: ArmorBreach) {
        self.sets.entry(tank).or_default().add(breach);
    }

    /// This hull's perforations, or the empty set for a hull that has never been holed.
    pub fn get(&self, tank: TankId) -> ArmorBreachSet {
        self.sets.get(&tank).cloned().unwrap_or_default()
    }

    pub fn is_empty(&self) -> bool {
        self.sets.is_empty()
    }

    pub fn len(&self) -> usize {
        self.sets.len()
    }

    /// Forget hulls that have left the battle. Tank ids are never reused, so without this the
    /// map would keep every vehicle a battle ever swapped away — the same leak the spotting
    /// hold-memory had. The length guard keeps the ordinary frame free.
    pub fn retain_live(&mut self, live: &[TankId]) {
        if self.sets.len() <= live.len() {
            return;
        }
        self.sets.retain(|tank, _| live.contains(tank));
    }
}

#[cfg(test)]
mod tests {
    use game_core::{
        ApertureLobe, ArmorBreachDescriptor, ArmorFrame, ArmorMaterial, ArmorSurfaceId, ArmorZone,
        BreachContour, BreachFace, ShellType,
    };
    use glam::Vec3;

    use super::*;

    fn breach(x: f32, id: u64) -> ArmorBreach {
        ArmorBreach::new(
            ArmorBreachDescriptor {
                breach_id: id,
                surface: ArmorSurfaceId::new(ArmorFrame::Hull, ArmorZone::HullSide),
                frame: ArmorFrame::Hull,
                zone: ArmorZone::HullSide,
                material: ArmorMaterial::RolledSteel,
                face: BreachFace::Ingress,
                shell_type: ShellType::ArmorPiercing,
                created_tick: 1,
                impact_angle_degrees: 0.0,
                impact_energy_kj: 100.0,
                projectile_diameter_m: 0.1,
                residual_penetration_mm: 10.0,
            },
            ApertureLobe {
                entry_local: Vec3::new(x, 0.5, 1.0),
                exit_local: Vec3::new(x, 0.5, 0.9),
                entry_normal_local: Vec3::Z,
                exit_normal_local: Vec3::NEG_Z,
                direction_local: Vec3::NEG_Z,
                thickness_m: 0.1,
                outer: BreachContour::new(0.05, 0.046, 0.3, 0.1),
                inner: BreachContour::new(0.07, 0.06, 0.4, 0.14),
                fracture_seed: id,
            },
        )
    }

    /// The whole point: a client replaying the stream of additions ends up with the set the
    /// authoritative simulation holds — merges and all.
    #[test]
    fn replaying_the_stream_reproduces_the_authoritative_set() {
        let stream: Vec<ArmorBreach> = (0..6).map(|i| breach(i as f32 * 0.045, i as u64)).collect();

        let mut authoritative = ArmorBreachSet::default();
        for breach in &stream {
            authoritative.add(breach.clone());
        }

        let mut store = ArmorBreachStore::default();
        for breach in &stream {
            store.apply(TankId(3), breach.clone());
        }

        assert_eq!(store.get(TankId(3)), authoritative);
        // Overlapping entries really did merge, so this is not a trivially-equal pair of lists.
        assert!(authoritative.breaches().len() < stream.len(), "the fixture must exercise merging");
    }

    #[test]
    fn an_unholed_hull_reads_as_the_empty_set_and_departed_hulls_are_forgotten() {
        let mut store = ArmorBreachStore::default();
        assert_eq!(store.get(TankId(9)), ArmorBreachSet::default());

        for tank in 1..=4 {
            store.apply(TankId(tank), breach(0.0, tank));
        }
        assert_eq!(store.len(), 4);
        store.retain_live(&[TankId(2), TankId(3)]);
        assert_eq!(store.len(), 2);
        assert_eq!(store.get(TankId(1)), ArmorBreachSet::default());
        assert!(!store.get(TankId(2)).breaches().is_empty());
    }
}
