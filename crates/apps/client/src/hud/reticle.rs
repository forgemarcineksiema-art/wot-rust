use game_core::math::HullPose;
use game_core::{ArmorFacing, TankId, TankSpec, TeamId, resolve_penetration_at_distance_on_zone};
use glam::{Mat4, Vec3, Vec4};
use net::TankSnapshot;
use terrain::{HeightMap, StaticCoverObject};

/// Vertical slack under the sight point that still counts as "the shell got there": the terrain
/// sweep resolves a touchdown to within a few centimetres of height (measured at 5 cm on flat
/// ground), and the server flies the very same trace, so this is a shared artefact rather than a
/// preview error.
const GRAZE_SLACK_M: f32 = 0.10;

/// How far the previewed impact may sit from the sight point and still count as "arrives there".
///
/// The honest scale is the ARRIVAL ANGLE, not the range. A shell coming down steeply lands
/// within centimetres of its solution; the same shell arriving nearly flat touches shallow
/// ground metres early, because those few centimetres of vertical slack divide by the sine of a
/// very small angle. A flat 4.5 m constant priced that grazing case into every shot — including
/// the thirty-metre street corner, where 4.5 m is the whole difference between the window and
/// the wall.
fn aim_match_tolerance_m(fired_direction: Vec3, distance_m: f32, muzzle_velocity_mps: f32) -> f32 {
    // Arrival = launch elevation minus what gravity took over the flight (time ~ range/speed).
    let launch = fired_direction.normalize_or_zero().y;
    let fall =
        game_core::math::GRAVITY_MPS2 * distance_m / muzzle_velocity_mps.max(1.0).powi(2).max(1.0);
    let sin_arrival = (launch - fall).abs().max(0.01);
    (GRAZE_SLACK_M / sin_arrival).clamp(0.75, 8.0)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ReticleStatus {
    Clear,
    Blocked,
}

/// Which honesty regime the reticle draws under (`docs/aiming-model-policy.md`): third person is
/// situational awareness — a fully neutral sight that never speaks armor — while sniper mode is
/// deliberate aimed fire, where the pen verdict, the millimeters and the real impact point are
/// the skill loop and may print.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ReticleMode {
    ThirdPerson,
    Sniper,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct ReticleFeedback {
    pub status: ReticleStatus,
    pub aim_world_point: Vec3,
    pub gun_world_point: Vec3,
    pub actual_impact_world_point: Vec3,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct PenetrationHint {
    pub penetrates: bool,
    pub shell_pen_mm: f32,
    pub armor_mm: f32,
    pub facing: ArmorFacing,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct ReticleFeedbackQuery<'a> {
    pub heightmap: &'a HeightMap,
    pub cover: &'a [StaticCoverObject],
    /// The map's standing water — the previewed splash must be the server's splash.
    pub water: Option<terrain::WaterBody>,
    /// The PLAYER'S gun arc (min, max) in radians — a per-vehicle property since the fleet
    /// stopped sharing one hard-coded pair. HULL-relative, like the simulation's own limits.
    pub gun_pitch_limits_rad: (f32, f32),
    /// The firing hull's attitude. The arc test lives in the hull frame, so a tank nose-down on
    /// a ridge is judged by what its gun can reach FROM THAT SLOPE — the depression hull-down
    /// actually buys.
    pub hull_pose: HullPose,
    pub tanks: &'a [TankSnapshot],
    /// Spec of the firing (local) vehicle. Penetration hints read the shell from here so the HUD
    /// shows the *player's* gun, not the gun of whatever tank happens to be under the reticle.
    pub player_spec: &'a TankSpec,
    pub owner: TankId,
    pub owner_team: TeamId,
    pub muzzle: Vec3,
    pub aim: Vec3,
    /// World-space unit direction of the barrel (hull pose already applied).
    pub gun_direction: Vec3,
    pub muzzle_velocity_mps: f32,
    /// The player shell's linear drag — feedback and pen hint fly the server's air.
    pub drag_per_s: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct ReticleReport {
    pub feedback: ReticleFeedback,
    pub penetration: Option<PenetrationHint>,
}

/// One authoritative-shaped ballistic trace serves both the reticle status and the penetration
/// hint — running the identical trace twice per frame doubled the most expensive HUD work.
///
/// It flies THE SHOT THE PLAYER IS ASKING FOR — the firing solution this gun can take toward the
/// sight point ([`crate::aim::firing_solution`]) — not wherever the barrel happens to sweep
/// mid-traverse. That is what makes one trace enough to answer every question the sight asks:
/// does the shell arrive (status), where does it land if the arc cannot reach (impact X), and
/// what does it meet there (penetration). It also stops the verdict from flickering through
/// every tank and barn the barrel crosses on its way to the target.
pub(crate) fn reticle_report(query: ReticleFeedbackQuery<'_>) -> ReticleReport {
    let solution = crate::aim::firing_solution(
        query.muzzle,
        query.aim,
        query.hull_pose,
        query.gun_pitch_limits_rad,
        query.muzzle_velocity_mps,
        query.drag_per_s,
    );
    // Sight point on the muzzle (no bearing to solve): fall back to the live barrel.
    let fired_direction = solution.map_or(query.gun_direction, |s| s.world_direction);
    let outcome =
        crate::hud::reticle_sweep::reticle_trace(crate::hud::reticle_sweep::ReticleTraceQuery {
            heightmap: query.heightmap,
            cover: query.cover,
            water: query.water,
            tanks: query.tanks,
            owner: query.owner,
            owner_team: query.owner_team,
            muzzle: query.muzzle,
            gun_direction: fired_direction,
            muzzle_velocity_mps: query.muzzle_velocity_mps,
            projectile_radius_m: query.player_spec.gun.shell.collision_radius_m(),
            drag_per_s: query.drag_per_s,
        });
    ReticleReport {
        feedback: feedback_from_outcome(&query, &outcome, solution, fired_direction),
        penetration: penetration_from_outcome(&query, &outcome),
    }
}

#[cfg(test)]
pub(crate) fn reticle_feedback(query: ReticleFeedbackQuery<'_>) -> ReticleFeedback {
    reticle_report(query).feedback
}

#[cfg(test)]
pub(crate) fn penetration_hint(query: ReticleFeedbackQuery<'_>) -> Option<PenetrationHint> {
    reticle_report(query).penetration
}

/// ONE verdict, read off the one trace: does the shot the player is asking for arrive where they
/// are pointing?
///
/// This used to be an OR of three separate geometries — a flat 4.5 m interception window, a
/// straight muzzle->aim chord sampled against the heightmap, and an arc test that compared a
/// world elevation to hull-relative limits. Each patched a hole in the previous one and opened
/// its own: the chord lies the other way (a ballistic arc rides ABOVE it, so a crest it grazes
/// read as blocked while the shell would clear it), and the arc test failed on exactly the
/// nose-down ridge pose hull-down exists for.
fn feedback_from_outcome(
    query: &ReticleFeedbackQuery<'_>,
    outcome: &sim::TraceOutcome,
    solution: Option<crate::aim::FiringSolution>,
    fired_direction: Vec3,
) -> ReticleFeedback {
    let actual_impact_world_point = outcome.impact_point();
    let distance = query.aim.distance(query.muzzle).max(1.0);
    // The arc is the simulation's gun arc for THIS vehicle, judged in the hull frame: the T-54's
    // -5 is the whole reason it cannot fight from a ridge, and the slope it stands on decides
    // what that -5 reaches in the world.
    let in_arc = solution.is_none_or(|solution| solution.in_arc);
    // BLOCKED means something *stops* the shell short of where the player points: the gun cannot
    // reach that elevation, or the shot dies on terrain, cover, an ally or a wreck along the way.
    // An open-sky shot (the trace expires in flight with nothing hit) is NOT blocked — it is a
    // shot with no target, and flagging the whole horizon taught players to ignore the signal.
    let arrives = matches!(outcome, sim::TraceOutcome::Expired(_))
        || actual_impact_world_point.distance(query.aim)
            <= aim_match_tolerance_m(fired_direction, distance, query.muzzle_velocity_mps);
    let status = if in_arc && arrives { ReticleStatus::Clear } else { ReticleStatus::Blocked };
    // The gun marker reports the LIVE barrel, not the solution — that is its whole job.
    let gun_world_point = query.muzzle + query.gun_direction.normalize_or_zero() * distance;

    ReticleFeedback {
        status,
        aim_world_point: query.aim,
        gun_world_point,
        actual_impact_world_point,
    }
}

/// The verdict for the shot under the crosshair. It rides the same solution trace, so it follows
/// what the player is AIMING AT instead of flashing green/red through every hull the barrel
/// happens to sweep across on its way there.
fn penetration_from_outcome(
    query: &ReticleFeedbackQuery<'_>,
    outcome: &sim::TraceOutcome,
) -> Option<PenetrationHint> {
    // Only a tank impact has armor to penetrate; terrain/cover/open-sky shots have no hint.
    let sim::TraceOutcome::Tank { id, facing, zone, impact_angle_degrees, distance_m, .. } =
        *outcome
    else {
        return None;
    };
    // The player fires their own shell at the *target's* armor — read the shell from the player's
    // spec, not from the tank we are pointing at.
    let target_hull = query.tanks.iter().find(|tank| tank.tank_id == id)?.vehicle.spec_ref().hull;
    let result = resolve_penetration_at_distance_on_zone(
        &query.player_spec.gun.shell,
        &target_hull,
        zone,
        impact_angle_degrees,
        distance_m,
    );
    Some(PenetrationHint {
        penetrates: result.penetrated,
        shell_pen_mm: result.effective_armor_mm + result.remaining_penetration_mm,
        armor_mm: result.effective_armor_mm,
        facing,
    })
}

pub(crate) fn world_to_clip_xy(point: Vec3, view_projection: [[f32; 4]; 4]) -> Option<[f32; 2]> {
    let clip =
        Mat4::from_cols_array_2d(&view_projection) * Vec4::new(point.x, point.y, point.z, 1.0);
    if clip.w <= f32::EPSILON {
        return None;
    }
    let ndc = clip.truncate() / clip.w;
    (ndc.x.abs() <= 1.2 && ndc.y.abs() <= 1.2).then_some([ndc.x, ndc.y])
}

#[cfg(test)]
mod seam_tests;
#[cfg(test)]
mod tests;
