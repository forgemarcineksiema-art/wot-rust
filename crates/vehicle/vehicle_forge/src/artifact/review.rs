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
    CloseFront,
    RunningGear,
    TurretMantlet,
    TopPlan,
    BattleClose,
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
            ReviewCamera::CloseFront => "close_front.png",
            ReviewCamera::RunningGear => "running_gear.png",
            ReviewCamera::TurretMantlet => "turret_mantlet.png",
            ReviewCamera::TopPlan => "top_plan.png",
            ReviewCamera::BattleClose => "battle_close.png",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReviewCameraSet {
    cameras: Vec<ReviewCameraSpec>,
}

impl ReviewCameraSet {
    /// The generic review set every vehicle gets: the six all-round silhouette views. Families that
    /// need closer scrutiny extend this; see [`ReviewCameraSet::t54_benchmark_review`].
    pub fn standard_vehicle_review() -> Self {
        Self { cameras: base_cameras() }
    }

    /// The T-54 benchmark set: the generic views plus five close-up regression views (running gear,
    /// mantlet, top plan, battle close) that lock the forge's reference vehicle in extra detail.
    pub fn t54_benchmark_review() -> Self {
        let mut cameras = base_cameras();
        cameras.extend([
            camera(ReviewCamera::CloseFront, 0.0, -5.0, 0.82),
            camera(ReviewCamera::RunningGear, 90.0, -7.0, 0.88),
            camera(ReviewCamera::TurretMantlet, 0.0, -12.0, 0.70),
            camera(ReviewCamera::TopPlan, 0.0, -86.0, 1.10),
            camera(ReviewCamera::BattleClose, 35.0, -10.0, 0.90),
        ]);
        Self { cameras }
    }

    pub fn cameras(&self) -> &[ReviewCameraSpec] {
        &self.cameras
    }
}

/// The six all-round silhouette cameras shared by every review set.
fn base_cameras() -> Vec<ReviewCameraSpec> {
    vec![
        camera(ReviewCamera::Front, 0.0, -6.0, 1.45),
        camera(ReviewCamera::Rear, 180.0, -6.0, 1.45),
        camera(ReviewCamera::LeftProfile, -90.0, -4.0, 1.55),
        camera(ReviewCamera::RightProfile, 90.0, -4.0, 1.55),
        camera(ReviewCamera::Top, 0.0, -82.0, 1.70),
        camera(ReviewCamera::BattleOblique, 35.0, -12.0, 1.35),
    ]
}

fn camera(
    kind: ReviewCamera,
    yaw_deg: f32,
    pitch_deg: f32,
    distance_scale: f32,
) -> ReviewCameraSpec {
    ReviewCameraSpec { kind, yaw_deg, pitch_deg, distance_scale }
}
