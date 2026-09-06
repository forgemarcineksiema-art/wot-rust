//! The interaction machine (interface program F4/F7): hover, press, release, keyboard focus and
//! the tooltip delay, for any draw list whose elements are keyed by `K`.
//!
//! One machine per screen. The builder asks `state_of(id, disabled)` for every element it
//! emits, so the pressed plate is inset and the hovered one washed by the emitter, not by
//! per-element code; the app feeds the machine the cursor, the button and the keys, and reads
//! back the element a click landed on. A click is a RELEASE on the element that was pressed —
//! press on one control, drag to another, release: nothing fires, which is how every desktop
//! has behaved for forty years.

use std::hash::Hash;

use crate::draw_list::{DrawList, WidgetState};

/// A key the machine understands; the app maps its own bindings onto these.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NavKey {
    /// Tab: focus the next interactive element in layout order.
    Next,
    /// Shift+Tab.
    Prev,
    /// Enter or Space: activate the focused element.
    Activate,
    /// Escape: drop focus.
    Escape,
}

/// How long the cursor rests on an element before its tooltip shows.
pub const TOOLTIP_DELAY_S: f32 = 0.45;

#[derive(Debug, Clone, PartialEq)]
pub struct Interaction<K> {
    cursor_px: [f32; 2],
    hover: Option<K>,
    pressed: Option<K>,
    focus: Option<K>,
    /// How long the cursor has rested on `hover`.
    hover_age_s: f32,
    /// Whether the keyboard moved the focus after the mouse last moved: the ring shows only then.
    keyboard_led: bool,
}

impl<K: Copy + Eq + Hash + std::fmt::Debug> Default for Interaction<K> {
    fn default() -> Self {
        Self {
            cursor_px: [-1.0, -1.0],
            hover: None,
            pressed: None,
            focus: None,
            hover_age_s: 0.0,
            keyboard_led: false,
        }
    }
}

impl<K: Copy + Eq + Hash + std::fmt::Debug> Interaction<K> {
    pub fn cursor_px(&self) -> [f32; 2] {
        self.cursor_px
    }

    pub fn hover(&self) -> Option<K> {
        self.hover
    }

    pub fn pressed(&self) -> Option<K> {
        self.pressed
    }

    pub fn focus(&self) -> Option<K> {
        self.focus
    }

    /// The cursor moved to `px`; `list` is the screen it moves over.
    pub fn on_cursor(&mut self, px: [f32; 2], list: &DrawList<K>) {
        self.cursor_px = px;
        self.keyboard_led = false;
        let now = list.hit(px);
        if now != self.hover {
            self.hover = now;
            self.hover_age_s = 0.0;
        }
    }

    /// The primary button went down over `list`.
    pub fn on_press(&mut self, list: &DrawList<K>) {
        self.pressed = list.hit(self.cursor_px);
        if self.pressed.is_some() {
            self.focus = self.pressed;
            self.keyboard_led = false;
        }
    }

    /// The primary button came up: the click, if the cursor is still on what it pressed.
    pub fn on_release(&mut self, list: &DrawList<K>) -> Option<K> {
        let pressed = self.pressed.take()?;
        (list.hit(self.cursor_px) == Some(pressed)).then_some(pressed)
    }

    /// A navigation key; returns the element to activate, if any.
    pub fn on_key(&mut self, key: NavKey, list: &DrawList<K>) -> Option<K> {
        let order: Vec<K> = list
            .iter()
            .filter(|e| e.interactive && e.state != WidgetState::Disabled)
            .map(|e| e.id)
            .collect();
        match key {
            NavKey::Next | NavKey::Prev => {
                if order.is_empty() {
                    self.focus = None;
                    return None;
                }
                let at = self.focus.and_then(|f| order.iter().position(|id| *id == f));
                let forward = key == NavKey::Next;
                let next = match at {
                    Some(i) if forward => (i + 1) % order.len(),
                    Some(i) => (i + order.len() - 1) % order.len(),
                    None if forward => 0,
                    None => order.len() - 1,
                };
                self.focus = Some(order[next]);
                self.keyboard_led = true;
                None
            }
            NavKey::Activate => self.focus.filter(|f| order.contains(f)),
            NavKey::Escape => {
                self.focus = None;
                None
            }
        }
    }

    /// Time passes; the tooltip clock runs while the cursor rests.
    pub fn tick(&mut self, dt_s: f32) {
        if self.hover.is_some() {
            self.hover_age_s += dt_s.max(0.0);
        }
    }

    /// The element whose tooltip is due: hovered for the delay, not pressed.
    pub fn tooltip(&self) -> Option<K> {
        (self.hover_age_s >= TOOLTIP_DELAY_S && self.pressed.is_none())
            .then_some(self.hover)
            .flatten()
    }

    /// What the emitter should draw `id` as. `disabled` is the builder's word; the machine's
    /// own record decides between the rest.
    pub fn state_of(&self, id: K, disabled: bool) -> WidgetState {
        if disabled {
            WidgetState::Disabled
        } else if self.pressed == Some(id) && self.hover == Some(id) {
            WidgetState::Pressed
        } else if self.hover == Some(id) {
            WidgetState::Hover
        } else if self.focus == Some(id) && self.keyboard_led {
            WidgetState::Focused
        } else {
            WidgetState::Idle
        }
    }

    /// The press is void — the screen changed under it (a modal list closed on it) — so no
    /// release will fire it.
    pub fn cancel_press(&mut self) {
        self.pressed = None;
    }

    /// The screen went away (a modal opened, the garage closed): every latch drops.
    pub fn clear(&mut self) {
        self.hover = None;
        self.pressed = None;
        self.focus = None;
        self.hover_age_s = 0.0;
        self.keyboard_led = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::draw_list::{Element, Payload};
    use crate::rect::Rect;

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    enum Id {
        A,
        B,
        C,
    }

    fn screen(disabled: Option<Id>) -> DrawList<Id> {
        let mut list = DrawList::new();
        for (i, id) in [Id::A, Id::B, Id::C].into_iter().enumerate() {
            let mut e = Element::new(
                id,
                Rect::new(i as f32 * 100.0, 0.0, 80.0, 40.0),
                Payload::Legacy(Vec::new()),
            );
            e.interactive = true;
            if disabled == Some(id) {
                e.state = WidgetState::Disabled;
            }
            list.push(e);
        }
        list
    }

    #[test]
    fn a_click_is_a_release_on_the_element_that_was_pressed() {
        let list = screen(None);
        let mut m = Interaction::<Id>::default();
        m.on_cursor([10.0, 10.0], &list);
        m.on_press(&list);
        assert_eq!(m.state_of(Id::A, false), WidgetState::Pressed);
        m.on_cursor([110.0, 10.0], &list);
        assert_eq!(m.on_release(&list), None, "released over B after pressing A: nothing fires");
        m.on_cursor([110.0, 10.0], &list);
        m.on_press(&list);
        assert_eq!(m.on_release(&list), Some(Id::B));
    }

    #[test]
    fn tab_walks_focus_in_layout_order_and_skips_disabled() {
        let list = screen(Some(Id::B));
        let mut m = Interaction::<Id>::default();
        assert_eq!(m.on_key(NavKey::Next, &list), None);
        assert_eq!(m.focus(), Some(Id::A));
        assert_eq!(m.state_of(Id::A, false), WidgetState::Focused, "the ring shows after a key");
        m.on_key(NavKey::Next, &list);
        assert_eq!(m.focus(), Some(Id::C), "B is disabled and skipped");
        m.on_key(NavKey::Next, &list);
        assert_eq!(m.focus(), Some(Id::A), "wraps");
        assert_eq!(m.on_key(NavKey::Activate, &list), Some(Id::A));
        m.on_key(NavKey::Escape, &list);
        assert_eq!(m.focus(), None);
    }

    #[test]
    fn a_tooltip_waits_its_delay_and_dies_with_the_hover() {
        let list = screen(None);
        let mut m = Interaction::<Id>::default();
        m.on_cursor([10.0, 10.0], &list);
        m.tick(0.2);
        assert_eq!(m.tooltip(), None);
        m.tick(0.3);
        assert_eq!(m.tooltip(), Some(Id::A));
        m.on_cursor([500.0, 500.0], &list);
        assert_eq!(m.tooltip(), None);
        assert_eq!(m.state_of(Id::A, false), WidgetState::Idle);
    }

    #[test]
    fn the_mouse_hides_the_focus_ring_and_a_disabled_element_says_so() {
        let list = screen(None);
        let mut m = Interaction::<Id>::default();
        m.on_key(NavKey::Next, &list);
        assert_eq!(m.state_of(Id::A, false), WidgetState::Focused);
        m.on_cursor([500.0, 500.0], &list);
        assert_eq!(m.state_of(Id::A, false), WidgetState::Idle, "a moving mouse hides the ring");
        assert_eq!(m.state_of(Id::A, true), WidgetState::Disabled);
        m.on_cursor([10.0, 10.0], &list);
        assert_eq!(m.state_of(Id::A, false), WidgetState::Hover);
    }
}
