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
use map_forge::blueprint::SymmetrySpec;
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
use crate::gameplay::GameplayTool;
use crate::grab::{Form, Transform};
use crate::objects::{PaletteEntry, Selection};
use crate::overlay::{OverlayModel, overlay};
use crate::roads::PendingRoad;
use crate::stamp::{Parameter, StampKind};
use crate::stroke::{PendingStroke, StrokeKind};
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

const HELP_LINE: &str = "B brush  T stamp  C stroke  H grab  O object  L road  G gameplay  V viewshed  F3 water  F1 overview  Ctrl+P playtest";

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
    stamp: Option<StampState>,
    palette: Option<PaletteEntry>,
    road: Option<PendingRoad>,
    stroke_tool: Option<PendingStroke>,
    grab_armed: bool,
    grab_drag: Option<GrabDrag>,
    gameplay: Option<(GameplayTool, usize)>,
    viewshed: Option<(Vec<SceneVertex>, Vec<u32>)>,
    show_water: bool,
    water_tint: (Vec<SceneVertex>, Vec<u32>),
    selection: Option<Selection>,
    selection_knob: usize,
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
            stamp: None,
            palette: None,
            road: None,
            stroke_tool: None,
            grab_armed: false,
            grab_drag: None,
            gameplay: None,
            viewshed: None,
            show_water: false,
            water_tint: (Vec::new(), Vec::new()),
            selection: None,
            selection_knob: 0,
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
        self.viewshed = None;
        self.stroke = None;
        self.selection_knob = self.selection_knob.min(2);
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

        // Born-ruins preview (PR-07): the editor shows a "ruin"-tagged box as the mound the
        // battle will actually open on, not the intact building it never is.
        let born_phases = terrain::initial_cover_phase_bytes(&battlefield.static_cover);
        let (ground, statics) = scene_build::battlefield::battlefield_ground_and_statics_meshes(
            battlefield,
            &born_phases,
        );
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
            KeyCode::KeyT if pressed => self.cycle_stamp(),
            KeyCode::KeyC if pressed => self.cycle_stroke_tool(),
            KeyCode::KeyH if pressed => self.toggle_grab(),
            KeyCode::KeyO if pressed => self.cycle_palette(),
            KeyCode::KeyL if pressed => self.toggle_road(),
            KeyCode::KeyG if pressed => self.cycle_gameplay(),
            KeyCode::KeyV if pressed => self.toggle_viewshed(),
            KeyCode::F3 if pressed => self.toggle_water_panel(),
            KeyCode::Enter if pressed => {
                // Only one pending tool is ever armed; each commit no-ops without its own.
                self.commit_road();
                self.commit_stroke();
            }
            KeyCode::Backspace if pressed => {
                if let Some(road) = &mut self.road {
                    road.pop_point();
                } else if let Some(pending) = &mut self.stroke_tool {
                    pending.pop_point();
                    if pending.raw.is_empty() {
                        self.restore_compiled_ground();
                    }
                }
            }
            KeyCode::Delete if pressed => self.delete_selection(),
            KeyCode::KeyR if pressed => self.rotate_selection(),
            KeyCode::KeyX if pressed && self.show_water => {
                if self.document.blueprint().water.is_some() {
                    self.document.apply_edit(|blueprint| blueprint.water = None);
                    self.reload_scene();
                    self.status = "standing water removed".into();
                }
            }
            KeyCode::Tab if pressed => self.cycle_stamp_parameter(),
            KeyCode::Escape if pressed => self.cancel_stamp_anchor(),
            KeyCode::BracketLeft if pressed => self.resize_brush(1.0 / 1.25),
            KeyCode::BracketRight if pressed => self.resize_brush(1.25),
            KeyCode::Minus if pressed => self.adjust_knob(-1.0),
            KeyCode::Equal if pressed => self.adjust_knob(1.0),
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
        self.gameplay = None;
        self.stamp = None;
        self.palette = None;
        self.road = None;
        self.stroke_tool = None;
        self.disarm_grab();
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
            terrace_step_m: self.brush.map_or(2.0, |brush| brush.terrace_step_m),
        });
        self.status = match &self.brush {
            Some(brush) => brush_status(brush),
            None => format!("brush off - {HELP_LINE}"),
        };
    }

    fn resize_brush(&mut self, factor: f32) {
        if let Some(drag) = &mut self.grab_drag {
            drag.transform.widen *= factor;
            let wider = if drag.transform.widen > 1.0 { "wider" } else { "narrower" };
            self.status = format!("grab: {} - {wider} (release sets)", drag.form.label);
            return;
        }
        if let Some(pending) = &mut self.stroke_tool {
            pending.adjust_width(factor);
            self.status = stroke_status(pending);
            return;
        }
        if let Some(brush) = &mut self.brush {
            brush.radius_m = (brush.radius_m * factor).clamp(4.0, 80.0);
            self.status = brush_status(brush);
        }
    }

    fn adjust_knob(&mut self, steps: f32) {
        if let Some(road) = &mut self.road {
            road.adjust_width(steps);
            self.status = format!("road width {:.1} m (Tab surface, Enter commits)", road.width_m);
            return;
        }
        if let Some(pending) = &mut self.stroke_tool {
            pending.parameter.adjust(steps);
            self.status = stroke_status(pending);
            return;
        }
        if self.show_water && self.stamp.is_none() && self.selection.is_none() {
            let level =
                self.document.blueprint().water.as_ref().map_or(4.0, |water| water.surface_level_m)
                    + steps * 0.25;
            let mut summary = String::new();
            self.document.apply_edit(|blueprint| {
                summary = crate::roads::set_water_level(blueprint, level);
            });
            self.reload_scene();
            self.status = summary;
            return;
        }
        if self.stamp.is_none()
            && self.brush.is_none()
            && let Some(selection) = self.selection.clone()
        {
            let knob = self.selection_knob;
            let mut message = None;
            self.document.apply_edit(|blueprint| {
                message = crate::objects::adjust_cover(blueprint, &selection, knob, steps);
            });
            match message {
                Some(message) => {
                    self.status = message;
                    self.reload_scene();
                    self.selection = Some(selection);
                }
                None => {
                    // Nothing adjustable on this selection: drop the no-op undo entry.
                    self.document.undo();
                }
            }
            return;
        }
        if let Some(stamp) = &mut self.stamp {
            if let Some(parameter) = stamp.parameters.get_mut(stamp.active) {
                parameter.adjust(steps);
            }
            self.status = stamp_status(stamp);
            return;
        }
        if let Some(brush) = &mut self.brush {
            let factor = if steps > 0.0 { 1.25 } else { 1.0 / 1.25 };
            brush.rate_m_s = (brush.rate_m_s * factor).clamp(1.5, 24.0);
            self.status = brush_status(brush);
        }
    }

    fn cycle_stamp(&mut self) {
        self.gameplay = None;
        self.brush = None;
        self.palette = None;
        self.road = None;
        self.stroke_tool = None;
        self.disarm_grab();
        self.restore_compiled_ground();
        let next = match self.stamp.as_ref().map(|stamp| stamp.kind) {
            None => Some(StampKind::CYCLE[0]),
            Some(kind) => {
                let position =
                    StampKind::CYCLE.iter().position(|&cycle| cycle == kind).unwrap_or(0);
                StampKind::CYCLE.get(position + 1).copied()
            }
        };
        self.stamp = next.map(|kind| StampState {
            kind,
            parameters: kind.parameters(),
            active: 0,
            anchor: None,
        });
        self.status = match &self.stamp {
            Some(stamp) => stamp_status(stamp),
            None => format!("stamp off - {HELP_LINE}"),
        };
    }

    fn cycle_stamp_parameter(&mut self) {
        if let Some((GameplayTool::NavPoint, role_index)) = &mut self.gameplay {
            *role_index = (*role_index + 1) % crate::gameplay::ROLES.len();
            self.status = format!(
                "gameplay: nav point - role {:?} (Tab cycles)",
                crate::gameplay::ROLES[*role_index]
            );
            return;
        }
        if let Some(road) = &mut self.road {
            road.cycle_surface();
            self.status = format!("road surface {:?} (Enter commits)", road.surface);
            return;
        }
        if let Some(brush) = &mut self.brush
            && brush.mode == BrushMode::Terrace
        {
            brush.terrace_step_m = match brush.terrace_step_m {
                step if step < 1.5 => 2.0,
                step if step < 3.0 => 4.0,
                _ => 1.0,
            };
            self.status = brush_status(brush);
            return;
        }
        if let Some(stamp) = &mut self.stamp {
            stamp.active = (stamp.active + 1) % stamp.parameters.len().max(1);
            self.status = stamp_status(stamp);
        } else if self.selection.is_some() {
            self.selection_knob = (self.selection_knob + 1) % 3;
        }
    }

    fn cancel_stamp_anchor(&mut self) {
        if let Some(stamp) = &mut self.stamp
            && stamp.anchor.is_some()
        {
            stamp.anchor = None;
            self.restore_compiled_ground();
            self.status = "stamp anchor cancelled".into();
        } else if let Some(drag) = self.grab_drag.take() {
            self.restore_compiled_ground();
            self.status = format!("grab: {} - let go", drag.form.label);
        } else if self.stroke_tool.as_ref().is_some_and(|pending| !pending.raw.is_empty()) {
            if let Some(pending) = &mut self.stroke_tool {
                pending.raw.clear();
            }
            self.restore_compiled_ground();
            self.status = "stroke cleared".into();
        } else if self.selection.take().is_some() {
            self.status = HELP_LINE.into();
        }
    }

    /// Put the COMPILED ground back on screen (a ghost preview or a cancelled gesture
    /// must never leave a phantom hill in the viewport).
    fn restore_compiled_ground(&mut self) {
        if self.working.take().is_some()
            && let Some(renderer) = self.renderer.as_mut()
        {
            let (vertices, indices) = scene_build::battlefield::terrain_scene_mesh_with_water(
                &self.compiled.battlefield.heightmap,
                self.compiled.battlefield.water,
            );
            renderer.update_battlefield_ground_geometry(&vertices, &indices);
        }
    }

    /// The stamp gesture: first click anchors, second click places the quantized op(s)
    /// through the document's single `apply_edit` door — one gesture, one undo step.
    fn stamp_click(&mut self) -> bool {
        let Some(hit) = self.probe else { return self.stamp.is_some() };
        let Some(stamp) = &mut self.stamp else { return false };
        match stamp.anchor {
            None => {
                stamp.anchor = Some([hit.x, hit.z]);
                self.status = format!("{} - {}", stamp.kind.label(), stamp.kind.gesture_hint());
            }
            Some(anchor) => {
                let placed = crate::stamp::build_stamp(
                    stamp.kind,
                    &stamp.parameters,
                    self.document.blueprint(),
                    anchor,
                    [hit.x, hit.z],
                );
                match placed {
                    Some((ops, summary)) => {
                        self.document.apply_edit(|blueprint| crate::stamp::insert(blueprint, ops));
                        self.reload_scene();
                        self.status = format!("stamped: {summary}");
                    }
                    None => {
                        let why = crate::stamp::refusal(stamp.kind, self.document.blueprint());
                        stamp.anchor = None;
                        self.restore_compiled_ground();
                        self.status = format!("stamp refused: {why}");
                    }
                }
            }
        }
        true
    }

    /// The ghost: with an anchor down, the viewport previews the op the second click
    /// WOULD place at the cursor — same throttle discipline as the sculpt stroke.
    fn preview_stamp(&mut self) {
        let Some(hit) = self.probe else { return };
        let Some(stamp) = &self.stamp else { return };
        let Some(anchor) = stamp.anchor else { return };
        if self.last_mesh_swap.elapsed() < Duration::from_millis(50) {
            return;
        }
        self.last_mesh_swap = Instant::now();
        let Some((ops, _)) = crate::stamp::build_stamp(
            stamp.kind,
            &stamp.parameters,
            self.document.blueprint(),
            anchor,
            [hit.x, hit.z],
        ) else {
            return;
        };
        let mut ghost = self.document.blueprint().clone();
        crate::stamp::insert(&mut ghost, ops);
        let (battlefield, _) = map_forge::compile(&ghost);
        if let Some(renderer) = self.renderer.as_mut() {
            let (vertices, indices) = scene_build::battlefield::terrain_scene_mesh_with_water(
                &battlefield.heightmap,
                battlefield.water,
            );
            renderer.update_battlefield_ground_geometry(&vertices, &indices);
        }
        self.working = Some(battlefield.heightmap);
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

    /// C cycles the stroke tool: ridge → valley → plateau → off. Drawing replaces
    /// parametrizing — the fit happens on commit, the hand only clicks the line.
    fn cycle_stroke_tool(&mut self) {
        self.gameplay = None;
        self.brush = None;
        self.stamp = None;
        self.palette = None;
        self.road = None;
        self.disarm_grab();
        self.restore_compiled_ground();
        let width = self.stroke_tool.as_ref().map(|pending| pending.half_width_m);
        let next = match self.stroke_tool.as_ref().map(|pending| pending.kind) {
            None => Some(StrokeKind::CYCLE[0]),
            Some(kind) => {
                let position =
                    StrokeKind::CYCLE.iter().position(|&cycle| cycle == kind).unwrap_or(0);
                StrokeKind::CYCLE.get(position + 1).copied()
            }
        };
        self.stroke_tool = next.map(|kind| {
            let mut pending = PendingStroke::new(kind);
            if let Some(width) = width {
                pending.half_width_m = width;
            }
            pending
        });
        self.status = match &self.stroke_tool {
            Some(pending) => stroke_status(pending),
            None => format!("stroke off - {HELP_LINE}"),
        };
    }

    /// H toggles the grab tool: existing forms (hills, strokes, benches) become handles.
    fn toggle_grab(&mut self) {
        self.gameplay = None;
        self.brush = None;
        self.stamp = None;
        self.palette = None;
        self.road = None;
        self.stroke_tool = None;
        self.restore_compiled_ground();
        self.grab_armed = !self.grab_armed;
        self.grab_drag = None;
        self.status = if self.grab_armed {
            "grab: LMB holds a form (hills, strokes, benches) - drag moves, Shift lifts, [/] width"
                .into()
        } else {
            format!("grab off - {HELP_LINE}")
        };
    }

    fn disarm_grab(&mut self) {
        self.grab_armed = false;
        self.grab_drag = None;
    }

    /// LMB with the grab tool armed: hold the nearest form under the cursor ray.
    fn grab_click(&mut self) -> bool {
        if !self.grab_armed {
            return false;
        }
        let Some(cursor) = self.cursor_px else { return true };
        let ndc = [
            cursor[0] / self.viewport_px[0] * 2.0 - 1.0,
            1.0 - cursor[1] / self.viewport_px[1] * 2.0,
        ];
        let camera = self.camera.camera();
        let aspect = (self.viewport_px[0] / self.viewport_px[1]).max(0.01);
        let (origin, direction) = pick::cursor_ray(&camera, aspect, ndc);
        let forms = crate::grab::enumerate_forms(
            self.document.blueprint(),
            &self.compiled.battlefield.heightmap,
        );
        let mut best: Option<(f32, Form)> = None;
        for form in forms {
            if let Some(t) = pick::ray_aabb(origin, direction, form.center, form.half)
                && best.as_ref().is_none_or(|(closest, _)| t < *closest)
            {
                best = Some((t, form));
            }
        }
        match best {
            Some((_, form)) => {
                let start = self.probe.map_or([form.center.x, form.center.z], |hit| [hit.x, hit.z]);
                self.status = format!("grab: {} - drag moves, Shift lifts, [/] width", form.label);
                self.grab_drag = Some(GrabDrag {
                    form,
                    start,
                    last_cursor_y: cursor[1],
                    transform: Transform::default(),
                });
            }
            None => self.status = "grab: nothing under the cursor".into(),
        }
        true
    }

    /// One held frame: the drag feeds the transform (ground = move, Shift = lift), and the
    /// ghost previews EXACTLY what release would set — same snapped door, same truth.
    fn grab_frame(&mut self) {
        let (shift, probe, cursor_y) =
            (self.input.shift, self.probe, self.cursor_px.map(|cursor| cursor[1]));
        let Some(drag) = &mut self.grab_drag else { return };
        if shift {
            if let Some(y) = cursor_y {
                drag.transform.raise_m -= (y - drag.last_cursor_y) * (0.25 / 12.0);
            }
        } else if let Some(hit) = probe {
            drag.transform.move_m = [hit.x - drag.start[0], hit.z - drag.start[1]];
        }
        if let Some(y) = cursor_y {
            drag.last_cursor_y = y;
        }
        let (form, transform) = (drag.form.clone(), drag.transform);
        if self.last_mesh_swap.elapsed() < Duration::from_millis(50) {
            return;
        }
        self.last_mesh_swap = Instant::now();
        let mut ghost = self.document.blueprint().clone();
        let spoken = crate::grab::apply_transform(&mut ghost, &form, &transform);
        self.status = format!("grab: {spoken} (release sets, Esc lets go)");
        let (battlefield, _) = map_forge::compile(&ghost);
        if let Some(renderer) = self.renderer.as_mut() {
            let (vertices, indices) = scene_build::battlefield::terrain_scene_mesh_with_water(
                &battlefield.heightmap,
                battlefield.water,
            );
            renderer.update_battlefield_ground_geometry(&vertices, &indices);
        }
        self.working = Some(battlefield.heightmap);
    }

    /// Release sets: replay the snapped transform onto the document through ONE
    /// `apply_edit`. A sub-snap drag leaves the document untouched.
    fn end_grab(&mut self) {
        let Some(drag) = self.grab_drag.take() else { return };
        if !drag.transform.is_meaningful() {
            self.restore_compiled_ground();
            self.status = format!("grab: {} - untouched", drag.form.label);
            return;
        }
        let (form, transform) = (drag.form, drag.transform);
        let mut spoken = String::new();
        self.document.apply_edit(|blueprint| {
            spoken = crate::grab::apply_transform(blueprint, &form, &transform);
        });
        self.reload_scene();
        self.status = format!("set: {spoken}");
    }

    /// LMB with the stroke tool armed: append a waypoint at the probe.
    fn stroke_click(&mut self) -> bool {
        let Some(pending) = &mut self.stroke_tool else { return false };
        if let Some(hit) = self.probe {
            pending.add_point([hit.x, hit.z], hit.y);
            self.status = format!(
                "stroke: {} points (Enter commits, Backspace pops, Esc clears)",
                pending.raw.len()
            );
        }
        true
    }

    /// Fold the drawn line into the document: fit, insert, ONE `apply_edit` — then re-arm
    /// the same profile so the hand keeps drawing.
    fn commit_stroke(&mut self) {
        let Some(pending) = &self.stroke_tool else { return };
        match crate::stroke::fit_stroke(pending, self.document.blueprint()) {
            Some((ops, summary)) => {
                self.document.apply_edit(|blueprint| crate::stamp::insert(blueprint, ops));
                let (kind, width) = (pending.kind, pending.half_width_m);
                self.stroke_tool = Some({
                    let mut next = PendingStroke::new(kind);
                    next.half_width_m = width;
                    next
                });
                self.reload_scene();
                self.status = format!("committed: {summary}");
            }
            None => self.status = "a stroke needs two points (LMB adds them)".into(),
        }
    }

    /// The stroke ghost: with waypoints down, the viewport previews the terrain the commit
    /// WOULD build with the cursor as the provisional next point — the stamp's discipline.
    fn preview_stroke(&mut self) {
        let Some(hit) = self.probe else { return };
        let Some(pending) = &self.stroke_tool else { return };
        if pending.raw.is_empty() {
            return;
        }
        if self.last_mesh_swap.elapsed() < Duration::from_millis(50) {
            return;
        }
        self.last_mesh_swap = Instant::now();
        let mut preview = pending.clone();
        preview.add_point([hit.x, hit.z], hit.y);
        let Some((ops, _)) = crate::stroke::fit_stroke(&preview, self.document.blueprint()) else {
            return;
        };
        let mut ghost = self.document.blueprint().clone();
        crate::stamp::insert(&mut ghost, ops);
        let (battlefield, _) = map_forge::compile(&ghost);
        if let Some(renderer) = self.renderer.as_mut() {
            let (vertices, indices) = scene_build::battlefield::terrain_scene_mesh_with_water(
                &battlefield.heightmap,
                battlefield.water,
            );
            renderer.update_battlefield_ground_geometry(&vertices, &indices);
        }
        self.working = Some(battlefield.heightmap);
    }

    fn cycle_palette(&mut self) {
        self.gameplay = None;
        self.brush = None;
        self.stamp = None;
        self.road = None;
        self.stroke_tool = None;
        self.disarm_grab();
        self.restore_compiled_ground();
        let next = match self.palette {
            None => Some(PaletteEntry::CYCLE[0]),
            Some(entry) => {
                let position =
                    PaletteEntry::CYCLE.iter().position(|&cycle| cycle == entry).unwrap_or(0);
                PaletteEntry::CYCLE.get(position + 1).copied()
            }
        };
        self.palette = next;
        self.status = match self.palette {
            Some(entry) => format!("object: {} - LMB places", entry.label()),
            None => format!("object off - {HELP_LINE}"),
        };
    }

    /// LMB with a palette armed: place at the probe. Returns whether the click was ours.
    fn palette_click(&mut self) -> bool {
        let Some(entry) = self.palette else { return false };
        let Some(hit) = self.probe else { return true };
        let mut summary = String::new();
        self.document.apply_edit(|blueprint| {
            summary = crate::objects::place_entry(blueprint, entry, [hit.x, hit.z]);
        });
        self.reload_scene();
        self.status = summary;
        true
    }

    /// LMB in navigate mode: select what the cursor ray touches.
    fn select_click(&mut self) {
        let Some(cursor) = self.cursor_px else { return };
        let ndc = [
            cursor[0] / self.viewport_px[0] * 2.0 - 1.0,
            1.0 - cursor[1] / self.viewport_px[1] * 2.0,
        ];
        let camera = self.camera.camera();
        let aspect = (self.viewport_px[0] / self.viewport_px[1]).max(0.01);
        let (origin, direction) = pick::cursor_ray(&camera, aspect, ndc);
        self.selection = crate::objects::pick(
            self.document.blueprint(),
            &self.compiled.battlefield,
            origin,
            direction,
        );
        self.selection_knob = 0;
        self.status = match &self.selection {
            Some(Selection::Cover { id, .. }) => format!("selected {id}"),
            Some(Selection::TownGridMember { .. }) => "selected a town-grid member".into(),
            Some(Selection::FixedScenery { kind, .. }) => format!("selected a {kind:?} (fixed)"),
            Some(Selection::ScatterInstance { kind, .. }) => {
                format!("selected a {kind:?} (scatter-born)")
            }
            None => HELP_LINE.into(),
        };
    }

    fn delete_selection(&mut self) {
        let Some(selection) = self.selection.take() else { return };
        let map = self.compiled.battlefield.clone();
        let mut outcome: Result<String, &'static str> = Err("nothing selected");
        self.document.apply_edit(|blueprint| {
            outcome = crate::objects::delete(blueprint, &map, &selection);
        });
        match outcome {
            Ok(summary) => {
                self.status = summary;
                self.reload_scene();
            }
            Err(reason) => {
                self.document.undo();
                self.selection = Some(selection);
                self.status = format!("delete refused: {reason}");
            }
        }
    }

    fn rotate_selection(&mut self) {
        let Some(selection) = self.selection.clone() else { return };
        let mut message = None;
        self.document.apply_edit(|blueprint| {
            message = crate::objects::rotate_cover(blueprint, &selection);
        });
        match message {
            Some(message) => {
                self.status = message;
                self.reload_scene();
                self.selection = Some(selection);
            }
            None => {
                self.document.undo();
            }
        }
    }

    fn toggle_road(&mut self) {
        self.gameplay = None;
        self.brush = None;
        self.stamp = None;
        self.palette = None;
        self.stroke_tool = None;
        self.disarm_grab();
        self.road = match self.road {
            Some(_) => None,
            None => Some(PendingRoad::default()),
        };
        self.status = match &self.road {
            Some(_) => {
                "road: LMB adds points, Backspace pops, Tab surface, -/= width, Enter commits"
                    .into()
            }
            None => format!("road off - {HELP_LINE}"),
        };
    }

    fn toggle_water_panel(&mut self) {
        self.show_water = !self.show_water;
        self.rebuild_water_tint();
        self.status = if self.show_water {
            "water: -/= level, X removes (the tint speaks the gameplay thresholds)".into()
        } else {
            HELP_LINE.into()
        };
    }

    fn rebuild_water_tint(&mut self) {
        // ONE walk through the resolution rule (table + sheets): the overlay shows every
        // pool the sim will drown a hull in, each at its own level, nothing outside its
        // governing rect.
        self.water_tint = match (self.show_water, self.compiled.battlefield.water_view()) {
            (true, water) if water.table.is_some() || !water.sheets.is_empty() => {
                crate::roads::depth_tint_mesh(
                    &self.compiled.battlefield.heightmap,
                    water,
                    &map_forge::WaterThresholds::default(),
                )
            }
            _ => (Vec::new(), Vec::new()),
        };
    }

    /// LMB with the road tool armed: append a waypoint at the probe.
    fn road_click(&mut self) -> bool {
        let Some(road) = &mut self.road else { return false };
        if let Some(hit) = self.probe {
            road.add_point([hit.x, hit.z]);
            self.status =
                format!("road: {} points (Enter commits, Backspace pops)", road.points.len());
        }
        true
    }

    fn commit_road(&mut self) {
        let Some(road) = &self.road else { return };
        match road.committed(self.document.blueprint()) {
            Some((spec, summary)) => {
                self.document.apply_edit(|blueprint| blueprint.roads.push(spec));
                self.road = Some(PendingRoad::default());
                self.reload_scene();
                self.status = format!("committed: {summary}");
            }
            None => self.status = "a road needs at least two points".into(),
        }
    }

    fn cycle_gameplay(&mut self) {
        self.brush = None;
        self.stamp = None;
        self.palette = None;
        self.road = None;
        self.stroke_tool = None;
        self.disarm_grab();
        let next = match self.gameplay {
            None => Some(GameplayTool::CYCLE[0]),
            Some((tool, _)) => {
                let position =
                    GameplayTool::CYCLE.iter().position(|&cycle| cycle == tool).unwrap_or(0);
                GameplayTool::CYCLE.get(position + 1).copied()
            }
        };
        self.gameplay = next.map(|tool| (tool, 0));
        self.status = match self.gameplay {
            Some((tool, _)) => format!("gameplay: {} - LMB places", tool.label()),
            None => format!("gameplay off - {HELP_LINE}"),
        };
    }

    /// LMB with a gameplay tool armed. Every gesture is fair by construction.
    fn gameplay_click(&mut self) -> bool {
        let Some((tool, role_index)) = self.gameplay else { return false };
        let Some(hit) = self.probe else { return true };
        let at = [hit.x, hit.z];
        let mut summary = String::new();
        self.document.apply_edit(|blueprint| {
            summary = match tool {
                GameplayTool::MoveSpawn1 => crate::gameplay::move_spawn(blueprint, 1, at),
                GameplayTool::MoveSpawn2 => crate::gameplay::move_spawn(blueprint, 2, at),
                GameplayTool::NavPoint => {
                    crate::gameplay::add_point(blueprint, crate::gameplay::ROLES[role_index], at)
                }
                GameplayTool::Zone => crate::gameplay::add_zone(blueprint, at),
            };
        });
        self.reload_scene();
        self.status = summary;
        true
    }

    /// V computes the turret-eye viewshed from the cursor; V again (or a reload) clears.
    fn toggle_viewshed(&mut self) {
        if self.viewshed.take().is_some() {
            self.status = "viewshed cleared".into();
            return;
        }
        let Some(hit) = self.probe else {
            self.status = "viewshed: point the cursor at the ground first".into();
            return;
        };
        self.viewshed = Some(crate::visibility::viewshed_mesh(
            &self.compiled.battlefield.heightmap,
            &self.compiled.battlefield.static_cover,
            [hit.x, hit.z],
            400.0,
        ));
        self.status = format!(
            "viewshed from {:.0}, {:.0}: dark = hull DEAD ground for the sim's eye (V clears)",
            hit.x, hit.z
        );
    }

    fn overlay_model(&self) -> OverlayModel {
        let problems = crate::overlay::problem_rows(&self.compiled.report);
        let eye = self.camera.eye;
        let camera_line = format!("cam {:.0} {:.0}  h {:.0}", eye.x, eye.z, eye.y);
        let probe_line = self
            .probe
            .map(|hit| format!("cursor {:.0}, {:.0}  h {:.1}", hit.x, hit.z, hit.y))
            .unwrap_or_default();
        let brush_line = match (&self.stamp, &self.brush) {
            (Some(stamp), _) => stamp_status(stamp),
            (None, Some(brush)) => brush_status(brush),
            _ => String::new(),
        };
        let (inspector_title, stamp_lines) = if let Some((tool, role_index)) = self.gameplay {
            let mut lines = vec![(format!("{} - LMB places", tool.label()), false)];
            if tool == GameplayTool::NavPoint {
                for (index, role) in crate::gameplay::ROLES.iter().enumerate() {
                    lines.push((format!("{role:?}"), index == role_index));
                }
                lines.push(("Tab cycles the role".to_string(), false));
            }
            ("GAMEPLAY".to_string(), lines)
        } else if let Some(road) = &self.road {
            (
                "ROAD".to_string(),
                vec![
                    (format!("{} points", road.points.len()), false),
                    (format!("surface: {:?} (Tab)", road.surface), false),
                    (format!("width: {:.1} m", road.width_m), true),
                    ("Enter commits   Backspace pops".to_string(), false),
                ],
            )
        } else if let Some(pending) = &self.stroke_tool {
            (
                "STROKE".to_string(),
                vec![
                    (format!("{} - LMB draws the line", pending.kind.label()), false),
                    (
                        format!("{}: {:.1} m", pending.parameter.name, pending.parameter.value_m),
                        true,
                    ),
                    (format!("width: {:.1} m", pending.half_width_m), false),
                    (format!("{} points", pending.raw.len()), false),
                    ("Enter commits   Backspace pops   Esc clears".to_string(), false),
                ],
            )
        } else if self.grab_armed {
            (
                "GRAB".to_string(),
                vec![
                    ("LMB holds a form: hills, strokes, benches".to_string(), false),
                    ("drag moves (1 m)   Shift+drag lifts (0.25 m)".to_string(), false),
                    ("[ ] narrower / wider".to_string(), false),
                    ("release sets   Esc lets go".to_string(), false),
                ],
            )
        } else if self.show_water {
            let level = self
                .document
                .blueprint()
                .water
                .as_ref()
                .map_or("- (press = to add)".to_string(), |water| {
                    format!("{:.2} m", water.surface_level_m)
                });
            (
                "WATER".to_string(),
                vec![
                    (format!("level: {level}"), true),
                    ("green: ford  amber: marginal".to_string(), false),
                    ("red: the current drowns".to_string(), false),
                    ("X removes standing water".to_string(), false),
                ],
            )
        } else if let Some(selection) = &self.selection {
            (
                "OBJECT".to_string(),
                crate::objects::inspector_lines(
                    self.document.blueprint(),
                    selection,
                    self.selection_knob,
                ),
            )
        } else {
            (
                "STAMP".to_string(),
                self.stamp
                    .as_ref()
                    .map(|stamp| {
                        let mut lines = vec![(
                            format!("{} - {}", stamp.kind.label(), stamp.kind.gesture_hint()),
                            false,
                        )];
                        for (index, parameter) in stamp.parameters.iter().enumerate() {
                            lines.push((
                                format!("{}: {:.1} m", parameter.name, parameter.value_m),
                                index == stamp.active,
                            ));
                        }
                        lines.push((
                            match stamp.anchor {
                                Some(anchor) => {
                                    format!("anchor {:.0}, {:.0}", anchor[0], anchor[1])
                                }
                                None => "anchor: click LMB".to_string(),
                            },
                            false,
                        ));
                        lines
                    })
                    .unwrap_or_default(),
            )
        };
        OverlayModel {
            brush_line,
            inspector_title,
            stamp_lines,
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
        } else {
            self.preview_stamp();
            self.preview_stroke();
            self.grab_frame();
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
            if let Some(entry) = self.palette {
                let heightmap =
                    self.working.as_ref().unwrap_or(&self.compiled.battlefield.heightmap);
                let ground = heightmap.sample_height(hit.x, hit.z).unwrap_or(hit.y);
                if let Some((_, half)) = entry.cover() {
                    markers::aabb_outline(
                        &mut vertices,
                        &mut indices,
                        Vec3::new(hit.x, ground + half[1], hit.z),
                        Vec3::from_array(half),
                        markers::brush_color("flatten"),
                    );
                } else {
                    markers::probe_marker(
                        &mut vertices,
                        &mut indices,
                        Vec3::new(hit.x, ground, hit.z),
                        1.2,
                    );
                }
            } else if let Some(stamp) = &self.stamp {
                let heightmap =
                    self.working.as_ref().unwrap_or(&self.compiled.battlefield.heightmap);
                if let Some(anchor) = stamp.anchor {
                    let ground = heightmap.sample_height(anchor[0], anchor[1]).unwrap_or(hit.y);
                    markers::probe_marker(
                        &mut vertices,
                        &mut indices,
                        Vec3::new(anchor[0], ground, anchor[1]),
                        1.2,
                    );
                }
                markers::brush_ring(
                    &mut vertices,
                    &mut indices,
                    heightmap,
                    [hit.x, hit.z],
                    6.0,
                    markers::brush_color("flatten"),
                );
            } else {
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
                        // On a fair map the twin ring shows WHERE the twin stamp lands.
                        if let Some(symmetry) = self.document.blueprint().symmetry {
                            let size = self.compiled.battlefield.size_m;
                            let twin = symmetry.twin([hit.x, hit.z], [size[0], size[1]]);
                            if (twin[0] - hit.x).abs() > 0.5 || (twin[1] - hit.z).abs() > 0.5 {
                                markers::brush_ring(
                                    &mut vertices,
                                    &mut indices,
                                    heightmap,
                                    twin,
                                    brush.radius_m,
                                    color.map(|c| c * 0.55),
                                );
                            }
                        }
                    }
                    None => markers::probe_marker(&mut vertices, &mut indices, hit, 1.5),
                }
            }
        }
        if let Some(road) = &self.road {
            let heightmap = self.working.as_ref().unwrap_or(&self.compiled.battlefield.heightmap);
            let mut preview = road.clone();
            if let Some(hit) = self.probe {
                preview.add_point([hit.x, hit.z]);
            }
            crate::roads::ribbon_mesh(&mut vertices, &mut indices, heightmap, &preview);
        }
        if let Some(pending) = &self.stroke_tool {
            let heightmap = self.working.as_ref().unwrap_or(&self.compiled.battlefield.heightmap);
            let mut points = pending.raw.clone();
            if let Some(hit) = self.probe {
                points.push([hit.x, hit.z]);
            }
            crate::stroke::chalk_mesh(
                &mut vertices,
                &mut indices,
                heightmap,
                &points,
                pending.half_width_m,
            );
        }
        if let Some(drag) = &self.grab_drag {
            // The held form's foot ring (amber), riding the drag; the twin's ring dimmer.
            let heightmap = self.working.as_ref().unwrap_or(&self.compiled.battlefield.heightmap);
            let at = [
                drag.form.center.x + drag.transform.move_m[0],
                drag.form.center.z + drag.transform.move_m[1],
            ];
            markers::brush_ring(
                &mut vertices,
                &mut indices,
                heightmap,
                at,
                drag.form.footprint_m,
                [0.95, 0.65, 0.15],
            );
            if drag.form.twin.is_some() {
                let symmetry = self.document.blueprint().symmetry.unwrap_or(SymmetrySpec::MirrorZ);
                let size = self.compiled.battlefield.size_m;
                // The twin ghost rides the twin of the DRAGGED position: the same door the
                // commit walks (apply_transform), so release sets exactly what the eye saw.
                let twin = symmetry.twin(
                    [
                        drag.form.center.x + drag.transform.move_m[0],
                        drag.form.center.z + drag.transform.move_m[1],
                    ],
                    [size[0], size[1]],
                );
                markers::brush_ring(
                    &mut vertices,
                    &mut indices,
                    heightmap,
                    twin,
                    drag.form.footprint_m,
                    [0.52, 0.36, 0.08],
                );
            }
        }
        if self.show_water {
            let base = vertices.len() as u32;
            vertices.extend_from_slice(&self.water_tint.0);
            indices.extend(self.water_tint.1.iter().map(|index| index + base));
        }
        if let Some((shed_vertices, shed_indices)) = &self.viewshed {
            let base = vertices.len() as u32;
            vertices.extend_from_slice(shed_vertices);
            indices.extend(shed_indices.iter().map(|index| index + base));
        }
        if let Some(selection) = &self.selection {
            match selection {
                Selection::Cover { .. } | Selection::TownGridMember { .. } => {
                    if let Some(cover) = self.compiled.battlefield.static_cover.iter().find(
                        |cover| match selection {
                            Selection::Cover { id, .. } => cover.id == *id,
                            Selection::TownGridMember { .. } => false,
                            _ => false,
                        },
                    ) {
                        markers::aabb_outline(
                            &mut vertices,
                            &mut indices,
                            Vec3::from_array(cover.center),
                            Vec3::from_array(cover.half_extents_m),
                            [0.95, 0.65, 0.15],
                        );
                    }
                }
                Selection::FixedScenery { .. } | Selection::ScatterInstance { .. } => {}
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
        let born_phases =
            terrain::initial_cover_phase_bytes(&self.compiled.battlefield.static_cover);
        let (_, statics) = scene_build::battlefield::battlefield_ground_and_statics_meshes(
            &self.compiled.battlefield,
            &born_phases,
        );
        match WindowRenderer::new(
            window.clone(),
            size.width.max(1),
            size.height.max(1),
            &statics.0,
            &statics.1,
        ) {
            Ok(mut renderer) => {
                let (width, height, coverage) = ui_kit::font::hud_font_atlas();
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
                    if !self.gameplay_click()
                        && !self.road_click()
                        && !self.stroke_click()
                        && !self.grab_click()
                        && !self.palette_click()
                        && !self.stamp_click()
                    {
                        if self.brush.is_some() {
                            self.begin_stroke();
                        } else {
                            self.select_click();
                        }
                    }
                } else {
                    self.end_grab();
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
            Layer::CaptureZones => ("zones", map.capture_zones.len().to_string()),
            Layer::BotNavigation => ("nav points", map.strategic_points.len().to_string()),
            Layer::VisibilitySectors => ("visibility", "V viewshed".to_string()),
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

/// A form held by the grab tool: what is held, where the hand started on the ground, the
/// last cursor height (Shift-lift reads screen motion), and the accumulated transform.
struct GrabDrag {
    form: Form,
    start: [f32; 2],
    last_cursor_y: f32,
    transform: Transform,
}

/// The armed stamp tool: its kind, the inspector knobs, which knob Tab points at, and
/// the pending first click.
struct StampState {
    kind: StampKind,
    parameters: Vec<Parameter>,
    active: usize,
    anchor: Option<[f32; 2]>,
}

fn stamp_status(stamp: &StampState) -> String {
    let parameter = &stamp.parameters[stamp.active.min(stamp.parameters.len() - 1)];
    format!(
        "stamp: {}  {}: {:.1} m (Tab next, -/= adjust)",
        stamp.kind.label(),
        parameter.name,
        parameter.value_m
    )
}

fn stroke_status(pending: &PendingStroke) -> String {
    format!(
        "stroke: {}  {}: {:.1} m  w {:.1} m ([/] width, -/= value, Enter commits)",
        pending.kind.label(),
        pending.parameter.name,
        pending.parameter.value_m,
        pending.half_width_m
    )
}

fn brush_status(brush: &BrushSettings) -> String {
    let step = match brush.mode {
        BrushMode::Terrace => format!("  step {:.0} m (Tab)", brush.terrace_step_m),
        _ => String::new(),
    };
    format!(
        "brush: {}  r {:.0} m  rate {:.1} m/s{step}",
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
