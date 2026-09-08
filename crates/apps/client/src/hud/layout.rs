//! The HUD layout (interface program H21): every instrument but the reticle has a PLACEMENT
//! — its designed anchor, a nudge from its designed place, a scale — and the layout is three
//! presets over those placements, persisted to `hud_layout.json` on the garage's pattern.
//! The instruments themselves know nothing of this: the builder hands each one a context
//! nudged and scaled for it (`Ui::nudged`), and the reticle stack never gets one.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use ui_kit::ui::{Anchor, Ui};

use super::elements::HudElement;
use crate::ui_strings::battle as words;

/// The movable instruments. Append-only: the slugs are the file's keys.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Instrument {
    TopBar,
    TeamLists,
    /// The sixth-sense lamp, the budget line, the team's word and the spectate strip: one
    /// stack under the top bar.
    TopStack,
    Minimap,
    DamagePanel,
    Speed,
    Ammo,
    HitLog,
    KillFeed,
    NetReadout,
}

impl Instrument {
    pub const ALL: [Instrument; 10] = [
        Instrument::TopBar,
        Instrument::TeamLists,
        Instrument::TopStack,
        Instrument::Minimap,
        Instrument::DamagePanel,
        Instrument::Speed,
        Instrument::Ammo,
        Instrument::HitLog,
        Instrument::KillFeed,
        Instrument::NetReadout,
    ];

    pub fn slug(self) -> &'static str {
        match self {
            Instrument::TopBar => "top_bar",
            Instrument::TeamLists => "team_lists",
            Instrument::TopStack => "top_stack",
            Instrument::Minimap => "minimap",
            Instrument::DamagePanel => "damage_panel",
            Instrument::Speed => "speed",
            Instrument::Ammo => "ammo",
            Instrument::HitLog => "hit_log",
            Instrument::KillFeed => "kill_feed",
            Instrument::NetReadout => "net_readout",
        }
    }

    pub fn from_slug(slug: &str) -> Option<Instrument> {
        Instrument::ALL.into_iter().find(|instrument| instrument.slug() == slug)
    }

    /// The word on the editor's frame.
    pub fn name(self) -> &'static str {
        match self {
            Instrument::TopBar => words::INSTRUMENT_TOP_BAR,
            Instrument::TeamLists => words::INSTRUMENT_TEAM_LISTS,
            Instrument::TopStack => words::INSTRUMENT_TOP_STACK,
            Instrument::Minimap => words::INSTRUMENT_MINIMAP,
            Instrument::DamagePanel => words::INSTRUMENT_DAMAGE_PANEL,
            Instrument::Speed => words::INSTRUMENT_SPEED,
            Instrument::Ammo => words::INSTRUMENT_AMMO,
            Instrument::HitLog => words::INSTRUMENT_HIT_LOG,
            Instrument::KillFeed => words::INSTRUMENT_KILL_FEED,
            Instrument::NetReadout => words::INSTRUMENT_NET,
        }
    }

    /// Where the instrument hangs by design (the ears mirror: the enemy's is the right one).
    pub fn default_anchor(self) -> Anchor {
        match self {
            Instrument::TopBar | Instrument::TopStack => Anchor::Top,
            Instrument::TeamLists | Instrument::KillFeed | Instrument::NetReadout => {
                Anchor::TopRight
            }
            Instrument::Minimap => Anchor::BottomRight,
            Instrument::DamagePanel | Instrument::Speed => Anchor::BottomLeft,
            Instrument::Ammo => Anchor::Bottom,
            Instrument::HitLog => Anchor::Center,
        }
    }
}

/// Which instrument an element belongs to; `None` for the reticle stack, the markers, the
/// wheel, the banner, the modal — the parts of the HUD the editor never moves.
pub fn instrument_of(id: HudElement) -> Option<Instrument> {
    use HudElement as E;
    Some(match id {
        E::TopBar
        | E::TopBarClock
        | E::TopBarClockGlass
        | E::TopBarAllyFrags
        | E::TopBarEnemyFrags
        | E::TopBarAllyPool
        | E::TopBarEnemyPool => Instrument::TopBar,
        E::TeamRow { .. } => Instrument::TeamLists,
        E::SixthSenseLamp | E::SixthSenseText | E::BudgetLine | E::TeamWord | E::Spectate(_) => {
            Instrument::TopStack
        }
        E::Minimap
        | E::MinimapPlate
        | E::MinimapRelief
        | E::MinimapGlass
        | E::MinimapBlip(_)
        | E::MinimapSeat(_)
        | E::MinimapSeatPlate(_)
        | E::MinimapGridLabel(_) => Instrument::Minimap,
        E::DamagePanel(_) => Instrument::DamagePanel,
        E::Speed(_) => Instrument::Speed,
        E::Ammo(_) => Instrument::Ammo,
        E::HitLog(_) => Instrument::HitLog,
        E::KillFeed(_) => Instrument::KillFeed,
        E::NetReadout => Instrument::NetReadout,
        _ => return None,
    })
}

/// One instrument's placement: where it hangs, how far from its designed place, how big.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Placement {
    pub anchor: Anchor,
    /// The nudge from the designed place, in `u`: +x right, +y down.
    pub offset_u: [f32; 2],
    pub scale: f32,
}

/// The three presets: what shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Preset {
    /// The essentials: no kill feed, no connection line, no budget line, the hit log folded.
    Minimal,
    #[default]
    Standard,
    /// Everything, the hit log open.
    Full,
}

impl Preset {
    pub const ALL: [Preset; 3] = [Preset::Minimal, Preset::Standard, Preset::Full];

    pub fn slug(self) -> &'static str {
        match self {
            Preset::Minimal => "minimal",
            Preset::Standard => "standard",
            Preset::Full => "full",
        }
    }

    pub fn from_slug(slug: &str) -> Option<Preset> {
        Preset::ALL.into_iter().find(|preset| preset.slug() == slug)
    }
}

/// The layout: a preset and the placements that differ from the design.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct HudLayout {
    pub preset: Preset,
    nudges: BTreeMap<Instrument, [f32; 2]>,
    scales: BTreeMap<Instrument, f32>,
}

impl HudLayout {
    pub fn placement(&self, instrument: Instrument) -> Placement {
        Placement {
            anchor: instrument.default_anchor(),
            offset_u: self.nudge(instrument),
            scale: self.scale(instrument),
        }
    }

    pub fn nudge(&self, instrument: Instrument) -> [f32; 2] {
        self.nudges.get(&instrument).copied().unwrap_or([0.0, 0.0])
    }

    pub fn scale(&self, instrument: Instrument) -> f32 {
        self.scales.get(&instrument).copied().unwrap_or(1.0)
    }

    pub fn set_nudge(&mut self, instrument: Instrument, nudge_u: [f32; 2]) {
        if nudge_u == [0.0, 0.0] {
            self.nudges.remove(&instrument);
        } else {
            self.nudges.insert(instrument, nudge_u);
        }
    }

    pub fn set_scale(&mut self, instrument: Instrument, scale: f32) {
        let scale = scale.clamp(0.5, 2.0);
        if (scale - 1.0).abs() < 1e-6 {
            self.scales.remove(&instrument);
        } else {
            self.scales.insert(instrument, scale);
        }
    }

    /// Ctrl+R: the design, at the standard preset.
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    /// Whether the preset shows the instrument. The policy's word (`docs/interface-policy.md`,
    /// "Three presets"): standard is EVERYTHING BUT the budget line and the RTT lamp — the
    /// code showed both in standard until U22 (2026-09-08); full shows them.
    pub fn shows(&self, instrument: Instrument) -> bool {
        match self.preset {
            Preset::Minimal => !matches!(instrument, Instrument::KillFeed | Instrument::NetReadout),
            Preset::Standard => instrument != Instrument::NetReadout,
            Preset::Full => true,
        }
    }

    /// Whether the preset shows the budget line (the lamp always shows): full only (U22).
    pub fn shows_budget(&self) -> bool {
        self.preset == Preset::Full
    }

    /// The hit log's fold the preset asks for; N still toggles it in the moment.
    pub fn hit_log_folded(&self) -> bool {
        self.preset == Preset::Minimal
    }

    /// U15 (2026-09-08): the rows are short by default; N prints the detail in Standard, Full
    /// always prints it, Minimal never does (one short row).
    pub fn hit_log_detail(&self, detail_by_key: bool) -> bool {
        match self.preset {
            Preset::Minimal => false,
            Preset::Standard => detail_by_key,
            Preset::Full => true,
        }
    }

    /// The context one instrument lays out in: nudged and scaled by its placement.
    pub fn ui_for(&self, ui: &Ui, instrument: Instrument) -> Ui {
        let placement = self.placement(instrument);
        ui.nudged(placement.offset_u, placement.scale)
    }
}

// ---------------------------------------------------------------- the file

const SAVE_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct LayoutFile {
    version: u32,
    #[serde(default)]
    preset: String,
    #[serde(default)]
    nudges: BTreeMap<String, [f32; 2]>,
    #[serde(default)]
    scales: BTreeMap<String, f32>,
}

impl HudLayout {
    fn to_file(&self) -> LayoutFile {
        LayoutFile {
            version: SAVE_VERSION,
            preset: self.preset.slug().to_string(),
            nudges: self.nudges.iter().map(|(i, n)| (i.slug().to_string(), *n)).collect(),
            scales: self.scales.iter().map(|(i, s)| (i.slug().to_string(), *s)).collect(),
        }
    }

    /// A layout off the file: unknown instruments and presets are dropped, never guessed.
    fn from_file(file: &LayoutFile) -> Self {
        let mut layout = Self {
            preset: Preset::from_slug(&file.preset).unwrap_or_default(),
            ..Default::default()
        };
        for (slug, nudge) in &file.nudges {
            if let Some(instrument) = Instrument::from_slug(slug) {
                layout.set_nudge(instrument, *nudge);
            }
        }
        for (slug, scale) in &file.scales {
            if let Some(instrument) = Instrument::from_slug(slug) {
                layout.set_scale(instrument, *scale);
            }
        }
        layout
    }
}

/// Where the layout lives: `hud_layout.json` beside the garage's save.
pub(crate) fn layout_path() -> PathBuf {
    crate::app::garage::persistence::config_dir()
        .map_or_else(|| PathBuf::from("hud_layout.json"), |dir| dir.join("hud_layout.json"))
}

/// The layout at `path`; a missing, unreadable or foreign-version file is `None` (the design).
pub(crate) fn load_layout(path: &Path) -> Option<HudLayout> {
    let raw = std::fs::read_to_string(path).ok()?;
    match serde_json::from_str::<LayoutFile>(&raw) {
        Ok(file) if file.version == SAVE_VERSION => Some(HudLayout::from_file(&file)),
        Ok(file) => {
            tracing::warn!(
                found = file.version,
                expected = SAVE_VERSION,
                "hud layout version mismatch; using the design"
            );
            None
        }
        Err(error) => {
            tracing::warn!(%error, "hud layout is unreadable; using the design");
            None
        }
    }
}

/// Persist `layout` to `path` atomically; a failed write warns and is otherwise ignored.
pub(crate) fn store_layout(path: &Path, layout: &HudLayout) {
    if let Some(parent) = path.parent()
        && let Err(error) = std::fs::create_dir_all(parent)
    {
        tracing::warn!(%error, "could not create the hud layout directory");
        return;
    }
    let json = match serde_json::to_string_pretty(&layout.to_file()) {
        Ok(json) => json,
        Err(error) => {
            tracing::warn!(%error, "could not serialize the hud layout");
            return;
        }
    };
    let tmp = path.with_extension("json.tmp");
    if let Err(error) = std::fs::write(&tmp, json) {
        tracing::warn!(%error, "could not write the hud layout temp");
        return;
    }
    if let Err(error) = std::fs::rename(&tmp, path) {
        tracing::warn!(%error, "could not replace the hud layout");
        let _ = std::fs::remove_file(&tmp);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hud::{build_battle_hud_list, demo};
    use ui_kit::draw_list::Payload;

    fn everything() -> crate::hud::BattleHudModel {
        let mut model = demo::demo_model(false);
        model.sixth_sense_lit = true;
        model.kill_feed = Some(demo::demo_kill_feed());
        model.net = Some(crate::hud::net_readout::NetReadoutModel {
            local: false,
            rtt_ms: Some(48),
            snapshot_age_ms: 32,
        });
        model.team_word = Some(demo::demo_team_word());
        model
    }

    /// H21: every instrument moves with its placement — every element of it by exactly the
    /// nudge, the minimap's overlay too — and the reticle never does; a layout written to
    /// disk comes back the same.
    #[test]
    fn every_hud_element_but_the_reticle_is_movable_and_its_placement_survives_a_restart() {
        let ui = Ui::reference();
        let designed = build_battle_hud_list(&everything(), &ui);
        for instrument in Instrument::ALL {
            let mut model = everything();
            model.layout.set_nudge(instrument, [40.0, 30.0]);
            let moved = build_battle_hud_list(&model, &ui);
            let mut counted = 0;
            for (a, b) in designed.iter().zip(moved.iter()) {
                assert_eq!(a.id, b.id, "{instrument:?}: the list keeps its order");
                let mine = instrument_of(a.id) == Some(instrument);
                match (&a.payload, &b.payload) {
                    (Payload::Legacy(va), Payload::Legacy(vb)) => {
                        for (pa, pb) in va.iter().zip(vb.iter()) {
                            let dx = (pb.position[0] - pa.position[0]) * 960.0;
                            let dy = (pa.position[1] - pb.position[1]) * 540.0;
                            if mine {
                                assert!(
                                    (dx - 40.0).abs() < 0.05 && (dy - 30.0).abs() < 0.05,
                                    "{instrument:?} {:?}: moved by ({dx:.2}, {dy:.2})",
                                    a.id
                                );
                            } else {
                                assert!(
                                    dx.abs() < 1e-4 && dy.abs() < 1e-4,
                                    "{:?} moved with {instrument:?}",
                                    a.id
                                );
                            }
                        }
                    }
                    _ => {
                        let (dx, dy) = (b.rect.x - a.rect.x, b.rect.y - a.rect.y);
                        if mine {
                            assert!(
                                (dx - 40.0).abs() < 0.05 && (dy - 30.0).abs() < 0.05,
                                "{instrument:?} {:?}: moved by ({dx:.2}, {dy:.2})",
                                a.id
                            );
                        } else {
                            assert!(
                                dx.abs() < 1e-4 && dy.abs() < 1e-4,
                                "{:?} moved with {instrument:?}",
                                a.id
                            );
                        }
                    }
                }
                counted += usize::from(mine);
            }
            assert!(counted > 0, "{instrument:?} is on the staged HUD");
        }
        assert_eq!(instrument_of(HudElement::Reticle), None, "the reticle is not movable");
        assert_eq!(instrument_of(HudElement::Readouts), None);
        // The file.
        let mut layout = HudLayout { preset: Preset::Full, ..Default::default() };
        layout.set_nudge(Instrument::Minimap, [-120.0, 0.0]);
        layout.set_scale(Instrument::Ammo, 1.25);
        let dir = std::env::temp_dir().join(format!("wot-hud-layout-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let path = dir.join("hud_layout.json");
        assert!(load_layout(&path).is_none(), "no file: the design");
        store_layout(&path, &layout);
        assert_eq!(load_layout(&path), Some(layout.clone()));
        std::fs::write(&path, "{ not json").expect("scribble");
        assert!(load_layout(&path).is_none(), "a corrupt file is the design, never a panic");
        let _ = std::fs::remove_dir_all(&dir);
        // The presets.
        let minimal = HudLayout { preset: Preset::Minimal, ..Default::default() };
        assert!(!minimal.shows(Instrument::KillFeed) && minimal.shows(Instrument::Minimap));
        assert!(minimal.hit_log_folded() && !minimal.hit_log_detail(true));
        let full = HudLayout { preset: Preset::Full, ..Default::default() };
        assert!(
            !full.hit_log_folded() && full.hit_log_detail(false),
            "FULL keeps the log open, in detail"
        );
        let standard = HudLayout { preset: Preset::Standard, ..Default::default() };
        assert!(
            !standard.hit_log_folded()
                && !standard.hit_log_detail(false)
                && standard.hit_log_detail(true)
        );
        // U22: the policy's presets, to the word — standard is everything but the budget line
        // and the RTT lamp; full shows both; minimal neither.
        assert!(!standard.shows_budget() && !standard.shows(Instrument::NetReadout));
        assert!(standard.shows(Instrument::KillFeed) && standard.shows(Instrument::Minimap));
        assert!(full.shows_budget() && full.shows(Instrument::NetReadout));
        assert!(!minimal.shows_budget() && !minimal.shows(Instrument::NetReadout));
        layout.reset();
        assert_eq!(layout, HudLayout::default());
    }
}
