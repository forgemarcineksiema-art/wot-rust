//! The fonts the interface is set in (interface program F3): a PAIR of faces under the SIL Open
//! Font License, embedded in the binary, each with its licence beside the file and its bytes
//! under a hash lock — the flora manifest's shape (`assets/flora/bark/*/LICENSE.md`).
//!
//! **Big Shoulders Stencil Display** is the paint through a stencil on a steel plate: labels,
//! headers, banners. **IBM Plex Sans Condensed** is the engineer's lettering: values and text,
//! with real Latin Extended-A so a Polish string is a string and not a row of gaps. The stencil
//! face's static weights were instanced from the family's variable font with fontTools; the
//! licence permits it and the manifest says so.
//!
//! No font is ever read from disk at runtime: a picture that depends on a working directory is
//! not a deterministic picture (`world_forge::tree::authored` says the same of the trees).

/// The two faces of the pair.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Face {
    /// Big Shoulders Stencil Display — labels, headers, banners.
    Display,
    /// IBM Plex Sans Condensed — values and text.
    Text,
}

/// The weights the pair is embedded at. Append-only.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Weight {
    Regular,
    Medium,
    SemiBold,
    Bold,
    Black,
}

impl Weight {
    /// The CSS weight number, for the record and the licence manifests.
    pub const fn css(self) -> u16 {
        match self {
            Weight::Regular => 400,
            Weight::Medium => 500,
            Weight::SemiBold => 600,
            Weight::Bold => 700,
            Weight::Black => 900,
        }
    }
}

/// One face at one weight: what a run of text is set in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Style {
    pub face: Face,
    pub weight: Weight,
}

impl Style {
    /// Labels and headers: the stencil at medium weight. Headings only, at
    /// [`Self::STENCIL_FLOOR_U`] and above (U12, 2026-09-08): the stencil's stems are 1.0 px at
    /// 16 u on a 900p screen and its counters close — a label set in it is a texture, not a word.
    pub const LABEL: Style = Style { face: Face::Display, weight: Weight::Medium };
    /// The smallest size the stencil faces (`LABEL`, `BANNER`) may be set at, in units. Below
    /// it a label is `VALUE_STRONG`. Locked on every garage list and every HUD state.
    pub const STENCIL_FLOOR_U: f32 = 24.0;
    /// Banners and the largest words: the stencil at black.
    pub const BANNER: Style = Style { face: Face::Display, weight: Weight::Black };
    /// Values and running text.
    pub const VALUE: Style = Style { face: Face::Text, weight: Weight::Regular };
    /// A value the eye should land on first.
    pub const VALUE_STRONG: Style = Style { face: Face::Text, weight: Weight::SemiBold };
    /// A value that must read at a glance under stress.
    pub const VALUE_BOLD: Style = Style { face: Face::Text, weight: Weight::Bold };
    /// What the legacy `push_text` / `text_width` set their text in until every caller names
    /// a style of its own (the H and G waves).
    pub const DEFAULT: Style = Style::VALUE;
    /// Every embedded style, in `FONT_FILES` order. Append-only.
    pub const ALL: [Style; 5] =
        [Style::LABEL, Style::BANNER, Style::VALUE, Style::VALUE_STRONG, Style::VALUE_BOLD];

    /// This style's position in [`Style::ALL`] and in the baked atlas's face table.
    pub fn index(self) -> usize {
        Style::ALL.iter().position(|s| *s == self).expect("every Style is listed in Style::ALL")
    }
}

/// One embedded font file: its style, where it came from, its bytes, and the two things that
/// keep it honest — the hash the bytes must match and the licence that must sit beside it.
pub struct FontFile {
    pub style: Style,
    pub family: &'static str,
    /// Repo-relative path of the embedded file.
    pub file: &'static str,
    pub bytes: &'static [u8],
    /// FNV-1a over `bytes`, pinned by `every_embedded_font_matches_its_manifest_hash`.
    pub fnv1a: u64,
    /// Repo-relative path of the licence manifest beside the file.
    pub licence: &'static str,
}

impl FontFile {
    /// FNV-1a over the embedded bytes.
    pub fn hash(&self) -> u64 {
        let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
        for byte in self.bytes {
            hash ^= u64::from(*byte);
            hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
        }
        hash
    }
}

macro_rules! font_file {
    ($style:expr, $family:literal, $dir:literal, $name:literal, $hash:literal) => {
        FontFile {
            style: $style,
            family: $family,
            file: concat!("assets/fonts/", $dir, "/", $name),
            bytes: include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../../assets/fonts/",
                $dir,
                "/",
                $name
            )),
            fnv1a: $hash,
            licence: concat!("assets/fonts/", $dir, "/LICENSE.md"),
        }
    };
}

/// The pair, one entry per [`Style::ALL`] element, in the same order.
pub const FONT_FILES: [FontFile; 5] = [
    font_file!(
        Style::LABEL,
        "Big Shoulders Stencil Display",
        "big-shoulders-stencil-display",
        "BigShouldersStencilDisplay-Medium.ttf",
        0xa99f_ca4b_3f8e_4510
    ),
    font_file!(
        Style::BANNER,
        "Big Shoulders Stencil Display",
        "big-shoulders-stencil-display",
        "BigShouldersStencilDisplay-Black.ttf",
        0x9d64_1ceb_7a93_0116
    ),
    font_file!(
        Style::VALUE,
        "IBM Plex Sans Condensed",
        "ibm-plex-sans-condensed",
        "IBMPlexSansCondensed-Regular.ttf",
        0x5d3f_cd9b_e696_6542
    ),
    font_file!(
        Style::VALUE_STRONG,
        "IBM Plex Sans Condensed",
        "ibm-plex-sans-condensed",
        "IBMPlexSansCondensed-SemiBold.ttf",
        0x16ed_2d98_9986_36e6
    ),
    font_file!(
        Style::VALUE_BOLD,
        "IBM Plex Sans Condensed",
        "ibm-plex-sans-condensed",
        "IBMPlexSansCondensed-Bold.ttf",
        0x5725_70d2_d0fd_5e3d
    ),
];

/// The Polish alphabet's letters outside ASCII — the first localisation this atlas owes.
pub const POLISH_LETTERS: &str = "ĄĆĘŁŃÓŚŹŻąćęłńóśźż";

/// Latin Extended-A code points a face may lack without failing the coverage lock: the Dutch
/// ligature pair, the apostrophe-n and the long s, which no string of ours will ever set.
pub const LATIN_EXTENDED_A_EXEMPT: [char; 4] = ['\u{132}', '\u{133}', '\u{149}', '\u{17f}'];

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn workspace_root() -> std::path::PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../..")
            .canonicalize()
            .expect("the workspace")
    }

    /// The bytes in the binary are the bytes the manifest names. A font swapped, re-instanced
    /// or re-exported is a deliberate diff with a new hash, never drift.
    #[test]
    fn every_embedded_font_matches_its_manifest_hash() {
        for file in &FONT_FILES {
            assert_eq!(
                file.hash(),
                file.fnv1a,
                "{} changed — its hash is {:#018x}; bless it in the manifest on purpose",
                file.file,
                file.hash()
            );
            assert!(!file.bytes.is_empty(), "{} is empty", file.file);
        }
    }

    /// CC0/OFL only, with the licence beside the file: the manifest points at a `LICENSE.md`
    /// that exists and names the SIL Open Font License, next to a copy of the licence text.
    #[test]
    fn every_font_has_its_licence_beside_it() {
        let root = workspace_root();
        for file in &FONT_FILES {
            let font_path = root.join(file.file);
            assert!(font_path.exists(), "{} is not in the tree", file.file);
            let licence_path = root.join(file.licence);
            let licence = std::fs::read_to_string(&licence_path)
                .unwrap_or_else(|_| panic!("{} should sit beside {}", file.licence, file.file));
            assert!(
                licence.contains("SIL Open Font License"),
                "{} does not name the SIL Open Font License",
                file.licence
            );
            assert!(
                licence.contains(file.family),
                "{} does not name the family it licenses ({})",
                file.licence,
                file.family
            );
            let ofl = font_path.parent().expect("a directory").join("OFL.txt");
            assert!(ofl.exists(), "the licence text OFL.txt must sit beside {}", file.file);
        }
    }

    #[test]
    fn every_style_is_embedded_once_in_manifest_order() {
        assert_eq!(FONT_FILES.len(), Style::ALL.len());
        for (index, (file, style)) in FONT_FILES.iter().zip(Style::ALL).enumerate() {
            assert_eq!(file.style, style, "FONT_FILES[{index}] is out of Style::ALL order");
            assert_eq!(style.index(), index);
        }
        assert_eq!(Style::LABEL.face, Face::Display);
        assert_eq!(Style::VALUE.face, Face::Text);
        assert_eq!(Weight::Black.css(), 900);
    }
}
