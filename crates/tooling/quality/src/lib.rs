pub mod duplication;
mod file_budget;
pub mod workspace;

pub use file_budget::{
    MAX_RUST_FILE_LINES, MAX_RUST_FILE_TOTAL_LINES, is_test_code_path, production_line_count,
};
pub use workspace::{crate_manifests, crate_src_dir, crate_src_dirs};
