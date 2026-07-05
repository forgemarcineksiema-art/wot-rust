mod aim_dispersion;
mod aiming;
mod clock;
mod combat;
mod command;
mod drive_modules;
mod landing;
mod module_hit;
mod ramming;
mod replay;
mod shell;
mod shell_splash;
mod shell_step;
mod shell_trace;
mod spotting;
mod state;
mod tank_drive;
mod tank_factory;
mod tank_state;
mod timestep;

pub use aim_dispersion::recover_dispersion;
pub use aiming::{
    AimingState, GUN_ELEVATION_RATE_RAD_S, MAX_GUN_PITCH_RAD, MIN_GUN_PITCH_RAD, step_aiming,
};
pub use clock::{
    DEFAULT_SERVER_TICK_HZ, DEFAULT_SIMULATION_TICK_HZ, DEFAULT_SNAPSHOT_HZ, SimulationClock,
};
pub use command::TankCommand;
pub use drive_modules::{DriveModuleStatus, TrackDriveStatus};
pub use landing::SAFE_LANDING_MPS;
pub use replay::{Replay, ReplayExpected, ReplayFrame, ReplayReport, ReplaySpawn, run_replay};
pub use shell::ShellState;
pub use shell_trace::{
    SHELL_MAX_AGE_SECONDS, SegmentImpact, ShellTraceWorld, TraceOutcome, TraceTank, segment_impact,
    trace_shell,
};
pub use spotting::{SPOTTING_INTERVAL_TICKS, VIEW_RANGE_M, compute_spotted_masks, line_of_sight};
pub use state::SimulationState;
pub use tank_drive::{TankDriveState, TankDriveWorld, step_tank_drive};
pub use tank_state::TankState;
pub use timestep::FixedTimestep;
