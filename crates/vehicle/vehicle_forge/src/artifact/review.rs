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

/// What a close-up tile centres on. A review tile used to frame the whole vehicle and
/// `distance_scale` only magnified about ITS centre, so a "running gear" view at 0.88 was the
/// tank again, a little larger — no instrument for the belt the owner wants "mega dopracowane"
/// (2026-09-05, K22/K23). Resolved per vehicle at render time from its kinematics and mounts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReviewFocus {
    /// The drive sprocket's axle: the belt's engagement, the teeth in the links, the wrap.
    DriveEnd,
    /// The gun's trunnion: the mantlet, the barrel's root, the turret front around them.
    Trunnion,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReviewCameraSpec {
    kind: ReviewCamera,
    yaw_deg: f32,
    pitch_deg: f32,
    distance_scale: f32,
    /// Appended 2026-09-05 (the close-up instrument); `None` frames the whole vehicle.
    #[serde(default)]
    focus: Option<ReviewFocus>,
}

impl ReviewCameraSpec {
    pub fn kind(&self) -> ReviewCamera {
        self.kind
    }

    pub fn focus(&self) -> Option<ReviewFocus> {
        self.focus
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
    /// The generic review set every vehicle gets: the six all-round silhouette views and, since
    /// 2026-09-05, the two close-ups the owner judges a vehicle by — the running gear at the
    /// drive sprocket and the gun at its trunnion (K22, K23). Families that need more extend
    /// this; see [`ReviewCameraSet::t54_benchmark_review`].
    pub fn standard_vehicle_review() -> Self {
        let mut cameras = base_cameras();
        cameras.extend(close_up_cameras());
        Self { cameras }
    }

    /// The T-54 benchmark set: the generic views and close-ups plus three more regression views
    /// (close front, top plan, battle close) that lock the forge's reference vehicle in extra
    /// detail.
    pub fn t54_benchmark_review() -> Self {
        let mut cameras = base_cameras();
        cameras.push(camera(ReviewCamera::CloseFront, 0.0, -5.0, 0.82));
        cameras.extend(close_up_cameras());
        cameras.extend([
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
        // Profile yaws re-labelled with the 2026-08-12 chirality fix: +X is the vehicle's PORT
        // side (right-handed, +Y up, +Z forward), so the LEFT profile camera stands at +90°.
        // The old -90/+90 pair was named under the inverted belief and, together with the
        // mirrored basis, produced tiles whose gun pointed the right way while every
        // asymmetry sat on the wrong side.
        camera(ReviewCamera::LeftProfile, 90.0, -4.0, 1.55),
        camera(ReviewCamera::RightProfile, -90.0, -4.0, 1.55),
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
    ReviewCameraSpec { kind, yaw_deg, pitch_deg, distance_scale, focus: None }
}

/// The two close-ups every vehicle carries: the belt at its drive sprocket from the port side,
/// a third of the fit distance (three times the silhouette's magnification), and the mantlet
/// from ahead and slightly above at half the fit distance.
fn close_up_cameras() -> [ReviewCameraSpec; 2] {
    [
        ReviewCameraSpec {
            kind: ReviewCamera::RunningGear,
            yaw_deg: 90.0,
            pitch_deg: -7.0,
            distance_scale: 0.32,
            focus: Some(ReviewFocus::DriveEnd),
        },
        ReviewCameraSpec {
            kind: ReviewCamera::TurretMantlet,
            yaw_deg: 0.0,
            pitch_deg: -12.0,
            distance_scale: 0.45,
            focus: Some(ReviewFocus::Trunnion),
        },
    ]
}
