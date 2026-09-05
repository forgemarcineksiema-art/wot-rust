//! The HUD review set (interface program F8): every `HudState` in every `HudSizeClass`, each
//! one view with one golden. `every_hud_state_is_under_an_image_lock` refuses a state without
//! a view, so a state the HUD can show and the review set cannot is not a state that ships.

use super::states::{HudSizeClass, HudState};

/// One view of the HUD golden instrument.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HudReviewView {
    /// The golden's file name stem, e.g. `hud_reloading_large`.
    pub name: String,
    pub state: HudState,
    pub size: HudSizeClass,
}

/// The whole set: states × size classes, in `ALL` order.
pub fn hud_review_views() -> Vec<HudReviewView> {
    let mut views = Vec::with_capacity(HudState::ALL.len() * HudSizeClass::ALL.len());
    for state in HudState::ALL {
        for size in HudSizeClass::ALL {
            views.push(HudReviewView {
                name: format!("hud_{}{}", state.name(), size.suffix()),
                state,
                size,
            });
        }
    }
    views
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A state the HUD can show and the review set cannot is a state that ships unreviewed.
    #[test]
    fn every_hud_state_is_under_an_image_lock() {
        let views = hud_review_views();
        let mut names = std::collections::HashSet::new();
        for state in HudState::ALL {
            for size in HudSizeClass::ALL {
                let matching: Vec<_> =
                    views.iter().filter(|v| v.state == state && v.size == size).collect();
                assert_eq!(matching.len(), 1, "{state:?} {size:?}: one view, no strays");
                assert!(names.insert(matching[0].name.clone()), "{state:?} {size:?} shares a name");
            }
        }
        assert_eq!(views.len(), HudState::ALL.len() * HudSizeClass::ALL.len());
        assert!(
            views.iter().all(|v| v.name.starts_with("hud_")),
            "the hud_ prefix keeps the goldens apart"
        );
    }
}
