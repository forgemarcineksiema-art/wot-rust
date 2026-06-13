use std::path::PathBuf;

use anyhow::Context;
use clap::{Parser, Subcommand};
use game_core::{TankSpec, VehicleKind};
use serde::Serialize;
use terrain::{HeightMap, prokhorovka_hill_252_2};
use vehicle_forge::{BakeProfile, ForgeArtifact, ReferencePack};
use vehicle_geometry::bake_vehicle;

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
        Command::ForgeVehicle { vehicle, profile, out } => {
            let kind = parse_vehicle_kind(&vehicle)?;
            let profile: BakeProfile = profile.parse()?;
            ForgeArtifact::bake(kind, profile)?.write_to_dir(&out)?;
        }
        Command::ForgeReport { vehicle, out } => {
            let kind = parse_vehicle_kind(&vehicle)?;
            let baked = bake_vehicle(kind)?;
            let reference = ReferencePack::for_vehicle(kind)
                .with_context(|| format!("no Forge ReferencePack for {vehicle}"))?;
            let report = reference
                .measure_baked_vehicle(&baked)
                .with_context(|| format!("Forge ReferencePack rejected {vehicle}"))?;
            write_text(out, &report.markdown_summary())?;
        }
        Command::ForgeLineup { out } => {
            write_forge_lineup(out)?;
        }
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

fn write_text(path: PathBuf, text: &str) -> anyhow::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("failed to create output directory {}", parent.display()))?;
    }

    std::fs::write(&path, text).with_context(|| format!("failed to write {}", path.display()))?;
    Ok(())
}

fn parse_vehicle_kind(slug: &str) -> anyhow::Result<VehicleKind> {
    match slug {
        "prototype-medium" | "prototype_medium" => Ok(VehicleKind::PrototypeMedium),
        "t54-1951" | "t54_1951" => Ok(VehicleKind::T54_1951),
        "t55a" => Ok(VehicleKind::T55A),
        "tiger-i-ausf-e" | "tiger_i_ausf_e" => Ok(VehicleKind::TigerI),
        "tiger-ii-ausf-b" | "tiger_ii_ausf_b" => Ok(VehicleKind::TigerII),
        "jagdtiger" => Ok(VehicleKind::Jagdtiger),
        "panther-ii" | "panther_ii" => Ok(VehicleKind::PantherII),
        other => anyhow::bail!("unknown vehicle profile: {other}"),
    }
}

fn write_forge_lineup(out: PathBuf) -> anyhow::Result<()> {
    std::fs::create_dir_all(&out)
        .with_context(|| format!("failed to create output directory {}", out.display()))?;
    let vehicles = [VehicleKind::T54_1951, VehicleKind::T55A];
    let mut index = String::from("# Armored Vehicle Forge lineup\n\n");
    index.push_str("| Vehicle | Profile | Artifact | Source hash |\n");
    index.push_str("| --- | --- | --- | ---: |\n");

    for kind in vehicles {
        let artifact = ForgeArtifact::bake(kind, BakeProfile::Lod0)?;
        let slug = artifact.manifest().vehicle_slug().to_string();
        let vehicle_dir = out.join(&slug);
        artifact.write_to_dir(&vehicle_dir)?;
        index.push_str(&format!(
            "| {} | {} | `{}/manifest.json` | {} |\n",
            slug,
            artifact.manifest().profile().slug(),
            slug,
            artifact.manifest().source_hash()
        ));
    }

    write_text(out.join("index.md"), &index)
}
