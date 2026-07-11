use std::path::PathBuf;

use anyhow::Context;
use game_core::{GunModule, TankSpec, VehicleKind};
use serde::Serialize;
use terrain::{HeightMap, MapId};
use vehicle_forge::{BakeProfile, ForgeArtifact, ReferencePack, TankCompileRequest, compile_tank};
use vehicle_geometry::bake_vehicle;

use crate::cli::Command;

#[derive(Debug, Serialize)]
struct ConvertedAssetManifest {
    source: String,
    format: &'static str,
    version: u32,
    meshes: usize,
    materials: usize,
    nodes: usize,
}
#[derive(Debug, Serialize)]
struct CompiledTankManifest {
    vehicle: VehicleKind,
    profile: &'static str,
    source_hash: u64,
    spec: TankSpec,
}

pub fn dispatch(command: Command) -> anyhow::Result<()> {
    match command {
        Command::ConvertGltf { input, output } => {
            let gltf = gltf::Gltf::open(&input)
                .with_context(|| format!("failed to open glTF source {}", input.display()))?;
            write_json(
                output,
                &ConvertedAssetManifest {
                    source: input.display().to_string(),
                    format: "wot-asset-manifest",
                    version: 1,
                    meshes: gltf.meshes().count(),
                    materials: gltf.materials().count(),
                    nodes: gltf.nodes().count(),
                },
            )?;
        }
        Command::MakeFlatHeightmap { output, width, height, cell_size, height_m } => {
            write_json(output, &HeightMap::flat(width, height, cell_size, height_m)?)?
        }
        Command::GenerateMap { output, map } => match MapId::from_slug(&map) {
            Some(id) => write_json(output, &id.battlefield())?,
            None => {
                let known: Vec<&str> = MapId::ALL.iter().map(|id| id.slug()).collect();
                anyhow::bail!("unknown map profile: {map} (known: {})", known.join(", "))
            }
        },
        Command::GenerateVehicle { output, vehicle } => {
            write_json(output, &vehicle_spec(&vehicle)?)?
        }
        Command::ForgeVehicle { vehicle, profile, out } => {
            ForgeArtifact::bake(parse_vehicle_kind(&vehicle)?, profile.parse()?)?
                .write_to_dir(&out)?
        }
        Command::ForgeReport { vehicle, out } => forge_report(&vehicle, out)?,
        Command::ForgeLineup { out } => write_forge_lineup(out)?,
        Command::CompileTank { vehicle, gun, profile, out } => {
            compile_tank_command(&vehicle, gun.as_deref(), profile.parse()?, out)?
        }
        Command::ExportBlueprints { out } => export_blueprints(out)?,
        Command::Studio { vehicle, out } => studio_command(&vehicle, out)?,
    }
    Ok(())
}

/// Serialize every blueprint-backed vehicle to `<out>/<slug>.blueprint.ron` — the migration
/// exporter, kept as a permanent normalizer for hand-edited files. f32 serializes at
/// shortest-round-trip precision, so parse-back is bit-identical to the source values.
fn export_blueprints(out: PathBuf) -> anyhow::Result<()> {
    std::fs::create_dir_all(&out)
        .with_context(|| format!("failed to create output directory {}", out.display()))?;
    let pretty = ron::ser::PrettyConfig::new().struct_names(true).depth_limit(4);
    for kind in VehicleKind::ALL {
        let Some(blueprint) = game_core::VehicleBlueprint::for_vehicle(kind) else {
            continue;
        };
        let file = game_core::BlueprintFile::from_blueprint(&blueprint);
        let text = ron::ser::to_string_pretty(&file, pretty.clone())
            .with_context(|| format!("failed to serialize {kind:?}"))?;
        let path = out.join(format!("{}.blueprint.ron", kind.slug()));
        std::fs::write(&path, text.as_bytes())
            .with_context(|| format!("failed to write {}", path.display()))?;
        println!("wrote {}", path.display());
    }
    Ok(())
}

/// The AI review loop: bake one vehicle, write the studio bundle (contact sheet, per-view
/// tiles, report.md) under `target/studio/<slug>` unless `--out` overrides it.
fn studio_command(vehicle: &str, out: Option<PathBuf>) -> anyhow::Result<()> {
    let kind = parse_vehicle_kind(vehicle)?;
    let bundle = vehicle_forge::bake_studio_bundle(kind)?;
    let dir = out.unwrap_or_else(|| PathBuf::from("target/studio").join(kind.slug()));
    bundle.write_to_dir(&dir)?;
    println!("wrote studio bundle to {}", dir.display());
    println!("  read {}\\report.md first, then contact_sheet.png", dir.display());
    Ok(())
}

fn vehicle_spec(slug: &str) -> anyhow::Result<TankSpec> {
    Ok(match slug {
        "t54-1951" => TankSpec::t54_1951(),
        "tiger-i-ausf-e" => TankSpec::tiger_i_ausf_e(),
        "tiger-ii-ausf-b" => TankSpec::tiger_ii_ausf_b(),
        "jagdtiger" => TankSpec::jagdtiger(),
        "panther-ii" => TankSpec::panther_ii(),
        "t55a" => TankSpec::t55a(),
        "is3" | "is-3" => TankSpec::is3(),
        "centurion-mk3" | "centurion_mk3" => TankSpec::centurion_mk3(),
        other => anyhow::bail!("unknown vehicle profile: {other}"),
    })
}
fn forge_report(vehicle: &str, out: PathBuf) -> anyhow::Result<()> {
    let kind = parse_vehicle_kind(vehicle)?;
    let report = ReferencePack::for_vehicle(kind)
        .with_context(|| format!("no Forge ReferencePack for {vehicle}"))?
        .measure_baked_vehicle(&bake_vehicle(kind)?)
        .with_context(|| format!("Forge ReferencePack rejected {vehicle}"))?;
    write_text(out, &report.markdown_summary())
}
fn compile_tank_command(
    vehicle: &str,
    gun: Option<&str>,
    profile: BakeProfile,
    out: PathBuf,
) -> anyhow::Result<()> {
    let kind = parse_vehicle_kind(vehicle)?;
    let mut modules = kind.default_loadout();
    if let Some(gun) = gun {
        modules.gun = select_gun(kind, gun)?;
    }
    let compiled = compile_tank(TankCompileRequest { vehicle: kind, modules, profile })?;
    compiled.artifact.write_to_dir(&out)?;
    write_json(
        out.join("compiled-tank.json"),
        &CompiledTankManifest {
            vehicle: kind,
            profile: compiled.artifact.manifest().profile().slug(),
            source_hash: compiled.source_hash,
            spec: compiled.spec,
        },
    )
}
fn write_json(path: PathBuf, value: &impl Serialize) -> anyhow::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("failed to create output directory {}", parent.display()))?;
    }
    std::fs::write(&path, serde_json::to_string_pretty(value)?)
        .with_context(|| format!("failed to write {}", path.display()))?;
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
        "tiger-i-ausf-e" | "tiger_i_ausf_e" => Ok(VehicleKind::TigerI),
        "tiger-ii-ausf-b" | "tiger_ii_ausf_b" => Ok(VehicleKind::TigerII),
        "jagdtiger" => Ok(VehicleKind::Jagdtiger),
        "panther-ii" | "panther_ii" => Ok(VehicleKind::PantherII),
        "t55a" => Ok(VehicleKind::T55A),
        "is3" | "is-3" => Ok(VehicleKind::IS3),
        "centurion-mk3" | "centurion_mk3" => Ok(VehicleKind::Centurion),
        other => anyhow::bail!("unknown vehicle profile: {other}"),
    }
}
fn write_forge_lineup(out: PathBuf) -> anyhow::Result<()> {
    std::fs::create_dir_all(&out)
        .with_context(|| format!("failed to create output directory {}", out.display()))?;
    let mut index = String::from(
        "# Armored Vehicle Forge lineup\n\n| Vehicle | Profile | Artifact | Source hash |\n| --- | --- | --- | ---: |\n",
    );
    for kind in VehicleKind::PLAYABLE {
        let artifact = ForgeArtifact::bake(kind, BakeProfile::Lod0)?;
        let slug = artifact.manifest().vehicle_slug().to_string();
        artifact.write_to_dir(&out.join(&slug))?;
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
fn select_gun(kind: VehicleKind, slug: &str) -> anyhow::Result<GunModule> {
    kind.gun_options()
        .into_iter()
        .find(|gun| {
            gun.spec
                .name
                .to_ascii_lowercase()
                .replace(' ', "-")
                .contains(&slug.to_ascii_lowercase())
        })
        .with_context(|| format!("unknown gun variant {slug} for {kind:?}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn t54_compiler_selects_the_requested_gun_variant() {
        assert_eq!(
            select_gun(VehicleKind::T54_1951, "d-10t2s").unwrap().spec.name,
            "100 mm D-10T2S"
        );
    }
}
