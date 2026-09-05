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
        Command::Bless { vehicle } => bless_command(&vehicle)?,
        Command::ForgeVehicle { vehicle, profile, out } => {
            ForgeArtifact::bake(parse_vehicle_kind(&vehicle)?, profile.parse()?)?
                .write_to_dir(&out)?
        }
        Command::ForgeReport { vehicle, out } => forge_report(&vehicle, out)?,
        Command::OutlineOverlay { vehicle, out } => outline_overlay(&vehicle, out)?,
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
        Command::MapAtlas { out, map, res, skip_exposure } => {
            map_atlas_command(out, map.as_deref(), res, skip_exposure)?
        }
    }
    Ok(())
}

/// Render the terrain atlas: per map a directory of instrument PNGs plus one `atlas.md`
/// with the measured stats. Exposure is the slow sweep (~seconds per map in release);
/// `--skip-exposure` renders everything else in a blink for the tight loop.
fn map_atlas_command(
    out: Option<PathBuf>,
    map_slug: Option<&str>,
    res: f32,
    skip_exposure: bool,
) -> anyhow::Result<()> {
    use crate::atlas;

    let out = out.unwrap_or_else(|| PathBuf::from("target/map_atlas"));
    let maps: Vec<MapId> =
        match map_slug {
            Some(slug) => vec![MapId::from_slug(slug).with_context(|| {
                format!("unknown map slug {slug} (known: {:?})", MapId::SHIPPED)
            })?],
            None => MapId::SHIPPED.to_vec(),
        };
    let mut report = String::from(
        "# Terrain atlas\n\nMeasured through the game's own rules: `sample_height` \
         (bilinear, the sim's lane), `GroundClassifier` (splat = drive), \
         `game_core::MAX_CLIMB_GRADE`/`ROAD_COMFORT_GRADE`, the physics wading bands, \
         `sim::DROWN_DEPTH_M`, and `sim::line_of_sight` over the born cover state with the \
         T-54 benchmark geometry.\n\n",
    );
    for id in maps {
        let map = map_forge::battlefield(id);
        let classifier = terrain::GroundClassifier::new(&map);
        let dir = out.join(id.slug());
        std::fs::create_dir_all(&dir)
            .with_context(|| format!("failed to create {}", dir.display()))?;
        let mut layers = vec![
            ("form", atlas::form_layer(&map, res)),
            ("ground", atlas::ground_layer(&map, &classifier, res)),
            ("drive", atlas::drive_layer(&map, res)),
            ("tactical", atlas::tactical_layer(&map, res)),
        ];
        if !skip_exposure {
            let from_south = atlas::exposure_field(&map, false, 5.0, 60.0);
            let from_north = atlas::exposure_field(&map, true, 5.0, 60.0);
            layers.push(("exposure_from_south", atlas::exposure_layer(&map, &from_south, res)));
            layers.push(("exposure_from_north", atlas::exposure_layer(&map, &from_north, res)));
            report.push_str(&atlas::atlas_stats(&map, &from_south, &from_north).markdown_section());
        }
        for (name, raster) in layers {
            let path = dir.join(format!("{name}.png"));
            std::fs::write(&path, raster.to_png_bytes()?)
                .with_context(|| format!("failed to write {}", path.display()))?;
            println!("wrote {}", path.display());
        }
    }
    if !skip_exposure {
        let path = out.join("atlas.md");
        std::fs::write(&path, &report)
            .with_context(|| format!("failed to write {}", path.display()))?;
        println!("wrote {}", path.display());
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
/// The repository root: three directories above this crate's manifest (`crates/apps/tools`).
fn repo_root_of_tools() -> PathBuf {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest.ancestors().nth(3).expect("crates/apps/tools sits three levels deep").to_path_buf()
}

/// Every golden of one vehicle, re-recorded in one run (acceleration step 1): the bake hashes
/// row, the studio tiles, the asset snapshot; then the K0 outline scores, so the blessing is
/// looked at. Nothing here decides whether the change was right — the commit does.
fn bless_command(vehicle: &str) -> anyhow::Result<()> {
    let kind = parse_vehicle_kind(vehicle)?;
    let root = repo_root_of_tools();

    // 1. The bake hashes row.
    let recipe = vehicle_recipes::bake_vehicle(kind)?.deterministic_hash();
    let shipped = vehicle_forge::authoritative_baked_vehicle(kind)?.deterministic_hash();
    let goldens = root.join("crates/vehicle/vehicle_recipes/goldens/bake_hashes.txt");
    let text = std::fs::read_to_string(&goldens)
        .with_context(|| format!("failed to read {}", goldens.display()))?;
    let name = format!("{kind:?}");
    let shipped_column = if shipped == recipe { "-".to_string() } else { shipped.to_string() };
    let mut replaced = false;
    let rows: Vec<String> = text
        .lines()
        .map(|line| {
            if line.split_whitespace().next() == Some(name.as_str()) {
                replaced = true;
                format!("{name} {recipe} {shipped_column}")
            } else {
                line.to_string()
            }
        })
        .collect();
    anyhow::ensure!(replaced, "{name} has no row in {}", goldens.display());
    std::fs::write(&goldens, rows.join("\n") + "\n")
        .with_context(|| format!("failed to write {}", goldens.display()))?;
    println!("bake_hashes.txt: {name} recipe {recipe} shipped {shipped_column}");

    // 2. The studio tiles.
    let bundle = vehicle_forge::bake_studio_bundle(kind)?;
    let tiles = root.join("crates/apps/tools/tests/goldens/studio").join(kind.slug());
    std::fs::create_dir_all(&tiles)
        .with_context(|| format!("failed to create {}", tiles.display()))?;
    for view in bundle.views() {
        std::fs::write(tiles.join(view.name), &view.png)
            .with_context(|| format!("failed to write {}", tiles.join(view.name).display()))?;
    }
    println!("studio tiles: {} re-recorded in {}", bundle.views().len(), tiles.display());

    // 3. The asset snapshot.
    let asset = root.join("assets/vehicles").join(format!("{}.vehicle.json", kind.slug()));
    write_json(asset.clone(), &vehicle_spec(vehicle)?)?;
    println!("asset: {}", asset.display());

    // 4. The K0 scores, to be looked at (the overlays land in target/forge/outlines).
    if vehicle_forge::ReferencePack::for_vehicle(kind)
        .is_some_and(|pack| !pack.outlines().is_empty())
    {
        outline_overlay(vehicle, None)?;
    }
    Ok(())
}

fn outline_overlay(vehicle: &str, out: Option<PathBuf>) -> anyhow::Result<()> {
    let kind = parse_vehicle_kind(vehicle)?;
    let pack = ReferencePack::for_vehicle(kind)
        .with_context(|| format!("no Forge ReferencePack for {vehicle}"))?;
    anyhow::ensure!(
        !pack.outlines().is_empty(),
        "{vehicle} has no reference outlines yet (vehicle_forge/outlines/<slug>.outline.ron)"
    );
    let out = out.unwrap_or_else(|| PathBuf::from("target/forge/outlines"));
    std::fs::create_dir_all(&out)
        .with_context(|| format!("failed to create output directory {}", out.display()))?;
    let baked = vehicle_forge::authoritative_baked_vehicle(kind)?;
    let tris = vehicle_forge::composed_triangles_for(&baked);
    for spec in pack.outlines() {
        let measurement = vehicle_forge::measure_outline(&tris, spec);
        println!("{}", measurement.summary_line(kind.slug()));
        let (bake_lo, bake_hi) = extents(tris.iter().flatten().map(|p| spec.view().project(*p)));
        let (line_lo, line_hi) =
            extents(spec.loops().iter().flatten().map(|p| glam::Vec2::from(*p)));
        println!(
            "  extents h: bake [{:.3}, {:.3}] outline [{:.3}, {:.3}]  v: bake [{:.3}, {:.3}] outline [{:.3}, {:.3}]",
            bake_lo.x, bake_hi.x, line_lo.x, line_hi.x, bake_lo.y, bake_hi.y, line_lo.y, line_hi.y
        );
        // Sensitivity: the same outline slid 10 cm each way. A slide that IMPROVES the score
        // says where the outline (or the bake) sits off its drawing.
        for (name, delta) in [
            ("+h", glam::Vec2::new(0.10, 0.0)),
            ("-h", glam::Vec2::new(-0.10, 0.0)),
            ("+v", glam::Vec2::new(0.0, 0.10)),
            ("-v", glam::Vec2::new(0.0, -0.10)),
        ] {
            let slid = vehicle_forge::measure_outline(&tris, &spec.translated(delta));
            println!("  slid {name} 0.10 m: IoU {:.3}", slid.iou());
        }
        let path = out.join(format!("{}-{}.png", kind.slug(), spec.view().label()));
        std::fs::write(&path, vehicle_forge::outline_overlay_png(&tris, spec)?)
            .with_context(|| format!("failed to write {}", path.display()))?;
        println!("  overlay: {}", path.display());
    }
    Ok(())
}
fn extents(points: impl Iterator<Item = glam::Vec2>) -> (glam::Vec2, glam::Vec2) {
    points.fold((glam::Vec2::splat(f32::MAX), glam::Vec2::splat(f32::MIN)), |(lo, hi), p| {
        (lo.min(p), hi.max(p))
    })
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
