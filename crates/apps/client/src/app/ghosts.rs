//! The minimap's memory of enemies it has seen (interface program H15): when a spotted hull
//! drops out of the snapshot the map keeps a GHOST at its last known position, fading over ten
//! seconds — a memory drawn as a memory (hollow, dim), never as a sighting. A hull the crew has
//! never seen has no ghost: the map cannot invent one, because nothing here comes from anywhere
//! but the snapshots that named it.

use std::collections::HashMap;

use game_core::{TankId, VehicleClass};

/// How long a ghost lingers.
pub(crate) const GHOST_TTL_S: f32 = 10.0;

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct LastKnown {
    pub xz: [f32; 2],
    pub class: VehicleClass,
    /// Seconds since the hull was last in the snapshot.
    pub age_s: f32,
}

#[derive(Debug, Default)]
pub(crate) struct GhostMemory {
    seen: HashMap<TankId, LastKnown>,
}

impl GhostMemory {
    /// One frame: every hull in `spotted` is a fresh sighting; everything else ages, and a hull
    /// unseen for the TTL is forgotten. `gone` names hulls the map must drop at once (wrecks).
    pub(crate) fn observe(
        &mut self,
        spotted: impl Iterator<Item = (TankId, [f32; 2], VehicleClass)>,
        gone: impl Iterator<Item = TankId>,
        dt: f32,
    ) {
        for known in self.seen.values_mut() {
            known.age_s += dt;
        }
        for (id, xz, class) in spotted {
            self.seen.insert(id, LastKnown { xz, class, age_s: 0.0 });
        }
        for id in gone {
            self.seen.remove(&id);
        }
        self.seen.retain(|_, known| known.age_s <= GHOST_TTL_S);
    }

    /// The ghosts: hulls seen before and not seen now, oldest last.
    pub(crate) fn ghosts<'a>(
        &'a self,
        spotted_now: impl Fn(TankId) -> bool + 'a,
    ) -> impl Iterator<Item = LastKnown> + 'a {
        let mut ghosts: Vec<(TankId, LastKnown)> = self
            .seen
            .iter()
            .filter(|(id, known)| !spotted_now(**id) && known.age_s > 0.0)
            .map(|(id, known)| (*id, *known))
            .collect();
        ghosts.sort_by(|a, b| a.1.age_s.total_cmp(&b.1.age_s).then(a.0.0.cmp(&b.0.0)));
        ghosts.into_iter().map(|(_, known)| known)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// H15: a ghost stands where the hull was last seen, fades out at ten seconds, and a hull
    /// never seen leaves none — the map remembers, it never invents.
    #[test]
    fn the_minimap_forgets_a_ghost_in_ten_seconds_and_never_invents_one() {
        let mut memory = GhostMemory::default();
        let seen = |ids: &[u64]| {
            let set: Vec<u64> = ids.to_vec();
            move |id: TankId| set.contains(&id.0)
        };
        // Never seen: nothing.
        memory.observe(std::iter::empty(), std::iter::empty(), 0.1);
        assert_eq!(memory.ghosts(seen(&[])).count(), 0);
        // Seen, then gone: a ghost at the last position.
        memory.observe(
            [(TankId(7), [120.0, 340.0], VehicleClass::Heavy)].into_iter(),
            std::iter::empty(),
            0.1,
        );
        assert_eq!(
            memory.ghosts(seen(&[7])).count(),
            0,
            "a hull in the snapshot is a blip, not a ghost"
        );
        memory.observe(std::iter::empty(), std::iter::empty(), 0.5);
        let ghosts: Vec<LastKnown> = memory.ghosts(seen(&[])).collect();
        assert_eq!(ghosts.len(), 1);
        assert_eq!(ghosts[0].xz, [120.0, 340.0]);
        assert_eq!(ghosts[0].class, VehicleClass::Heavy);
        assert!((ghosts[0].age_s - 0.5).abs() < 1e-6);
        // Nine and a half seconds later it is still there; at ten it is gone.
        memory.observe(std::iter::empty(), std::iter::empty(), 9.4);
        assert_eq!(memory.ghosts(seen(&[])).count(), 1);
        memory.observe(std::iter::empty(), std::iter::empty(), 0.2);
        assert_eq!(memory.ghosts(seen(&[])).count(), 0, "forgotten at ten seconds");
        // A wreck is dropped at once, whatever its age.
        memory.observe(
            [(TankId(8), [1.0, 2.0], VehicleClass::Medium)].into_iter(),
            std::iter::empty(),
            0.1,
        );
        memory.observe(std::iter::empty(), [TankId(8)].into_iter(), 0.1);
        assert_eq!(memory.ghosts(seen(&[])).count(), 0);
    }
}
