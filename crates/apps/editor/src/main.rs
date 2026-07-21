//! The map editor binary: `cargo run -p editor [-- path/to.map.ron]`. Without a path the
//! shell opens a fresh scratch document (File → New); with one it opens the blueprint.

fn main() -> anyhow::Result<()> {
    let path = std::env::args().nth(1).map(std::path::PathBuf::from);
    editor::app::run(path)
}
