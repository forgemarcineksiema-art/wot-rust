//! The running-gear shape data, split from `mod.rs` for the file budget: the wrapped belt, road
//! wheels, idler/sprocket placement, and the return rollers of layouts that carry their top run.
/// Running-gear shape: the wrapped track belt, road wheels, drive sprocket, and idler.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TrackShape {
    pub center_x: f32,
    pub belt_half_thickness: f32,
    pub top_y: f32,
    pub bottom_y: f32,
    pub wheel_radius: f32,
    pub wheel_count: usize,
    pub wheel_first_z: f32,
    pub wheel_last_z: f32,
    pub end_radius: f32,
    /// |Z| of the idler (front) and drive-sprocket (rear) axles. Beyond `wheel_last_z` on the
    /// T-54, which carries separate end wheels the belt ramps up to; equal to the wheel-span end
    /// for vehicles whose belt still wraps at the outermost road wheels (the stadium loop).
    pub end_z: f32,
    /// Y of the idler/sprocket axles. Raised above the road-wheel axle line on the T-54; equal to
    /// the axle line for the stadium wrap.
    pub end_y: f32,
    pub inner_x: f32,
    pub outer_x: f32,
    pub segments: usize,
    /// Explicit hull-local Z of each road-wheel axle, when the layout is irregular (the T-54's
    /// signature wider first/second gap). `None` spreads `wheel_count` wheels evenly between
    /// `wheel_first_z` and `wheel_last_z`. Read through [`TrackShape::wheel_stations`] — the one
    /// source both the physics contact footprint and the rendered running gear place wheels by.
    pub wheel_stations: Option<&'static [f32]>,
    /// Return rollers carrying the top run (`0` for layouts whose top run rests on the road
    /// wheels, like the T-54 family). The IS family runs three small rollers per side.
    pub return_rollers: usize,
    /// Radius of one return roller (ignored when `return_rollers` is 0).
    pub roller_radius: f32,
    /// Schachtellaufwerk: how far INBOARD every odd-indexed road wheel sits relative to the even
    /// row. `0` is the ordinary single file; a large offset (≈ a wheel's width) reads as the
    /// Tiger's interleaved double row, a small one as the Tiger II/Panther overlapped stagger.
    /// Presentation only — the contact footprint and belt stay on the shared centreline.
    pub overlap_inner_dx: f32,
}

impl TrackShape {
    /// Hull-local Z of each road-wheel axle, front positive: the explicit stations when authored,
    /// otherwise an even spread between the first and last wheel.
    pub fn wheel_stations(&self) -> Vec<f32> {
        if let Some(stations) = self.wheel_stations {
            return stations.to_vec();
        }
        match self.wheel_count {
            0 => Vec::new(),
            1 => vec![self.wheel_first_z],
            count => {
                let step = (self.wheel_last_z - self.wheel_first_z) / (count - 1) as f32;
                (0..count).map(|i| self.wheel_first_z + step * i as f32).collect()
            }
        }
    }
}
