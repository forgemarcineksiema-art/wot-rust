use std::path::PathBuf;

use clap::{Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(name = "tools", about = "Asset and world tooling for the tank prototype")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    ConvertGltf {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        output: PathBuf,
    },
    MakeFlatHeightmap {
        #[arg(long)]
        output: PathBuf,
        #[arg(long, default_value_t = 129)]
        width: usize,
        #[arg(long, default_value_t = 129)]
        height: usize,
        #[arg(long, default_value_t = 1.0)]
        cell_size: f32,
        #[arg(long, default_value_t = 0.0)]
        height_m: f32,
    },
    GenerateMap {
        #[arg(long)]
        output: PathBuf,
        #[arg(long, default_value = "prokhorovka-hill-252-2")]
        map: String,
    },
    GenerateVehicle {
        #[arg(long)]
        output: PathBuf,
        #[arg(long)]
        vehicle: String,
    },
    ForgeVehicle {
        #[arg(long)]
        vehicle: String,
        #[arg(long, default_value = "lod0")]
        profile: String,
        #[arg(long)]
        out: PathBuf,
    },
    ForgeReport {
        #[arg(long)]
        vehicle: String,
        #[arg(long)]
        out: PathBuf,
    },
    ForgeLineup {
        #[arg(long)]
        out: PathBuf,
    },
    CompileTank {
        #[arg(long)]
        vehicle: String,
        #[arg(long)]
        gun: Option<String>,
        #[arg(long, default_value = "lod0")]
        profile: String,
        #[arg(long)]
        out: PathBuf,
    },
    /// Re-serialize every vehicle blueprint to its RON source file — the one-off migration
    /// exporter, kept as a permanent normalizer (a hand-edited file can be re-canonicalized).
    ExportBlueprints {
        #[arg(long)]
        out: PathBuf,
    },
    /// Import a CC0 foliage model (Imported Flora 2.0, FL-3): glTF/GLB in, a validated
    /// `.flora.json` + `.flora.png` pair out. Refuses without a manifest naming a CC0
    /// license — provenance is part of the asset.
    ImportFlora {
        /// The source .gltf/.glb file.
        #[arg(long)]
        input: PathBuf,
        /// The RON license manifest (name, author, source_url, spdx = "CC0-1.0").
        #[arg(long)]
        manifest: PathBuf,
        /// Output directory (typically assets/flora).
        #[arg(long, default_value = "assets/flora")]
        out: PathBuf,
    },
    /// Bake one vehicle and write the AI review bundle: a multi-view contact sheet, per-view
    /// tiles, and a report.md with ratios, budgets, mesh quality and blueprint lint.
    Studio {
        #[arg(long)]
        vehicle: String,
        #[arg(long)]
        out: Option<PathBuf>,
        /// Live-override: bake THIS RON file instead of the embedded blueprint (no rebuild) —
        /// the fast loop for shape-tuning. The file's `kind` must match --vehicle.
        #[arg(long)]
        blueprint_file: Option<PathBuf>,
    },
}
