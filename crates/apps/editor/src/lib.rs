//! The map editor (map-editor plan M3): a blueprint authoring shell on the game's own
//! render path. Everything the editor does edits a [`map_forge::blueprint::MapBlueprint`]
//! DOCUMENT; the compiled battlefield is always a full, fresh `map_forge::compile` of that
//! document — no incremental state, so the viewport can never show a world the game would
//! not build. Undo/redo is a document snapshot stack (the quantized brush ops of M4 will
//! ride on top of this same `apply_edit` door).

pub mod app;
pub mod brush;
pub mod markers;
pub mod objects;
pub mod overlay;
pub mod pick;
pub mod stamp;

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use map_forge::blueprint::MapBlueprint;
use map_forge::{MapReport, compile};
use terrain::BattlefieldMap;

/// One full recompile of the document: the battlefield, its contract report, and how long
/// the compile took (the edit-loop budget is watched, not assumed).
pub struct CompiledMap {
    pub battlefield: BattlefieldMap,
    pub report: MapReport,
    pub compile_time: Duration,
}

/// The document under edit: the blueprint, where it lives on disk, and the undo/redo
/// snapshot stacks. Every mutation goes through [`Self::apply_edit`] — there is no other
/// door, which is what keeps undo total.
pub struct EditorDocument {
    path: Option<PathBuf>,
    blueprint: MapBlueprint,
    undo: Vec<MapBlueprint>,
    redo: Vec<MapBlueprint>,
    dirty: bool,
}

impl EditorDocument {
    /// A fresh document from the scratch placeholder — the "File → New" of the shell.
    pub fn new_scratch() -> Self {
        let blueprint = map_forge::blueprint_for(terrain::MapId::Scratch);
        Self { path: None, blueprint, undo: Vec::new(), redo: Vec::new(), dirty: false }
    }

    /// Open a blueprint document from disk.
    pub fn open(path: &Path) -> anyhow::Result<Self> {
        let source = std::fs::read_to_string(path)?;
        let blueprint = MapBlueprint::from_ron(&source)
            .map_err(|error| anyhow::anyhow!("{}: {error}", path.display()))?;
        Ok(Self {
            path: Some(path.to_path_buf()),
            blueprint,
            undo: Vec::new(),
            redo: Vec::new(),
            dirty: false,
        })
    }

    pub fn blueprint(&self) -> &MapBlueprint {
        &self.blueprint
    }

    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    pub fn dirty(&self) -> bool {
        self.dirty
    }

    /// Apply one edit to the document. The pre-edit state lands on the undo stack and the
    /// redo stack clears — exactly the everyday editor contract.
    pub fn apply_edit(&mut self, edit: impl FnOnce(&mut MapBlueprint)) {
        self.undo.push(self.blueprint.clone());
        edit(&mut self.blueprint);
        self.redo.clear();
        self.dirty = true;
    }

    pub fn undo(&mut self) -> bool {
        match self.undo.pop() {
            Some(previous) => {
                self.redo.push(std::mem::replace(&mut self.blueprint, previous));
                self.dirty = true;
                true
            }
            None => false,
        }
    }

    pub fn redo(&mut self) -> bool {
        match self.redo.pop() {
            Some(next) => {
                self.undo.push(std::mem::replace(&mut self.blueprint, next));
                self.dirty = true;
                true
            }
            None => false,
        }
    }

    /// Serialize the canonical RON form to the document's path (or the given one, which
    /// becomes the document's home — "Save As").
    pub fn save_as(&mut self, path: &Path) -> anyhow::Result<()> {
        std::fs::write(path, self.blueprint.to_ron())?;
        self.path = Some(path.to_path_buf());
        self.dirty = false;
        Ok(())
    }

    pub fn save(&mut self) -> anyhow::Result<()> {
        let Some(path) = self.path.clone() else {
            anyhow::bail!("the document has no path yet — save it somewhere first");
        };
        self.save_as(&path)
    }

    /// Full recompile — the ONLY way the editor turns the document into a world, measured
    /// so the edit-loop budget stays a number, not a feeling.
    pub fn recompile(&self) -> CompiledMap {
        let started = Instant::now();
        let (battlefield, report) = compile(&self.blueprint);
        CompiledMap { battlefield, report, compile_time: started.elapsed() }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn edits_are_undoable_and_redoable_and_the_stacks_behave() {
        let mut document = EditorDocument::new_scratch();
        let original = document.blueprint().meta.name.clone();
        document.apply_edit(|blueprint| blueprint.meta.name = "Edited".into());
        assert_eq!(document.blueprint().meta.name, "Edited");
        assert!(document.dirty());

        assert!(document.undo());
        assert_eq!(document.blueprint().meta.name, original);
        assert!(document.redo());
        assert_eq!(document.blueprint().meta.name, "Edited");

        // A fresh edit clears the redo branch — no time-travel forks.
        assert!(document.undo());
        document.apply_edit(|blueprint| blueprint.meta.name = "Other".into());
        assert!(!document.redo());
        assert!(!EditorDocument::new_scratch().undo());
    }

    #[test]
    fn open_save_round_trips_the_document() {
        let directory = std::env::temp_dir().join("wot_editor_test");
        std::fs::create_dir_all(&directory).expect("temp dir");
        let path = directory.join("roundtrip.map.ron");
        let mut document = EditorDocument::new_scratch();
        document.apply_edit(|blueprint| blueprint.meta.name = "Roundtrip".into());
        document.save_as(&path).expect("saves");
        assert!(!document.dirty());

        let reopened = EditorDocument::open(&path).expect("opens");
        assert_eq!(reopened.blueprint(), document.blueprint());
        std::fs::remove_file(&path).ok();
    }

    /// The edit-loop strategy is FULL recompile per edit; the plan pins a <50 ms budget in
    /// the editor's release build. This lock runs in debug (roughly 5× slower), so the
    /// ceiling here is 250 ms — a regression that trips this is a real edit-loop problem,
    /// not test noise. Measured on the heaviest shipped document (the Bystra valley).
    #[test]
    fn a_full_recompile_of_the_heaviest_map_fits_the_edit_loop_budget() {
        let blueprint = map_forge::blueprint_for(terrain::MapId::BystraValley);
        let document = EditorDocument {
            path: None,
            blueprint,
            undo: Vec::new(),
            redo: Vec::new(),
            dirty: false,
        };
        // Warm-up, then the measured pass.
        document.recompile();
        let compiled = document.recompile();
        assert!(
            compiled.compile_time < Duration::from_millis(250),
            "full recompile took {:?} — the live edit loop is broken",
            compiled.compile_time
        );
        assert!(!compiled.report.has_errors(), "the shipped valley compiles clean");
    }
}
