//! The gameplay-layer tools (M7): spawns move, nav points and capture zones place — all
//! document edits through `apply_edit`, all mirror-fair BY CONSTRUCTION on fair maps
//! (a point or zone lands with its twin; the symmetry Error cannot fire on a gesture).

use map_forge::blueprint::{CaptureZoneSpec, MapBlueprint, SpawnSpec, StrategicPointSpec, XCoord};
use terrain::StrategicRole;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GameplayTool {
    MoveSpawn1,
    MoveSpawn2,
    NavPoint,
    Zone,
}

impl GameplayTool {
    pub const CYCLE: [GameplayTool; 4] = [
        GameplayTool::MoveSpawn1,
        GameplayTool::MoveSpawn2,
        GameplayTool::NavPoint,
        GameplayTool::Zone,
    ];

    pub fn label(self) -> &'static str {
        match self {
            GameplayTool::MoveSpawn1 => "move spawn 1",
            GameplayTool::MoveSpawn2 => "move spawn 2",
            GameplayTool::NavPoint => "nav point",
            GameplayTool::Zone => "capture zone",
        }
    }
}

pub const ROLES: [StrategicRole; 5] = [
    StrategicRole::HighGround,
    StrategicRole::Crossing,
    StrategicRole::Observation,
    StrategicRole::HullDown,
    StrategicRole::FlankRoute,
];

/// Move (or create) a team's spawn. On a fair map the OTHER team's spawn mirrors too —
/// both teams get equivalent ground, which is the entire fairness contract.
pub fn move_spawn(blueprint: &mut MapBlueprint, team: u16, at: [f32; 2]) -> String {
    let at = [at[0].round(), at[1].round()];
    let axis_z = blueprint.grid.axis_z();
    let mirrored = blueprint.symmetry.is_some();
    let place = |spawns: &mut Vec<SpawnSpec>, team: u16, at: [f32; 2], yaw: f32| match spawns
        .iter_mut()
        .find(|spawn| spawn.team == team)
    {
        Some(spawn) => {
            spawn.at = at;
            spawn.facing_yaw_rad = yaw;
        }
        None => spawns.push(SpawnSpec { team, at, facing_yaw_rad: yaw, radius_m: None }),
    };
    // Face the map's centre: south spawns look north (0), north spawns look south (pi).
    let yaw = if at[1] <= axis_z { 0.0 } else { std::f32::consts::PI };
    place(&mut blueprint.gameplay.spawns, team, at, yaw);
    if mirrored {
        let twin_team = if team == 1 { 2 } else { 1 };
        let twin_at = [at[0], axis_z * 2.0 - at[1]];
        place(&mut blueprint.gameplay.spawns, twin_team, twin_at, std::f32::consts::PI - yaw);
        format!("spawn {team} at {:.0}, {:.0} (team {twin_team} mirrored)", at[0], at[1])
    } else {
        format!("spawn {team} at {:.0}, {:.0}", at[0], at[1])
    }
}

/// Add a strategic point (with its mirror twin on fair maps).
pub fn add_point(blueprint: &mut MapBlueprint, role: StrategicRole, at: [f32; 2]) -> String {
    let at = [at[0].round(), at[1].round()];
    let axis_z = blueprint.grid.axis_z();
    let index = blueprint.gameplay.strategic_points.len() + 1;
    let mut push = |suffix: &str, at: [f32; 2]| {
        blueprint.gameplay.strategic_points.push(StrategicPointSpec {
            id: format!("point_{index}{suffix}"),
            name: format!("{role:?} (editor)"),
            role,
            at: [XCoord::Fixed(at[0]), XCoord::Fixed(at[1])],
            radius_m: 25.0,
        });
    };
    if blueprint.symmetry.is_some() && (at[1] - axis_z).abs() > 1.0 {
        push("_s", [at[0], at[1].min(axis_z * 2.0 - at[1])]);
        push("_n", [at[0], at[1].max(axis_z * 2.0 - at[1])]);
        format!("{role:?} point_{index} at {:.0}, {:.0} (with its twin)", at[0], at[1])
    } else {
        push("", at);
        format!("{role:?} point_{index} at {:.0}, {:.0}", at[0], at[1])
    }
}

/// Add a capture zone (with its mirror twin off-axis on fair maps).
pub fn add_zone(blueprint: &mut MapBlueprint, at: [f32; 2]) -> String {
    let at = [at[0].round(), at[1].round()];
    let axis_z = blueprint.grid.axis_z();
    let index = blueprint.gameplay.capture_zones.len() + 1;
    let mut push = |suffix: &str, at: [f32; 2]| {
        blueprint.gameplay.capture_zones.push(CaptureZoneSpec {
            id: format!("zone_{index}{suffix}"),
            at,
            radius_m: 20.0,
        });
    };
    if blueprint.symmetry.is_some() && (at[1] - axis_z).abs() > 1.0 {
        push("_s", [at[0], at[1].min(axis_z * 2.0 - at[1])]);
        push("_n", [at[0], at[1].max(axis_z * 2.0 - at[1])]);
        format!("zone_{index} at {:.0}, {:.0} (with its twin)", at[0], at[1])
    } else {
        push("", at);
        format!("zone_{index} at {:.0}, {:.0}", at[0], at[1])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use map_forge::blueprint::SymmetrySpec;

    #[test]
    fn gameplay_gestures_stay_fair_by_construction() {
        let mut document = crate::EditorDocument::new_scratch();
        document.apply_edit(|blueprint| {
            blueprint.symmetry = Some(SymmetrySpec::MirrorZ);
            move_spawn(blueprint, 1, [140.0, 60.0]);
            add_point(blueprint, StrategicRole::HullDown, [180.0, 100.0]);
            add_zone(blueprint, [150.0, 150.0]);
        });
        let compiled = document.recompile();
        // The mirrored spawn moved team 2 to the twin position.
        let spawns = &compiled.battlefield.spawn_zones;
        assert_eq!(spawns.len(), 2);
        assert_eq!(spawns[0].center[2], 60.0);
        assert_eq!(spawns[1].center[2], 240.0, "team 2 mirrors team 1");
        // The point landed with its twin; the on-axis zone stayed single.
        assert_eq!(compiled.battlefield.strategic_points.len(), 2);
        assert_eq!(compiled.battlefield.capture_zones.len(), 1);
        // And the whole document passes the fairness checks it just danced with.
        assert!(
            !compiled.report.errors().any(|entry| entry.check == "symmetry"),
            "gestures cannot break the mirror: {:?}",
            compiled.report.errors().map(|e| e.message.clone()).collect::<Vec<_>>()
        );
    }
}
