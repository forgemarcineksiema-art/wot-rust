mod audio_link;
mod battle_scars;
mod camera_link;
#[cfg(test)]
mod camera_tests;
#[cfg(test)]
mod fire_fx_tests;
mod frame_scene;
mod garage;
mod garage_render;
#[cfg(test)]
mod hit_mark_tests;
mod ingest;
mod input;
mod input_state;
#[cfg(test)]
mod input_tests;
mod lifecycle;
mod live_cover;
#[cfg(test)]
mod live_cover_tests;
mod loop_step;
mod minimap_build;
pub(crate) mod motion_fx;
mod prediction;
mod reconcile;
mod remote_events;
mod remote_input;
mod render;
#[cfg(test)]
mod render_tests;
mod reticle;
pub(crate) mod session;
mod vehicle_assets;

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

use anyhow::Context;
use game_core::{TankId, VehicleKind};
use renderer_wgpu::WindowRenderer;
use server::{LocalAuthoritativeServer, RandomBattleConfig, ServerTickConfig};
use sim::DEFAULT_SIMULATION_TICK_HZ;
use terrain::BattlefieldMap;
use winit::event_loop::{ControlFlow, EventLoop};
use winit::window::Window;

use crate::aim::DesiredAim;
use crate::app::garage::GarageState;
pub use crate::app::garage::{garage_overlay, garage_overlay_option_list};
use crate::fx::FxSystem;
use crate::hit_indicator::HitIndicator;
use crate::predict::LocalPredictor;
use crate::{
    BattleCameraController, InterpolatedBattleState, VehicleAssetCatalog, WinitLoopDriver,
};

/// Which static scene the renderer currently holds. The garage and the battlefield share one
/// renderer; the active scene's geometry is swapped in on transition (see `render`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SceneKind {
    Garage,
    Battle,
}

/// A finished background statics rebuild: the re-baked dirty buckets plus the phase/scar
/// baseline they were baked against (urban-map program PR-04).
pub(crate) struct StaticsRebuild {
    pub(crate) phases: Vec<u8>,
    pub(crate) scars: Vec<terrain::CoverScar>,
    pub(crate) buckets: Vec<(usize, scene_build::battlefield::SceneMeshData)>,
}

/// The battle scene's baked CPU meshes — see `ClientApp::battle_scene_meshes`.
pub(crate) struct BattleSceneMeshes {
    /// The heightfield the terrain pipeline shades (splat layers + macro normals).
    pub(crate) ground_vertices: Vec<renderer_api::SceneVertex>,
    pub(crate) ground_indices: Vec<u32>,
    /// The baked ground maps (splat + macro normal) — cover changes never touch these.
    ///
    /// Behind an `Arc` because the crater re-mesh hands them to a worker thread: at 1024² they
    /// are ~12 MB (splat + macro normal + puddle propensity), and deep-copying that on the
    /// RENDER thread — which is where the handoff happens — cost a visible hitch in the frame
    /// an HE round landed. A worker only ever reads them.
    pub(crate) ground_maps: std::sync::Arc<renderer_api::TerrainGroundMaps>,
    /// The mid-field card meadow (Żywy Step P2) — the renderer's dressing slot.
    pub(crate) dressing_vertices: Vec<renderer_api::SceneVertex>,
    pub(crate) dressing_indices: Vec<u32>,
    /// Cover, backdrop skirt and scenery — the generic scene pipeline's slot, assembled from
    /// the per-bucket fragments below.
    pub(crate) statics_vertices: Vec<renderer_api::SceneVertex>,
    pub(crate) statics_indices: Vec<u32>,
    /// The statics bake partitioned into XZ buckets (+ backdrop) — a cover-phase change
    /// re-bakes only the dirty buckets and reassembles (urban-map program PR-04).
    pub(crate) statics_buckets: Vec<scene_build::battlefield::SceneMeshData>,
    /// The cover phases and scars the current `statics_buckets` were baked against — the
    /// baseline the next rebuild diffs to find its dirty buckets.
    pub(crate) statics_baked_phases: Vec<u8>,
    pub(crate) statics_baked_scars: Vec<terrain::CoverScar>,
    pub(crate) water_vertices: Vec<renderer_api::WaterVertex>,
    pub(crate) water_indices: Vec<u32>,
}

/// A whole world, compiled and baked off the main thread, waiting for the Battle press that
/// asked for it. Everything here is a pure function of the [`terrain::MapId`], which is what
/// makes the speculative bake safe: if the pick changes, the result is simply dropped.
pub(crate) struct PrebakedWorld {
    battlefield: std::sync::Arc<BattlefieldMap>,
    meshes: BattleSceneMeshes,
    minimap: crate::app::minimap_build::MinimapStaticLayers,
}

/// The in-flight (or finished) speculative bake and the map it is for. At most one exists at a
/// time: cycling the map ring is a click storm, and a bake per click would put four full map
/// bakes on the cores at once — worse than the stall being cured.
pub(crate) struct MapPrebake {
    map: terrain::MapId,
    /// `None` once the worker has been harvested (or has died).
    rx: Option<std::sync::mpsc::Receiver<PrebakedWorld>>,
    ready: Option<PrebakedWorld>,
}

impl MapPrebake {
    /// Harvest the worker without blocking. `true` once the world is in hand.
    fn poll(&mut self) -> bool {
        if self.ready.is_some() {
            return true;
        }
        let Some(rx) = &self.rx else {
            return false;
        };
        match rx.try_recv() {
            Ok(world) => {
                self.ready = Some(world);
                self.rx = None;
                true
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => false,
            // A panicked bake never arrives; forget it so a fresh one can be scheduled.
            Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                self.rx = None;
                false
            }
        }
    }

    fn is_running(&self) -> bool {
        self.rx.is_some()
    }
}

/// Compile and bake a map's whole client-side world. Pure — no `ClientApp` state is read, so it
/// runs on a worker. Cover phases come from the BIRTH rule, which is exactly where a fresh
/// battle's replicated states start; a battle that somehow disagrees repairs itself through the
/// ordinary dirty-bucket path (see `adopt_session_map`).
fn bake_world_for(map: terrain::MapId) -> PrebakedWorld {
    let battlefield = std::sync::Arc::new(map_forge::battlefield(map));
    let phases = live_cover::LiveCoverCache::from_born_phases(&battlefield.static_cover);
    let meshes = bake_battle_scene_meshes(&battlefield, map, phases.phase_bytes());
    let minimap = crate::app::minimap_build::minimap_static_layers(&battlefield);
    PrebakedWorld { battlefield, meshes, minimap }
}

/// The battle scene's CPU meshes for one battlefield. Free function so both the lazy in-place
/// bake and the speculative worker bake produce the same thing from the same code.
fn bake_battle_scene_meshes(
    battlefield: &BattlefieldMap,
    map: terrain::MapId,
    cover_phases: &[u8],
) -> BattleSceneMeshes {
    let cover_phases = cover_phases.to_vec();
    let (ground_vertices, ground_indices) = crate::battlefield_ground_mesh(battlefield);
    let statics_buckets = crate::battlefield_statics_buckets(battlefield, &cover_phases, &[]);
    let (statics_vertices, statics_indices) = crate::assemble_statics_mesh(&statics_buckets);
    let ground_maps =
        std::sync::Arc::new(scene_build::terrain_maps::bake_terrain_ground_maps(battlefield));
    let (water_vertices, water_indices) = scene_build::water::battlefield_water_mesh(battlefield);
    let (dressing_vertices, dressing_indices) = scene_build::grass_cards::grass_card_dressing_mesh(
        battlefield,
        &ground_maps,
        &scene_build::terrain_maps::terrain_material_set_for(map),
    );
    BattleSceneMeshes {
        ground_vertices,
        ground_indices,
        ground_maps,
        dressing_vertices,
        dressing_indices,
        statics_vertices,
        statics_indices,
        statics_buckets,
        statics_baked_phases: cover_phases,
        statics_baked_scars: Vec::new(),
        water_vertices,
        water_indices,
    }
}

impl ClientApp {
    /// Keep the speculative bake pointed at the map the garage is showing, so the Battle press
    /// finds a finished world instead of baking one. Called once per garage frame rather than on
    /// the click: cycling the ring fires several picks in a second, and one bake per click would
    /// pile four full map bakes onto the cores while the garage is still drawing.
    ///
    /// At most one worker runs. A pick that lands while a bake is in flight waits for it — worst
    /// case one wasted bake and a press that falls back to baking in place, which is exactly the
    /// behaviour this replaces, never worse.
    ///
    /// Only an EXPLICIT pick is speculated on. AUTO is the map already loaded on the first
    /// deploy (nothing to bake), and resolving it otherwise means re-reading `WOT_MAP` — a file
    /// read, on the editor's playtest path — which has no business running every frame.
    pub(in crate::app) fn poll_map_prebake(&mut self) {
        let Some(target) = self.garage.selected_map() else {
            self.drop_idle_prebake();
            return;
        };
        if target == self.session.map_id() {
            self.drop_idle_prebake();
            return;
        }
        if let Some(prebake) = self.map_prebake.as_mut() {
            prebake.poll();
            if prebake.map == target || prebake.is_running() {
                return;
            }
        }
        // Let the pick settle first. Cycling the ring passes THROUGH maps on the way to the one
        // the player wants; baking each stop would spend a core-second per click and hitch the
        // garage that is drawing right now. A human's next click is ~200-400 ms away, so a short
        // window skips the fly-past and still leaves the real pick baking long before the press.
        const MAP_PICK_SETTLE: std::time::Duration = std::time::Duration::from_millis(200);
        let now = Instant::now();
        match self.map_pick_settling {
            Some((seen, _)) if seen == target => {}
            _ => self.map_pick_settling = Some((target, now)),
        }
        if self
            .map_pick_settling
            .is_some_and(|(_, since)| now.saturating_duration_since(since) < MAP_PICK_SETTLE)
        {
            return;
        }
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let _ = tx.send(bake_world_for(target));
        });
        self.map_prebake = Some(MapPrebake { map: target, rx: Some(rx), ready: None });
    }

    /// Forget a bake nothing is waiting for. A RUNNING worker is kept: dropping the receiver
    /// would not stop the thread, and holding it means a pick that swings back reuses the work.
    fn drop_idle_prebake(&mut self) {
        if !self.map_prebake.as_ref().is_some_and(MapPrebake::is_running) {
            self.map_prebake = None;
        }
    }

    /// Claim the speculative bake if it is for `map`, waiting for the worker if the press beat
    /// it. A bake for a different map (or a worker that died) yields `None` and the caller bakes
    /// in place.
    fn take_prebaked_world(&mut self, map: terrain::MapId) -> Option<PrebakedWorld> {
        let prebake = self.map_prebake.take()?;
        if prebake.map != map {
            return None;
        }
        prebake.ready.or_else(|| prebake.rx?.recv().ok())
    }

    /// Adopt the world the session is now playing on. The map the server simulates is the
    /// ONLY map the client may hold: the heightmap the local predictor stands on, the cover
    /// it pushes against, the minimap, the camera leash and the baked scene all come from
    /// that one id. Deploying onto a different map without this left the client on the
    /// previous world — every map looked identical (the scene bake is cached for the app's
    /// lifetime), remote tanks hung at server altitudes over the wrong ground, and the
    /// player's own hull fought a correction every snapshot because prediction and authority
    /// disagreed about where the floor was.
    ///
    /// Costly — a full map compile plus the terrain bake — so it runs for map CHANGES only: a
    /// redeploy onto the same map keeps its cached scene, which is what the cache is for.
    ///
    /// Measured on the Battle press, release, Ostrogorsk from the Bystra default:
    /// 517 ms originally; 307 ms once the ground-map and statics bakes went parallel; **49 ms**
    /// with [`Self::poll_map_prebake`] having done the work behind the garage — against 29 ms
    /// for a redeploy that changes no map at all, so the world swap itself now costs ~20 ms.
    /// A pick pressed with no frame in between still falls back to baking here (275 ms).
    pub(crate) fn adopt_session_map(&mut self) {
        let map = self.session.map_id();
        let prebaked = self.take_prebaked_world(map);
        let (battlefield, meshes, minimap) = match prebaked {
            Some(world) => (world.battlefield, Some(world.meshes), world.minimap),
            None => {
                let battlefield = std::sync::Arc::new(map_forge::battlefield(map));
                let minimap = crate::app::minimap_build::minimap_static_layers(&battlefield);
                (battlefield, None, minimap)
            }
        };
        let opening = self.session.latest_snapshot_for_player();
        self.live_cover = live_cover::LiveCoverCache::from_replicated(
            &battlefield.static_cover,
            &opening.cover_states,
        )
        .unwrap_or_else(|| live_cover::LiveCoverCache::from_born_phases(&battlefield.static_cover));
        self.minimap_static = minimap;
        self.predictor.set_water(battlefield.water);
        self.camera_controller = BattleCameraController::new(Self::map_camera_settings(map));
        self.battlefield = battlefield;
        // Everything keyed to the old world is void: the cached bake, the bakes in flight (their
        // worker holds a clone of the previous battlefield), and the ledgers those bakes diff
        // against. Clearing the flags too keeps a stale dirty bit from re-baking the fresh scene.
        self.battle_scene_meshes = meshes;
        self.scene_rebuild_rx = None;
        self.ground_rebuild_rx = None;
        self.scene_cover_dirty = false;
        self.ground_deform_dirty = false;
        self.cover_scar_list.clear();
        // The near-field grass population is world-anchored and re-keyed on eye travel and the
        // crater ledger — neither of which notices that the ground under it is a different map.
        self.grass_cache.clear();
        self.grass_cache_eye = None;
        // The speculative bake had to guess the cover phases (birth rule) before the battle
        // existed. If the authority opened on anything else, the ordinary dirty-bucket rebuild
        // repairs exactly the buckets that differ instead of re-baking the map.
        if self
            .battle_scene_meshes
            .as_ref()
            .is_some_and(|meshes| meshes.statics_baked_phases != self.live_cover.phase_bytes())
        {
            self.scene_cover_dirty = true;
        }
        self.ensure_battle_scene_meshes();
        // The renderer still holds the previous world in its static slots. Rather than push the
        // new one here — behind the garage, whose own scene is what is actually on screen — mark
        // the upload stale and let `ensure_scene` do it on the transition it already owns.
        self.scene_upload_dirty = true;
    }

    /// Bake the battle scene's meshes if they are not cached yet. Idempotent; the heavy CPU
    /// work (the full 1000 m terrain + cover + backdrop bake) runs at most once per app.
    pub(crate) fn ensure_battle_scene_meshes(&mut self) {
        if self.battle_scene_meshes.is_some() {
            return;
        }
        // Born-ruins (PR-07): the pre-snapshot bake reads the same birth rule the server's
        // states start from, so a ruined block is a mound from the very first frame — baked
        // per bucket (PR-04), with the birth phases as the dirty-diff baseline.
        self.battle_scene_meshes = Some(bake_battle_scene_meshes(
            &self.battlefield,
            self.session.map_id(),
            self.live_cover.phase_bytes(),
        ));
    }
}

#[derive(Default)]
pub(crate) struct InputState {
    forward: bool,
    back: bool,
    left: bool,
    right: bool,
    brake: bool,
    mouse_dx: f32,
    mouse_dy: f32,
    fire_pending: bool,
    /// Ammo slot requested with 1/2/3 this frame, consumed once by the next fixed-tick batch.
    pending_ammo_select: Option<u8>,
    free_look: bool,
    /// Camera pitch captured when free look began, restored on release.
    free_look_return_pitch: Option<f32>,
    /// Fractional wheel motion below one notch, carried between scroll events.
    wheel_pending_lines: f32,
    /// Whether Shift is currently held — the garage uses Shift+click to cycle a module backward.
    shift: bool,
    /// Camera mode to restore when a Shift sniper-hold ends; `Some` only while Shift holds the
    /// scope open. Mirrors `free_look_return_pitch`: captured on press, restored on release.
    sniper_hold_return: Option<crate::BattleCameraMode>,
}

/// The ESC modal's live state. Only the cursor is stored: which button is hot is DERIVED from it
/// through the same hit test the click uses, so the lit button and the activated button cannot
/// drift apart.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct PauseMenuState {
    cursor_clip: [f32; 2],
}

impl PauseMenuState {
    /// Opens with the cursor parked outside clip space, so nothing is pre-lit until the player
    /// actually moves the mouse — the destructive choice must never sit hot under a still cursor.
    fn opened() -> Self {
        Self { cursor_clip: [f32::MAX, f32::MAX] }
    }

    fn hovered(&self) -> Option<crate::hud::pause_menu::PauseMenuButton> {
        crate::hud::pause_menu::button_at(self.cursor_clip)
    }
}

pub(crate) struct ClientApp {
    window: Option<Arc<Window>>,
    renderer: Option<WindowRenderer>,
    loop_driver: WinitLoopDriver,
    last_loop_time: Instant,
    session: session::BattleSessionKind,
    weather_timeline: scene_build::weather_timeline::WeatherTimeline,
    weather_frame: scene_build::weather_timeline::WeatherFrame,
    render_state: InterpolatedBattleState,
    camera_controller: BattleCameraController,
    /// Phase-consistent blocking geometry for prediction, sight, and camera. The authored
    /// battlefield slice remains untouched and index-stable for scene rebuilds and scars.
    live_cover: live_cover::LiveCoverCache,
    desired_aim: DesiredAim,
    garage: GarageState,
    /// Behind an `Arc` because background bakes (statics rebuild, crater re-mesh) take a handle
    /// to it. The map carries a 201² heightfield plus every cover/scenery record; deep-copying
    /// it per crater on the RENDER thread was pure hitch, and the workers only read it.
    battlefield: std::sync::Arc<BattlefieldMap>,
    /// The map's ground rule, built once from the battlefield. Not replicated — it is derived
    /// wholly from the map, so the client resolves the same surfaces the server does and the
    /// predictor grips the road exactly where the authority grips it.
    ground: terrain::GroundClassifier,
    player_tank: TankId,
    client_tick: u64,
    /// Fixed ticks run since the last ingested snapshot. Together with the sub-tick remainder it
    /// is the remote interpolation phase — the same clock the snapshots are produced on, so the
    /// remote blend can neither freeze at 1.0 nor jump (which wall-clock integration did).
    ticks_since_snapshot: u32,
    input: InputState,
    predictor: LocalPredictor,
    vehicle_asset_catalog: VehicleAssetCatalog,
    /// Persistent render-side ECS projected from the snapshot buffer; the renderer/HUD read from
    /// this rather than rebuilding the scene from `Vec<TankSnapshot>` each frame.
    presentation: engine::PresentationWorld,
    last_render_time: Instant,
    hit_indicator: HitIndicator,
    /// Rolling dealt/taken damage feed for the left-edge battle log.
    damage_log: crate::hud::damage_log::DamageLog,
    track_feedback: crate::hud::track_callout::TrackFeedback,
    /// Incoming hits awaiting their screen-bearing arcs.
    incoming_hits: crate::hud::hit_direction::IncomingHitFeed,
    /// Battle effects (muzzle flash, smoke, dust, impact bursts, tracers): one particle pool
    /// ticked per presented frame and drawn by the renderer's unlit FX pass.
    fx: FxSystem,
    /// Accumulated battle scars per tank (hit decals in hull/turret local frames). Persistent
    /// across snapshots — the snapshot replicates damage STATE, the scars record its history.
    tank_scars: HashMap<game_core::TankId, crate::vehicle::variation::VehicleVariation>,
    /// Craters and scorch marks where shells struck the ground: a budgeted world-space pool
    /// stamped onto the terrain through the same FX pass as the on-tank decals.
    terrain_scars: crate::fx::TerrainScars,
    /// Ruts the rolling tracks press into the soil (Inna Liga D5): same budgeted world-space
    /// pool discipline as the craters, written from accumulated track travel.
    track_marks: crate::fx::TrackMarks,
    /// Per-tank emission clock for the dead-engine smoke column (seconds since last puff).
    engine_smoke_accum_s: HashMap<game_core::TankId, f32>,
    motion_fx: HashMap<game_core::TankId, motion_fx::MotionFxState>,
    /// Flying-turret animation per decapitated wreck (ammo-rack detonation, protocol v20). Started
    /// when a tank first appears in `Snapshot.detached_turrets`; the turret and gun render objects
    /// of that wreck are then driven from this deterministic arc instead of the snapshot pose.
    turret_popoffs: HashMap<game_core::TankId, crate::vehicle::turret_popoff::TurretPopoff>,
    /// Thrown tracks lying on the field (D6): a budgeted list, oldest shed first out.
    track_ribbons: Vec<crate::vehicle::track_ribbon::TrackRibbon>,
    /// Seconds since each wreck died — the burn-out epilogue's clock (flames, then smoke).
    wreck_age_s: HashMap<game_core::TankId, f32>,
    /// An in-flight background statics rebuild (F7): the cover-collapse bake runs on a worker
    /// thread; the render thread only harvests, reassembles and uploads. The worker bakes ONLY
    /// the dirty buckets (urban-map program PR-04) and reports the phase/scar baseline it baked
    /// against, so the next diff starts from the truth that actually landed.
    scene_rebuild_rx: Option<std::sync::mpsc::Receiver<StaticsRebuild>>,
    /// An in-flight background GROUND re-mesh (true deformation, protocol v31): fresh craters
    /// re-mesh the heightfield on a worker thread; the render thread harvests and swaps the
    /// geometry under the still-bound splat/macro maps.
    /// The channel carries (ground mesh, dressing mesh): the burst that dug the hole also
    /// mowed the card meadow around it, so both rebake from the same ledger in one worker.
    #[allow(clippy::type_complexity)]
    ground_rebuild_rx: Option<
        std::sync::mpsc::Receiver<(
            (Vec<renderer_api::SceneVertex>, Vec<u32>),
            (Vec<renderer_api::SceneVertex>, Vec<u32>),
        )>,
    >,
    /// Set when the replicated crater ledger changed: the next frame kicks a ground re-mesh.
    ground_deform_dirty: bool,
    /// The world-anchored grass population, cached with an invisible margin around the shader's
    /// visible ring. It rebuilds after a four-metre planar step or any crater-ledger mutation.
    grass_cache: Vec<renderer_api::RenderObject>,
    grass_cache_eye: Option<glam::Vec3>,
    grass_cache_crater_fingerprint: u64,
    /// Shells whose flyby crack already played (D8): one N-wave per shell, ever.
    cracked_shells: std::collections::HashSet<game_core::ShellId>,
    /// Per-instance dented hull mesh for each wreck, built once from its recorded penetrations.
    /// The wreck's hull render object is swapped to this handle so a knocked-out tank reads beaten
    /// and dented, not pristine-but-tinted. Presentation only (see `vehicle::wreck_deform`).
    wreck_hull_meshes: HashMap<game_core::TankId, renderer_api::MeshHandle>,
    /// Set when the live-cover phase bytes changed: the next frame rebuilds and re-uploads the
    /// indexed authored scene.
    scene_cover_dirty: bool,
    /// Replicated shell wounds on cover faces (protocol v32); a change re-dresses the statics.
    cover_scar_list: Vec<terrain::CoverScar>,
    /// Smoothed frames-per-second for the HUD readout (EMA over instantaneous frame rate).
    fps_estimate: f32,
    /// Last ~1.5 s of raw frame intervals (seconds) — the HUD's p95 readout turns "it drops
    /// sometimes" into a number per scenario (F9).
    /// Every hull's perforations, accumulated from the reliable lane (protocol v39): they no
    /// longer ride the snapshot, so this is the client's own copy of that permanent state.
    armor_breaches: engine::ArmorBreachStore,
    frame_dt_history: std::collections::VecDeque<f32>,
    /// Reused scratch for the p95 selection — see `ClientApp::frame_p95_ms`.
    frame_p95_scratch: Vec<f32>,
    /// The minimap's static layers (terrain relief, water, roads, cover), computed once per
    /// battlefield instead of resampled every frame. Rebuild alongside `battlefield` if a
    /// future map rotation swaps it mid-session.
    minimap_static: crate::app::minimap_build::MinimapStaticLayers,
    /// Local battle result banner state, derived from the authoritative server outcome.
    battle_outcome: Option<crate::hud::BattleHudOutcome>,
    /// The ESC modal while it is up; `None` is closed. The battle behind it keeps running
    /// (see `hud/pause_menu.rs`).
    pause_menu: Option<PauseMenuState>,
    /// Seconds since the player's latest kill, driving the reticle confirmation; `None` when the
    /// confirmation has played out (see `hud/kill_marker.rs`).
    kill_confirm_age_s: Option<f32>,
    /// Reload seconds remaining at the previous presented frame, for the ready-flash crossing.
    prev_reload_remaining_s: f32,
    /// Seconds since the reload finished, driving the gun-ready flash at the reticle.
    reload_ready_age_s: Option<f32>,
    /// Seconds since a fire click was refused (see `register_fire_intent_feedback`); drives the
    /// red denial pulse at the reticle and expires with it.
    fire_denied_age_s: Option<f32>,
    /// Static scene geometry currently uploaded to the renderer (garage hangar vs battlefield).
    current_scene: SceneKind,
    /// Set when the world behind `current_scene` was replaced (a garage map pick), so the next
    /// `ensure_scene` re-uploads even though the scene KIND did not change.
    scene_upload_dirty: bool,
    /// The speculative bake of the map the garage is pointing at, started once the pick settles
    /// so the Battle press finds it done. Dropped whenever the pick moves elsewhere.
    map_prebake: Option<MapPrebake>,
    /// Which map the pick has been resting on, and since when — the settle window that keeps a
    /// fly-past through the map ring from baking every stop.
    map_pick_settling: Option<(terrain::MapId, Instant)>,
    /// The battle scene's CPU meshes (terrain+cover+backdrop, water), baked lazily ONCE — the
    /// battlefield never changes within a `ClientApp`. Rebaking them synchronously inside the
    /// first battle frame of every garage→battle swap froze that frame for hundreds of
    /// milliseconds on integrated-GPU laptops, and the accumulator then dumped a burst of
    /// catch-up ticks into the next one. A few MB of CPU residency buys a swap that costs only
    /// the GPU upload.
    battle_scene_meshes: Option<BattleSceneMeshes>,
    /// Last known framebuffer size, used to map cursor pixels into clip space for the garage UI.
    viewport: (u32, u32),
    /// Camera mode at the previous presented frame; a change clicks the optics cue.
    prev_camera_mode: Option<crate::BattleCameraMode>,
    /// The platform audio stream; `None` headless or without an output device (silent game).
    audio: Option<crate::audio_out::AudioOutput>,
    /// Sounds produced since the last presented frame, flushed to the device with the frame —
    /// the audio twin of the FX queue (see `app/audio_link.rs`).
    pending_audio: Vec<audio::AudioEvent>,
}

impl ClientApp {
    fn new() -> Self {
        Self::new_with_default_vehicle_artifacts()
    }

    fn new_without_vehicle_artifacts() -> Self {
        // N3: `WOT_CONNECT=host:port` joins a dedicated server instead of hosting the battle
        // in-process. Env-var entry is the honest MVP; the garage UI field is a follow-up.
        if let Ok(target) = std::env::var("WOT_CONNECT")
            && let Some(app) = Self::from_remote_env(target.trim())
        {
            return app;
        }
        Self::from_battle_config(RandomBattleConfig::runtime_from_env(VehicleKind::default()))
    }

    /// Connect, wait to be seated (the lobby may hold us until its deadline), then build the
    /// app around the ASSIGNED tank and the SERVER's map. `None` falls back to a local battle
    /// (bad address, no answer) — the game always starts.
    fn from_remote_env(target: &str) -> Option<Self> {
        let addr: std::net::SocketAddr = target.parse().ok()?;
        let transport =
            net::transport::UdpTransport::bind("0.0.0.0:0".parse().expect("addr")).ok()?;
        let mut remote = session::RemoteSession::connect(addr, Box::new(transport));
        // Pump until seated: the lobby deadline is the server's, so wait generously (session
        // timeout aborts a silent server after 10 s regardless).
        for _ in 0..0_u32.wrapping_add(60_000 / 10) {
            remote.pump();
            if remote.is_seated() {
                break;
            }
            if remote.is_terminal()
                || matches!(remote.state(), net::session::SessionState::Failed(_))
            {
                tracing::warn!(target, "remote connect failed; falling back to a local battle");
                return None;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        if !remote.is_seated() {
            tracing::warn!(target, "the lobby never seated us; falling back to a local battle");
            return None;
        }
        Some(Self::from_session(session::BattleSessionKind::Remote(Box::new(remote))))
    }

    fn from_session(session: session::BattleSessionKind) -> Self {
        let weather_timeline = scene_build::weather_timeline::WeatherTimeline::new(
            session.map_id(),
            session.weather(),
        );
        let weather_frame = weather_timeline.sample(0.0);
        let mut app = Self::from_battle_config(RandomBattleConfig::new(
            server::BattleSeed::fixed(1),
            VehicleKind::default(),
        ));
        let player_tank = session.player_tank();
        let battlefield = std::sync::Arc::new(map_forge::battlefield(session.map_id()));
        let opening_snapshot = session.latest_snapshot_for_player();
        let live_cover = live_cover::LiveCoverCache::from_replicated(
            &battlefield.static_cover,
            &opening_snapshot.cover_states,
        )
        .unwrap_or_else(|| live_cover::LiveCoverCache::from_born_phases(&battlefield.static_cover));
        app.minimap_static = crate::app::minimap_build::minimap_static_layers(&battlefield);
        app.battlefield = battlefield;
        app.live_cover = live_cover;
        app.player_tank = player_tank;
        app.camera_controller =
            BattleCameraController::new(Self::map_camera_settings(session.map_id()));
        app.session = session;
        app.weather_timeline = weather_timeline;
        app.weather_frame = weather_frame;
        app.render_state = InterpolatedBattleState::default();
        app.render_state.accept_authoritative_snapshot(opening_snapshot);
        app
    }

    /// A deterministic app for tests that drive real battle ticks: a runtime-seeded battle
    /// makes such tests flaky by construction (an unlucky roster can reach the player inside
    /// the test window and perturb whatever is being asserted).
    #[cfg(test)]
    pub(crate) fn new_seeded(seed: u64) -> Self {
        Self::from_battle_config(RandomBattleConfig::new(
            server::BattleSeed::fixed(seed),
            VehicleKind::default(),
        ))
    }

    fn from_battle_config(config: RandomBattleConfig) -> Self {
        let local_server = session::BattleSessionKind::Local(Box::new(
            LocalAuthoritativeServer::new_random_7v7(ServerTickConfig::default(), config),
        ));
        let player_tank = local_server.player_tank();
        let opening_snapshot = local_server.latest_snapshot_for_player();
        let opening_cover_phases = opening_snapshot.cover_states.clone();
        let mut render_state = InterpolatedBattleState::default();
        render_state.accept_authoritative_snapshot(opening_snapshot);
        let player_spec = render_state
            .latest_snapshot()
            .and_then(|snapshot| snapshot.tanks.iter().find(|tank| tank.tank_id == player_tank))
            .map_or_else(|| VehicleKind::default().spec(), |tank| tank.vehicle.spec());
        // The authoritative server names the map; the client regenerates the identical
        // battlefield locally (the world never crosses the wire — see `terrain::MapId`).
        let battlefield = std::sync::Arc::new(map_forge::battlefield(local_server.map_id()));
        let camera_settings = Self::map_camera_settings(local_server.map_id());
        let live_cover = live_cover::LiveCoverCache::from_replicated(
            &battlefield.static_cover,
            &opening_cover_phases,
        )
        .unwrap_or_else(|| live_cover::LiveCoverCache::from_born_phases(&battlefield.static_cover));
        let mut predictor = LocalPredictor::new(&player_spec);
        predictor.set_water(battlefield.water);
        let minimap_static = crate::app::minimap_build::minimap_static_layers(&battlefield);
        let weather_timeline = scene_build::weather_timeline::WeatherTimeline::new(
            local_server.map_id(),
            local_server.weather(),
        );
        let weather_frame = weather_timeline.sample(0.0);
        Self {
            ground: terrain::GroundClassifier::new(&battlefield),
            window: None,
            renderer: None,
            loop_driver: WinitLoopDriver::new(DEFAULT_SIMULATION_TICK_HZ),
            last_loop_time: Instant::now(),
            session: local_server,
            weather_timeline,
            weather_frame,
            render_state,
            camera_controller: BattleCameraController::new(camera_settings),
            live_cover,
            desired_aim: DesiredAim::default(),
            garage: GarageState::default(),
            battlefield,
            player_tank,
            client_tick: 0,
            ticks_since_snapshot: 0,
            input: InputState::default(),
            predictor,
            vehicle_asset_catalog: VehicleAssetCatalog::default(),
            presentation: engine::PresentationWorld::default(),
            last_render_time: Instant::now(),
            hit_indicator: HitIndicator::default(),
            damage_log: crate::hud::damage_log::DamageLog::default(),
            track_feedback: crate::hud::track_callout::TrackFeedback::default(),
            incoming_hits: crate::hud::hit_direction::IncomingHitFeed::default(),
            fx: FxSystem::default(),
            tank_scars: HashMap::new(),
            turret_popoffs: HashMap::new(),
            track_ribbons: Vec::new(),
            wreck_age_s: HashMap::new(),
            scene_rebuild_rx: None,
            cover_scar_list: Vec::new(),
            ground_rebuild_rx: None,
            ground_deform_dirty: false,
            grass_cache: Vec::new(),
            grass_cache_eye: None,
            grass_cache_crater_fingerprint: 0,
            cracked_shells: std::collections::HashSet::new(),
            terrain_scars: crate::fx::TerrainScars::default(),
            track_marks: crate::fx::TrackMarks::default(),
            engine_smoke_accum_s: HashMap::new(),
            motion_fx: HashMap::new(),
            wreck_hull_meshes: HashMap::new(),
            scene_cover_dirty: false,
            fps_estimate: 0.0,
            armor_breaches: engine::ArmorBreachStore::default(),
            frame_dt_history: std::collections::VecDeque::with_capacity(96),
            frame_p95_scratch: Vec::with_capacity(96),
            minimap_static,
            battle_outcome: None,
            pause_menu: None,
            kill_confirm_age_s: None,
            prev_reload_remaining_s: 0.0,
            reload_ready_age_s: None,
            fire_denied_age_s: None,
            // The renderer is created with the battlefield mesh (see `create_renderer`); the first
            // garage frame swaps in the hangar. Starting at `Garage` here would skip that swap.
            current_scene: SceneKind::Battle,
            scene_upload_dirty: false,
            map_prebake: None,
            map_pick_settling: None,
            battle_scene_meshes: None,
            viewport: (1280, 720),
            prev_camera_mode: None,
            audio: None,
            pending_audio: Vec::new(),
        }
    }
}

/// Run the desktop client with winit, the local server, and the real wgpu renderer.
pub fn run() -> anyhow::Result<()> {
    let event_loop = EventLoop::new().context("failed to create winit event loop")?;
    event_loop.set_control_flow(ControlFlow::Poll);
    let mut app = ClientApp::new();
    app.enable_garage_persistence();
    app.prebake_playable_vehicle_assets();
    event_loop.run_app(&mut app).context("winit app failed")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixed_ticks_do_not_consume_mouse_look_so_it_stays_per_frame() {
        let mut app = ClientApp::new();
        app.confirm_garage_selection();
        // First step seeds prediction and anchors the camera orbit to the hull facing.
        app.run_fixed_ticks(1);
        let before = app.camera_controller.orbit_yaw_rad();

        app.input.mouse_dx = 120.0;
        app.run_fixed_ticks(1);

        // The fixed step must leave the accumulated mouse delta untouched: look is applied
        // once per rendered frame in `render_now`, decoupled from the 60 Hz tick cadence.
        assert_eq!(app.input.mouse_dx, 120.0, "fixed ticks must not consume mouse look");
        assert!((app.camera_controller.orbit_yaw_rad() - before).abs() < 1.0e-9);

        // Applying look (as render does) consumes the delta and rotates the camera.
        app.apply_mouse_look();
        assert!((app.camera_controller.orbit_yaw_rad() - before).abs() > 1.0e-4);
        assert_eq!(app.input.mouse_dx, 0.0);
    }

    #[test]
    fn new_app_accepts_the_player_filtered_server_snapshot() {
        let app = ClientApp::new();
        let client_view = app.render_state.latest_snapshot().expect("initial client snapshot");
        let server_view = app.session.latest_snapshot_for_player();

        assert_eq!(client_view, &server_view);
        assert!(client_view.tanks.iter().any(|tank| tank.tank_id == app.player_tank));
    }

    #[test]
    fn new_app_uses_random_7v7_local_battle() {
        let app = ClientApp::new();
        let full_snapshot = app.session.latest_snapshot();

        assert_eq!(full_snapshot.tanks.len(), 14);
        assert_eq!(
            full_snapshot.tanks.iter().filter(|tank| tank.team == game_core::TeamId(1)).count(),
            7
        );
        assert_eq!(
            full_snapshot.tanks.iter().filter(|tank| tank.team == game_core::TeamId(2)).count(),
            7
        );
        assert!(app.render_state.latest_snapshot().is_some_and(|snapshot| {
            snapshot.tanks.iter().any(|tank| tank.tank_id == app.player_tank)
        }));
    }

    #[test]
    fn startup_can_preload_forge_artifacts_before_first_vehicle_render() {
        let root = std::env::temp_dir()
            .join(format!("wot_client_startup_forge_artifacts_{}", std::process::id()));
        if root.exists() {
            std::fs::remove_dir_all(&root).expect("remove stale startup artifact root");
        }
        let vehicle_dir = root.join("t54-1951");
        vehicle_forge::ForgeArtifact::bake(
            game_core::VehicleKind::T54_1951,
            vehicle_forge::BakeProfile::Lod0,
        )
        .expect("bake startup artifact")
        .write_to_dir(&vehicle_dir)
        .expect("write startup artifact");

        let app = ClientApp::new_with_vehicle_artifact_root(Some(&root));

        assert_eq!(app.vehicle_asset_catalog.cached_vehicle_count(), 1);
        assert_eq!(app.vehicle_asset_catalog.material_count(), 1);

        std::fs::remove_dir_all(root).expect("remove startup artifact root");
    }
}
