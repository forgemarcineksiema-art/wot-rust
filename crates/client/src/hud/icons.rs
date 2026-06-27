//! Procedural HUD icons. We have no art assets, so — exactly like the font — icons are baked as
//! single-channel coverage masks into the shared HUD atlas and drawn as tinted textured quads.
//! Each icon is composed from a few primitives (disc, ring, rect, triangle) on a small canvas.

mod canvas;

use canvas::Canvas;

/// One icon's coverage canvas is `ICON_PX` square; the linear atlas sampler softens the downscale.
pub(crate) const ICON_PX: u32 = 40;

/// Every HUD icon. Monochrome masks, tinted at draw time to match the flat-shaded look.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum HudIcon {
    Crew,
    AmmoAp,
    AmmoApcr,
    AmmoHe,
    SlotTurret,
    SlotGun,
    SlotHull,
    SlotEngine,
    SlotSuspension,
    SlotRadio,
    StatHp,
    StatSpeed,
    StatPower,
    StatTraverse,
    StatPenetration,
    StatReload,
    StatSignal,
}

impl HudIcon {
    pub(crate) const ALL: [HudIcon; 17] = [
        HudIcon::Crew,
        HudIcon::AmmoAp,
        HudIcon::AmmoApcr,
        HudIcon::AmmoHe,
        HudIcon::SlotTurret,
        HudIcon::SlotGun,
        HudIcon::SlotHull,
        HudIcon::SlotEngine,
        HudIcon::SlotSuspension,
        HudIcon::SlotRadio,
        HudIcon::StatHp,
        HudIcon::StatSpeed,
        HudIcon::StatPower,
        HudIcon::StatTraverse,
        HudIcon::StatPenetration,
        HudIcon::StatReload,
        HudIcon::StatSignal,
    ];
}

/// Rasterize `icon` into an `ICON_PX * ICON_PX` coverage bitmap (row-major, R8).
pub(crate) fn raster(icon: HudIcon) -> Vec<u8> {
    let mut c = Canvas::new(ICON_PX as i32);
    match icon {
        HudIcon::Crew => {
            c.disc(0.5, 0.30, 0.16);
            c.tri([0.5, 0.50], [0.18, 0.92], [0.82, 0.92]);
        }
        HudIcon::AmmoAp => {
            c.tri([0.5, 0.10], [0.30, 0.42], [0.70, 0.42]);
            c.rect(0.32, 0.42, 0.68, 0.90);
        }
        HudIcon::AmmoApcr => {
            c.tri([0.5, 0.10], [0.30, 0.42], [0.70, 0.42]);
            c.rect(0.32, 0.42, 0.68, 0.90);
            c.punch_disc(0.5, 0.64, 0.12);
        }
        HudIcon::AmmoHe => {
            c.disc(0.5, 0.5, 0.18);
            for k in 0..8 {
                let a = k as f32 / 8.0 * std::f32::consts::TAU;
                c.disc(0.5 + 0.34 * a.cos(), 0.5 + 0.34 * a.sin(), 0.05);
            }
        }
        HudIcon::SlotTurret => {
            c.tri([0.22, 0.66], [0.5, 0.26], [0.78, 0.66]);
            c.rect(0.18, 0.66, 0.82, 0.80);
        }
        HudIcon::SlotGun => {
            c.rect(0.10, 0.44, 0.86, 0.56);
            c.rect(0.78, 0.36, 0.92, 0.64);
        }
        HudIcon::SlotHull => {
            c.rect(0.14, 0.40, 0.86, 0.66);
            c.disc(0.30, 0.72, 0.10);
            c.disc(0.70, 0.72, 0.10);
        }
        HudIcon::SlotEngine => {
            c.ring(0.5, 0.5, 0.32, 0.18);
            c.disc(0.5, 0.5, 0.10);
            for k in 0..4 {
                let a = k as f32 / 4.0 * std::f32::consts::TAU;
                c.disc(0.5 + 0.34 * a.cos(), 0.5 + 0.34 * a.sin(), 0.07);
            }
        }
        HudIcon::SlotSuspension => {
            c.ring(0.5, 0.5, 0.34, 0.20);
            c.rect(0.46, 0.16, 0.54, 0.84);
            c.rect(0.16, 0.46, 0.84, 0.54);
        }
        HudIcon::SlotRadio => {
            c.rect(0.30, 0.62, 0.70, 0.88);
            c.rect(0.47, 0.18, 0.53, 0.62);
            c.disc(0.5, 0.16, 0.07);
        }
        HudIcon::StatHp => {
            c.rect(0.42, 0.18, 0.58, 0.82);
            c.rect(0.18, 0.42, 0.82, 0.58);
        }
        HudIcon::StatSpeed => {
            c.tri([0.30, 0.18], [0.30, 0.82], [0.78, 0.5]);
        }
        HudIcon::StatPower => {
            c.tri([0.56, 0.12], [0.28, 0.56], [0.52, 0.56]);
            c.tri([0.48, 0.88], [0.72, 0.44], [0.48, 0.44]);
        }
        HudIcon::StatTraverse => {
            c.ring(0.5, 0.5, 0.34, 0.20);
            c.tri([0.5, 0.06], [0.34, 0.28], [0.66, 0.28]);
        }
        HudIcon::StatPenetration => {
            c.rect(0.12, 0.46, 0.66, 0.54);
            c.tri([0.62, 0.34], [0.62, 0.66], [0.90, 0.5]);
            c.rect(0.74, 0.20, 0.82, 0.80);
        }
        HudIcon::StatReload => {
            c.ring(0.5, 0.5, 0.36, 0.24);
            c.rect(0.47, 0.30, 0.53, 0.52);
            c.rect(0.50, 0.48, 0.70, 0.54);
        }
        HudIcon::StatSignal => {
            c.disc(0.5, 0.78, 0.08);
            c.ring(0.5, 0.78, 0.30, 0.22);
            c.ring(0.5, 0.78, 0.52, 0.44);
        }
    }
    c.px
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_icon_rasters_to_a_nonempty_mask() {
        for icon in HudIcon::ALL {
            let mask = raster(icon);
            assert_eq!(mask.len(), (ICON_PX * ICON_PX) as usize);
            assert!(mask.iter().any(|&p| p > 0), "{icon:?} drew nothing");
        }
    }
}
