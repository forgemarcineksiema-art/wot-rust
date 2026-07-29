//! Geometry for the animatable running gear: the closed belt path sampling and the unit meshes
//! (one road wheel, one end wheel, one shoe link) the renderer instances. Split from
//! [`crate::running_gear`] to keep each module small; the kinematics and placement live there.

use glam::{Vec2, Vec3};

use crate::running_gear::RunningGearKinematics;
use crate::{Axis, ExtrudeSpec, GeometryMesh, MaterialRole, MeshBuilder, SmoothingGroup};

const SG_HARD: SmoothingGroup = SmoothingGroup::hard_edges();

/// One shoe link, centred at the origin: a short box spanning the belt width (X) whose cross
/// section lies in the Z/Y plane, so a rotation about X aligns it with the belt tangent.
/// The PATTERN is per family (audit #14): the same generator used to run on every vehicle
/// across three nations, so the Germans, the Centurion and the T-34 read as wearing the
/// T-54's track. Negative Y is the wheel side (guide horns); positive Y the ground face.
pub fn track_link_unit_mesh(kin: &RunningGearKinematics) -> GeometryMesh {
    if kin.detail == crate::GearDetail::Far {
        return distant_link(kin);
    }
    let shoe = match kin.shoe {
        game_core::ShoePattern::Omsh => omsh_link(kin),
        game_core::ShoePattern::Kgs => kgs_link(kin),
        game_core::ShoePattern::Waffle => waffle_link(kin),
        game_core::ShoePattern::BritishCast => british_link(kin),
    };
    // The continuous belt skin must move and break with the links, not live in the static hull.
    // This thin wheel-side backing overlaps its neighbours slightly, closing the triangular gaps
    // that rectangular shoe faces otherwise expose around an idler/sprocket wrap. Because it is
    // part of the instanced link, live sag, terrain travel, and a thrown track all affect the same
    // geometry instead of revealing an immovable ghost band underneath.
    let pitch = kin.belt_length() / kin.link_count().max(1) as f32;
    MeshBuilder::new()
        .append(&box_prism(
            Vec3::new(0.0, -0.038, 0.0),
            kin.band_half_width * 0.96,
            0.012,
            pitch * 0.56,
        ))
        .append(&shoe)
        .build()
}

/// At range a shoe IS its plate.
///
/// Every pattern above is a plate plus features that distinguish it — the OMSh guide pads and
/// edge rails, the Kgs centre horn and grousers, the waffle's ridges, the Centurion's twin horns.
/// All of them are under 50 mm on a shoe that is a fifth of a metre long, and there are 180 shoes
/// per tank: the belt alone is 23k of a T-54's 38.6k gear triangles, sixty per cent of the
/// largest body of geometry on the vehicle. It is the first thing a distance tier should spend.
///
/// The plate and the backing skin stay, because those are the belt's silhouette and the belt's
/// silhouette is what you still see at 60 m.
fn distant_link(kin: &RunningGearKinematics) -> GeometryMesh {
    let pitch = kin.belt_length() / kin.link_count().max(1) as f32;
    MeshBuilder::new()
        .append(&box_prism(
            Vec3::new(0.0, -0.038, 0.0),
            kin.band_half_width * 0.96,
            0.012,
            pitch * 0.56,
        ))
        .append(&box_prism(
            Vec3::new(0.0, -0.004, 0.0),
            kin.band_half_width,
            0.026,
            kin.link_half_length(),
        ))
        .build()
}

/// The Soviet small-pitch **OMSh** shoe (T-54 family, IS-3), built the way it is built.
///
/// What it had before: a plate, two 10 mm "guide pads" and two edge rails that were entirely
/// buried inside the backing skin, and two "pin bars" lying across the GROUND face. Four of its
/// seven detail boxes rendered nothing at all — 64 triangles per shoe, 11,520 per tank, drawing
/// the inside of another box — and the two that did show were on the wrong face and implied
/// pin heads the real track does not have.
///
/// What it is (dossier, "Part construction", session S1b):
///
/// - **One guide horn per shoe.** Every OMSh link is horned from September 1949; the alternating
///   horned/flat belt belongs to the pre-1947 track. The horn stands up on the WHEEL side and
///   rides in the gap between the road wheel's twin tyres — which is why
///   [`RunningGearKinematics::tyre_gap_half`] is the one number that sizes both parts.
/// - **Hinge eyes at each joint.** The projecting barrel of eyes around the floating pin — the
///   цевка — is what a sprocket tooth bears on. Not the horn: the tooth enters the window BESIDE
///   the horn and pushes on the eye barrel. From the side these read as the knuckle line running
///   the length of the belt.
/// - **No pin heads.** The pin is ⌀20-22 x 520 mm and floats; its ends sit about 30 mm inboard of
///   the belt edges, so nothing of it shows from outside. The eye bars stop short accordingly.
/// - **Ribbed ground face.** Stiffening ribs raised in September 1949 — not chevrons, not smooth.
fn omsh_link(kin: &RunningGearKinematics) -> GeometryMesh {
    let half_z = kin.link_half_length();
    // The shoe plate spans the full belt band, so its outer face sits AT the blueprint's
    // `outer_x` — the documented "width over tracks".
    let plate_half_x = kin.band_half_width;
    // The horn fits the slot between the twin tyres with clearance to either side. A wheel with
    // no gap (the German dish) gets no horn from this generator — it does not have one.
    let horn_half_x = kin.tyre_gap_half() * 0.72;
    // How deep the slot between the tyres actually is: from the tread the belt rides on down to
    // the steel seat the tyres are pressed onto. The horn is sized from THAT, with a little
    // clearance, so it is swallowed by the slot instead of bottoming out on the wheel — by
    // construction, not by a constant someone tuned until the test went quiet.
    let slot_depth = (kin.wheel_radius - kin.tyre_seat_radius() - 0.006).max(0.008);
    let plate_face_y = -0.030_f32;
    let horn_half_y = (slot_depth + 0.020) * 0.5;
    let horn_y = plate_face_y - horn_half_y;
    // The eye barrel: at the joint, standing proud of the backing on the wheel side, stopping
    // short of the belt edges because the pin does.
    let eye_inset = 0.030_f32.min(plate_half_x * 0.12);
    let eye_half_x = (plate_half_x - eye_inset).max(plate_half_x * 0.5);

    let mut builder = MeshBuilder::new()
        .append(&box_prism(Vec3::new(0.0, -0.004, 0.0), plate_half_x, 0.026, half_z))
        // The two hinge-eye barrels — the belt's knuckle line, and the surface the drive
        // sprocket actually bears on.
        .append(&box_prism(
            Vec3::new(0.0, -0.044, -half_z * 0.90),
            eye_half_x,
            0.017,
            half_z * 0.14,
        ))
        .append(&box_prism(Vec3::new(0.0, -0.044, half_z * 0.90), eye_half_x, 0.017, half_z * 0.14))
        // Stiffening ribs across the ground face.
        .append(&box_prism(
            Vec3::new(0.0, 0.026, -half_z * 0.44),
            plate_half_x * 0.92,
            0.006,
            half_z * 0.11,
        ))
        .append(&box_prism(
            Vec3::new(0.0, 0.026, half_z * 0.44),
            plate_half_x * 0.92,
            0.006,
            half_z * 0.11,
        ));
    if horn_half_x > 0.0 {
        builder = builder.append(&box_prism(
            Vec3::new(0.0, horn_y, 0.0),
            horn_half_x,
            horn_half_y,
            half_z * 0.46,
        ));
    }
    builder.build()
}

/// German Kgs 63/725 double-pin shoe (Tiger I/II, Jagdtiger, Panther II): a wide plate with
/// one TALL centre guide horn that rides between the interleaved wheel rows, two transverse
/// grouser cleats on the ground face, and prominent pin tubes at both joints.
fn kgs_link(kin: &RunningGearKinematics) -> GeometryMesh {
    let half_z = kin.link_half_length();
    let plate_half_x = kin.band_half_width;
    let pin_half_z = (half_z * 0.09).max(0.012);
    MeshBuilder::new()
        .append(&box_prism(Vec3::new(0.0, -0.004, 0.0), plate_half_x, 0.026, half_z))
        // The single tall centre horn — the Schachtellaufwerk's guide between the wheel rows.
        .append(&box_prism(Vec3::new(0.0, -0.052, 0.0), 0.032, 0.024, half_z * 0.30))
        // Two transverse grouser cleats gripping the ground.
        .append(&box_prism(
            Vec3::new(0.0, 0.022, -half_z * 0.38),
            plate_half_x * 0.96,
            0.008,
            half_z * 0.10,
        ))
        .append(&box_prism(
            Vec3::new(0.0, 0.022, half_z * 0.38),
            plate_half_x * 0.96,
            0.008,
            half_z * 0.10,
        ))
        // Pin tubes at the joints.
        .append(&box_prism(
            Vec3::new(0.0, 0.014, -half_z * 0.82),
            plate_half_x * 0.90,
            0.012,
            pin_half_z,
        ))
        .append(&box_prism(
            Vec3::new(0.0, 0.014, half_z * 0.82),
            plate_half_x * 0.90,
            0.012,
            pin_half_z,
        ))
        .build()
}

/// The T-34's stamped "waffle" plate: three low transverse ridges on the ground face under a
/// low broad centre horn — pressed steel, no separate pin tubes standing proud.
fn waffle_link(kin: &RunningGearKinematics) -> GeometryMesh {
    let half_z = kin.link_half_length();
    let plate_half_x = kin.band_half_width;
    let mut b = MeshBuilder::new()
        .append(&box_prism(Vec3::new(0.0, -0.004, 0.0), plate_half_x, 0.026, half_z))
        // Low broad centre horn.
        .append(&box_prism(Vec3::new(0.0, -0.044, 0.0), 0.045, 0.016, half_z * 0.22));
    // The waffle: three low ridges across the ground face.
    for i in -1..=1 {
        b = b.append(&box_prism(
            Vec3::new(0.0, 0.020, half_z * 0.52 * i as f32),
            plate_half_x * 0.92,
            0.006,
            half_z * 0.11,
        ));
    }
    b.build()
}

/// The Centurion's cast manganese shoe: TWIN spaced guide horns riding either side of the
/// centreline and one heavy transverse bar across the ground face.
fn british_link(kin: &RunningGearKinematics) -> GeometryMesh {
    let half_z = kin.link_half_length();
    let plate_half_x = kin.band_half_width;
    let pin_half_z = (half_z * 0.08).max(0.010);
    MeshBuilder::new()
        .append(&box_prism(Vec3::new(0.0, -0.004, 0.0), plate_half_x, 0.026, half_z))
        // Twin spaced horns.
        .append(&box_prism(
            Vec3::new(-plate_half_x * 0.28, -0.048, 0.0),
            0.026,
            0.020,
            half_z * 0.24,
        ))
        .append(&box_prism(
            Vec3::new(plate_half_x * 0.28, -0.048, 0.0),
            0.026,
            0.020,
            half_z * 0.24,
        ))
        // One heavy transverse bar.
        .append(&box_prism(Vec3::new(0.0, 0.024, 0.0), plate_half_x * 0.94, 0.010, half_z * 0.16))
        .append(&box_prism(
            Vec3::new(0.0, 0.012, -half_z * 0.80),
            plate_half_x * 0.86,
            0.010,
            pin_half_z,
        ))
        .append(&box_prism(
            Vec3::new(0.0, 0.012, half_z * 0.80),
            plate_half_x * 0.86,
            0.010,
            pin_half_z,
        ))
        .build()
}

fn box_prism(center: Vec3, half_x: f32, half_y: f32, half_z: f32) -> GeometryMesh {
    MeshBuilder::new()
        .extrude(
            center,
            ExtrudeSpec {
                section: vec![
                    Vec2::new(-half_z, -half_y),
                    Vec2::new(half_z, -half_y),
                    Vec2::new(half_z, half_y),
                    Vec2::new(-half_z, half_y),
                ],
                axis: Axis::X,
                half_depth: half_x,
                material: MaterialRole::TrackMetal,
                smoothing: SG_HARD,
            },
        )
        .build()
}
