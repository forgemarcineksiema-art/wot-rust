//! The editor window shell: winit `ApplicationHandler` (event-loop policy), a 3D viewport
//! through the game's OWN render path (`scene_build` meshes into `renderer_wgpu`'s
//! `WindowRenderer`), a free-fly camera, and panels drawn with the client's in-house UI
//! toolkit (map-editor D3) — the editor looks like the game because it IS the game's eyes.
//!
//! Every viewport reload is a full document recompile ([`EditorDocument::recompile`]): the
//! editor can never show a world the game would not build.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use glam::Vec3;
use renderer_api::{
    Camera, CameraProjectionPolicy, HudVertex, RenderFrame, view_projection_matrix,
};
use renderer_wgpu::WindowRenderer;
use winit::application::ApplicationHandler;
use winit::event::{DeviceEvent, DeviceId, ElementState, MouseButton, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::{Window, WindowId};

use crate::{CompiledMap, EditorDocument};

/// Run the editor shell. `path` opens an existing document; `None` starts from the scratch
/// placeholder (File → New).
pub fn run(path: Option<PathBuf>) -> anyhow::Result<()> {
    let document = match &path {
        Some(path) => EditorDocument::open(path)?,
        None => EditorDocument::new_scratch(),
    };
    let event_loop = EventLoop::new()?;
    event_loop.set_control_flow(ControlFlow::Poll);
    let mut app = EditorApp::new(document);
    event_loop.run_app(&mut app)?;
    Ok(())
}

/// Free-fly viewport camera: yaw/pitch look (hold RMB), WASD + QE move, Shift sprints.
struct FlyCamera {
    eye: Vec3,
    yaw_rad: f32,
    pitch_rad: f32,
    speed_m_s: f32,
}

impl FlyCamera {
    /// Start high on the southern edge, looking down the map's long axis.
    fn overviewing(map_size: [f32; 2]) -> Self {
        Self {
            eye: Vec3::new(map_size[0] * 0.5, 90.0, -40.0),
            yaw_rad: 0.0,
            pitch_rad: -0.45,
            speed_m_s: 60.0,
        }
    }

    fn forward(&self) -> Vec3 {
        let (sin_yaw, cos_yaw) = self.yaw_rad.sin_cos();
        let (sin_pitch, cos_pitch) = self.pitch_rad.sin_cos();
        Vec3::new(sin_yaw * cos_pitch, sin_pitch, cos_yaw * cos_pitch)
    }

    fn camera(&self) -> Camera {
        let target = self.eye + self.forward();
        Camera { eye: self.eye.to_array(), target: target.to_array(), vertical_fov_degrees: 55.0 }
    }
}

#[derive(Default)]
struct InputState {
    forward: bool,
    back: bool,
    left: bool,
    right: bool,
    up: bool,
    down: bool,
    sprint: bool,
    ctrl: bool,
    looking: bool,
    mouse_dx: f32,
    mouse_dy: f32,
}

struct EditorApp {
    window: Option<Arc<Window>>,
    renderer: Option<WindowRenderer>,
    document: EditorDocument,
    compiled: CompiledMap,
    camera: FlyCamera,
    input: InputState,
    show_layers: bool,
    status: String,
    started: Instant,
    last_tick: Instant,
}

impl EditorApp {
    fn new(document: EditorDocument) -> Self {
        let compiled = document.recompile();
        let camera = FlyCamera::overviewing(compiled.battlefield.size_m);
        Self {
            window: None,
            renderer: None,
            document,
            compiled,
            camera,
            input: InputState::default(),
            show_layers: false,
            status: String::from(
                "F1 layers  F5 recompile  Ctrl+S save  Ctrl+Z/Y undo/redo  Ctrl+P playtest",
            ),
            started: Instant::now(),
            last_tick: Instant::now(),
        }
    }

    /// Full document → viewport reload: recompile, then re-upload every scene slot the
    /// battle path uploads (ground + statics + water + dressing + lighting).
    fn reload_scene(&mut self) {
        self.compiled = self.document.recompile();
        let Some(renderer) = self.renderer.as_mut() else { return };
        let battlefield = &self.compiled.battlefield;
        let blueprint = self.document.blueprint();

        let (ground, statics) =
            scene_build::battlefield::battlefield_ground_and_statics_meshes(battlefield, &[]);
        let maps = scene_build::terrain_maps::bake_terrain_ground_maps(battlefield);
        let materials = match &blueprint.materials {
            Some(spec) => scene_build::terrain_maps::material_set_from(spec),
            None => renderer_api::TerrainMaterialSet::default(),
        };
        renderer.set_terrain(&statics.0, &statics.1);
        renderer.set_battlefield_ground(&ground.0, &ground.1, &maps, &materials);
        let (water_vertices, water_indices) =
            scene_build::water::battlefield_water_mesh(battlefield);
        renderer.set_water(&water_vertices, &water_indices);
        let (dressing_vertices, dressing_indices) =
            scene_build::grass_cards::grass_card_dressing_mesh(battlefield, &maps, &materials);
        renderer.set_dressing(&dressing_vertices, &dressing_indices);

        // The document's default look lights the viewport — same binding the game uses.
        let look = match blueprint.environment.as_ref().and_then(|env| env.looks.first()) {
            Some(look) => scene_build::weather::realize_look(look),
            None => scene_build::weather::hazy_noon_fallback(),
        };
        renderer.set_outdoor_sky(look.sky.0, look.sky.1, look.sky.2);
        renderer.set_scene_lighting(look.lighting);
        renderer.set_rain_intensity(look.rain_intensity);
        renderer.set_wetness(look.wetness);

        let (errors, warnings) = report_counts(&self.compiled);
        self.status = format!(
            "compiled in {:.1} ms — {errors} errors, {warnings} warnings",
            self.compiled.compile_time.as_secs_f32() * 1000.0
        );
    }

    fn save(&mut self) {
        let result = match self.document.path() {
            Some(_) => self.document.save(),
            None => self.document.save_as(&scratch_path()),
        };
        self.status = match result {
            Ok(()) => format!("saved {}", self.document.path().unwrap().display()),
            Err(error) => format!("save failed: {error}"),
        };
    }

    /// The playtest loop (map-editor D2): save the document, then launch the client with
    /// `WOT_MAP` pointing at the file — the client's local server installs it as
    /// `MapId::Scratch` and the battle runs on exactly what the viewport shows.
    fn playtest(&mut self) {
        if self.compiled.report.has_errors() {
            self.status =
                "playtest refused: the report has errors (a broken map is a build-time bug)".into();
            return;
        }
        self.save();
        let Some(path) = self.document.path().map(std::path::Path::to_path_buf) else { return };
        let launched = std::process::Command::new("cargo")
            .args(["run", "--release", "-p", "client"])
            .env("WOT_MAP", &path)
            .spawn();
        self.status = match launched {
            Ok(_) => format!("playtest launched on {}", path.display()),
            Err(error) => format!("playtest failed to launch: {error}"),
        };
    }

    fn on_key(&mut self, code: KeyCode, pressed: bool) {
        // Ctrl chords first — Ctrl+S must never read as "reverse".
        if pressed && self.input.ctrl {
            match code {
                KeyCode::KeyZ => {
                    if self.document.undo() {
                        self.reload_scene();
                    } else {
                        self.status = "nothing to undo".into();
                    }
                    return;
                }
                KeyCode::KeyY => {
                    if self.document.redo() {
                        self.reload_scene();
                    } else {
                        self.status = "nothing to redo".into();
                    }
                    return;
                }
                KeyCode::KeyS => {
                    self.save();
                    return;
                }
                KeyCode::KeyP => {
                    self.playtest();
                    return;
                }
                _ => {}
            }
        }
        match code {
            KeyCode::KeyW | KeyCode::ArrowUp => self.input.forward = pressed,
            KeyCode::KeyS | KeyCode::ArrowDown => self.input.back = pressed,
            KeyCode::KeyA | KeyCode::ArrowLeft => self.input.left = pressed,
            KeyCode::KeyD | KeyCode::ArrowRight => self.input.right = pressed,
            KeyCode::KeyE => self.input.up = pressed,
            KeyCode::KeyQ => self.input.down = pressed,
            KeyCode::ShiftLeft | KeyCode::ShiftRight => self.input.sprint = pressed,
            KeyCode::ControlLeft | KeyCode::ControlRight => self.input.ctrl = pressed,
            KeyCode::F1 if pressed => self.show_layers = !self.show_layers,
            KeyCode::F5 if pressed => self.reload_scene(),
            _ => {}
        }
    }

    /// Integrate the fly camera from wall-clock elapsed — the editor has no simulation, so
    /// `about_to_wait` owns movement and `RedrawRequested` only draws (event-loop policy).
    fn tick_camera(&mut self) {
        let elapsed = self.last_tick.elapsed().as_secs_f32().min(0.1);
        self.last_tick = Instant::now();
        if self.input.looking {
            self.camera.yaw_rad -= self.input.mouse_dx * 0.003;
            self.camera.pitch_rad =
                (self.camera.pitch_rad - self.input.mouse_dy * 0.003).clamp(-1.5, 1.5);
        }
        self.input.mouse_dx = 0.0;
        self.input.mouse_dy = 0.0;

        let forward = self.camera.forward();
        let flat_forward = Vec3::new(forward.x, 0.0, forward.z).normalize_or_zero();
        let right = flat_forward.cross(Vec3::Y);
        let mut motion = Vec3::ZERO;
        if self.input.forward {
            motion += forward;
        }
        if self.input.back {
            motion -= forward;
        }
        if self.input.right {
            motion -= right;
        }
        if self.input.left {
            motion += right;
        }
        if self.input.up {
            motion += Vec3::Y;
        }
        if self.input.down {
            motion -= Vec3::Y;
        }
        let speed = self.camera.speed_m_s * if self.input.sprint { 4.0 } else { 1.0 };
        self.camera.eye += motion.normalize_or_zero() * speed * elapsed;
    }

    fn render_now(&mut self) {
        let Some(renderer) = self.renderer.as_mut() else { return };
        let camera = self.camera.camera();
        let aspect = renderer.aspect_ratio();
        let projection = CameraProjectionPolicy::webgpu_default();
        let view_proj = view_projection_matrix(
            &camera,
            aspect,
            projection.near_plane_m(),
            projection.far_plane_m(),
        );
        renderer.set_scene_time_s(self.started.elapsed().as_secs_f32());
        renderer.set_render_frame(&RenderFrame {
            camera,
            objects: Vec::new(),
            armor_damage: Vec::new(),
        });
        let hud = overlay(&self.document, &self.compiled, &self.status, self.show_layers, aspect);
        renderer.set_hud(&hud);
        if renderer.render(view_proj, self.camera.eye.to_array()).is_err() {
            // A lost surface reconfigures itself on the next frame; nothing to do here.
        }
    }
}

impl ApplicationHandler for EditorApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let attributes = Window::default_attributes()
            .with_title("WOT map editor")
            .with_inner_size(winit::dpi::LogicalSize::new(1280.0, 720.0))
            .with_maximized(true);
        let window = match event_loop.create_window(attributes) {
            Ok(window) => Arc::new(window),
            Err(error) => {
                eprintln!("editor: cannot create a window: {error}");
                event_loop.exit();
                return;
            }
        };
        let size = window.inner_size();
        // The initial terrain slot is the statics mesh, exactly like the client boot path;
        // reload_scene immediately re-uploads every slot from the document.
        let (_, statics) = scene_build::battlefield::battlefield_ground_and_statics_meshes(
            &self.compiled.battlefield,
            &[],
        );
        match WindowRenderer::new(
            window.clone(),
            size.width.max(1),
            size.height.max(1),
            &statics.0,
            &statics.1,
        ) {
            Ok(mut renderer) => {
                let (width, height, coverage) = client::hud_font_atlas();
                renderer.set_hud_font_atlas(width, height, coverage);
                self.renderer = Some(renderer);
            }
            Err(error) => {
                eprintln!("editor: cannot create the renderer: {error:?}");
                event_loop.exit();
                return;
            }
        }
        self.window = Some(window);
        self.reload_scene();
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => {
                if let Some(renderer) = self.renderer.as_mut() {
                    renderer.resize(size.width.max(1), size.height.max(1));
                }
            }
            WindowEvent::RedrawRequested => self.render_now(),
            WindowEvent::KeyboardInput { event, .. } => {
                if let PhysicalKey::Code(code) = event.physical_key {
                    self.on_key(code, event.state == ElementState::Pressed);
                }
            }
            WindowEvent::MouseInput { state, button: MouseButton::Right, .. } => {
                self.input.looking = state == ElementState::Pressed;
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let steps = match delta {
                    winit::event::MouseScrollDelta::LineDelta(_, y) => y,
                    winit::event::MouseScrollDelta::PixelDelta(position) => {
                        position.y as f32 / 40.0
                    }
                };
                self.camera.speed_m_s =
                    (self.camera.speed_m_s * (1.0 + steps * 0.15)).clamp(4.0, 400.0);
            }
            _ => {}
        }
    }

    fn device_event(
        &mut self,
        _event_loop: &ActiveEventLoop,
        _device_id: DeviceId,
        event: DeviceEvent,
    ) {
        if let DeviceEvent::MouseMotion { delta } = event {
            self.input.mouse_dx += delta.0 as f32;
            self.input.mouse_dy += delta.1 as f32;
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        self.tick_camera();
        if let Some(window) = &self.window {
            window.request_redraw();
        }
    }
}

fn report_counts(compiled: &CompiledMap) -> (usize, usize) {
    (compiled.report.errors().count(), compiled.report.warnings().count())
}

/// Where a pathless document lands for save/playtest: one well-known scratch file.
fn scratch_path() -> PathBuf {
    std::env::temp_dir().join("wot_editor_scratch.map.ron")
}

/// The editor overlay, drawn with the client's own UI toolkit: a status header, the
/// contract report (the editor's early warning), and the F1 layer checklist.
fn overlay(
    document: &EditorDocument,
    compiled: &CompiledMap,
    status: &str,
    show_layers: bool,
    aspect: f32,
) -> Vec<HudVertex> {
    const INK: [f32; 4] = [0.86, 0.88, 0.84, 0.95];
    const DIM: [f32; 4] = [0.62, 0.66, 0.62, 0.85];
    const ERROR: [f32; 4] = [0.95, 0.42, 0.34, 0.95];
    const WARNING: [f32; 4] = [0.92, 0.78, 0.35, 0.95];
    const TEXT_H: f32 = 0.042;
    let mut vertices = Vec::new();

    let name = document.path().map_or_else(
        || format!("{} (unsaved)", document.blueprint().meta.id),
        |path| path.display().to_string(),
    );
    let dirty = if document.dirty() { " *" } else { "" };
    let (errors, warnings) = report_counts(compiled);
    client::push_panel(
        &mut vertices,
        [0.0, 0.93],
        [0.995, 0.062],
        0.02,
        aspect,
        [0.05, 0.06, 0.05, 0.72],
    );
    client::push_text(
        &mut vertices,
        &format!(
            "{name}{dirty}   {:.0} m   compile {:.1} ms   E:{errors} W:{warnings}",
            compiled.battlefield.size_m[0],
            compiled.compile_time.as_secs_f32() * 1000.0
        ),
        -0.98,
        0.975,
        TEXT_H,
        aspect,
        INK,
    );

    // The report: jump-to-problem comes later (M7 dashboard); the shell already SHOWS the
    // problems, worst first.
    let mut y = 0.86;
    for entry in compiled.report.errors().take(8) {
        client::push_text(
            &mut vertices,
            &format!("E {}: {}", entry.check, entry.message),
            -0.98,
            y,
            TEXT_H,
            aspect,
            ERROR,
        );
        y -= 0.055;
    }
    for entry in compiled.report.warnings().take(4) {
        client::push_text(
            &mut vertices,
            &format!("W {}: {}", entry.check, entry.message),
            -0.98,
            y,
            TEXT_H,
            aspect,
            WARNING,
        );
        y -= 0.055;
    }

    if show_layers {
        client::push_panel(
            &mut vertices,
            [0.78, 0.28],
            [0.215, 0.56],
            0.02,
            aspect,
            [0.05, 0.06, 0.05, 0.72],
        );
        client::push_text(
            &mut vertices,
            "layers (M4+ own the tools)",
            0.575,
            0.80,
            TEXT_H,
            aspect,
            INK,
        );
        let mut layer_y = 0.73;
        for layer in terrain::TerrainMapPlan::wot_like().required_layers() {
            client::push_text(
                &mut vertices,
                &format!("- {layer:?}"),
                0.575,
                layer_y,
                TEXT_H,
                aspect,
                DIM,
            );
            layer_y -= 0.055;
        }
    }

    client::push_text(&mut vertices, status, -0.98, -0.93, TEXT_H, aspect, DIM);
    vertices
}
