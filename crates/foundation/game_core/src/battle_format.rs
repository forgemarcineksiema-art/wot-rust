use serde::{Deserialize, Serialize};

/// Compatibility defaults for callers that explicitly request the seven-seat format.
pub const SEVEN_VS_SEVEN_SEATS: usize = 7;
pub const SEVEN_VS_SEVEN_TIME_LIMIT_S: u32 = 420;

/// Battle size and clock share one identity. Append variants: map assets will store them.
/// The owner's mode rules live in `docs/game-modes.md` (R1 and R8).
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum BattleFormat {
    #[default]
    SevenVsSeven,
    FifteenVsFifteen,
}

impl BattleFormat {
    /// Every format, in declaration order (the `quality` gate walks it and pins the order).
    pub const ALL: [BattleFormat; 2] = [BattleFormat::SevenVsSeven, BattleFormat::FifteenVsFifteen];
    pub const LARGEST: Self = Self::FifteenVsFifteen;

    pub const fn seats_per_team(self) -> usize {
        match self {
            Self::SevenVsSeven => SEVEN_VS_SEVEN_SEATS,
            Self::FifteenVsFifteen => 15,
        }
    }

    pub const fn total_seats(self) -> usize {
        self.seats_per_team() * 2
    }

    pub const fn time_limit_s(self) -> u32 {
        match self {
            Self::SevenVsSeven => SEVEN_VS_SEVEN_TIME_LIMIT_S,
            Self::FifteenVsFifteen => 900,
        }
    }

    /// Formation in metres (right, forward), before the seed's ±1.5 m jitter.
    /// The seven-seat deployment is preserved exactly; fifteen gets five columns.
    /// An invalid seat has no position and must never wrap onto an occupied one.
    pub fn spawn_offset(self, seat: usize) -> Option<[f32; 2]> {
        const SEVEN: [[f32; 2]; 7] = [
            [0.0, -8.0],
            [-22.0, -22.0],
            [0.0, -22.0],
            [22.0, -22.0],
            [-22.0, -40.0],
            [0.0, -40.0],
            [22.0, -40.0],
        ];
        // Recenter the wider formation within the existing 55 m spawn disk instead of
        // extending its last row behind the zone. Vehicle clearance is checked per map.
        const FIFTEEN: [[f32; 2]; 15] = [
            [0.0, 12.0],
            [-44.0, -2.0],
            [-22.0, -2.0],
            [0.0, -2.0],
            [22.0, -2.0],
            [44.0, -2.0],
            [-44.0, -20.0],
            [-22.0, -20.0],
            [0.0, -20.0],
            [22.0, -20.0],
            [44.0, -20.0],
            [-33.0, -38.0],
            [-11.0, -38.0],
            [11.0, -38.0],
            [33.0, -38.0],
        ];
        match self {
            Self::SevenVsSeven => SEVEN.get(seat),
            Self::FifteenVsFifteen => FIFTEEN.get(seat),
        }
        .copied()
    }

    /// How far the host's seeded jitter moves a seat on each local axis, metres (±).
    pub const SPAWN_JITTER_M: f32 = 1.5;

    /// A seat's point on the ground plane: the formation offset (right, forward) plus a local
    /// jitter, rotated by the zone's facing yaw about its centre `[x, z]`. The host deploys
    /// through this and the map report (`map_forge`, the `formats` check of M4) judges the same
    /// points — a map cannot certify a seat the host would put somewhere else. `None` for a seat
    /// the format does not have.
    pub fn seat_position(
        self,
        seat: usize,
        center_xz: [f32; 2],
        facing_yaw_rad: f32,
        jitter_local: [f32; 2],
    ) -> Option<[f32; 2]> {
        let [local_x, local_z] = self.spawn_offset(seat)?;
        let (sin, cos) = facing_yaw_rad.sin_cos();
        let dx = local_x + jitter_local[0];
        let dz = local_z + jitter_local[1];
        // right = (cos, -sin), forward = (sin, cos); summed in the host's historical order so
        // the seven-seat deployment stays byte-identical.
        Some([center_xz[0] + cos * dx + sin * dz, center_xz[1] + (-sin) * dx + cos * dz])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_format_has_distinct_seats_and_never_wraps_an_invalid_seat() {
        for format in BattleFormat::ALL {
            let offsets: Vec<_> = (0..format.seats_per_team())
                .map(|seat| format.spawn_offset(seat).expect("valid seat"))
                .collect();
            for (i, a) in offsets.iter().enumerate() {
                for b in &offsets[i + 1..] {
                    // Even opposite jitter extremes leave more than ten metres between
                    // centres: the largest shipped hull can spawn without touching another.
                    let dx = ((a[0] - b[0]).abs() - 3.0).max(0.0);
                    let dz = ((a[1] - b[1]).abs() - 3.0).max(0.0);
                    assert!(dx.hypot(dz) > 10.0, "{format:?}: {a:?} against {b:?}");
                }
            }
            assert_eq!(format.spawn_offset(format.seats_per_team()), None);
            assert_eq!(format.spawn_offset(usize::MAX), None);
            assert!(format.total_seats() <= BattleFormat::LARGEST.total_seats());
        }
    }
}
