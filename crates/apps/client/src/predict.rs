use game_core::{MODULE_SLOT_COUNT, ModuleSlot, TankSpec, TrackHealth};
use glam::Vec3;
use physics::{ContactBody, ContactCache, TankKinematicState};
use sim::{
    AimingState, DriveModuleStatus, TankCommand, TankDriveState, TankDriveWorld, TrackDriveStatus,
    advance_tank_drive, recover_dispersion, settle_tank_drive,
};
use terrain::{HeightMap, StaticCoverObject};

/// The local hull's slot in the predictor's own little roster. Neighbours carry their real tank
/// ids, and no vehicle is ever id 0, so nothing can collide with it.
const LOCAL_HULL_ID: u64 = 0;

/// Lockstep client-side prediction of the local tank's hull: stepped with the same fixed
/// dt and the same input as the authoritative server, so it matches the server tick-for-tick
/// and yields a smooth 60 Hz local position between 20 Hz snapshots — without interpolation
/// lag and, crucially, without per-snapshot reconciliation jitter.
pub struct LocalPredictor {
    spec: TankSpec,
    drive: TankDriveState,
    hit_points: u32,
    module_hit_points: [u32; MODULE_SLOT_COUNT],
    destroyed_modules_mask: u8,
    tracks: TrackHealth,
    /// Crew battle wounds, reconciled from snapshots and ticked between them (v46) — the
    /// predictor must drive and settle the sight with the same wounded hands the server does.
    crew: game_core::CrewVitals,
    /// The ammo slot the player believes is loaded: set optimistically on the 1/2/3 keys so the
    /// reticle's ballistics answer the same frame, reconciled from snapshots.
    selected_ammo: u8,
    /// Hardest landing (m/s of absorbed fall speed) since the render loop last consumed it.
    pending_landing_impact_mps: f32,
    seeded: bool,
    /// Pose at the start of the most recent tick, kept so rendering can interpolate the
    /// gap between the previous and current tick instead of snapping at 60 Hz boundaries.
    previous: PredictedPose,
    /// Signed forward speed at the start of the most recent tick — the motion twin of
    /// `previous`, so the presented speed interpolates alongside the presented pose.
    previous_forward_speed_mps: f32,
    /// Forward acceleration over the most recent tick (m/s², tick-domain, exact).
    tick_accel_long_mps2: f32,
    /// The battle map's standing water — a map property, so it survives vehicle resets. The
    /// predicted wading drag must match the server's or fording would reconciliation-jitter.
    water: Option<terrain::WaterBody>,
    /// The contact solver's memory, exactly as the server keeps one. Without it the predicted hull
    /// would rediscover every contact from nothing each tick while the authority did not, and the
    /// two would disagree by more the longer they leaned on each other.
    contacts: ContactCache,
}

impl LocalPredictor {
    pub fn new(spec: &TankSpec) -> Self {
        Self {
            spec: spec.clone(),
            drive: TankDriveState {
                kinematic: TankKinematicState::default(),
                aiming: AimingState::default(),
                aim_dispersion_mrad: spec.gun.dispersion_mrad,
            },
            hit_points: spec.hit_points,
            module_hit_points: spec.module_health.hit_points_by_slot(),
            destroyed_modules_mask: 0,
            crew: game_core::CrewVitals::default(),
            tracks: TrackHealth::healthy(),
            selected_ammo: spec.ammo.initial_selected,
            pending_landing_impact_mps: 0.0,
            seeded: false,
            previous_forward_speed_mps: 0.0,
            tick_accel_long_mps2: 0.0,
            water: None,
            contacts: ContactCache::default(),
            previous: PredictedPose {
                position: Vec3::ZERO,
                yaw_rad: 0.0,
                hull_pitch_rad: 0.0,
                hull_roll_rad: 0.0,
                turret_yaw_rad: 0.0,
                gun_pitch_rad: 0.0,
            },
        }
    }

    pub fn is_seeded(&self) -> bool {
        self.seeded
    }

    pub fn reset_to_spec(&mut self, spec: &TankSpec) {
        // Water belongs to the map, not the vehicle — a garage swap keeps the same river.
        let water = self.water;
        *self = Self::new(spec);
        self.water = water;
    }

    /// Install the battle map's standing water (see [`terrain::WaterBody`]) so predicted wading
    /// matches the authoritative step. `None` (the default) is a dry map.
    pub fn set_water(&mut self, water: Option<terrain::WaterBody>) {
        self.water = water;
    }

    pub fn spec(&self) -> &TankSpec {
        &self.spec
    }

    pub fn selected_ammo(&self) -> u8 {
        self.selected_ammo
    }

    /// Adopt an ammo switch optimistically; the server applies the same request on the next
    /// command, so lockstep keeps them agreeing (invalid slots are clamped like the sim does).
    pub fn set_selected_ammo(&mut self, slot: u8) {
        self.selected_ammo = slot.min(game_core::MAX_AMMO_SLOTS as u8 - 1);
    }

    /// The shell the currently selected slot fires, from the predictor's (custom) loadout.
    pub fn selected_shell(&self) -> game_core::ShellSpec {
        let options = self.spec.gun.ammo_options();
        options[(self.selected_ammo as usize).min(options.len() - 1)]
    }

    /// Advance one fixed tick with the same input being sent to the server.
    #[allow(clippy::too_many_arguments)]
    pub fn step(
        &mut self,
        command: TankCommand,
        heightmap: &HeightMap,
        cover: &[StaticCoverObject],
        neighbours: &[ContactBody],
        rubble: &[terrain::RubbleMound],
        ground: Option<&terrain::GroundClassifier>,
        dt: f32,
    ) {
        // Record the pre-step pose so the renderer can interpolate previous -> current over
        // the sub-tick remainder. Captured before any early return so it always tracks the
        // last fully-resolved tick.
        self.previous = self.current_pose();
        let speed_before = self.drive.kinematic.forward_speed();
        self.previous_forward_speed_mps = speed_before;
        self.step_drive(command, heightmap, cover, neighbours, rubble, ground, dt);
        self.tick_accel_long_mps2 =
            (self.drive.kinematic.forward_speed() - speed_before) / dt.max(1.0e-6);
    }

    #[allow(clippy::too_many_arguments)]
    fn step_drive(
        &mut self,
        command: TankCommand,
        heightmap: &HeightMap,
        cover: &[StaticCoverObject],
        neighbours: &[ContactBody],
        rubble: &[terrain::RubbleMound],
        ground: Option<&terrain::GroundClassifier>,
        dt: f32,
    ) {
        // Mirror the server: dispersion recovers every tick, even for a dead hull — with the
        // same wounded gunner's hands the authority settles with (crew-damage, v46).
        let gun_damage_fraction = self.module_damage_fraction(ModuleSlot::Gun);
        let gunner = self.crew.effectiveness(game_core::CrewRole::Gunner);
        recover_dispersion(
            &mut self.drive.aim_dispersion_mrad,
            &self.spec,
            gun_damage_fraction,
            dt * gunner,
        );
        if self.hit_points == 0 {
            self.drive.kinematic.velocity = Vec3::ZERO;
            self.drive.kinematic.yaw_rate_rad_s = 0.0;
            return;
        }
        // First aid ticks between snapshots exactly as the server ticks it, so the drive
        // penalty lifts the same tick on both sides instead of jumping at snapshot cadence.
        self.crew.step_first_aid(dt);
        let tracks = if self.module_destroyed(ModuleSlot::Suspension) {
            TrackDriveStatus::broken()
        } else {
            TrackDriveStatus::from_track_health(&self.tracks)
        };
        let modules = DriveModuleStatus::from_module_hp(
            tracks,
            self.module_hit_points,
            &self.spec,
            self.crew.effectiveness(game_core::CrewRole::Driver),
        );
        let footprint = self.spec.contact_footprint();
        let world = TankDriveWorld {
            heightmap: Some(heightmap),
            cover,
            footprint: Some(&footprint),
            water: self.water,
            rubble,
            // The predictor must read the SAME surface the server does, or the local hull grips
            // differently on a road than the authority says it does.
            ground,
        };
        let command = command.clamped();

        // The authority's tick, in the authority's order: decide a velocity, exchange contacts
        // against it, then spend what survived. The predictor used to REFUSE a move that would
        // overlap a neighbour instead — a hard stop the server has not used since contact started
        // carrying momentum, and the two answered differently by the whole of the old skin. It
        // also meant a shove could never be predicted at all, only discovered as a correction.
        let phase = advance_tank_drive(&mut self.drive, &self.spec, modules, world, command, dt);
        self.exchange_contacts(neighbours, dt);
        let ground =
            settle_tank_drive(&mut self.drive, &self.spec, modules, world, command, phase, dt);
        self.pending_landing_impact_mps =
            self.pending_landing_impact_mps.max(ground.landing_impact_mps);
    }

    /// Solve the local hull against its neighbours under the SERVER'S rules — both hulls movable,
    /// because that is what the authority solves and the local half of the answer is the half that
    /// has to match. Only `bodies[0]` is taken: the neighbour's share of the exchange is computed
    /// and thrown away, so the client still never predicts where anyone else ends up.
    ///
    /// This used to pin neighbours immovable, reasoning that predicting a shove would "only mint a
    /// correction". It was backwards. Refusing to predict the shove is what minted the correction:
    /// the server let the local hull push a hull aside and kept its speed, the predictor stopped it
    /// dead, and every snapshot yanked the camera forward to catch up — 20 times a second, for
    /// about a second, on every flank ram. Measured in `parity_tests`.
    fn exchange_contacts(&mut self, neighbours: &[ContactBody], dt: f32) {
        if neighbours.is_empty() {
            self.contacts = ContactCache::default();
            return;
        }
        let mut bodies = Vec::with_capacity(neighbours.len() + 1);
        bodies.push(ContactBody {
            id: LOCAL_HULL_ID,
            position: self.drive.kinematic.position,
            velocity: self.drive.kinematic.velocity,
            yaw_rad: self.drive.kinematic.yaw_rad,
            yaw_rate_rad_s: self.drive.kinematic.yaw_rate_rad_s,
            footprint: physics::TankFootprint::from_plan(self.spec.hull_plan()),
            mass_kg: self.spec.mass_kg,
            movable: true,
        });
        bodies.extend_from_slice(neighbours);
        let report = physics::resolve_contacts(&bodies, &mut self.contacts, dt);
        let taken = report.bodies[0];
        if taken.is_empty() {
            return;
        }
        self.drive.kinematic.velocity += taken.delta_velocity;
        self.drive.kinematic.yaw_rate_rad_s += taken.delta_yaw_rate_rad_s;
    }

    /// The hardest landing since the last call, consumed by the render loop for the camera slam.
    pub fn take_landing_impact_mps(&mut self) -> f32 {
        std::mem::take(&mut self.pending_landing_impact_mps)
    }

    fn module_destroyed(&self, slot: ModuleSlot) -> bool {
        self.destroyed_modules_mask & slot.destroyed_mask_bit() != 0
    }

    fn module_damage_fraction(&self, slot: ModuleSlot) -> f32 {
        let full_hp = self.spec.module_health.hit_points(slot).max(1) as f32;
        let live_hp = self.module_hit_points[slot.wire_index()] as f32;
        (1.0 - live_hp / full_hp).clamp(0.0, 1.0)
    }

    pub fn position(&self) -> Vec3 {
        self.drive.kinematic.position
    }

    pub fn yaw(&self) -> f32 {
        self.drive.kinematic.yaw_rad
    }

    pub fn speed_mps(&self) -> f32 {
        self.drive.kinematic.speed()
    }

    /// How hard the powerplant is actually working, 0..1, as the ear should hear it
    /// (Immersja C1). Raw throttle used to BE the engine load, so a hull crawling up a
    /// grade at full pedal and one idling forward on flat reported the same number.
    /// Composed from what the rigid body already knows this tick:
    /// - a BASE share of the demand (an engine at flat-out cruise works, relaxed),
    /// - a CLIMB term: the hull's nose-up grade in the direction of drive,
    /// - a LAG term: demand that became neither speed nor acceleration — the drivetrain
    ///   fighting a grade, rolling resistance or a wall.
    ///
    /// Freeze-safe by construction: `freeze_motion()` retires the speed and acceleration
    /// this reads, and the climb term dies with the demand.
    pub fn drive_strain(&self, throttle: f32) -> f32 {
        let demand = throttle.abs().clamp(0.0, 1.0);
        if demand <= f32::EPSILON {
            return 0.0;
        }
        let drive_sign = throttle.signum();
        // 0.30 rad (~17 deg) of grade under power is a full-strain climb.
        let climb = ((self.drive.kinematic.pitch_rad * drive_sign).max(0.0) / 0.30).min(1.0);
        let top_speed = self.spec.max_forward_speed_mps.max(1.0);
        let speed_share =
            ((self.drive.kinematic.forward_speed() * drive_sign).max(0.0) / top_speed).min(1.0);
        // ~2.5 m/s^2 is a healthy pull-away: acceleration that materialized is not strain.
        let accel_share = ((self.tick_accel_long_mps2 * drive_sign).max(0.0) / 2.5).min(1.0);
        let lag = (demand - speed_share.max(accel_share)).max(0.0);
        (demand * 0.55 + (climb + lag) * 0.45).clamp(0.0, 1.0)
    }

    /// Freeze the locally presented hull when authority is no longer accepting commands.
    ///
    /// Merely skipping future prediction is not enough: the last predicted velocity would keep
    /// engine audio, suspension lean and the speedometer alive over a frozen world. Anchor both
    /// interpolation endpoints on the current pose and retire every motion derivative.
    pub fn freeze_motion(&mut self) {
        self.drive.kinematic.velocity = Vec3::ZERO;
        self.drive.kinematic.yaw_rate_rad_s = 0.0;
        self.drive.aiming.turret_yaw_velocity_rad_s = 0.0;
        self.previous = self.current_pose();
        self.previous_forward_speed_mps = 0.0;
        self.tick_accel_long_mps2 = 0.0;
    }

    #[cfg(test)]
    pub(crate) fn authoritative_motion(&self) -> net::AuthoritativeMotion {
        net::AuthoritativeMotion {
            velocity_mps: self.drive.kinematic.velocity.to_array(),
            hull_yaw_velocity_rad_s: self.drive.kinematic.yaw_rate_rad_s,
        }
    }

    /// Tick-domain hull motion for the presentation cues (sprung attitude, camera feel), with
    /// the forward speed blended `alpha` into the current tick alongside `interpolated_pose`.
    /// This is the motion source of truth for the local tank: the rigid body knows its velocity,
    /// so the presentation never has to differentiate presented positions against the render
    /// clock (which is what used to jitter the hull).
    pub fn motion(&self, alpha: f32) -> engine::TankMotion {
        let alpha = alpha.clamp(0.0, 1.0);
        let current = self.drive.kinematic.forward_speed();
        engine::TankMotion {
            forward_speed_mps: self.previous_forward_speed_mps
                + (current - self.previous_forward_speed_mps) * alpha,
            accel_long_mps2: self.tick_accel_long_mps2,
            yaw_rate_rad_s: self.drive.kinematic.yaw_rate_rad_s,
        }
    }

    pub fn turret_yaw(&self) -> f32 {
        self.drive.aiming.turret_yaw_rad
    }

    pub fn gun_pitch(&self) -> f32 {
        self.drive.aiming.gun_pitch_rad
    }

    /// The predicted hull orientation as the shared frame the muzzle chain hangs off.
    pub fn hull_pose(&self) -> game_core::math::HullPose {
        game_core::math::HullPose {
            yaw_rad: self.drive.kinematic.yaw_rad,
            pitch_rad: self.drive.kinematic.pitch_rad,
            roll_rad: self.drive.kinematic.roll_rad,
        }
    }

    /// Locally predicted aim dispersion in milliradians, evolved at 60 Hz from the last snapshot
    /// so the reticle's aim circle tracks the server between 20 Hz updates instead of lagging it.
    pub fn aim_dispersion_mrad(&self) -> f32 {
        self.drive.aim_dispersion_mrad
    }

    /// The dispersion this gun is recovering TOWARD — the settled minimum including whatever the
    /// gun module has taken. The same value [`Self::step`] feeds the recovery curve, so the sight
    /// can ask "has the aim been taken?" against the floor the circle is actually falling to.
    pub fn settled_dispersion_mrad(&self) -> f32 {
        sim::base_dispersion_mrad(&self.spec, self.module_damage_fraction(ModuleSlot::Gun))
    }
}

mod pose;
mod sync;

pub use pose::PredictedPose;

#[cfg(test)]
mod interpolation_tests;
#[cfg(test)]
mod obstacle_tests;
#[cfg(test)]
mod parity_tests;
#[cfg(test)]
mod tests;
