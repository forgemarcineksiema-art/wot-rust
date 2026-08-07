use std::path::PathBuf;

use anyhow::Context;
use game_core::{GunModule, TankSpec, VehicleKind};
use serde::Serialize;
use terrain::{HeightMap, MapId};
use vehicle_forge::{
    BakeProfile, ForgeArtifact, ReferencePack, TankCompileRequest, bake_production_vehicle,
    compile_tank, export_obj,
};

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
            Some(id) => write_json(output, &map_forge::battlefield(id))?,
            None => {
                let known: Vec<&str> = MapId::SHIPPED.iter().map(|id| id.slug()).collect();
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
        Command::Studio { vehicle, out, blueprint_file } => {
            studio_command(&vehicle, out, blueprint_file)?
        }
        Command::ExportMesh { vehicle, out, profile } => {
            export_mesh_command(&vehicle, out, profile.parse()?)?
        }
    }
    Ok(())
}

/// Serialize every blueprint-backed vehicle to `<out>/<slug>.blueprint.ron` — the migration
/// exporter, kept as a permanent normalizer for hand-edited files. f32 serializes at
/// shortest-round-trip precision, so parse-back is bit-identical to the source values.
/// A vehicle carrying a visual-detail tree (W4 F2c) also gets `<out>/<slug>.visual.ron` —
/// the file the fleet slot (F3) will read; today it is a generated export, not a source.
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

        if let Some(detail) = blueprint.visual_detail() {
            // The visual tree nests deeper than the flat blueprint shapes (loft stations,
            // profile arrays); the default depth limit would inline the part a human most
            // wants to diff line by line.
            let pretty = ron::ser::PrettyConfig::new().struct_names(true).depth_limit(6);
            let file = game_core::VisualDetailFile::from_parts(kind, detail);
            let text = ron::ser::to_string_pretty(&file, pretty)
                .with_context(|| format!("failed to serialize {kind:?} visual detail"))?;
            let path = out.join(format!("{}.visual.ron", kind.slug()));
            std::fs::write(&path, text.as_bytes())
                .with_context(|| format!("failed to write {}", path.display()))?;
            println!("wrote {}", path.display());
        }
    }
    Ok(())
}

/// The AI review loop: bake one vehicle, write the studio bundle (contact sheet, per-view
/// tiles, report.md) under `target/studio/<slug>` unless `--out` overrides it.
fn studio_command(
    vehicle: &str,
    out: Option<PathBuf>,
    blueprint_file: Option<PathBuf>,
) -> anyhow::Result<()> {
    let kind = parse_vehicle_kind(vehicle)?;
    let bundle = match blueprint_file {
        Some(path) => {
            let text = std::fs::read_to_string(&path)
                .with_context(|| format!("failed to read {}", path.display()))?;
            let blueprint = game_core::parse_blueprint(kind, &text)
                .map_err(|error| anyhow::anyhow!("{error}"))?;
            println!("live override: baking {} from {}", kind.slug(), path.display());
            vehicle_forge::bake_studio_bundle_from_blueprint(&blueprint)?
        }
        None => vehicle_forge::bake_studio_bundle(kind)?,
    };
    let dir = out.unwrap_or_else(|| PathBuf::from("target/studio").join(kind.slug()));
    bundle.write_to_dir(&dir)?;
    println!("wrote studio bundle to {}", dir.display());
    println!("  read {}\\report.md first, then contact_sheet.png", dir.display());
    Ok(())
}

/// Export a baked vehicle to OBJ+MTL for external inspection (the master-reference loop).
///
/// The bake is the PRODUCTION one, so what an inspector measures in Blender is what the battle
/// draws — including the instanced running gear at rest pose, which lives outside the static
/// submeshes and is therefore invisible to anyone reading the artifact alone.
fn export_mesh_command(
    vehicle: &str,
    out: Option<PathBuf>,
    profile: BakeProfile,
) -> anyhow::Result<()> {
    let kind = parse_vehicle_kind(vehicle)?;
    let baked = bake_production_vehicle(kind, profile)?;
    let obj_path =
        out.unwrap_or_else(|| PathBuf::from("target/export").join(format!("{}.obj", kind.slug())));
    let mtl_path = obj_path.with_extension("mtl");
    let mtl_name = mtl_path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| format!("{}.mtl", kind.slug()));

    let export = export_obj(kind, &baked, &mtl_name);
    if let Some(dir) = obj_path.parent() {
        std::fs::create_dir_all(dir)
            .with_context(|| format!("failed to create {}", dir.display()))?;
    }
    std::fs::write(&obj_path, export.obj.as_bytes())
        .with_context(|| format!("failed to write {}", obj_path.display()))?;
    std::fs::write(&mtl_path, export.mtl.as_bytes())
        .with_context(|| format!("failed to write {}", mtl_path.display()))?;

    println!(
        "wrote {} ({} objects, {} tris, {} verts) + {}",
        obj_path.display(),
        export.objects.len(),
        export.triangle_count,
        export.vertex_count,
        mtl_path.display(),
    );
    println!(
        "  Blender import: forward Z, up Y (model frame is +X right, +Y up, +Z forward, origin \
         on the ground)"
    );
    Ok(())
}

fn vehicle_spec(slug: &str) -> anyhow::Result<TankSpec> {
    Ok(parse_vehicle_kind(slug)?.spec())
}
fn forge_report(vehicle: &str, out: PathBuf) -> anyhow::Result<()> {
    let kind = parse_vehicle_kind(vehicle)?;
    let report = ReferencePack::for_vehicle(kind)
        .with_context(|| format!("no Forge ReferencePack for {vehicle}"))?
        // The AUTHORITATIVE bake, not the raw procedural recipe: for the T-54 those are
        // different meshes, and a report about a mesh nobody ships is worse than no report.
        .measure_baked_vehicle(&bake_production_vehicle(kind, BakeProfile::Lod0)?)
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
/// Resolve a CLI slug against the two DATA tables — the forge's hyphenated slug and the asset
/// stem — so every vehicle accepts both spellings and the CLI never names one.
///
/// This replaces two hand-written match tables whose aliases had already drifted apart: the old
/// `vehicle_spec` accepted `"is-3"` and `"t34_85"` but only `"t54-1951"`, purely because each row
/// was typed by hand on a different day.
fn parse_vehicle_kind(slug: &str) -> anyhow::Result<VehicleKind> {
    VehicleKind::ALL
        .iter()
        .copied()
        .find(|kind| slug == vehicle_forge::forge_vehicle_slug(*kind) || slug == kind.slug())
        .ok_or_else(|| anyhow::anyhow!("unknown vehicle profile: {slug}"))
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
