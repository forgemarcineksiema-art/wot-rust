//! The garage HUD regions, arranged like the WoT beta garage. Each region draws into the shared
//! HUD vertex buffer using rects from [`super::layout`]; `overlay` orchestrates and hit-tests them.

pub(in crate::app::garage) mod carousel;
pub(in crate::app::garage) mod crew;
pub(in crate::app::garage) mod loadout;
pub(in crate::app::garage) mod stats;
pub(in crate::app::garage) mod techtree;
pub(in crate::app::garage) mod topbar;
