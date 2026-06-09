use std::path::PathBuf;

use anyhow::Context;
use clap::{Parser, Subcommand};
use game_core::TankSpec;
use serde::Serialize;
use terrain::{HeightMap, prokhorovka_hill_252_2};

#[derive(Debug, Parser)]
#[command(name = "tools", about = "Asset and world tooling for the tank prototype")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
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
}

#[derive(Debug, Serialize)]
struct ConvertedAssetManifest {
    source: String,
    format: &'static str,
    version: u32,
    meshes: usize,
    materials: usize,
    nodes: usize,
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Command::ConvertGltf { input, output } => {
            let gltf = gltf::Gltf::open(&input)
                .with_context(|| format!("failed to open glTF source {}", input.display()))?;
            let manifest = ConvertedAssetManifest {
                source: input.display().to_string(),
                format: "wot-asset-manifest",
                version: 1,
                meshes: gltf.meshes().count(),
                materials: gltf.materials().count(),
                nodes: gltf.nodes().count(),
            };
            write_json(output, &manifest)?;
        }
        Command::MakeFlatHeightmap { output, width, height, cell_size, height_m } => {
            let heightmap = HeightMap::flat(width, height, cell_size, height_m)?;
            write_json(output, &heightmap)?;
        }
        Command::GenerateMap { output, map } => match map.as_str() {
            "prokhorovka-hill-252-2" => write_json(output, &prokhorovka_hill_252_2())?,
            other => anyhow::bail!("unknown map profile: {other}"),
        },
        Command::GenerateVehicle { output, vehicle } => match vehicle.as_str() {
            "t54-1951" => write_json(output, &TankSpec::t54_1951())?,
            "t55a" => write_json(output, &TankSpec::t55a())?,
            "tiger-i-ausf-e" => write_json(output, &TankSpec::tiger_i_ausf_e())?,
            "tiger-ii-ausf-b" => write_json(output, &TankSpec::tiger_ii_ausf_b())?,
            "jagdtiger" => write_json(output, &TankSpec::jagdtiger())?,
            "panther-ii" => write_json(output, &TankSpec::panther_ii())?,
            other => anyhow::bail!("unknown vehicle profile: {other}"),
        },
    }

    Ok(())
}

fn write_json(path: PathBuf, value: &impl Serialize) -> anyhow::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("failed to create output directory {}", parent.display()))?;
    }

    let json = serde_json::to_string_pretty(value)?;
    std::fs::write(&path, json).with_context(|| format!("failed to write {}", path.display()))?;
    Ok(())
}
