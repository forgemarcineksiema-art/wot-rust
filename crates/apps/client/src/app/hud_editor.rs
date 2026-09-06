//! The HUD editor (interface program H21): opened from the escape menu, the cursor free over
//! the living HUD; a press on an instrument's frame takes it, the mouse moves it, the release
//! drops it; 1/2/3 pick the preset, Ctrl+R the design, Esc closes — and every close writes
//! `hud_layout.json`. The battle runs on underneath; the reticle is never on offer.

use std::path::PathBuf;

use ui_kit::rect::Rect;

use super::ClientApp;
use crate::hud::editor::EditorModel;
use crate::hud::layout::{HudLayout, Instrument, Preset, layout_path, load_layout, store_layout};

/// The editor's state while it is open.
#[derive(Debug, Clone, PartialEq, Default)]
pub(crate) struct HudEditorState {
    pub hovered: Option<Instrument>,
    /// The instrument in hand: where it was grabbed, and its nudge when it was.
    pub drag: Option<(Instrument, [f32; 2], [f32; 2])>,
    /// Whether a Control key is down (Ctrl+R).
    pub ctrl: bool,
}

impl HudEditorState {
    pub fn model(&self) -> EditorModel {
        EditorModel { hovered: self.hovered, dragging: self.drag.map(|(i, _, _)| i) }
    }
}

impl ClientApp {
    /// Turn on layout persistence at `path`, loading what is there first; the real startup
    /// path calls this once, tests never touch the user's file.
    pub(super) fn enable_layout_persistence(&mut self, path: PathBuf) {
        if let Some(layout) = load_layout(&path) {
            self.layout = layout;
        }
        self.layout_path = Some(path);
    }

    /// The default file beside the garage's save.
    pub(super) fn default_layout_path() -> PathBuf {
        layout_path()
    }

    fn persist_layout(&self) {
        if let Some(path) = &self.layout_path {
            store_layout(path, &self.layout);
        }
    }

    pub(crate) fn hud_layout(&self) -> &HudLayout {
        &self.layout
    }

    /// From the escape menu: the menu closes, the cursor is freed, the editor takes the keys.
    pub(super) fn open_hud_editor(&mut self) {
        self.pause_menu = None;
        self.command_wheel = None;
        self.input.release_driving();
        self.hud_editor = Some(HudEditorState::default());
        self.set_cursor_captured(false);
    }

    /// Esc: the layout is written, the cursor captured, the battle has the keys again.
    pub(super) fn close_hud_editor(&mut self) {
        if self.hud_editor.take().is_some() {
            self.persist_layout();
            self.input.clear_mouse_look();
            self.set_cursor_captured(true);
        }
    }

    pub(crate) fn hud_editor_open(&self) -> bool {
        self.hud_editor.is_some()
    }

    /// The frame under a screen point, the smallest wins when frames overlap.
    fn instrument_at(&self, point_px: [f32; 2]) -> Option<Instrument> {
        self.hud_frames
            .iter()
            .filter(|(_, rect)| rect.contains(point_px))
            .min_by(|a, b| (a.1.w * a.1.h).total_cmp(&(b.1.w * b.1.h)))
            .map(|(instrument, _)| *instrument)
    }

    /// Pixels per `u` on the current viewport, at scale one.
    fn px_per_u(&self) -> f32 {
        self.viewport.1 as f32 / ui_kit::ui::REFERENCE_HEIGHT_PX
    }

    /// The cursor moved: the hover follows it; an instrument in hand follows it too.
    pub(super) fn hud_editor_cursor(&mut self, cursor_px: [f32; 2]) {
        let Some(editor) = self.hud_editor.as_mut() else { return };
        editor.hovered = None;
        if let Some((instrument, grab, start)) = editor.drag {
            let per_u = self.px_per_u().max(1e-3);
            let nudge = [
                (start[0] + (cursor_px[0] - grab[0]) / per_u).round(),
                (start[1] + (cursor_px[1] - grab[1]) / per_u).round(),
            ];
            self.layout.set_nudge(instrument, nudge);
            if let Some(editor) = self.hud_editor.as_mut() {
                editor.hovered = Some(instrument);
            }
            return;
        }
        let hovered = self.instrument_at(cursor_px);
        if let Some(editor) = self.hud_editor.as_mut() {
            editor.hovered = hovered;
        }
    }

    /// A press takes the instrument under the cursor in hand.
    pub(super) fn hud_editor_press(&mut self) {
        let cursor = self.cursor_px;
        let Some(instrument) = self.instrument_at(cursor) else { return };
        let start = self.layout.nudge(instrument);
        if let Some(editor) = self.hud_editor.as_mut() {
            editor.drag = Some((instrument, cursor, start));
            editor.hovered = Some(instrument);
        }
        self.queue_audio(audio::AudioEvent::UiClick { accent: false });
    }

    /// A release drops it where it is and writes the layout.
    pub(super) fn hud_editor_release(&mut self) {
        if let Some(editor) = self.hud_editor.as_mut()
            && editor.drag.take().is_some()
        {
            self.persist_layout();
        }
    }

    /// The editor's keys: 1/2/3 the presets, Ctrl+R the design, Esc done. Returns whether the
    /// key was the editor's.
    pub(super) fn hud_editor_key(
        &mut self,
        key: winit::keyboard::PhysicalKey,
        pressed: bool,
    ) -> bool {
        use winit::keyboard::{KeyCode, PhysicalKey};
        let Some(editor) = self.hud_editor.as_mut() else { return false };
        match key {
            PhysicalKey::Code(KeyCode::ControlLeft | KeyCode::ControlRight) => {
                editor.ctrl = pressed;
            }
            PhysicalKey::Code(KeyCode::Escape) if pressed => self.close_hud_editor(),
            PhysicalKey::Code(KeyCode::Digit1) if pressed => self.set_preset(Preset::Minimal),
            PhysicalKey::Code(KeyCode::Digit2) if pressed => self.set_preset(Preset::Standard),
            PhysicalKey::Code(KeyCode::Digit3) if pressed => self.set_preset(Preset::Full),
            PhysicalKey::Code(KeyCode::KeyR) if pressed && editor.ctrl => {
                self.layout.reset();
                self.persist_layout();
                self.queue_audio(audio::AudioEvent::UiClick { accent: true });
            }
            _ => {}
        }
        true
    }

    fn set_preset(&mut self, preset: Preset) {
        self.layout.preset = preset;
        self.persist_layout();
        self.queue_audio(audio::AudioEvent::UiClick { accent: false });
    }

    /// The frames the editor hit-tests against: the last built HUD's, per instrument.
    pub(super) fn remember_hud_frames(&mut self, frames: Vec<(Instrument, Rect)>) {
        self.hud_frames = frames;
    }
}
