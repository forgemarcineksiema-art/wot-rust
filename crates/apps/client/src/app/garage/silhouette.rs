//! The carousel's recognition card: three rectangles from gameplay-true numbers — the hull
//! box to the turret split, the turret (or casemate) box above it, and the stock barrel from
//! the turret front — nose right, in the cell's pixels. No art assets, no mesh walk: the same
//! `HitboxProfile`/barrel data the battle resolves hits against draws the card, so the shape
//! cannot lie about the vehicle. One shared scale keeps the profiles honestly comparable — the
//! IS-3 genuinely reads smaller than a Tiger — capped per vehicle only when a long gun would
//! otherwise leave the cell, and the cap shrinks both axes so a capped card keeps its proportion.

use game_core::VehicleKind;
use ui_kit::rect::Rect;

/// Pixels per metre at one `u` per pixel (the cell is 116 u wide: a 10 m span fits).
const PX_PER_M_U: f32 = 8.4;

/// The three boxes in the cell's pixels: hull, turret, barrel.
pub(crate) fn silhouette_rects(kind: VehicleKind, cell: Rect, px_per_u: f32) -> [Rect; 3] {
    let hitbox = game_core::HitboxProfile::for_vehicle(kind);
    let split_y = hitbox.center_y_m + hitbox.turret_min_y_m;
    let top_y = hitbox.center_y_m + hitbox.half_height_m;
    let turret_front = hitbox.turret_center_z_m + hitbox.turret_half_length_m;
    let muzzle = turret_front + kind.stock_barrel_length_m();

    let (z_min, z_max) = (-hitbox.half_length_m, muzzle.max(hitbox.half_length_m));
    let max_span_px = cell.w * 0.92;
    let scale = (PX_PER_M_U * px_per_u).min(max_span_px / (z_max - z_min).max(0.1));
    let mid = (z_min + z_max) * 0.5;
    let ground_y = cell.bottom() - cell.h * 0.12;
    let cx = cell.x + cell.w * 0.5;
    // A box from its metre extents: z along the cell (nose right), y up from the ground line.
    let boxed = |z0: f32, z1: f32, y0: f32, y1: f32| {
        Rect::new(
            cx + (z0 - mid) * scale,
            ground_y - y1 * scale,
            (z1 - z0) * scale,
            (y1 - y0) * scale,
        )
    };
    let hull = boxed(-hitbox.half_length_m, hitbox.half_length_m, 0.0, split_y);
    let turret =
        boxed(hitbox.turret_center_z_m - hitbox.turret_half_length_m, turret_front, split_y, top_y);
    let gun_y = split_y + (top_y - split_y) * 0.45;
    let barrel_half_h = (0.06 * scale).max(1.0);
    let barrel = Rect::new(
        cx + (turret_front - mid) * scale,
        ground_y - gun_y * scale - barrel_half_h,
        (muzzle - turret_front) * scale,
        2.0 * barrel_half_h,
    );
    [hull, turret, barrel]
}

#[cfg(test)]
mod tests {
    use super::*;

    const CELL: Rect = Rect::new(100.0, 100.0, 116.0, 150.0);

    /// The card is gameplay-true: hull box + turret box + barrel, nose right, inside the cell,
    /// and the shapes genuinely differ — the Jagdtiger spans farther than the compact IS-3, the
    /// low T-54 stands lower than the Tiger II.
    #[test]
    fn silhouettes_draw_three_boxes_from_real_dimensions_and_stay_in_the_cell() {
        for kind in VehicleKind::PLAYABLE {
            let rects = silhouette_rects(kind, CELL, 1.0);
            for rect in rects {
                assert!(rect.w > 0.0 && rect.h > 0.0, "{kind:?}: an empty box");
                assert!(
                    rect.x >= CELL.x - 1e-3 && rect.right() <= CELL.right() + 1e-3,
                    "{kind:?}: {rect:?} leaves the cell"
                );
                assert!(
                    rect.y >= CELL.y - 1e-3 && rect.bottom() <= CELL.bottom() + 1e-3,
                    "{kind:?}: {rect:?} leaves the cell"
                );
            }
            let [hull, turret, barrel] = rects;
            assert!(turret.y < hull.y, "{kind:?}: the turret sits above the hull");
            assert!(
                barrel.x >= turret.right() - 1e-3,
                "{kind:?}: the barrel leaves the turret front, nose right"
            );
        }
        let span = |kind: VehicleKind| {
            let [hull, _, barrel] = silhouette_rects(kind, CELL, 1.0);
            barrel.right() - hull.x
        };
        assert!(span(VehicleKind::Jagdtiger) > span(VehicleKind::IS3));
        let height = |kind: VehicleKind| {
            let [hull, turret, _] = silhouette_rects(kind, CELL, 1.0);
            hull.bottom() - turret.y
        };
        assert!(
            height(VehicleKind::TigerII) > height(VehicleKind::T54_1951),
            "the famously low T-54 reads lower than the Tiger II"
        );
    }

    /// A metre of length and a metre of height land on the same number of pixels: the card is
    /// in true proportion, and the cap scales both axes.
    #[test]
    fn the_silhouette_is_drawn_in_true_proportion() {
        for kind in VehicleKind::PLAYABLE {
            let hitbox = game_core::HitboxProfile::for_vehicle(kind);
            let turret_front = hitbox.turret_center_z_m + hitbox.turret_half_length_m;
            let muzzle = turret_front + kind.stock_barrel_length_m();
            let range_m = hitbox.half_length_m + muzzle.max(hitbox.half_length_m);
            let height_m = hitbox.center_y_m + hitbox.half_height_m;
            let [hull, turret, barrel] = silhouette_rects(kind, CELL, 1.0);
            let span_px = barrel.right().max(hull.right()) - hull.x;
            let height_px = hull.bottom() - turret.y;
            let screen_ratio = span_px / height_px;
            let true_ratio = range_m / height_m;
            assert!(
                (screen_ratio / true_ratio - 1.0).abs() < 0.05,
                "{kind:?}: {screen_ratio:.2} vs {true_ratio:.2}"
            );
        }
    }
}
