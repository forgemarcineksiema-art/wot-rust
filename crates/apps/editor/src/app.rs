//! The editor window shell: winit `ApplicationHandler` (event-loop policy), a 3D viewport
//! through the game's OWN render path (`scene_build` meshes into `renderer_wgpu`'s
//! `WindowRenderer`), a free-fly camera with a cursor-to-ground probe, 3D annotations
//! (problem pylons, spawn rings, point posts), and panels drawn with the client's in-house
//! UI toolkit (map-editor D3) — the editor looks like the game because it IS the game's
//! eyes.
//!
//! Every viewport reload is a full document recompile ([`EditorDocument::recompile`]): the
//! editor can never show a world the game would not build.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use glam::Vec3;
use renderer_api::{
    Camera, CameraProjectionPolicy, RenderFrame, SceneVertex, view_projection_matrix,
};
use renderer_wgpu::WindowRenderer;
use winit::application::ApplicationHandler;
use winit::event::{DeviceEvent, DeviceId, ElementState, MouseButton, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::{CursorGrabMode, Window, WindowId};

use crate::brush::{BrushMode, BrushSettings, Stroke};
use crate::overlay::{OverlayModel, overlay};
use crate::{CompiledMap, EditorDocument, markers, pick};

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

/// The frame cadence: the editor has no simulation, so it paces itself instead of spinning
/// the event loop hot (one-look policy: frames are the app's job, never the laptop's).
const FRAME_INTERVAL: Duration = Duration::from_millis(12);

const HELP_LINE: &str =
    "B brush   [ ] radius   LMB paint   F1 overview   N problem   Ctrl+S save   Ctrl+P playtest";

/// Free-fly viewport camera: yaw/pitch look (hold RMB), WASD + QE move, Shift sprints,
/// wheel sets the pace.
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

    fn look_at(&mut self, target: Vec3) {
        let direction = (target - self.eye).normalize_or_zero();
        self.yaw_rad = direction.x.atan2(direction.z);
        self.pitch_rad = direction.y.clamp(-1.0, 1.0).asin();
    }

    fn camera(&self) -> Camera {
        let target = self.eye + self.forward();
        Camera { eye: self.eye.to_array(), target: target.to_array(), vertical_fov_degrees: 55.0 }
    }
}

/// A smooth camera flight to a problem: exponential approach, aimed at the target the
/// whole way — a glide, never a teleport. Any movement input hands the stick back.
struct Glide {
    eye_target: Vec3,
    look_at: Vec3,
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
    shift: bool,
    looking: bool,
    mouse_dx: f32,
    mouse_dy: f32,
}

impl InputState {
    fn moving(&self) -> bool {
        self.forward || self.back || self.left || self.right || self.up || self.down
    }
}

struct EditorApp {
    window: Option<Arc<Window>>,
    renderer: Option<WindowRenderer>,
    document: EditorDocument,
    compiled: CompiledMap,
    camera: FlyCamera,
    glide: Option<Glide>,
    input: InputState,
    cursor_px: Option<[f32; 2]>,
    viewport_px: [f32; 2],
    probe: Option<Vec3>,
    map_markers: (Vec<SceneVertex>, Vec<u32>),
    brush: Option<BrushSettings>,
    stroke: Option<Stroke>,
    working: Option<terrain::HeightMap>,
    painting: bool,
    last_paint: Instant,
    last_mesh_swap: Instant,
    problem_positions: Vec<[f32; 3]>,
    selected_problem: Option<usize>,
    show_overview: bool,
    close_armed: bool,
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
            glide: None,
            input: InputState::default(),
            cursor_px: None,
            viewport_px: [1280.0, 720.0],
            probe: None,
            map_markers: (Vec::new(), Vec::new()),
            brush: None,
            stroke: None,
            working: None,
            painting: false,
            last_paint: Instant::now(),
            last_mesh_swap: Instant::now(),
            problem_positions: Vec::new(),
            selected_problem: None,
            show_overview: true,
            close_armed: false,
            status: HELP_LINE.into(),
            started: Instant::now(),
            last_tick: Instant::now(),
        }
    }

    fn document_label(&self) -> String {
        self.document.path().map_or_else(
            || format!("{} (unsaved)", self.document.blueprint().meta.id),
            |path| path.display().to_string(),
        )
    }

    fn refresh_title(&self) {
        if let Some(window) = &self.window {
            let dirty = if self.document.dirty() { " *" } else { "" };
            window.set_title(&format!("{}{dirty} — WOT map editor", self.document_label()));
        }
    }

    /// Full document → viewport reload: recompile, then re-upload every scene slot the
    /// battle path uploads (ground + statics + water + dressing + lighting) plus the
    /// editor's annotation mesh.
    fn reload_scene(&mut self) {
        self.stroke = None;
        self.working = None;
        self.painting = false;
        self.compiled = self.document.recompile();
        self.problem_positions = markers::problem_positions(&self.compiled.report)
            .into_iter()
            .map(|(at, _)| at)
            .collect();
        if self.selected_problem.is_some_and(|index| index >= self.problem_positions.len()) {
            self.selected_problem = None;
        }
        self.refresh_markers();
        self.refresh_title();

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

        let errors = self.compiled.report.errors().count();
        let warnings = self.compiled.report.warnings().count();
        self.status = format!(
            "compiled in {:.1} ms — {errors} errors, {warnings} warnings",
            self.compiled.compile_time.as_secs_f32() * 1000.0
        );
    }

    /// Rebuild only the annotations (selection moved — the world did not change).
    fn refresh_markers(&mut self) {
        self.map_markers = markers::map_markers(
            &self.compiled.battlefield,
            &self.compiled.report,
            self.selected_problem,
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
        self.close_armed = false;
        self.refresh_title();
    }

    /// The playtest loop (map-editor D2): save the document, then launch the client with
    /// `WOT_MAP` pointing at the file — the client's local server installs it as
    /// `MapId::Scratch` and the battle runs on exactly what the viewport shows.
    fn playtest(&mut self) {
        if self.compiled.report.has_errors() {
            self.status =
                "playtest refused: fix the report errors first (a broken map is a build-time bug)"
                    .into();
            return;
        }
        self.save();
        let Some(path) = self.document.path().map(std::path::Path::to_path_buf) else { return };
        let (mut command, via_cargo) = client_command();
        let launched = command.env("WOT_MAP", &path).spawn();
        self.status = match launched {
            Ok(_) if via_cargo => format!(
                "playtest launched via cargo (first build takes a while) — {}",
                path.display()
            ),
            Ok(_) => format!("playtest launched on {}", path.display()),
            Err(error) => format!("playtest failed to launch: {error}"),
        };
    }

    /// Cycle the jump-to-problem cursor and glide the camera to it — the report promised
    /// "jump the camera straight to the problem"; this is that promise kept.
    fn jump_to_problem(&mut self, backwards: bool) {
        if self.problem_positions.is_empty() {
            self.status = "no positioned problems — the report is quiet".into();
            return;
        }
        let count = self.problem_positions.len();
        let next = match self.selected_problem {
            None if backwards => count - 1,
            None => 0,
            Some(index) if backwards => (index + count - 1) % count,
            Some(index) => (index + 1) % count,
        };
        self.selected_problem = Some(next);
        self.refresh_markers();
        let at = Vec3::from_array(self.problem_positions[next]);
        let ground = self.compiled.battlefield.heightmap.sample_height(at.x, at.z).unwrap_or(at.y);
        let target = Vec3::new(at.x, ground, at.z);
        // Approach from where the camera already is: back off along the flat line of sight.
        let mut away = self.camera.eye - target;
        away.y = 0.0;
        let away = if away.length() > 1.0 {
            away.normalize()
        } else {
            Vec3::new(-1.0, 0.0, -1.0).normalize()
        };
        self.glide =
            Some(Glide { eye_target: target + away * 30.0 + Vec3::Y * 20.0, look_at: target });
        self.status = format!("problem {}/{count}", next + 1);
    }

    fn set_look_capture(&mut self, looking: bool) {
        self.input.looking = looking;
        if let Some(window) = &self.window {
            if looking {
                let _ = window
                    .set_cursor_grab(CursorGrabMode::Confined)
                    .or_else(|_| window.set_cursor_grab(CursorGrabMode::Locked));
                window.set_cursor_visible(false);
            } else {
                let _ = window.set_cursor_grab(CursorGrabMode::None);
                window.set_cursor_visible(true);
            }
        }
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
            KeyCode::ShiftLeft | KeyCode::ShiftRight => {
                self.input.sprint = pressed;
                self.input.shift = pressed;
            }
            KeyCode::ControlLeft | KeyCode::ControlRight => self.input.ctrl = pressed,
            KeyCode::F1 if pressed => self.show_overview = !self.show_overview,
            KeyCode::F5 if pressed => self.reload_scene(),
            KeyCode::KeyN if pressed => self.jump_to_problem(self.input.shift),
            KeyCode::KeyB if pressed => self.cycle_brush(),
            KeyCode::BracketLeft if pressed => self.resize_brush(1.0 / 1.25),
            KeyCode::BracketRight if pressed => self.resize_brush(1.25),
            KeyCode::Minus if pressed => self.retune_brush(1.0 / 1.25),
            KeyCode::Equal if pressed => self.retune_brush(1.25),
            _ => {}
        }
        if pressed && self.input.moving() {
            // The stick is back in the author's hand.
            self.glide = None;
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
            self.glide = None;
        }
        self.input.mouse_dx = 0.0;
        self.input.mouse_dy = 0.0;

        if let Some(glide) = &self.glide {
            let approach = 1.0 - (-6.0 * elapsed).exp();
            self.camera.eye += (glide.eye_target - self.camera.eye) * approach;
            let look_at = glide.look_at;
            let arrived = (glide.eye_target - self.camera.eye).length() < 0.6;
            self.camera.look_at(look_at);
            if arrived {
                self.glide = None;
            }
            return;
        }

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

    /// The cursor's ground probe, refreshed per frame (it is the seed of the M4 brushes).
    fn refresh_probe(&mut self) {
        self.probe = None;
        let Some(cursor) = self.cursor_px else { return };
        if self.input.looking {
            return; // while orbiting, the hidden cursor names nothing
        }
        let ndc = [
            cursor[0] / self.viewport_px[0] * 2.0 - 1.0,
            1.0 - cursor[1] / self.viewport_px[1] * 2.0,
        ];
        let camera = self.camera.camera();
        let aspect = (self.viewport_px[0] / self.viewport_px[1]).max(0.01);
        let (origin, direction) = pick::cursor_ray(&camera, aspect, ndc);
        let heightmap = self.working.as_ref().unwrap_or(&self.compiled.battlefield.heightmap);
        self.probe = pick::ground_hit(heightmap, origin, direction);
    }

    fn cycle_brush(&mut self) {
        let next = match self.brush.map(|brush| brush.mode) {
            None => Some(BrushMode::CYCLE[0]),
            Some(mode) => {
                let position =
                    BrushMode::CYCLE.iter().position(|&cycle| cycle == mode).unwrap_or(0);
                BrushMode::CYCLE.get(position + 1).copied()
            }
        };
        self.brush = next.map(|mode| BrushSettings {
            mode,
            radius_m: self.brush.map_or(12.0, |brush| brush.radius_m),
            rate_m_s: 6.0,
        });
        self.status = match &self.brush {
            Some(brush) => brush_status(brush),
            None => format!("brush off - {HELP_LINE}"),
        };
    }

    fn resize_brush(&mut self, factor: f32) {
        if let Some(brush) = &mut self.brush {
            brush.radius_m = (brush.radius_m * factor).clamp(4.0, 80.0);
            self.status = brush_status(brush);
        }
    }

    fn retune_brush(&mut self, factor: f32) {
        if let Some(brush) = &mut self.brush {
            brush.rate_m_s = (brush.rate_m_s * factor).clamp(1.5, 24.0);
            self.status = brush_status(brush);
        }
    }

    fn begin_stroke(&mut self) {
        if self.brush.is_none() || self.probe.is_none() {
            return;
        }
        self.stroke =
            Some(Stroke::begin(self.document.blueprint(), &self.compiled.battlefield.heightmap));
        self.painting = true;
        self.last_paint = Instant::now();
    }

    /// One painting frame: dab under the cursor, rebuild the working ground and swap ONLY
    /// its geometry (the baked splat/macro maps stay bound) - the full recompile waits for
    /// the stroke to end.
    fn paint_frame(&mut self) {
        let dt = self.last_paint.elapsed().as_secs_f32().min(0.1);
        self.last_paint = Instant::now();
        let (Some(brush), Some(stroke), Some(hit)) = (self.brush, self.stroke.as_mut(), self.probe)
        else {
            return;
        };
        stroke.dab([hit.x, hit.z], &brush, dt);
        // The full ground remesh is throttled to ~30 ms — the stroke still reads live, the
        // laptop never chokes on a per-frame rebuild (one-look policy).
        if self.last_mesh_swap.elapsed() < Duration::from_millis(30) {
            return;
        }
        self.last_mesh_swap = Instant::now();
        let working = stroke.working_heightmap();
        if let Some(renderer) = self.renderer.as_mut() {
            let (vertices, indices) = scene_build::battlefield::terrain_scene_mesh_with_water(
                &working,
                self.compiled.battlefield.water,
            );
            renderer.update_battlefield_ground_geometry(&vertices, &indices);
        }
        self.working = Some(working);
    }

    /// Fold the stroke into the document through the single `apply_edit` door - one
    /// stroke, one undo step - then the full recompile the edit-loop contract demands.
    fn end_stroke(&mut self) {
        self.painting = false;
        let Some(stroke) = self.stroke.take() else { return };
        if !stroke.touched() {
            self.working = None;
            return;
        }
        self.document.apply_edit(|blueprint| {
            blueprint.sculpt = stroke.committed(blueprint.sculpt.as_ref());
        });
        self.reload_scene();
    }

    fn overlay_model(&self) -> OverlayModel {
        let problems = crate::overlay::problem_rows(&self.compiled.report);
        let eye = self.camera.eye;
        let camera_line = format!("cam {:.0} {:.0}  h {:.0}", eye.x, eye.z, eye.y);
        let probe_line = self
            .probe
            .map(|hit| format!("cursor {:.0}, {:.0}  h {:.1}", hit.x, hit.z, hit.y))
            .unwrap_or_default();
        let brush_line = self.brush.as_ref().map(brush_status).unwrap_or_default();
        OverlayModel {
            brush_line,
            document_label: self.document_label(),
            dirty: self.document.dirty(),
            compile_ms: self.compiled.compile_time.as_secs_f32() * 1000.0,
            problems,
            selected_problem: self.selected_problem,
            map_size_m: self.compiled.battlefield.size_m[0],
            layer_lines: layer_lines(self.document.blueprint(), &self.compiled),
            show_overview: self.show_overview,
            camera_line,
            probe_line,
            status: self.status.clone(),
        }
    }

    fn render_now(&mut self) {
        self.refresh_probe();
        if self.painting {
            self.paint_frame();
        }
        let model = self.overlay_model();
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
        // The shadow cascades follow the camera's gaze — without a focus the sun casts
        // nothing and the whole viewport reads flat.
        let focus =
            self.probe.unwrap_or(Vec3::from_array(camera.eye) + self.camera.forward() * 45.0);
        renderer.set_shadow_focus(Some(focus.to_array()));
        renderer.set_render_frame(&RenderFrame {
            camera,
            objects: Vec::new(),
            armor_damage: Vec::new(),
        });
        // Annotations: the cached map markers plus the live cursor - a probe disc when
        // navigating, the brush ring when a brush is armed.
        let (mut vertices, mut indices) = (self.map_markers.0.clone(), self.map_markers.1.clone());
        if let Some(hit) = self.probe {
            match self.brush {
                Some(brush) => {
                    let heightmap =
                        self.working.as_ref().unwrap_or(&self.compiled.battlefield.heightmap);
                    let color = markers::brush_color(brush.mode.label());
                    markers::brush_ring(
                        &mut vertices,
                        &mut indices,
                        heightmap,
                        [hit.x, hit.z],
                        brush.radius_m,
                        color,
                    );
                    // On a fair map the twin ring shows WHERE the mirror stamp lands.
                    if self.document.blueprint().symmetry.is_some() {
                        let axis_z = self.compiled.battlefield.size_m[1] * 0.5;
                        let mirrored_z = axis_z * 2.0 - hit.z;
                        if (mirrored_z - hit.z).abs() > 0.5 {
                            markers::brush_ring(
                                &mut vertices,
                                &mut indices,
                                heightmap,
                                [hit.x, mirrored_z],
                                brush.radius_m,
                                color.map(|c| c * 0.55),
                            );
                        }
                    }
                }
                None => markers::probe_marker(&mut vertices, &mut indices, hit, 1.5),
            }
        }
        renderer.set_dynamic_mesh(&vertices, &indices);
        renderer.set_hud(&overlay(&model, aspect));
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
        self.viewport_px = [size.width.max(1) as f32, size.height.max(1) as f32];
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
            WindowEvent::CloseRequested => {
                if self.document.dirty() && !self.close_armed {
                    self.close_armed = true;
                    self.status =
                        "unsaved changes — Ctrl+S to keep them, close again to discard".into();
                } else {
                    event_loop.exit();
                }
            }
            WindowEvent::Resized(size) => {
                self.viewport_px = [size.width.max(1) as f32, size.height.max(1) as f32];
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
            WindowEvent::CursorMoved { position, .. } => {
                self.cursor_px = Some([position.x as f32, position.y as f32]);
            }
            WindowEvent::CursorLeft { .. } => self.cursor_px = None,
            WindowEvent::MouseInput { state, button: MouseButton::Right, .. } => {
                self.set_look_capture(state == ElementState::Pressed);
            }
            WindowEvent::MouseInput { state, button: MouseButton::Left, .. } => {
                if state == ElementState::Pressed {
                    self.begin_stroke();
                } else {
                    self.end_stroke();
                }
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

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        self.tick_camera();
        if let Some(window) = &self.window {
            window.request_redraw();
        }
        event_loop.set_control_flow(ControlFlow::WaitUntil(Instant::now() + FRAME_INTERVAL));
    }
}

/// The document overview lines: every `TerrainMapLayer` with its LIVE count (or an honest
/// "- (M7)" for layers whose tools have not shipped), plus the document's water/river vitals.
pub fn layer_lines(
    blueprint: &map_forge::blueprint::MapBlueprint,
    compiled: &CompiledMap,
) -> Vec<(String, String)> {
    use terrain::TerrainMapLayer as Layer;
    let map = &compiled.battlefield;
    let heightmap = &map.heightmap;
    let mut lines: Vec<(String, String)> = Vec::new();
    for layer in terrain::TerrainMapPlan::wot_like().required_layers() {
        let (label, value) = match layer {
            Layer::HeightmapChunks => (
                "heightmap",
                format!(
                    "{}x{} @ {:.0} m",
                    heightmap.width(),
                    heightmap.height(),
                    heightmap.cell_size_m()
                ),
            ),
            Layer::CollisionTerrain => ("collision", "shared heightfield".to_string()),
            Layer::RenderLod => ("render", "ground + statics".to_string()),
            Layer::SplatMaps => ("splat", "baked 1024".to_string()),
            Layer::Roads => ("roads", map.roads.len().to_string()),
            Layer::Decorations => ("scenery", map.scenery.len().to_string()),
            Layer::CoverObjects => ("cover", map.static_cover.len().to_string()),
            Layer::SpawnPoints => ("spawns", map.spawn_zones.len().to_string()),
            Layer::CaptureZones => ("zones", "- (M7)".to_string()),
            Layer::BotNavigation => ("nav points", map.strategic_points.len().to_string()),
            Layer::VisibilitySectors => ("visibility", "- (M7)".to_string()),
            Layer::MinimapData => ("minimap", "- (M7)".to_string()),
        };
        lines.push((label.to_string(), value));
    }
    lines.push((
        "sculpt".to_string(),
        blueprint
            .sculpt
            .as_ref()
            .map_or("-".to_string(), |sculpt| format!("{} samples", sculpt.samples.len())),
    ));
    lines.push((
        "water".to_string(),
        map.water.map_or("-".to_string(), |w| format!("{:.1} m", w.surface_level_m)),
    ));
    lines.push((
        "river".to_string(),
        if map.river.is_some() { "yes".to_string() } else { "-".to_string() },
    ));
    lines
}

/// Where a pathless document lands for save/playtest: one well-known scratch file.
fn scratch_path() -> PathBuf {
    std::env::temp_dir().join("wot_editor_scratch.map.ron")
}

fn brush_status(brush: &BrushSettings) -> String {
    format!(
        "brush: {}  r {:.0} m  rate {:.1} m/s",
        brush.mode.label(),
        brush.radius_m,
        brush.rate_m_s
    )
}

/// The client to playtest in: the sibling binary when it exists (instant), `cargo run`
/// otherwise (honest about the wait).
fn client_command() -> (std::process::Command, bool) {
    if let Ok(exe) = std::env::current_exe()
        && let Some(directory) = exe.parent()
    {
        let sibling = directory.join(if cfg!(windows) { "client.exe" } else { "client" });
        if sibling.exists() {
            return (std::process::Command::new(sibling), false);
        }
    }
    let mut cargo = std::process::Command::new("cargo");
    cargo.args(["run", "--release", "-p", "client"]);
    (cargo, true)
}
