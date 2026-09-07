//! The building KIT (the one program's B3): a grammar-driven, instanced kit of parts.
//!
//! Until B3 every building was unique merged triangles in the 1 000 m static buffer — a
//! mullion was fresh geometry each time, and seven styles × four wall tones × three roof tones
//! of detached rectangles with the ridge along the long axis made the Ostrogorsk square six
//! near-identical white blocks. Here a building is a PLAN: a list of placements of a small
//! catalogue of parts (wall bays, pierced bays, door and shop bays, plinth, eave, roof slope,
//! gable end, ridge cap, chimney), each part one mesh in canonical metres, drawn through the
//! renderer's instanced path. The split grammar decides the plan from the collision box and
//! the building's seed, and the plan carries a SIGNATURE — massing, storeys, roof pitch and
//! orientation, bay rhythm, ground-floor kind, age — which is what the variety locks read.
//!
//! Honesty (the doctrine): the blocking volume is the wall-and-roof mass inside the box.
//! The eave and the plinth are scenery-class: they reach at most [`SCENERY_REACH_M`] past
//! the box face, never block, and cannot hide a hull. SDF/CSG per building is refused (CPU
//! at map swap, hostile to the golden-hash gate).
//!
//! Part frame: `+X` is the facade's OUTWARD normal, the outer face lies on `x = 0` and the
//! part reaches inward; `+Z` runs along the facade; `+Y` is up from the part's floor line;
//! the origin is the bay's centre along the facade. Roof slopes use `+X` as "from the ridge
//! toward the eave"; gable ends use the facade frame. Every part that may be scaled says so
//! in its doc — a scale on a pierced bay would widen its windows, which rule 5 forbids ("a
//! longer wall earns MORE windows, never wider ones"), so pierced bays are placed at unit
//! scale and only plain bays, plinths, eaves, ridges and roof slopes stretch.

use glam::{Mat4, Quat, Vec3};
use vehicle_geometry::GeometryMesh;

use crate::WorldMaterial;
use crate::building::{BuildingStyle, push_box, push_face};
use crate::shape::Rng;

/// How far a scenery-class part (eave, plinth) may reach past the collision box face.
pub const SCENERY_REACH_M: f32 = 0.40;
/// A wall leaf's thickness, from the outer face inward.
pub const WALL_THICKNESS_M: f32 = 0.30;
/// The eave's reach past the facade and its fascia depth.
const EAVE_REACH_M: f32 = 0.32;
const EAVE_DEPTH_M: f32 = 0.16;
/// The plinth stands this much proud of the wall face.
const PLINTH_PROUD_M: f32 = 0.05;
/// The storey height may flex this much either way to land the ridge on the box top.
const STOREY_FLEX: f32 = 0.15;
/// A knee wall (a blind band under the eaves — the attic's wall) may take up this much of the
/// box height the storeys and the roof leave over.
const MAX_KNEE_M: f32 = 1.4;
/// The least filler a facade keeps at each corner before its first pierced bay.
const CORNER_FILLER_M: f32 = 0.30;
/// A dormer's front: width and height.
const DORMER_WIDTH_M: f32 = 1.0;
const DORMER_HEIGHT_M: f32 = 1.1;
/// The footing course under the plinth (B2): its height and reach past the plinth face.
const FOOTING_HEIGHT_M: f32 = 0.12;

/// The two dwelling families the kit dresses (the landmarks — church, windmill, factory —
/// stay on the authored bake).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum KitFamily {
    /// Cottages and barns: rendered walls on a low fieldstone plinth, timber lintels, small
    /// windows, plank doors, a barn's portal.
    Village,
    /// Townhouses and tenements: plaster over a tall stone plinth, stone lintel bands and
    /// sills, tall windows, a shop front on the ground floor.
    Town,
}

impl KitFamily {
    /// The canonical storey height (before the ±8 % flex).
    pub fn storey_m(self) -> f32 {
        match self {
            KitFamily::Village => 2.6,
            // 2.9: a Galician town house of seven metres holds two storeys under a shallow
            // roof; the civic 3.2 left every Bystra house one storey and an attic.
            KitFamily::Town => 2.9,
        }
    }

    pub fn plinth_m(self) -> f32 {
        match self {
            KitFamily::Village => 0.35,
            KitFamily::Town => 0.60,
        }
    }

    /// The lintel bands, sills and jambs: a timber bressumer in the village, a stone band in
    /// the town.
    fn trim(self) -> WorldMaterial {
        match self {
            KitFamily::Village => WorldMaterial::Timber,
            KitFamily::Town => WorldMaterial::PlinthStone,
        }
    }

    /// (width, height, sill) of a dwelling window, metres.
    fn window(self) -> (f32, f32, f32) {
        match self {
            KitFamily::Village => (0.90, 1.10, 0.95),
            KitFamily::Town => (1.10, 1.70, 0.90),
        }
    }

    /// (width, height) of the door.
    fn door(self) -> (f32, f32) {
        match self {
            KitFamily::Village => (0.95, 2.05),
            KitFamily::Town => (1.20, 2.40),
        }
    }

    /// The storey height a STYLE builds at: a cottage and a small-town house at 2.6 m, a
    /// tenement at the civic 2.9 m. The family's canonical part height is the pierced bay's
    /// mesh; a style under it scales the bay down (a shorter storey has shorter windows).
    pub fn style_storey_m(style: BuildingStyle) -> f32 {
        match style {
            BuildingStyle::Tenement => 2.9,
            _ => 2.6,
        }
    }

    pub fn style_plinth_m(style: BuildingStyle) -> f32 {
        match style {
            BuildingStyle::Tenement => 0.60,
            BuildingStyle::Townhouse => 0.45,
            _ => 0.35,
        }
    }

    fn storey_range(self, style: BuildingStyle) -> std::ops::RangeInclusive<u8> {
        match (self, style) {
            (_, BuildingStyle::Barn) => 1..=1,
            (KitFamily::Village, _) => 1..=2,
            (KitFamily::Town, BuildingStyle::Townhouse) => 1..=3,
            (KitFamily::Town, _) => 2..=4,
        }
    }
}

/// The bay rhythm: three widths per family, one per building (the seed's pick).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BayWidth {
    Narrow,
    Medium,
    Wide,
}

impl BayWidth {
    pub const ALL: [BayWidth; 3] = [BayWidth::Narrow, BayWidth::Medium, BayWidth::Wide];

    pub fn metres(self, family: KitFamily) -> f32 {
        match (family, self) {
            (KitFamily::Village, BayWidth::Narrow) => 2.4,
            (KitFamily::Village, BayWidth::Medium) => 2.8,
            (KitFamily::Village, BayWidth::Wide) => 3.2,
            (KitFamily::Town, BayWidth::Narrow) => 2.8,
            (KitFamily::Town, BayWidth::Medium) => 3.2,
            (KitFamily::Town, BayWidth::Wide) => 3.6,
        }
    }
}

/// The roof pitch catalogue. A roof slope is a sloped quad scaled UNIFORMLY in its run and
/// rise (a non-uniform scale would skew its normal), so the pitch is one of these and the
/// storey flex lands the ridge on the box top.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Pitch {
    /// A shallow tin or tile roof over a low town house.
    P16,
    P22,
    P28,
    P36,
    P44,
    P52,
}

impl Pitch {
    pub const ALL: [Pitch; 6] =
        [Pitch::P16, Pitch::P22, Pitch::P28, Pitch::P36, Pitch::P44, Pitch::P52];

    pub fn degrees(self) -> f32 {
        match self {
            Pitch::P16 => 16.0,
            Pitch::P22 => 22.0,
            Pitch::P28 => 28.0,
            Pitch::P36 => 36.0,
            Pitch::P44 => 44.0,
            Pitch::P52 => 52.0,
        }
    }

    fn tan(self) -> f32 {
        self.degrees().to_radians().tan()
    }
}

/// What a dwelling's walls are clad in (B4): the surface role its wall vertices take.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Cladding {
    /// Rendered and limewashed.
    Plaster,
    /// Exposed running-bond brick.
    Brick,
}

impl Cladding {
    pub const ALL: [Cladding; 2] = [Cladding::Plaster, Cladding::Brick];
}

/// What the ground floor of the street facade carries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GroundKind {
    /// Windows and one plank door.
    Dwelling,
    /// Shop fronts and one door (the town only).
    Shops,
    /// A barn: blind walls and one portal.
    Portal,
}

/// The catalogue, append-only: a part's index is its mesh handle's offset.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum KitPart {
    /// A plain wall leaf: unit `z ∈ [−0.5, 0.5]`, `y ∈ [0, 1]`; SCALED to (1, storey, run).
    WallBay,
    /// A pierced bay with one window, at unit scale (storey flex only).
    WindowBay { family: KitFamily, width: BayWidth },
    /// A pierced bay with one door, at unit scale.
    DoorBay { family: KitFamily, width: BayWidth },
    /// A town ground-floor shop front, at unit scale.
    ShopBay { width: BayWidth },
    /// A barn's portal bay (3.2 m), at unit scale.
    PortalBay,
    /// The plinth course: unit along `z`, `y ∈ [0, 1]`; SCALED to (1, plinth, run).
    Plinth,
    /// The eave: unit along `z`, hanging from `y = 0`; SCALED to (1, 1, run).
    Eave,
    /// A roof slope at a pitch: unit run 1 from the ridge, rise `tan(pitch)`, unit along `z`;
    /// SCALED to (depth, depth, ridge run).
    RoofSlope { pitch: Pitch },
    /// A gable end at a pitch: base `z ∈ [−0.5, 0.5]`, apex `0.5·tan(pitch)`; SCALED to
    /// (1, span, span).
    GableEnd { pitch: Pitch },
    /// The ridge cap: unit along `z`; SCALED to (1, 1, ridge run).
    RidgeCap,
    /// A chimney stack: 0.6 × 0.6, unit height; SCALED to (1, height, 1).
    Chimney,
    /// B1: the corner of a hipped slope — a right triangle from the ridge end down to the
    /// eave corner, in the slope frame (unit run, rise `tan(pitch)`, unit along `z` toward
    /// `+z` for `right`, `−z` otherwise); SCALED uniformly to (depth, depth, depth).
    RoofSlopeCorner { pitch: Pitch, right: bool },
    /// B1: a hip end — the triangular slope over a gable facade, apex at the ridge end;
    /// SCALED uniformly to (depth, depth, depth).
    HipEnd { pitch: Pitch },
    /// B1: a dormer on a slope, at unit scale: a vertical front with a window at `x = 0`,
    /// its flat roof and cheeks running back to meet the slope at the pitch.
    Dormer { pitch: Pitch },
    /// B1: a downpipe at a corner, unit height; SCALED to (1, height, 1).
    Downpipe,
    /// B2: the footing course under the plinth, unit along `z`; SCALED to (1, 1, run).
    Footing,
    /// B5: a floor slab left standing in a ruin — a unit cube about its origin; SCALED to
    /// (half-span, thickness, half-span) in the box frame.
    Slab,
}

impl KitPart {
    /// Every part the catalogue holds, in handle order. Append-only.
    pub fn all() -> Vec<KitPart> {
        let mut parts = vec![KitPart::WallBay];
        for family in [KitFamily::Village, KitFamily::Town] {
            for width in BayWidth::ALL {
                parts.push(KitPart::WindowBay { family, width });
                parts.push(KitPart::DoorBay { family, width });
            }
        }
        for width in BayWidth::ALL {
            parts.push(KitPart::ShopBay { width });
        }
        parts.push(KitPart::PortalBay);
        parts.push(KitPart::Plinth);
        parts.push(KitPart::Eave);
        for pitch in Pitch::ALL {
            parts.push(KitPart::RoofSlope { pitch });
        }
        for pitch in Pitch::ALL {
            parts.push(KitPart::GableEnd { pitch });
        }
        parts.push(KitPart::RidgeCap);
        parts.push(KitPart::Chimney);
        for pitch in Pitch::ALL {
            parts.push(KitPart::RoofSlopeCorner { pitch, right: false });
            parts.push(KitPart::RoofSlopeCorner { pitch, right: true });
        }
        for pitch in Pitch::ALL {
            parts.push(KitPart::HipEnd { pitch });
        }
        for pitch in Pitch::ALL {
            parts.push(KitPart::Dormer { pitch });
        }
        parts.push(KitPart::Downpipe);
        parts.push(KitPart::Footing);
        parts.push(KitPart::Slab);
        parts
    }

    /// The part's position in [`KitPart::all`] — its handle offset.
    pub fn index(self) -> usize {
        KitPart::all().iter().position(|part| *part == self).expect("every part is catalogued")
    }

    /// Which instance tint lane the part rides.
    pub fn tint_lane(self) -> TintLane {
        match self {
            KitPart::WallBay
            | KitPart::WindowBay { .. }
            | KitPart::DoorBay { .. }
            | KitPart::ShopBay { .. }
            | KitPart::PortalBay
            | KitPart::GableEnd { .. } => TintLane::Wall,
            KitPart::RoofSlope { .. }
            | KitPart::RidgeCap
            | KitPart::RoofSlopeCorner { .. }
            | KitPart::HipEnd { .. } => TintLane::Roof,
            // The dormer's front is wall; its roof takes the roof tone through its own
            // instance, so the whole part rides the wall lane (the small roof reads as trim).
            KitPart::Dormer { .. } => TintLane::Wall,
            KitPart::Plinth
            | KitPart::Eave
            | KitPart::Chimney
            | KitPart::Downpipe
            | KitPart::Footing
            | KitPart::Slab => TintLane::Absolute,
        }
    }

    /// The part's mesh, in its own frame and canonical metres.
    pub fn mesh(self) -> GeometryMesh {
        let mut v = Vec::new();
        let mut i = Vec::new();
        let t = WALL_THICKNESS_M;
        match self {
            KitPart::WallBay => {
                push_box(
                    &mut v,
                    &mut i,
                    Vec3::new(-t * 0.5, 0.5, 0.0),
                    Vec3::new(t * 0.5, 0.5, 0.5),
                    WorldMaterial::Wall,
                );
            }
            KitPart::WindowBay { family, width } => {
                let w = width.metres(family);
                let h = family.storey_m();
                let (ww, wh, sill) = family.window();
                pierced_bay(
                    &mut v,
                    &mut i,
                    Bay { w, h, ow: ww, sill, head: sill + wh, opening: Opening::Window, family },
                );
            }
            KitPart::DoorBay { family, width } => {
                let w = width.metres(family);
                let h = family.storey_m();
                let (dw, dh) = family.door();
                pierced_bay(
                    &mut v,
                    &mut i,
                    Bay { w, h, ow: dw, sill: 0.0, head: dh, opening: Opening::Door, family },
                );
            }
            KitPart::ShopBay { width } => {
                let family = KitFamily::Town;
                let w = width.metres(family);
                let h = family.storey_m();
                pierced_bay(
                    &mut v,
                    &mut i,
                    Bay {
                        w,
                        h,
                        ow: (w - 0.9).min(2.4),
                        sill: 0.45,
                        head: 2.75,
                        opening: Opening::Window,
                        family,
                    },
                );
            }
            KitPart::PortalBay => {
                let family = KitFamily::Village;
                let w = 3.2;
                let h = family.storey_m();
                pierced_bay(
                    &mut v,
                    &mut i,
                    Bay { w, h, ow: 2.4, sill: 0.0, head: 2.45, opening: Opening::Door, family },
                );
            }
            KitPart::Plinth => {
                push_box(
                    &mut v,
                    &mut i,
                    Vec3::new((PLINTH_PROUD_M - 0.20) * 0.5, 0.5, 0.0),
                    Vec3::new((PLINTH_PROUD_M + 0.20) * 0.5, 0.5, 0.5),
                    WorldMaterial::PlinthStone,
                );
            }
            KitPart::Eave => {
                // The fascia and soffit: a timber box hanging under the eaves line, reaching
                // out past the wall face — scenery-class, never past SCENERY_REACH_M.
                push_box(
                    &mut v,
                    &mut i,
                    Vec3::new(EAVE_REACH_M * 0.5 - 0.02, -EAVE_DEPTH_M * 0.5, 0.0),
                    Vec3::new(EAVE_REACH_M * 0.5 + 0.02, EAVE_DEPTH_M * 0.5, 0.5),
                    WorldMaterial::Timber,
                );
            }
            KitPart::RoofSlope { pitch } => {
                let rise = pitch.tan();
                let corners = [
                    Vec3::new(0.0, 0.0, -0.5),
                    Vec3::new(0.0, 0.0, 0.5),
                    Vec3::new(1.0, -rise, 0.5),
                    Vec3::new(1.0, -rise, -0.5),
                ];
                let normal = Vec3::new(rise, 1.0, 0.0).normalize();
                push_face(&mut v, &mut i, corners, normal, WorldMaterial::Roof);
            }
            KitPart::GableEnd { pitch } => {
                let apex = 0.5 * pitch.tan();
                // A triangle as a degenerate quad (the apex twice): one outward face on the
                // facade plane.
                let corners = [
                    Vec3::new(0.0, 0.0, -0.5),
                    Vec3::new(0.0, 0.0, 0.5),
                    Vec3::new(0.0, apex, 0.0),
                    Vec3::new(0.0, apex, 0.0),
                ];
                push_face(&mut v, &mut i, corners, Vec3::X, WorldMaterial::Wall);
            }
            KitPart::RidgeCap => {
                push_box(
                    &mut v,
                    &mut i,
                    // Hung UNDER the ridge line: the ridge is the box's honest top, nothing rides over it.
                    Vec3::new(0.0, -0.06, 0.0),
                    Vec3::new(0.14, 0.06, 0.5),
                    WorldMaterial::Roof,
                );
            }
            KitPart::Chimney => {
                push_box(
                    &mut v,
                    &mut i,
                    Vec3::new(0.0, 0.5, 0.0),
                    Vec3::new(0.30, 0.5, 0.30),
                    WorldMaterial::PlinthStone,
                );
            }
            KitPart::RoofSlopeCorner { pitch, right } => {
                let rise = pitch.tan();
                let s = if right { 1.0 } else { -1.0 };
                // A triangle as a degenerate quad: the ridge end, the eave under it, the eave
                // corner (twice).
                let corners = [
                    Vec3::new(0.0, 0.0, 0.0),
                    Vec3::new(1.0, -rise, 0.0),
                    Vec3::new(1.0, -rise, s),
                    Vec3::new(1.0, -rise, s),
                ];
                let normal = Vec3::new(rise, 1.0, 0.0).normalize();
                push_face(&mut v, &mut i, corners, normal, WorldMaterial::Roof);
            }
            KitPart::HipEnd { pitch } => {
                let rise = pitch.tan();
                let corners = [
                    Vec3::new(1.0, -rise, -1.0),
                    Vec3::new(1.0, -rise, 1.0),
                    Vec3::new(0.0, 0.0, 0.0),
                    Vec3::new(0.0, 0.0, 0.0),
                ];
                let normal = Vec3::new(rise, 1.0, 0.0).normalize();
                push_face(&mut v, &mut i, corners, normal, WorldMaterial::Roof);
            }
            KitPart::Dormer { pitch } => {
                // The front stands on the slope at the origin and faces outward (+X); the body
                // runs back (−X) until its flat roof meets the slope.
                let back = DORMER_HEIGHT_M / pitch.tan();
                let hw = DORMER_WIDTH_M * 0.5;
                push_box(
                    &mut v,
                    &mut i,
                    Vec3::new(-0.08, DORMER_HEIGHT_M * 0.5, 0.0),
                    Vec3::new(0.08, DORMER_HEIGHT_M * 0.5, hw),
                    WorldMaterial::Wall,
                );
                // The window in the front: a pane recessed behind a small opening's frame.
                push_face(
                    &mut v,
                    &mut i,
                    [
                        Vec3::new(0.03, 0.25, -0.28),
                        Vec3::new(0.03, 0.25, 0.28),
                        Vec3::new(0.03, 0.85, 0.28),
                        Vec3::new(0.03, 0.85, -0.28),
                    ],
                    Vec3::X,
                    WorldMaterial::WindowGlass,
                );
                for corner in [(-0.31, 0.0), (0.31, 0.0)] {
                    push_box(
                        &mut v,
                        &mut i,
                        Vec3::new(0.02, 0.55, corner.0),
                        Vec3::new(0.03, 0.32, 0.03),
                        WorldMaterial::Timber,
                    );
                }
                // The cheeks: right triangles from the front back to the slope.
                for side in [-1.0, 1.0] {
                    let z = side * hw;
                    let corners = [
                        Vec3::new(0.0, 0.0, z),
                        Vec3::new(0.0, DORMER_HEIGHT_M, z),
                        Vec3::new(-back, DORMER_HEIGHT_M, z),
                        Vec3::new(-back, DORMER_HEIGHT_M, z),
                    ];
                    push_face(&mut v, &mut i, corners, Vec3::Z * side, WorldMaterial::Wall);
                }
                // The flat roof, a hair proud of the cheeks.
                push_face(
                    &mut v,
                    &mut i,
                    [
                        Vec3::new(-back, DORMER_HEIGHT_M + 0.02, -hw - 0.04),
                        Vec3::new(0.06, DORMER_HEIGHT_M + 0.02, -hw - 0.04),
                        Vec3::new(0.06, DORMER_HEIGHT_M + 0.02, hw + 0.04),
                        Vec3::new(-back, DORMER_HEIGHT_M + 0.02, hw + 0.04),
                    ],
                    Vec3::Y,
                    WorldMaterial::Roof,
                );
            }
            KitPart::Downpipe => {
                push_box(
                    &mut v,
                    &mut i,
                    Vec3::new(0.08, 0.5, 0.0),
                    Vec3::new(0.06, 0.5, 0.06),
                    WorldMaterial::Timber,
                );
            }
            KitPart::Footing => {
                push_box(
                    &mut v,
                    &mut i,
                    Vec3::new(0.01, 0.06, 0.0),
                    Vec3::new(0.11, 0.06, 0.5),
                    WorldMaterial::PlinthStone,
                );
            }
            KitPart::Slab => {
                push_box(&mut v, &mut i, Vec3::ZERO, Vec3::splat(0.5), WorldMaterial::PlinthStone);
            }
        }
        GeometryMesh::new(v, i)
    }
}

/// Which per-instance tint a placement takes: the building's wall tone (with its age), its
/// roof tone, or none (the part's own material colour).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TintLane {
    Wall,
    Roof,
    Absolute,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Opening {
    Window,
    Door,
}

/// One pierced bay's spec: a leaf of width `w` and height `h` with one opening of width
/// `ow` between `sill` and `head`.
#[derive(Debug, Clone, Copy)]
struct Bay {
    w: f32,
    h: f32,
    ow: f32,
    sill: f32,
    head: f32,
    opening: Opening,
    family: KitFamily,
}

/// One pierced bay, built AROUND its opening (an apron below, a lintel band above, two
/// piers), the pane or door recessed behind the face, jambs and a sill ledge proud of it.
fn pierced_bay(v: &mut Vec<vehicle_geometry::GeometryVertex>, i: &mut Vec<u32>, bay: Bay) {
    let Bay { w, h, ow, sill, head, opening, family } = bay;
    let t = WALL_THICKNESS_M;
    let ow = ow.min(w - 0.5);
    let head = head.min(h - 0.25);
    let pier = (w - ow) * 0.5;
    let leaf_x = Vec3::new(-t * 0.5, 0.0, 0.0);
    // Apron under the sill (none for a door).
    if sill > 0.01 {
        push_box(
            v,
            i,
            leaf_x + Vec3::new(0.0, sill * 0.5, 0.0),
            Vec3::new(t * 0.5, sill * 0.5, w * 0.5),
            WorldMaterial::Wall,
        );
    }
    // The lintel band over the head: the trim material, one band per storey.
    let band_h = (0.22_f32).min(h - head);
    push_box(
        v,
        i,
        leaf_x + Vec3::new(0.0, head + band_h * 0.5, 0.0),
        Vec3::new(t * 0.5, band_h * 0.5, w * 0.5),
        family.trim(),
    );
    if h - head - band_h > 0.01 {
        let top = h - head - band_h;
        push_box(
            v,
            i,
            leaf_x + Vec3::new(0.0, head + band_h + top * 0.5, 0.0),
            Vec3::new(t * 0.5, top * 0.5, w * 0.5),
            WorldMaterial::Wall,
        );
    }
    // The piers either side of the opening.
    for side in [-1.0, 1.0] {
        push_box(
            v,
            i,
            leaf_x + Vec3::new(0.0, (sill + head) * 0.5, side * (ow * 0.5 + pier * 0.5)),
            Vec3::new(t * 0.5, (head - sill) * 0.5, pier * 0.5),
            WorldMaterial::Wall,
        );
    }
    // The REVEALS (B4): the faces lining the opening — the jambs looking in, the head
    // looking down, the sill looking up — carry baked shade, so a window reads as a hole
    // with depth at forty metres instead of a painted rectangle.
    let reveal = |v: &mut Vec<vehicle_geometry::GeometryVertex>,
                  i: &mut Vec<u32>,
                  corners: [Vec3; 4],
                  normal: Vec3| {
        push_face_shaded(v, i, corners, normal, WorldMaterial::Wall, REVEAL_SHADE);
    };
    for side in [-1.0, 1.0] {
        let z = side * ow * 0.5;
        reveal(
            v,
            i,
            [
                Vec3::new(0.0, sill, z),
                Vec3::new(-t, sill, z),
                Vec3::new(-t, head, z),
                Vec3::new(0.0, head, z),
            ],
            Vec3::Z * -side,
        );
    }
    reveal(
        v,
        i,
        [
            Vec3::new(0.0, head, -ow * 0.5),
            Vec3::new(-t, head, -ow * 0.5),
            Vec3::new(-t, head, ow * 0.5),
            Vec3::new(0.0, head, ow * 0.5),
        ],
        -Vec3::Y,
    );
    if sill > 0.01 {
        reveal(
            v,
            i,
            [
                Vec3::new(0.0, sill, -ow * 0.5),
                Vec3::new(-t, sill, -ow * 0.5),
                Vec3::new(-t, sill, ow * 0.5),
                Vec3::new(0.0, sill, ow * 0.5),
            ],
            Vec3::Y,
        );
    }
    match opening {
        Opening::Window => {
            // The pane, recessed 0.09 m behind the face; the jambs and a mullion cross at
            // +0.012; the sill ledge proud of the face.
            let pane_x = -0.09;
            push_face(
                v,
                i,
                [
                    Vec3::new(pane_x, sill, -ow * 0.5),
                    Vec3::new(pane_x, sill, ow * 0.5),
                    Vec3::new(pane_x, head, ow * 0.5),
                    Vec3::new(pane_x, head, -ow * 0.5),
                ],
                Vec3::X,
                WorldMaterial::WindowGlass,
            );
            for side in [-1.0, 1.0] {
                push_box(
                    v,
                    i,
                    Vec3::new(-0.04, (sill + head) * 0.5, side * (ow * 0.5 - 0.035)),
                    Vec3::new(0.052, (head - sill) * 0.5, 0.035),
                    family.trim(),
                );
            }
            push_box(
                v,
                i,
                Vec3::new(-0.04, (sill + head) * 0.5, 0.0),
                Vec3::new(0.052, (head - sill) * 0.5, 0.025),
                family.trim(),
            );
            push_box(
                v,
                i,
                Vec3::new(-0.04, (sill + head) * 0.5, 0.0),
                Vec3::new(0.052, 0.025, ow * 0.5),
                family.trim(),
            );
            push_box(
                v,
                i,
                Vec3::new(0.0, sill - 0.03, 0.0),
                Vec3::new(0.06, 0.03, ow * 0.5 + 0.06),
                family.trim(),
            );
        }
        Opening::Door => {
            let door_x = -0.07;
            push_face(
                v,
                i,
                [
                    Vec3::new(door_x, 0.0, -ow * 0.5),
                    Vec3::new(door_x, 0.0, ow * 0.5),
                    Vec3::new(door_x, head, ow * 0.5),
                    Vec3::new(door_x, head, -ow * 0.5),
                ],
                Vec3::X,
                WorldMaterial::PlankDoor,
            );
            for side in [-1.0, 1.0] {
                push_box(
                    v,
                    i,
                    Vec3::new(-0.03, head * 0.5, side * (ow * 0.5 + 0.04)),
                    Vec3::new(0.045, head * 0.5, 0.04),
                    family.trim(),
                );
            }
        }
    }
}

/// The roof's massing (B1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RoofForm {
    /// Two slopes between two gable walls.
    Gable,
    /// Four slopes: the gable ends are hipped, the ridge shortened by the depth at each end.
    Hip,
    /// A near-square box: four hips to one apex.
    Pyramid,
}

/// The baked shade of a reveal face (B4) — the ambient a window's jamb sees.
pub const REVEAL_SHADE: f32 = 0.62;

/// A single-sided face with a baked shade on its vertices (the kit's reveals).
fn push_face_shaded(
    v: &mut Vec<vehicle_geometry::GeometryVertex>,
    i: &mut Vec<u32>,
    corners: [Vec3; 4],
    normal: Vec3,
    material: WorldMaterial,
    shade: f32,
) {
    let start = v.len();
    push_face(v, i, corners, normal, material);
    for vertex in &mut v[start..] {
        vertex.surface_shade = shade;
    }
}

/// The building's signature — the axes the variety locks read.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Signature {
    pub family: KitFamily,
    pub storeys: u8,
    pub pitch: Pitch,
    pub roof: RoofForm,
    /// Dormers per main slope.
    pub dormers: u8,
    pub cladding: Cladding,
    /// The ridge runs across the box's long axis (a near-square box's coin).
    pub ridge_across: bool,
    pub width: BayWidth,
    pub ground: GroundKind,
    /// 0 fresh, 1 weathered, 2 old — a tint on the walls today, the wear term of B4 later.
    pub age: u8,
}

impl Signature {
    /// The wall tint's multiplier for the age band.
    pub fn age_tint(self) -> f32 {
        match self.age {
            0 => 1.0,
            1 => 0.93,
            _ => 0.86,
        }
    }
}

/// One part placed in the building's box-local frame (the box centre at the origin).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Placement {
    pub part: KitPart,
    pub transform: Mat4,
    pub tint: TintLane,
}

/// A building as the kit draws it.
#[derive(Debug, Clone, PartialEq)]
pub struct BuildingPlan {
    pub signature: Signature,
    pub placements: Vec<Placement>,
    /// The ridge's height over the box floor.
    pub ridge_m: f32,
}

impl BuildingPlan {
    pub fn family(&self) -> KitFamily {
        self.signature.family
    }
}

/// Whether the kit dresses this style (the landmarks stay on the authored bake).
pub fn kit_family(style: BuildingStyle) -> Option<KitFamily> {
    match style {
        BuildingStyle::Cottage | BuildingStyle::Barn => Some(KitFamily::Village),
        BuildingStyle::Townhouse | BuildingStyle::Tenement => Some(KitFamily::Town),
        BuildingStyle::Church | BuildingStyle::Windmill | BuildingStyle::FactoryHall => None,
    }
}

/// The frame of one facade of the box: its outward normal, the along direction, the plane
/// distance and the run.
#[derive(Debug, Clone, Copy)]
struct Facade {
    outward: Vec3,
    along: Vec3,
    /// Rotation mapping the part frame (+X outward, +Z along) onto this facade.
    rotation: Quat,
    /// Half of the facade's run.
    half_run: f32,
    /// The plane's distance from the box centre.
    plane: f32,
}

fn facades(half: Vec3) -> [Facade; 4] {
    [
        Facade {
            outward: Vec3::X,
            along: Vec3::Z,
            rotation: Quat::IDENTITY,
            half_run: half.z,
            plane: half.x,
        },
        Facade {
            outward: -Vec3::X,
            along: -Vec3::Z,
            rotation: Quat::from_rotation_y(std::f32::consts::PI),
            half_run: half.z,
            plane: half.x,
        },
        Facade {
            outward: Vec3::Z,
            along: -Vec3::X,
            rotation: Quat::from_rotation_y(-std::f32::consts::FRAC_PI_2),
            half_run: half.x,
            plane: half.z,
        },
        Facade {
            outward: -Vec3::Z,
            along: Vec3::X,
            rotation: Quat::from_rotation_y(std::f32::consts::FRAC_PI_2),
            half_run: half.x,
            plane: half.z,
        },
    ]
}

impl Facade {
    /// A placement on this facade at `along` metres from its centre and `y` over the box
    /// floor, scaled by `scale` in the part frame.
    fn place(&self, half: Vec3, part: KitPart, along: f32, y: f32, scale: Vec3) -> Placement {
        let origin = self.outward * self.plane + self.along * along + Vec3::Y * (y - half.y);
        Placement {
            part,
            transform: Mat4::from_scale_rotation_translation(scale, self.rotation, origin),
            tint: part.tint_lane(),
        }
    }
}

/// The fit the grammar found for the box: how many storeys at what pitch and flex.
#[derive(Debug, Clone, Copy)]
struct Fit {
    storeys: u8,
    pitch: Pitch,
    storey_m: f32,
    /// The knee wall under the eaves.
    knee_m: f32,
}

/// Plan a building of `style` for a collision box of `half` extents (the box centre at the
/// origin, the floor at `−half.y`), seeded by the building's id. `None` when the style is a
/// landmark or the box cannot carry a kit plan (the authored bake stands in then).
pub fn plan_building(style: BuildingStyle, seed: u64, half: Vec3) -> Option<BuildingPlan> {
    let family = kit_family(style)?;
    let mut rng = Rng(seed ^ 0x6b69_745f_6233_2e31);
    let height = half.y * 2.0;
    // The ridge runs along the long axis; a near-square box may turn it across (the coin).
    let long_x = half.x >= half.z;
    let elongation = half.x.max(half.z) / half.x.min(half.z).max(0.1);
    let ridge_across = elongation < 1.25 && rng.unit() < 0.4;
    let ridge_along_x = long_x != ridge_across;
    // The gable half-span (the depth the roof runs down from the ridge).
    let depth = if ridge_along_x { half.z } else { half.x };
    let ridge_run = if ridge_along_x { half.x * 2.0 } else { half.z * 2.0 };
    let plinth = KitFamily::style_plinth_m(style);
    let storey0 = KitFamily::style_storey_m(style);
    // Every (storeys, pitch) whose ridge lands within the slack of the box top, with the
    // storey flexed to close the gap; the seed picks among them — that is where a street's
    // heights and pitches come from.
    let mut fits = Vec::new();
    for storeys in family.storey_range(style) {
        for pitch in Pitch::ALL {
            let rise = depth * pitch.tan();
            let room = height - plinth - rise;
            if room <= 0.0 {
                continue;
            }
            // The storeys at their canonical height; what is left over is the knee wall —
            // or, when the storeys do not fit, they flex down to the box.
            let knee = room - storey0 * f32::from(storeys);
            if (0.0..=MAX_KNEE_M).contains(&knee) {
                fits.push(Fit { storeys, pitch, storey_m: storey0, knee_m: knee });
                continue;
            }
            let storey_m = room / f32::from(storeys);
            let flex = storey_m / storey0;
            if knee < 0.0 && flex >= 1.0 - STOREY_FLEX {
                fits.push(Fit { storeys, pitch, storey_m, knee_m: 0.0 });
            }
        }
    }
    if fits.is_empty() {
        return None;
    }
    let fit = fits[(rng.next() % fits.len() as u64) as usize];
    let width = BayWidth::ALL[(rng.next() % 3) as usize];
    let bay_m = width.metres(family);
    let ground = match style {
        BuildingStyle::Barn => GroundKind::Portal,
        BuildingStyle::Townhouse | BuildingStyle::Tenement if rng.unit() < 0.45 => {
            GroundKind::Shops
        }
        _ => GroundKind::Dwelling,
    };
    let age = (rng.next() % 3) as u8;
    // The roof's massing (B1): a barn keeps its gable; a near-square box may take a pyramid;
    // otherwise the seed hips four in ten roofs when the ridge has room to shorten.
    let square = (half.x - half.z).abs() <= 0.05 * depth;
    let roof = if style == BuildingStyle::Barn {
        RoofForm::Gable
    } else if square && rng.unit() < 0.5 {
        RoofForm::Pyramid
    } else if ridge_run - 2.0 * depth >= 1.0 && rng.unit() < 0.4 {
        RoofForm::Hip
    } else {
        RoofForm::Gable
    };
    // Dormers (B1): an attic under a pitch steep enough to stand in, with room on the slope.
    let dormer_back = DORMER_HEIGHT_M / fit.pitch.tan();
    let dormers = if fit.pitch.degrees() >= 36.0
        && depth - 0.3 >= dormer_back + 0.4
        && roof != RoofForm::Pyramid
        && style != BuildingStyle::Barn
    {
        (rng.next() % 3) as u8
    } else {
        0
    };
    // The cladding (B4): a third of the town in brick, a sixth of the village.
    let cladding = match family {
        KitFamily::Town if rng.unit() < 0.35 => Cladding::Brick,
        KitFamily::Village if rng.unit() < 0.16 => Cladding::Brick,
        _ => Cladding::Plaster,
    };
    let signature = Signature {
        family,
        storeys: fit.storeys,
        pitch: fit.pitch,
        roof,
        dormers,
        cladding,
        ridge_across,
        width,
        ground,
        age,
    };

    let eaves_m = plinth + fit.storey_m * f32::from(fit.storeys) + fit.knee_m;
    let rise = depth * fit.pitch.tan();
    let ridge_m = eaves_m + rise;
    let mut placements = Vec::new();
    let faces = facades(half);
    // The street facade: the first eaves facade (the one whose normal is perpendicular to
    // the ridge); the door and the shops live there.
    let street = if ridge_along_x { 2 } else { 0 };
    let door_bay_seed = rng.next();
    let door_side_seed = rng.next();
    for (index, facade) in faces.iter().enumerate() {
        let run = facade.half_run * 2.0;
        let is_gable = if ridge_along_x { index < 2 } else { index >= 2 };
        // The footing course (B2) and the plinth along every facade.
        placements.push(facade.place(
            half,
            KitPart::Footing,
            0.0,
            0.0,
            Vec3::new(1.0, FOOTING_HEIGHT_M / 0.12, run + 0.24),
        ));
        placements.push(facade.place(half, KitPart::Plinth, 0.0, 0.0, Vec3::new(1.0, plinth, run)));
        // Bays per storey.
        let pierced = if style == BuildingStyle::Barn {
            0
        } else {
            ((run - 2.0 * CORNER_FILLER_M) / bay_m).floor().max(0.0) as usize
        };
        let filler = (run - pierced as f32 * bay_m) * 0.5;
        for storey in 0..fit.storeys {
            let y = plinth + fit.storey_m * f32::from(storey);
            let flex = fit.storey_m / family.storey_m();
            if pierced == 0 {
                placements.push(facade.place(
                    half,
                    KitPart::WallBay,
                    0.0,
                    y,
                    Vec3::new(1.0, fit.storey_m, run),
                ));
                continue;
            }
            for end in [-1.0, 1.0] {
                placements.push(facade.place(
                    half,
                    KitPart::WallBay,
                    end * (facade.half_run - filler * 0.5),
                    y,
                    Vec3::new(1.0, fit.storey_m, filler),
                ));
            }
            let door_index = (door_bay_seed % pierced as u64) as usize;
            for bay in 0..pierced {
                let along = -facade.half_run + filler + bay_m * (bay as f32 + 0.5);
                let ground_floor = storey == 0;
                let part = if ground_floor && index == street && bay == door_index {
                    KitPart::DoorBay { family, width }
                } else if ground_floor && index == street && ground == GroundKind::Shops {
                    KitPart::ShopBay { width }
                } else {
                    KitPart::WindowBay { family, width }
                };
                placements.push(facade.place(half, part, along, y, Vec3::new(1.0, flex, 1.0)));
            }
        }
        if fit.knee_m > 0.01 {
            // The knee wall: a blind band under the eaves on every facade.
            placements.push(facade.place(
                half,
                KitPart::WallBay,
                0.0,
                plinth + fit.storey_m * f32::from(fit.storeys),
                Vec3::new(1.0, fit.knee_m, run),
            ));
        }
        if style == BuildingStyle::Barn && index == street {
            // The portal replaces the middle of the ground leaf: a pierced bay over the
            // plain one (the plain leaf behind it is the barn's own wall).
            let side = if door_side_seed.is_multiple_of(2) { -1.0 } else { 1.0 };
            let along = side * (facade.half_run - 1.6 - CORNER_FILLER_M).max(0.0);
            placements.push(facade.place(
                half,
                KitPart::PortalBay,
                along,
                plinth,
                Vec3::new(1.0, fit.storey_m / family.storey_m(), 1.0),
            ));
        }
        if is_gable && roof == RoofForm::Gable {
            let span = facade.half_run * 2.0;
            placements.push(facade.place(
                half,
                KitPart::GableEnd { pitch: fit.pitch },
                0.0,
                eaves_m,
                Vec3::new(1.0, span, span),
            ));
        } else {
            // An eave on every facade the roof runs down to: the eaves facades of a gable,
            // all four of a hip or a pyramid.
            placements.push(facade.place(
                half,
                KitPart::Eave,
                0.0,
                eaves_m,
                Vec3::new(1.0, 1.0, run),
            ));
        }
        // Downpipes (B1) at the corners of the eaves facades of a dwelling.
        if !is_gable && style != BuildingStyle::Barn {
            for end in [-1.0, 1.0] {
                placements.push(facade.place(
                    half,
                    KitPart::Downpipe,
                    end * (facade.half_run - 0.3),
                    plinth,
                    Vec3::new(1.0, (eaves_m - plinth).max(0.5), 1.0),
                ));
            }
        }
    }
    // The roof (B1): the form decides the slopes. A gable runs two rectangular slopes the
    // full ridge; a hip shortens the ridge by the depth at each end, runs the rectangles over
    // the shortened ridge with a corner triangle at each end, and closes the gable facades
    // with hip ends; a pyramid is four hip ends to one apex.
    let ridge_dir = if ridge_along_x { Vec3::X } else { Vec3::Z };
    let ridge_y = ridge_m - half.y;
    let ridge_eff = match roof {
        RoofForm::Gable => ridge_run,
        RoofForm::Hip => ridge_run - 2.0 * depth,
        RoofForm::Pyramid => 0.0,
    };
    let dormer_seed = rng.next();
    for (index, facade) in faces.iter().enumerate() {
        let is_eaves = if ridge_along_x { index >= 2 } else { index < 2 };
        if is_eaves && ridge_eff > 0.01 {
            placements.push(Placement {
                part: KitPart::RoofSlope { pitch: fit.pitch },
                transform: Mat4::from_scale_rotation_translation(
                    Vec3::new(depth, depth, ridge_eff),
                    facade.rotation,
                    Vec3::Y * ridge_y,
                ),
                tint: TintLane::Roof,
            });
            // Dormers along this slope, spaced over the ridge's run.
            for d in 0..dormers {
                let t = (f64::from(d) + 0.5) / f64::from(dormers) - 0.5;
                let along = (ridge_eff - 1.4) * t as f32
                    + ((dormer_seed >> (index * 8 + d as usize * 2)) % 3) as f32 * 0.15
                    - 0.15;
                let run_out = dormer_back + 0.2;
                let origin = Vec3::Y * (ridge_y - run_out * fit.pitch.tan())
                    + facade.outward * run_out
                    + facade.along * along;
                placements.push(Placement {
                    part: KitPart::Dormer { pitch: fit.pitch },
                    transform: Mat4::from_scale_rotation_translation(
                        Vec3::ONE,
                        facade.rotation,
                        origin,
                    ),
                    tint: TintLane::Wall,
                });
            }
        }
        if is_eaves && roof == RoofForm::Hip {
            for end in [-1.0, 1.0] {
                let end_point = ridge_dir * (end * ridge_eff * 0.5);
                let right = facade.along.dot(ridge_dir) * end > 0.0;
                placements.push(Placement {
                    part: KitPart::RoofSlopeCorner { pitch: fit.pitch, right },
                    transform: Mat4::from_scale_rotation_translation(
                        Vec3::splat(depth),
                        facade.rotation,
                        Vec3::Y * ridge_y + end_point,
                    ),
                    tint: TintLane::Roof,
                });
            }
        }
        let hipped_here = match roof {
            RoofForm::Gable => false,
            RoofForm::Hip => !is_eaves,
            RoofForm::Pyramid => true,
        };
        if hipped_here {
            let toward = facade.outward.dot(ridge_dir);
            let end_point = ridge_dir * (toward.signum() * ridge_eff * 0.5);
            placements.push(Placement {
                part: KitPart::HipEnd { pitch: fit.pitch },
                transform: Mat4::from_scale_rotation_translation(
                    Vec3::splat(facade.half_run.min(depth)),
                    facade.rotation,
                    Vec3::Y * ridge_y + end_point,
                ),
                tint: TintLane::Roof,
            });
        }
    }
    if ridge_eff > 0.01 {
        let ridge_rotation = if ridge_along_x {
            Quat::from_rotation_y(std::f32::consts::FRAC_PI_2)
        } else {
            Quat::IDENTITY
        };
        placements.push(Placement {
            part: KitPart::RidgeCap,
            transform: Mat4::from_scale_rotation_translation(
                Vec3::new(1.0, 1.0, ridge_eff),
                ridge_rotation,
                Vec3::Y * ridge_y,
            ),
            tint: TintLane::Roof,
        });
    }
    // The chimney: a stack straddling one slope just off the ridge, its foot 0.9 m under the
    // ridge line (inside the roof) and its head at most 0.6 m over it — never over the box
    // top, which on a knee-walled fit IS the ridge, so the head sits at the ridge then and
    // the stack still shows above the slope it stands in.
    let chimney_top = (ridge_m + 0.6).min(height);
    let chimney_base = ridge_m - 0.9;
    if chimney_top - chimney_base >= 0.7 && ridge_eff > 4.0 {
        let side = if rng.unit() < 0.5 { -1.0 } else { 1.0 };
        let along = side * (ridge_eff * 0.5 - 1.2) * rng.unit().max(0.3);
        let off_ridge = if rng.unit() < 0.5 { -0.45 } else { 0.45 };
        let across = if ridge_along_x { Vec3::Z } else { Vec3::X };
        placements.push(Placement {
            part: KitPart::Chimney,
            transform: Mat4::from_scale_rotation_translation(
                Vec3::new(1.0, chimney_top - chimney_base, 1.0),
                Quat::IDENTITY,
                ridge_dir * along + across * off_ridge + Vec3::Y * (chimney_base - half.y),
            ),
            tint: TintLane::Absolute,
        });
    }
    Some(BuildingPlan { signature, placements, ridge_m })
}

/// The ruin of a dwelling (B5): what stands after the collapse, under the sim's rubble
/// height. Two ADJACENT facades keep standing wall — their run cut into segments whose
/// broken tops vary, the two segments meeting at the kept corner the tallest — the other
/// two keep stubs, every facade its footing and plinth, and a floor slab hangs in the corner
/// at half the ceiling. Everything stays under `ceiling_m` (the mound's top: what a shell
/// stops against) and inside the box. The heap inside is the static bake's mound.
pub fn plan_ruin(
    style: BuildingStyle,
    seed: u64,
    half: Vec3,
    ceiling_m: f32,
) -> Option<Vec<Placement>> {
    kit_family(style)?;
    let plinth = KitFamily::style_plinth_m(style).min(ceiling_m * 0.5);
    let mut rng = Rng(seed ^ 0x7275_696e_5f62_3521);
    let faces = facades(half);
    // The kept corner: the two facades that share it.
    let corner = (rng.next() % 4) as usize;
    let kept: [usize; 2] = match corner {
        0 => [0, 2],
        1 => [0, 3],
        2 => [1, 2],
        _ => [1, 3],
    };
    let mut placements = Vec::new();
    for (index, facade) in faces.iter().enumerate() {
        let run = facade.half_run * 2.0;
        placements.push(facade.place(
            half,
            KitPart::Footing,
            0.0,
            0.0,
            Vec3::new(1.0, FOOTING_HEIGHT_M / 0.12, run + 0.24),
        ));
        placements.push(facade.place(half, KitPart::Plinth, 0.0, 0.0, Vec3::new(1.0, plinth, run)));
        let standing = kept.contains(&index);
        // The corner end of a kept facade: the end that touches the other kept facade.
        let corner_sign = if standing {
            let other = if kept[0] == index { kept[1] } else { kept[0] };
            faces[other].outward.dot(facade.along).signum()
        } else {
            0.0
        };
        let segments = 3 + (rng.next() % 3) as usize;
        let mut cursor = -facade.half_run;
        for segment in 0..segments {
            let remaining = facade.half_run - cursor;
            let len = if segment + 1 == segments {
                remaining
            } else {
                (remaining / (segments - segment) as f32) * (0.6 + rng.unit() * 0.8)
            }
            .min(remaining)
            .max(0.2);
            let mid = cursor + len * 0.5;
            let room = (ceiling_m - plinth).max(0.1);
            let share = if standing {
                let at_corner = (mid * corner_sign) > facade.half_run - len;
                if at_corner { 0.85 + rng.unit() * 0.15 } else { 0.35 + rng.unit() * 0.65 }
            } else {
                0.08 + rng.unit() * 0.27
            };
            let height = (room * share).max(0.15);
            placements.push(facade.place(
                half,
                KitPart::WallBay,
                mid,
                plinth,
                Vec3::new(1.0, height, len),
            ));
            cursor += len;
            if cursor >= facade.half_run - 1e-3 {
                break;
            }
        }
    }
    // The floor slab in the kept corner, at half the ceiling.
    let a = faces[kept[0]].outward;
    let b = faces[kept[1]].outward;
    let span = Vec3::new(half.x * 0.55, 0.0, half.z * 0.55);
    let centre = a * (half.x.abs() * a.x.abs() + half.z * a.z.abs()) * 0.5
        + b * (half.x * b.x.abs() + half.z * b.z.abs()) * 0.5;
    let slab_y = ceiling_m * 0.5 - half.y;
    placements.push(Placement {
        part: KitPart::Slab,
        transform: Mat4::from_scale_rotation_translation(
            Vec3::new(span.x * 2.0 - 0.3, 0.16, span.z * 2.0 - 0.3),
            Quat::IDENTITY,
            Vec3::new(centre.x * 0.85, slab_y, centre.z * 0.85),
        ),
        tint: TintLane::Absolute,
    });
    Some(placements)
}

/// The farthest any vertex of a placement list reaches past the box, per axis.
pub fn placements_reach_past_box(placements: &[Placement], half: Vec3) -> Vec3 {
    let mut reach = Vec3::ZERO;
    for placement in placements {
        let mesh = placement.part.mesh();
        for vertex in mesh.vertices() {
            let p = placement.transform.transform_point3(vertex.position);
            reach = reach.max((p.abs() - half).max(Vec3::ZERO));
        }
    }
    reach
}

/// The farthest any vertex of the plan reaches past the box, per axis (0 when inside) — the
/// honesty number: at most [`SCENERY_REACH_M`] horizontally (the eave and the plinth), never
/// above the top or under the floor.
pub fn plan_reach_past_box(plan: &BuildingPlan, half: Vec3) -> Vec3 {
    let mut reach = Vec3::ZERO;
    for placement in &plan.placements {
        let mesh = placement.part.mesh();
        for vertex in mesh.vertices() {
            let p = placement.transform.transform_point3(vertex.position);
            reach = reach.max((p.abs() - half).max(Vec3::ZERO));
        }
    }
    reach
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The ridge may sit this far under the box top and the plan still stands (the roof is
    /// the box's honest top otherwise: a shell that stops in air over a visible roof is a lie).
    const MAX_RIDGE_SLACK_M: f32 = 0.35;

    fn boxes() -> Vec<(BuildingStyle, Vec3)> {
        vec![
            (BuildingStyle::Cottage, Vec3::new(3.4, 2.3, 4.6)),
            (BuildingStyle::Cottage, Vec3::new(5.5, 3.0, 4.8)),
            (BuildingStyle::Barn, Vec3::new(5.0, 3.5, 4.0)),
            (BuildingStyle::Barn, Vec3::new(8.0, 2.8, 4.4)),
            (BuildingStyle::Townhouse, Vec3::new(6.5, 3.6, 4.0)),
            (BuildingStyle::Townhouse, Vec3::new(7.0, 5.7, 5.5)),
            (BuildingStyle::Tenement, Vec3::new(8.0, 5.5, 5.5)),
            (BuildingStyle::Tenement, Vec3::new(6.5, 5.5, 5.0)),
            (BuildingStyle::Tenement, Vec3::new(4.6, 6.0, 6.0)),
        ]
    }

    /// Every part is a real mesh, in its frame: nothing behind the outer face's reach, nothing
    /// under its floor, and the catalogue's indices are stable and dense.
    #[test]
    fn every_part_is_catalogued_once_and_stays_in_its_frame() {
        let all = KitPart::all();
        for (index, part) in all.iter().enumerate() {
            assert_eq!(part.index(), index, "{part:?}");
            let mesh = part.mesh();
            assert!(
                !mesh.vertices().is_empty() && mesh.indices().len().is_multiple_of(3),
                "{part:?}"
            );
            for vertex in mesh.vertices() {
                let p = vertex.position;
                match part {
                    KitPart::RoofSlope { .. }
                    | KitPart::RoofSlopeCorner { .. }
                    | KitPart::HipEnd { .. } => {
                        assert!((0.0..=1.0).contains(&p.x), "{part:?} {p}")
                    }
                    KitPart::Dormer { .. } => assert!(p.x <= 0.07 && p.x >= -8.0, "{part:?} {p}"),
                    KitPart::Downpipe => assert!(p.x <= 0.15 && p.x >= 0.0, "{part:?} {p}"),
                    KitPart::Footing => assert!(p.x <= 0.13, "{part:?} {p}"),
                    KitPart::Slab => assert!(p.abs().max_element() <= 0.5 + 1e-4, "{part:?} {p}"),
                    KitPart::Eave => {
                        assert!(p.x <= SCENERY_REACH_M + 1e-4 && p.x >= -0.05, "{part:?} {p}")
                    }
                    KitPart::Plinth => assert!(p.x <= PLINTH_PROUD_M + 1e-4, "{part:?} {p}"),
                    KitPart::Chimney | KitPart::RidgeCap => {}
                    _ => assert!(p.x <= 0.07 && p.x >= -WALL_THICKNESS_M - 1e-4, "{part:?} {p}"),
                }
            }
        }
    }

    /// The doctrine, on the plan: every placed vertex stays inside the collision box, except
    /// the scenery-class eave and plinth, which reach at most `SCENERY_REACH_M` past a face
    /// and never above the top; the ridge lands within the slack of the top.
    #[test]
    fn every_plan_stays_inside_its_box_and_the_ridge_meets_the_top() {
        for (style, half) in boxes() {
            for seed in 0..6 {
                let plan = plan_building(style, seed, half)
                    .unwrap_or_else(|| panic!("{style:?} {half} seed {seed}"));
                let reach = plan_reach_past_box(&plan, half);
                assert!(
                    reach.x <= SCENERY_REACH_M + 1e-3 && reach.z <= SCENERY_REACH_M + 1e-3,
                    "{style:?} {half}: reach {reach}"
                );
                assert!(reach.y <= 1e-3, "{style:?} {half}: over the top by {}", reach.y);
                let slack = half.y * 2.0 - plan.ridge_m;
                assert!(
                    (-1e-3..=MAX_RIDGE_SLACK_M + 1e-3).contains(&slack),
                    "{style:?} {half}: ridge slack {slack}"
                );
            }
        }
    }

    /// The grammar is deterministic in its seed, and different seeds give a box different
    /// signatures — the variety is real, not a colour.
    #[test]
    fn seeds_vary_the_signature_and_the_plan_is_deterministic() {
        let half = Vec3::new(8.0, 5.5, 5.5);
        let a = plan_building(BuildingStyle::Tenement, 7, half).expect("plan");
        let b = plan_building(BuildingStyle::Tenement, 7, half).expect("plan");
        assert_eq!(a, b, "deterministic");
        let signatures: std::collections::HashSet<Signature> = (0..40)
            .map(|seed| plan_building(BuildingStyle::Tenement, seed, half).expect("plan").signature)
            .collect();
        assert!(
            signatures.len() >= 8,
            "forty seeds on one box give {} signatures",
            signatures.len()
        );
        let storeys: std::collections::HashSet<u8> = signatures.iter().map(|s| s.storeys).collect();
        let pitches: std::collections::HashSet<Pitch> =
            signatures.iter().map(|s| s.pitch).collect();
        assert!(
            storeys.len() >= 2 && pitches.len() >= 2,
            "storeys {storeys:?} pitches {pitches:?}"
        );
    }

    /// A longer wall earns MORE pierced bays, never wider ones: the bay parts are placed at
    /// unit scale along the facade, and doubling the run at least doubles the count.
    #[test]
    fn a_longer_wall_earns_more_bays_never_wider_ones() {
        let count = |half: Vec3| {
            let plan = plan_building(BuildingStyle::Townhouse, 3, half).expect("plan");
            let mut pierced = 0;
            for placement in &plan.placements {
                if matches!(
                    placement.part,
                    KitPart::WindowBay { .. } | KitPart::DoorBay { .. } | KitPart::ShopBay { .. }
                ) {
                    let (scale, _, _) = placement.transform.to_scale_rotation_translation();
                    assert!(
                        (scale.z - 1.0).abs() < 1e-4,
                        "a pierced bay is never stretched along the wall: {scale}"
                    );
                    pierced += 1;
                }
            }
            pierced
        };
        // The gable facades are the same on both boxes; the two long facades double.
        assert!(count(Vec3::new(12.0, 3.6, 4.0)) >= count(Vec3::new(6.0, 3.6, 4.0)) + 4);
    }

    /// B1: the silhouette is not flush-cut. Over sixty seeds on one tenement box the roofs
    /// take more than one form, dormers appear, every dwelling wears eaves and downpipes,
    /// and a chimney stands on most; B2: every dwelling stands on a footing course.
    #[test]
    fn roofs_take_more_than_one_form_and_every_dwelling_wears_eaves_and_a_footing() {
        let half = Vec3::new(8.0, 5.5, 5.5);
        let mut forms = std::collections::HashSet::new();
        let (mut dormers, mut chimneys) = (0, 0);
        for seed in 0..60 {
            let plan = plan_building(BuildingStyle::Tenement, seed, half).expect("plan");
            forms.insert(plan.signature.roof);
            let has = |part: fn(&KitPart) -> bool| plan.placements.iter().any(|p| part(&p.part));
            assert!(has(|p| matches!(p, KitPart::Eave)), "eaves on every dwelling");
            assert!(has(|p| matches!(p, KitPart::Footing)), "a footing under every dwelling");
            assert!(has(|p| matches!(p, KitPart::Downpipe)), "downpipes on every dwelling");
            dormers += usize::from(has(|p| matches!(p, KitPart::Dormer { .. })));
            chimneys += usize::from(has(|p| matches!(p, KitPart::Chimney)));
            if plan.signature.roof == RoofForm::Hip {
                assert!(has(|p| matches!(p, KitPart::HipEnd { .. })), "a hip closes its ends");
                assert!(!has(|p| matches!(p, KitPart::GableEnd { .. })), "a hip has no gable wall");
            }
        }
        assert!(forms.len() >= 2, "roof forms {forms:?}");
        assert!(dormers >= 10 && chimneys >= 30, "dormers {dormers} chimneys {chimneys}");
    }

    /// B4: a window is a hole with depth — every pierced bay carries shaded reveal faces —
    /// and the claddings both appear over a street of seeds.
    #[test]
    fn windows_carry_shaded_reveals_and_both_claddings_appear() {
        for part in KitPart::all() {
            if matches!(
                part,
                KitPart::WindowBay { .. } | KitPart::DoorBay { .. } | KitPart::ShopBay { .. }
            ) {
                let mesh = part.mesh();
                let shaded = mesh.vertices().iter().filter(|v| v.surface_shade < 0.7).count();
                assert!(shaded >= 12, "{part:?}: {shaded} reveal vertices");
            }
        }
        let half = Vec3::new(8.0, 5.5, 5.5);
        let claddings: std::collections::HashSet<Cladding> = (0..40)
            .map(|seed| {
                plan_building(BuildingStyle::Tenement, seed, half).expect("plan").signature.cladding
            })
            .collect();
        assert_eq!(claddings.len(), 2, "{claddings:?}");
    }

    /// B5: a ruin has FORM — two adjacent facades still stand (their tallest segment at
    /// least four fifths of the ceiling), the other two are stubs, a floor slab hangs in the
    /// kept corner, and nothing rises over the ceiling or leaves the box.
    #[test]
    fn a_ruin_keeps_two_wall_planes_and_a_floor_under_its_ceiling() {
        for (style, half) in boxes() {
            let ceiling = half.y * 2.0 * 0.4;
            for seed in 0..5 {
                let ruin = plan_ruin(style, seed, half, ceiling).expect("a kit ruin");
                assert_eq!(
                    ruin,
                    plan_ruin(style, seed, half, ceiling).expect("again"),
                    "deterministic"
                );
                let reach = placements_reach_past_box(&ruin, half);
                assert!(
                    reach.x <= SCENERY_REACH_M + 1e-3 && reach.z <= SCENERY_REACH_M + 1e-3,
                    "{style:?}: {reach}"
                );
                let top = ruin
                    .iter()
                    .flat_map(|p| {
                        let mesh = p.part.mesh();
                        mesh.vertices()
                            .iter()
                            .map(|v| p.transform.transform_point3(v.position).y + half.y)
                            .collect::<Vec<_>>()
                    })
                    .fold(f32::MIN, f32::max);
                assert!(
                    top <= ceiling + 1e-3,
                    "{style:?}: the ruin rises to {top} over its ceiling {ceiling}"
                );
                let mut tall_facades = std::collections::HashSet::new();
                for p in &ruin {
                    if p.part == KitPart::WallBay {
                        let (scale, rotation, _) = p.transform.to_scale_rotation_translation();
                        if scale.y
                            >= (ceiling - KitFamily::style_plinth_m(style).min(ceiling * 0.5)) * 0.8
                        {
                            tall_facades
                                .insert((rotation.to_axis_angle().1 * 100.0).round() as i32);
                        }
                    }
                }
                assert!(
                    tall_facades.len() >= 2,
                    "{style:?} seed {seed}: standing facades {tall_facades:?}"
                );
                assert!(ruin.iter().any(|p| p.part == KitPart::Slab), "{style:?}: a floor slab");
            }
        }
    }

    /// The landmarks stay on the authored bake.
    #[test]
    fn landmarks_are_not_kit_buildings() {
        for style in [BuildingStyle::Church, BuildingStyle::Windmill, BuildingStyle::FactoryHall] {
            assert!(plan_building(style, 1, Vec3::new(7.0, 13.5, 9.0)).is_none(), "{style:?}");
        }
    }
}
