pub mod duplication;
pub mod workspace;

pub use workspace::{
    CrateFacts, crate_facts, crate_manifests, crate_src_dir, crate_src_dirs, is_test_module_file,
    rust_files, workspace_root,
};
