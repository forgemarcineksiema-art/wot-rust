//! The material sheet (interface program F2): the steel, the enamel and the glass the interface
//! is made of, as one 512 x 512 RGBA texture of sixteen 128 px tiles, generated here and
//! nowhere else.
//!
//! Every tile is a MODULATION map centred at one half: the HUD shader multiplies a plate's own
//! colour by twice the tile, so a neutral tile changes nothing and a brushed tile leaves the
//! plate its colour with the brushing on it. Generated procedurally from integer hashes and
//! linear blends — no `sin`, no `exp`, nothing from a platform's libm — so the bytes are the same
//! on every machine, which is what lets a golden hash lock them: a sheet that changed by one
//! texel is a deliberate diff, never drift.
//!
//! The bottom-right quarter (tiles 10, 11, 14, 15) is reserved and left at zero: the H wave
//! bakes the minimap relief into it as one quad, which is what makes the HUD's vertex budget
//! sufficient (F9).

use std::sync::OnceLock;

use renderer_api::hud_style;

/// The sheet's side, in texels. Four tiles of `TILE_PX` a side.
pub const SHEET_SIZE: u32 = TILES_PER_SIDE * TILE_PX;
/// Tiles per side — the same four the shader divides the sheet by.
pub const TILES_PER_SIDE: u32 = hud_style::SHEET_TILES_PER_SIDE;
/// One tile's side, in texels — and the number of local units a plate's tile repeats over.
pub const TILE_PX: u32 = hud_style::TILE_UNITS as u32;

/// The tiles, by their index in the sheet (row-major, four per row). Append-only: a plate names
/// its tile by this number in its style lane.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SheetTile {
    /// Anisotropic line noise along x: the brushing on a steel plate.
    BrushedSteel = 0,
    /// Low-frequency blotches and a speckle: paint over steel, long in the field.
    PaintedSteel = 1,
    /// Almost uniform, a faint speckle: enamel.
    EnamelBlack = 2,
    /// A stencil's wear, in the red channel: where the paint is gone.
    WornMask = 3,
    /// A soft diagonal reflection, in the alpha channel: what glass gives back.
    GlassBand = 4,
    /// One shaded rivet head in the tile's centre.
    Rivet = 5,
    /// Nothing at all: the neutral tile a plain plate wears.
    Neutral = 6,
}

impl SheetTile {
    /// Every authored tile, in index order.
    pub const ALL: [SheetTile; 7] = [
        SheetTile::BrushedSteel,
        SheetTile::PaintedSteel,
        SheetTile::EnamelBlack,
        SheetTile::WornMask,
        SheetTile::GlassBand,
        SheetTile::Rivet,
        SheetTile::Neutral,
    ];

    /// The tile's index in the sheet — what a plate's style lane carries.
    pub const fn index(self) -> u32 {
        self as u32
    }
}

/// The reserved quarter, in texels: `(x, y, width, height)` of the region the minimap bake owns.
pub const MINIMAP_REGION: (u32, u32, u32, u32) =
    (TILE_PX * 2, TILE_PX * 2, TILE_PX * 2, TILE_PX * 2);

/// The sheet's texels, RGBA8, row-major.
pub struct MaterialSheet {
    rgba: Vec<u8>,
}

impl MaterialSheet {
    pub fn width(&self) -> u32 {
        SHEET_SIZE
    }

    pub fn height(&self) -> u32 {
        SHEET_SIZE
    }

    pub fn rgba(&self) -> &[u8] {
        &self.rgba
    }

    /// One texel, for the tests and the bakes.
    pub fn texel(&self, x: u32, y: u32) -> [u8; 4] {
        let at = ((y * SHEET_SIZE + x) * 4) as usize;
        [self.rgba[at], self.rgba[at + 1], self.rgba[at + 2], self.rgba[at + 3]]
    }

    /// FNV-1a over the texels: the sheet's identity, pinned by `the_material_sheet_is_deterministic`.
    pub fn hash(&self) -> u64 {
        let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
        for byte in &self.rgba {
            hash ^= u64::from(*byte);
            hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
        }
        hash
    }
}

/// The sheet, generated on first use and kept for the process.
pub fn sheet() -> &'static MaterialSheet {
    static SHEET: OnceLock<MaterialSheet> = OnceLock::new();
    SHEET.get_or_init(generate)
}

/// `(width, height, rgba)` in the shape the renderer's upload takes.
pub fn hud_material_sheet() -> (u32, u32, &'static [u8]) {
    let sheet = sheet();
    (sheet.width(), sheet.height(), sheet.rgba())
}

fn generate() -> MaterialSheet {
    let size = SHEET_SIZE as usize;
    let mut rgba = vec![0u8; size * size * 4];
    for tile in SheetTile::ALL {
        let index = tile.index();
        let origin_x = (index % TILES_PER_SIDE) * TILE_PX;
        let origin_y = (index / TILES_PER_SIDE) * TILE_PX;
        for y in 0..TILE_PX {
            for x in 0..TILE_PX {
                let texel = paint(tile, x, y);
                let at = (((origin_y + y) as usize) * size + (origin_x + x) as usize) * 4;
                rgba[at..at + 4].copy_from_slice(&texel);
            }
        }
    }
    // Tiles 7, 8, 9, 12, 13 are unauthored and neutral; the reserved quarter stays at zero.
    for index in [7u32, 8, 9, 12, 13] {
        let origin_x = (index % TILES_PER_SIDE) * TILE_PX;
        let origin_y = (index / TILES_PER_SIDE) * TILE_PX;
        for y in 0..TILE_PX {
            for x in 0..TILE_PX {
                let at = (((origin_y + y) as usize) * size + (origin_x + x) as usize) * 4;
                rgba[at..at + 4].copy_from_slice(&[128, 128, 128, 255]);
            }
        }
    }
    MaterialSheet { rgba }
}

/// One texel of one tile. `x`, `y` in `0..TILE_PX`; every tile wraps at its own edge.
fn paint(tile: SheetTile, x: u32, y: u32) -> [u8; 4] {
    let neutral = 0.5;
    match tile {
        SheetTile::BrushedSteel => {
            // Lines along x: the noise is stretched sixteen to one, and a fine speckle breaks it.
            let lines = periodic_noise(x, y, 32, 2, 0xB2) - 0.5;
            let grain = hash(x, y, 0xB3) - 0.5;
            let v = neutral + lines * 0.14 + grain * 0.04;
            grey(v, 255)
        }
        SheetTile::PaintedSteel => {
            // Two octaves of blotch and a speckle: paint that has seen weather.
            let blotch = periodic_noise(x, y, 32, 32, 0x51) - 0.5;
            let detail = periodic_noise(x, y, 8, 8, 0x52) - 0.5;
            let speckle = hash(x, y, 0x53) - 0.5;
            let v = neutral + blotch * 0.10 + detail * 0.05 + speckle * 0.03;
            // Paint is warm: the red channel a hair above the blue.
            [to_byte(v + 0.01), to_byte(v), to_byte(v - 0.01), 255]
        }
        SheetTile::EnamelBlack => {
            // Enamel is nearly flat, but a plate that reads as a flat fill is a flat fill: a
            // speckle wide enough to be a texture at all, and a faint blotch under it.
            let speckle = hash(x, y, 0xE1) - 0.5;
            let blotch = periodic_noise(x, y, 32, 32, 0xE2) - 0.5;
            grey(neutral + speckle * 0.05 + blotch * 0.04, 255)
        }
        SheetTile::WornMask => {
            // Where the stencil's paint is gone, in red; a soft threshold on a blotch field.
            let field = periodic_noise(x, y, 16, 16, 0x77);
            let worn = smooth_step(0.58, 0.70, field);
            [to_byte(worn), 128, 128, 255]
        }
        SheetTile::GlassBand => {
            // A diagonal band with soft edges, in alpha; the colour lanes neutral.
            let diagonal = ((x + y) % TILE_PX) as f32 / TILE_PX as f32;
            let band =
                smooth_step(0.30, 0.48, diagonal) * (1.0 - smooth_step(0.52, 0.70, diagonal));
            [128, 128, 128, to_byte(band)]
        }
        SheetTile::Rivet => {
            // A domed head at the centre, lit from the top-left, on a neutral field.
            let half = TILE_PX as f32 * 0.5;
            let (dx, dy) = (x as f32 + 0.5 - half, y as f32 + 0.5 - half);
            let radius = half * 0.36;
            let r2 = dx * dx + dy * dy;
            if r2 >= radius * radius {
                grey(neutral, 255)
            } else {
                let nz = (1.0 - r2 / (radius * radius)).max(0.0);
                let (nx, ny) = (dx / radius, dy / radius);
                let lit = (-nx - ny) * 0.5 + nz * 0.35;
                grey(neutral + lit * 0.4, 255)
            }
        }
        SheetTile::Neutral => grey(neutral, 255),
    }
}

fn grey(v: f32, alpha: u8) -> [u8; 4] {
    let b = to_byte(v);
    [b, b, b, alpha]
}

fn to_byte(v: f32) -> u8 {
    (v.clamp(0.0, 1.0) * 255.0 + 0.5) as u8
}

/// The one hash every tile is built from: the workspace's shared mixer over the texel's
/// coordinates and the tile's seed packed into one word — no floating point, no libm.
fn hash(x: u32, y: u32, seed: u32) -> f32 {
    game_core::math::hash_unit((u64::from(seed) << 40) ^ (u64::from(y) << 20) ^ u64::from(x))
}

fn smooth_step(edge0: f32, edge1: f32, x: f32) -> f32 {
    let t = ((x - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Value noise on a lattice of `cell_x` x `cell_y` texels, periodic over the tile so the
/// material wraps: the lattice has `TILE_PX / cell` nodes a side and the last blends into the
/// first. Only multiplies and adds, so it is the same to the bit everywhere.
fn periodic_noise(x: u32, y: u32, cell_x: u32, cell_y: u32, seed: u32) -> f32 {
    let nodes_x = TILE_PX / cell_x;
    let nodes_y = TILE_PX / cell_y;
    let fx = x as f32 / cell_x as f32;
    let fy = y as f32 / cell_y as f32;
    let ix = fx as u32;
    let iy = fy as u32;
    let tx = smooth_step(0.0, 1.0, fx - ix as f32);
    let ty = smooth_step(0.0, 1.0, fy - iy as f32);
    let node = |nx: u32, ny: u32| hash(nx % nodes_x, ny % nodes_y, seed);
    let top = node(ix, iy) + (node(ix + 1, iy) - node(ix, iy)) * tx;
    let bottom = node(ix, iy + 1) + (node(ix + 1, iy + 1) - node(ix, iy + 1)) * tx;
    top + (bottom - top) * ty
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The sheet's identity. A different number is a different material on every plate in the
    /// game: bless it on purpose, in the PR that changed the generator, with the tiles looked at.
    const SHEET_HASH: u64 = 0xcec3_ca5e_82e2_3060;

    #[test]
    fn the_material_sheet_is_deterministic() {
        let a = generate();
        let b = generate();
        assert_eq!(a.rgba, b.rgba, "two generations must agree to the byte");
        assert_eq!(a.rgba.len(), (SHEET_SIZE * SHEET_SIZE * 4) as usize);
        assert_eq!(
            a.hash(),
            SHEET_HASH,
            "the sheet changed — its hash is {:#018x}; bless it deliberately",
            a.hash()
        );
    }

    #[test]
    fn every_authored_tile_varies_and_the_reserved_quarter_is_empty() {
        let sheet = sheet();
        for tile in SheetTile::ALL {
            if tile == SheetTile::Neutral {
                continue;
            }
            let index = tile.index();
            let (ox, oy) = ((index % TILES_PER_SIDE) * TILE_PX, (index / TILES_PER_SIDE) * TILE_PX);
            let mut distinct = std::collections::BTreeSet::new();
            for y in 0..TILE_PX {
                for x in 0..TILE_PX {
                    distinct.insert(sheet.texel(ox + x, oy + y));
                }
            }
            assert!(distinct.len() > 8, "{tile:?} is flat: {} distinct texels", distinct.len());
        }
        let (rx, ry, rw, rh) = MINIMAP_REGION;
        for y in ry..ry + rh {
            for x in rx..rx + rw {
                assert_eq!(sheet.texel(x, y), [0, 0, 0, 0], "the reserved quarter must be empty");
            }
        }
    }

    /// The tiles wrap: a plate larger than a tile repeats it, and a seam every 128 units would
    /// read as a grid drawn on the steel.
    #[test]
    fn a_brushed_tile_wraps_at_its_edge() {
        let sheet = sheet();
        let mut seam = 0.0f32;
        let mut inner = 0.0f32;
        for y in 0..TILE_PX {
            let edge = f32::from(sheet.texel(TILE_PX - 1, y)[0]);
            let wrapped = f32::from(sheet.texel(0, y)[0]);
            let mid = f32::from(sheet.texel(TILE_PX / 2, y)[0]);
            let next = f32::from(sheet.texel(TILE_PX / 2 + 1, y)[0]);
            seam += (edge - wrapped).abs();
            inner += (mid - next).abs();
        }
        assert!(
            seam <= inner * 2.0 + f32::from(TILE_PX as u16),
            "the seam ({seam}) is louder than the interior ({inner})"
        );
    }

    #[test]
    fn the_sheet_is_the_size_the_shader_divides_by() {
        assert_eq!(SHEET_SIZE, 512);
        assert_eq!(TILES_PER_SIDE, 4);
        assert_eq!(TILE_PX, 128);
        assert_eq!(hud_material_sheet().0, SHEET_SIZE);
    }
}
