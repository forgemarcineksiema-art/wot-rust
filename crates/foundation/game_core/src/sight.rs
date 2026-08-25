//! The sight line's one tolerance, named once.
//!
//! The sim's terrain raycast forgives a graze: ground must rise THIS far above the sight
//! line before it blocks (`sim::spotting::terrain_clear`), so skylining a crest still
//! counts as seeing over it. Every instrument that reasons about masking — the hull-down
//! census in `map_forge`, any future viewshed — must price this in from the same constant,
//! or it certifies crests the live eye grazes straight over (the inverted-slack bug the
//! sight-rule-parity fix removed was exactly that drift, hand-copied).

/// How far ground must poke ABOVE a sight line before the terrain raycast calls it
/// blocked. In metres, on the deterministic spotting path.
pub const SIGHT_GRAZE_SLACK_M: f32 = 0.3;
