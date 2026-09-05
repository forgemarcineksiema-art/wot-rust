//! The top bar's and the team lists' models (interface program H1, H2), built each frame from
//! what the wire told this crew: the roster (W-1), the team pools (W-2), the kills (W-3) and the
//! wrecks the snapshot shows. Nothing here is inferred — an enemy the filter withholds is a row
//! with a name and no bar, and a kill between two hulls this crew never saw still counts.

use crate::app::ClientApp;
use crate::hud::team_list::TeamListsModel;
use crate::hud::top_bar::TopBarModel;

impl ClientApp {
    /// Both models, or neither: before the roster lands (a remote session's first ticks) the
    /// HUD shows no lists and no counters rather than a field of one.
    pub(super) fn battle_intel_models(&self) -> (Option<TopBarModel>, Option<TeamListsModel>) {
        let roster = self.session.roster();
        if roster.is_empty() {
            return (None, None);
        }
        let Some(snapshot) = self.render_state.latest_snapshot() else {
            return (None, None);
        };
        let player_team = self.player_team();
        let top_bar = TopBarModel::from_battle(
            &roster,
            &snapshot.tanks,
            snapshot.team_hit_points,
            self.intel.kills(),
            player_team,
        );
        let team_lists = TeamListsModel::from_battle(
            &roster,
            &snapshot.tanks,
            self.intel.kills(),
            self.player_tank,
            player_team,
        );
        (Some(top_bar), Some(team_lists))
    }
}
