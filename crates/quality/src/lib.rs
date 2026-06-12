pub mod duplication;
mod file_budget;

pub use file_budget::{
    MAX_RUST_FILE_LINES, MAX_RUST_FILE_TOTAL_LINES, is_test_code_path, production_line_count,
};
