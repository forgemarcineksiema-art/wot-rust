use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReviewCamera {
    Front,
    Rear,
    LeftProfile,
    RightProfile,
    Top,
    BattleOblique,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReviewCameraSpec {
    kind: ReviewCamera,
    yaw_deg: f32,
    pitch_deg: f32,
    distance_scale: f32,
}

impl ReviewCameraSpec {
    pub fn kind(&self) -> ReviewCamera {
        self.kind
    }

    pub fn yaw_deg(&self) -> f32 {
        self.yaw_deg
    }

    pub fn pitch_deg(&self) -> f32 {
        self.pitch_deg
    }

    pub fn distance_scale(&self) -> f32 {
        self.distance_scale
    }

    pub fn file_name(&self) -> &'static str {
        match self.kind {
            ReviewCamera::Front => "front.png",
            ReviewCamera::Rear => "rear.png",
            ReviewCamera::LeftProfile => "left_profile.png",
            ReviewCamera::RightProfile => "right_profile.png",
            ReviewCamera::Top => "top.png",
            ReviewCamera::BattleOblique => "battle_oblique.png",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReviewCameraSet {
    cameras: Vec<ReviewCameraSpec>,
}

impl ReviewCameraSet {
    pub fn standard_vehicle_review() -> Self {
        Self {
            cameras: vec![
                camera(ReviewCamera::Front, 0.0, -6.0, 1.45),
                camera(ReviewCamera::Rear, 180.0, -6.0, 1.45),
                camera(ReviewCamera::LeftProfile, -90.0, -4.0, 1.55),
                camera(ReviewCamera::RightProfile, 90.0, -4.0, 1.55),
                camera(ReviewCamera::Top, 0.0, -82.0, 1.70),
                camera(ReviewCamera::BattleOblique, 35.0, -12.0, 1.35),
            ],
        }
    }

    pub fn cameras(&self) -> &[ReviewCameraSpec] {
        &self.cameras
    }
}

fn camera(
    kind: ReviewCamera,
    yaw_deg: f32,
    pitch_deg: f32,
    distance_scale: f32,
) -> ReviewCameraSpec {
    ReviewCameraSpec { kind, yaw_deg, pitch_deg, distance_scale }
}
