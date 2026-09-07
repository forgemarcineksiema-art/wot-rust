//! One test binary for `sim`: every integration test file is a module here, so a change
//! links ONE program instead of one per file (2026-09-06: 314 test binaries across the
//! workspace were ~180 CPU-seconds of linking per heavy crate on every gate). Goldens and
//! env-gated tests (`gate_completeness.rs`) keep their own binaries: they re-record under an
//! env var and must not share a process with a value test of the same file.

#[path = "../common/mod.rs"]
mod common;

mod aim_dispersion;
mod ammo_rack;
mod armor_geometry;
mod backface_spall;
mod ballistics;
mod brake_dip;
mod bystra_hull_down;
mod bystra_river_contract;
mod combat_pipeline;
mod combat_replay_regression;
mod cookoff;
mod cover_combat;
mod cover_destruction;
mod crew_damage;
mod crew_repair;
mod drowning;
mod fire;
mod fire_buffer;
mod fixed_tick;
mod ghost_barrel_honesty;
mod ground_ricochet;
mod gun_arc;
mod he_splash;
mod hull_attitude_combat;
mod hull_rest;
mod hulls_touch;
mod impact_normal;
mod landing_damage;
mod landing_roll;
mod low_tier_fleet;
mod module_damage;
mod observer_cap;
mod orliny_watchtower;
mod ostrogorsk_urban;
mod perforation_exit;
mod perforation_replay;
mod queue_holds;
mod ramming_contact;
mod replay_regression;
mod ricochet_continuation;
mod separation_travels;
mod shell_blockers;
mod shell_trace;
mod shooter_aim_no_rewind;
mod spawn_yaw;
mod spotting;
mod spotting_grid;
mod standing_water_sim;
mod steering_into_a_neighbour;
mod tank_collision;
mod terrain_deformation;
mod terrain_movement;
mod tick_policy;
mod track_damage_state;
mod track_drive;
mod track_two_tier;
mod turret_detach;
mod turret_taper;
mod turret_volume;
mod vehicle_identity;
mod vehicle_replacement;
mod wreck_settle;
