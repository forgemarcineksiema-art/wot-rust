use serde::{Deserialize, Serialize};

/// A map's standing water: one flat table at `surface_level_m`. Water exists exactly where the
/// terrain dips below the level, so **depth = surface_level − heightmap height** and the
/// heightmap stays the single spatial source of truth — physics (wading drag), drowning, shell
/// splashes, the water mesh, and the minimap all derive from these two numbers. Mirror-symmetry
/// fairness of the water is inherited from the heightmap for free.
///
/// A struct (not a bare f32) so future surface behaviour (flow direction, murkiness) can append
/// without a schema break.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct WaterBody {
    /// World-space elevation of the still water surface.
    pub surface_level_m: f32,
}

impl WaterBody {
    /// Depth of water standing over terrain at height `ground_m`; zero on dry land.
    pub fn depth_over(&self, ground_m: f32) -> f32 {
        (self.surface_level_m - ground_m).max(0.0)
    }
}

/// One bounded standing-water sheet (teren W6): a rect `[x0, z0, x1, z1]` that CONTAINS the
/// pool, holding its own table. The pool's SHAPE is still the terrain's contour — water
/// exists where the ground dips under the sheet's level inside the rect, exactly the
/// `depth = level − ground` rule the global table lives by; the rect only scopes WHICH
/// table answers there. Two mountain tarns at different altitudes stop being impossible.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct StandingWater {
    /// `[x0, z0, x1, z1]`, world metres, inclusive.
    pub rect: [f32; 4],
    /// World-space elevation of this sheet's still surface.
    pub surface_level_m: f32,
}

impl StandingWater {
    pub fn contains(&self, x: f32, z: f32) -> bool {
        x >= self.rect[0] && x <= self.rect[2] && z >= self.rect[1] && z <= self.rect[3]
    }
}

/// The map's COMPLETE standing water: the legacy global table plus bounded sheets. Every
/// consumer — wading, drowning, shell splashes, the water mesh, the minimap, the report's
/// drown cells — resolves through the ONE rule in [`WaterView::level_at`], so a second body
/// of water can never fork the rules. Sheets answer first (document order), the table is
/// the fallback; a map with neither is dry.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct WaterField {
    pub table: Option<WaterBody>,
    pub sheets: Vec<StandingWater>,
}

impl WaterField {
    pub fn is_dry(&self) -> bool {
        self.table.is_none() && self.sheets.is_empty()
    }

    /// The borrowable, `Copy` view of this field — what tick-hot contexts carry.
    pub fn view(&self) -> WaterView<'_> {
        WaterView { table: self.table, sheets: &self.sheets }
    }

    /// The still-water level governing `(x, z)`, if any (see [`WaterView::level_at`]).
    pub fn level_at(&self, x: f32, z: f32) -> Option<f32> {
        self.view().level_at(x, z)
    }

    /// Depth of water over terrain at height `ground_m` at `(x, z)`; zero on dry land.
    pub fn depth_at(&self, ground_m: f32, x: f32, z: f32) -> f32 {
        self.view().depth_at(ground_m, x, z)
    }
}

/// The legacy single-table view IS a water field — `set_water(Some(WaterBody { .. }))`
/// keeps reading exactly as it always did at every call site.
impl From<Option<WaterBody>> for WaterField {
    fn from(table: Option<WaterBody>) -> Self {
        WaterField { table, sheets: Vec::new() }
    }
}

/// The borrowed, `Copy` form of a [`WaterField`] — sim ticks, physics contacts and shell
/// traces carry THIS. The resolution rule lives here, once, for owner and borrower alike.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WaterView<'a> {
    pub table: Option<WaterBody>,
    pub sheets: &'a [StandingWater],
}

impl WaterView<'_> {
    /// A dry map — the default world for fixtures and waterless contexts.
    pub const DRY: WaterView<'static> = WaterView { table: None, sheets: &[] };

    /// The still-water level governing `(x, z)`, if any: the first sheet containing the
    /// point (document order), else the global table.
    pub fn level_at(&self, x: f32, z: f32) -> Option<f32> {
        for sheet in self.sheets {
            if sheet.contains(x, z) {
                return Some(sheet.surface_level_m);
            }
        }
        self.table.map(|table| table.surface_level_m)
    }

    /// Depth of water over terrain at height `ground_m` at `(x, z)`; zero on dry land.
    pub fn depth_at(&self, ground_m: f32, x: f32, z: f32) -> f32 {
        self.level_at(x, z).map_or(0.0, |level| (level - ground_m).max(0.0))
    }

    /// Every distinct still-water level in play (the table and each sheet's), for callers
    /// that resolve analytic plane crossings (shell splashes) — each candidate must still
    /// be confirmed by [`Self::level_at`] at its crossing point.
    pub fn levels(&self) -> impl Iterator<Item = f32> + '_ {
        self.table
            .map(|table| table.surface_level_m)
            .into_iter()
            .chain(self.sheets.iter().map(|sheet| sheet.surface_level_m))
    }
}

/// The legacy single-table call shape (`with_water(Some(WaterBody { .. }))`) stays valid.
impl From<Option<WaterBody>> for WaterView<'static> {
    fn from(table: Option<WaterBody>) -> Self {
        WaterView { table, sheets: &[] }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn depth_is_level_minus_ground_and_never_negative() {
        let water = WaterBody { surface_level_m: 5.0 };
        assert_eq!(water.depth_over(3.0), 2.0);
        assert_eq!(water.depth_over(5.0), 0.0);
        assert_eq!(water.depth_over(9.0), 0.0, "dry land has zero depth, not negative");
    }

    /// Teren W6: the ONE resolution rule — sheets answer first in document order, the
    /// table is the fallback, and outside everything the field is dry.
    #[test]
    fn sheets_answer_first_and_the_table_is_the_fallback() {
        let field = WaterField {
            table: Some(WaterBody { surface_level_m: 2.0 }),
            sheets: vec![
                StandingWater { rect: [10.0, 10.0, 30.0, 30.0], surface_level_m: 8.0 },
                StandingWater { rect: [20.0, 20.0, 60.0, 60.0], surface_level_m: 5.0 },
            ],
        };
        assert_eq!(field.level_at(15.0, 15.0), Some(8.0), "the tarn's own table");
        assert_eq!(field.level_at(25.0, 25.0), Some(8.0), "overlap resolves in document order");
        assert_eq!(field.level_at(50.0, 50.0), Some(5.0), "the second sheet");
        assert_eq!(field.level_at(90.0, 90.0), Some(2.0), "the global table beyond the sheets");
        assert_eq!(field.depth_at(6.5, 15.0, 15.0), 1.5);
        assert_eq!(field.depth_at(6.5, 90.0, 90.0), 0.0, "dry over the low table");

        let sheets_only = WaterField { table: None, sheets: field.sheets.clone() };
        assert_eq!(sheets_only.level_at(90.0, 90.0), None, "no table, no water out there");
        assert!(!sheets_only.is_dry());
        assert!(WaterField::default().is_dry());
        assert_eq!(
            WaterField::from(Some(WaterBody { surface_level_m: 3.0 })).level_at(0.0, 0.0),
            Some(3.0),
            "the legacy view is the same field"
        );
    }
}
