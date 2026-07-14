use std::sync::Arc;
use std::time::Instant;

use tracing::{error, info};
use winit::application::ApplicationHandler;
use winit::event::{DeviceEvent, DeviceId, ElementState, MouseButton, WindowEvent};
use winit::event_loop::ActiveEventLoop;
use winit::window::{WindowAttributes, WindowId};

use super::ClientApp;
use crate::ClientLoopEvent;

impl ApplicationHandler for ClientApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let attrs = WindowAttributes::default()
            .with_title(crate::ui_strings::WINDOW_TITLE)
            .with_inner_size(winit::dpi::LogicalSize::new(1280.0, 720.0))
            .with_maximized(true);
        let window = match event_loop.create_window(attrs) {
            Ok(window) => Arc::new(window),
            Err(error) => {
                error!(%error, "failed to create window");
                event_loop.exit();
                return;
            }
        };
        let size = window.inner_size();
        self.viewport = (size.width.max(1), size.height.max(1));
        if let Err(error) = self.create_renderer(window.clone(), size.width, size.height) {
            error!(%error, "failed to create renderer");
            event_loop.exit();
            return;
        }
        // Pace the presentation to the actual display (F1). Fallback: 60 Hz.
        let refresh_hz = window
            .current_monitor()
            .and_then(|monitor| monitor.refresh_rate_millihertz())
            .map_or(60.0, |millihertz| f64::from(millihertz) / 1000.0);
        self.loop_driver.set_present_hz(refresh_hz);
        // F5: without 1 ms scheduler resolution, Windows quantizes the pacer's WaitUntil to
        // ~15.6 ms ticks — frames land on 16/31/47 ms (60/30/20 FPS), the exact oscillation
        // the pacer exists to remove.
        let millisecond_timer = timer_resolution::request_millisecond_timer();
        info!(refresh_hz, millisecond_timer, "frame pacing locked to the display");
        if cfg!(debug_assertions) {
            tracing::warn!(
                "DEV BUILD (opt-level 1): performance is NOT representative — use `cargo run --release -p client`"
            );
        }
        info!("client ready: WASD drive, mouse aim + camera, Space/left-click fire, Esc cursor");
        self.audio = crate::audio_out::AudioOutput::try_new();
        self.window = Some(window);
        // The garage is a mouse-driven menu: show the cursor there; the battle view captures it.
        self.set_cursor_captured(!self.garage.is_open());
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        let actions = match event {
            WindowEvent::CloseRequested => {
                self.loop_driver.handle_event(ClientLoopEvent::CloseRequested)
            }
            WindowEvent::Resized(size) => self
                .loop_driver
                .handle_event(ClientLoopEvent::Resized { width: size.width, height: size.height }),
            WindowEvent::RedrawRequested => {
                self.loop_driver.handle_event(ClientLoopEvent::RedrawRequested)
            }
            WindowEvent::KeyboardInput { event, .. } => {
                self.on_keyboard(&event);
                self.loop_driver.handle_event(ClientLoopEvent::Input)
            }
            WindowEvent::MouseWheel { delta, .. } => {
                self.on_mouse_wheel(delta);
                Vec::new()
            }
            WindowEvent::CursorMoved { position, .. } => {
                self.on_cursor_moved(position.x as f32, position.y as f32);
                Vec::new()
            }
            WindowEvent::MouseInput { state, button, .. } => {
                if self.garage.is_open() {
                    // Garage menu: left click drives selection / Battle / orbit, cursor stays free.
                    // Right click cycles a module slot backward (no other hit acts on it).
                    if button == MouseButton::Left {
                        match state {
                            ElementState::Pressed => self.garage_primary_press(),
                            ElementState::Released => self.garage_primary_release(),
                        }
                    } else if button == MouseButton::Right && state == ElementState::Pressed {
                        self.garage_secondary_press();
                    }
                } else if state == ElementState::Pressed {
                    self.set_cursor_captured(true);
                    if button == MouseButton::Left {
                        self.input.fire_pending = true;
                    }
                }
                Vec::new()
            }
            WindowEvent::ModifiersChanged(modifiers) => {
                self.input.set_shift(modifiers.state().shift_key());
                Vec::new()
            }
            WindowEvent::Focused(focused) => {
                // Keep the cursor free in the garage menu; only the battle view captures it.
                self.set_cursor_captured(focused && !self.garage.is_open());
                Vec::new()
            }
            _ => Vec::new(),
        };
        self.handle_actions(event_loop, actions);
    }

    fn device_event(&mut self, _loop: &ActiveEventLoop, _id: DeviceId, event: DeviceEvent) {
        if let DeviceEvent::MouseMotion { delta } = event {
            self.input.mouse_dx += delta.0 as f32;
            self.input.mouse_dy += delta.1 as f32;
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        let now = Instant::now();
        let elapsed = now.saturating_duration_since(self.last_loop_time);
        self.last_loop_time = now;
        let actions = self.loop_driver.handle_event(ClientLoopEvent::AboutToWait { elapsed });
        self.handle_actions(event_loop, actions);
        // Sleep until the next tick or presentation beat instead of spinning (F1): the pacer
        // owns the cadence, input events still wake the loop immediately.
        event_loop.set_control_flow(winit::event_loop::ControlFlow::WaitUntil(
            Instant::now() + self.loop_driver.suggested_wait(),
        ));
    }
}
