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
            Some(id) => write_json(output, &map_forge::battlefield(id))?,
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
        Command::ImportFlora { input, manifest, out } => import_flora(&input, &manifest, &out)?,
        Command::ExportBlueprints { out } => export_blueprints(out)?,
        Command::Studio { vehicle, out, blueprint_file } => {
            studio_command(&vehicle, out, blueprint_file)?
        }
    }
    Ok(())
}

/// The RON manifest the importer requires next to every source model: provenance is part of
/// the asset (Flora 2.0, doctrine decision 10).
#[derive(Debug, serde::Deserialize)]
struct FloraManifest {
    name: String,
    spdx: String,
    author: String,
    source_url: String,
}

/// Import a CC0 foliage model (FL-3): read the glTF/GLB, merge its triangle primitives,
/// normalize (recentre XZ, ground min-y to 0), pull the base-color texture, validate through
/// `world_forge::flora`, and write the `<name>.flora.json` + `<name>.flora.png` pair. Every
/// refusal is a named error — a downloaded model either passes whole or explains itself.
fn import_flora(
    input: &std::path::Path,
    manifest_path: &std::path::Path,
    out_dir: &std::path::Path,
) -> anyhow::Result<()> {
    let manifest: FloraManifest = ron::from_str(
        &std::fs::read_to_string(manifest_path)
            .with_context(|| format!("failed to read manifest {}", manifest_path.display()))?,
    )
    .with_context(|| format!("failed to parse manifest {}", manifest_path.display()))?;

    let (document, buffers, images) = gltf::import(input)
        .with_context(|| format!("failed to import glTF {}", input.display()))?;

    let mut positions: Vec<[f32; 3]> = Vec::new();
    let mut normals: Vec<[f32; 3]> = Vec::new();
    let mut uvs: Vec<[f32; 2]> = Vec::new();
    let mut indices: Vec<u32> = Vec::new();
    let mut texture_image: Option<usize> = None;
    for mesh in document.meshes() {
        for primitive in mesh.primitives() {
            anyhow::ensure!(
                primitive.mode() == gltf::mesh::Mode::Triangles,
                "primitive mode {:?} unsupported: the pipeline takes triangles only",
                primitive.mode()
            );
            let reader = primitive.reader(|buffer| buffers.get(buffer.index()).map(|b| &b.0[..]));
            let base = positions.len() as u32;
            let prim_positions: Vec<[f32; 3]> = reader
                .read_positions()
                .ok_or_else(|| anyhow::anyhow!("primitive without POSITION"))?
                .collect();
            let prim_normals: Vec<[f32; 3]> = reader
                .read_normals()
                .ok_or_else(|| anyhow::anyhow!("primitive without NORMAL"))?
                .collect();
            let prim_uvs: Vec<[f32; 2]> = reader
                .read_tex_coords(0)
                .ok_or_else(|| anyhow::anyhow!("primitive without TEXCOORD_0"))?
                .into_f32()
                .collect();
            let prim_indices: Vec<u32> = reader
                .read_indices()
                .ok_or_else(|| anyhow::anyhow!("primitive without indices"))?
                .into_u32()
                .collect();
            positions.extend(prim_positions);
            normals.extend(prim_normals);
            uvs.extend(prim_uvs);
            indices.extend(prim_indices.into_iter().map(|index| index + base));
            if texture_image.is_none() {
                texture_image = primitive
                    .material()
                    .pbr_metallic_roughness()
                    .base_color_texture()
                    .map(|info| info.texture().source().index());
            }
        }
    }
    let image_index =
        texture_image.ok_or_else(|| anyhow::anyhow!("no base-color texture in the source"))?;
    let image = images
        .get(image_index)
        .ok_or_else(|| anyhow::anyhow!("texture image {image_index} missing"))?;
    let rgba = image_to_rgba8(image)?;

    // Normalize: recentre in XZ, ground the lowest vertex at y = 0.
    let (mut min, mut max) = ([f32::INFINITY; 3], [f32::NEG_INFINITY; 3]);
    for p in &positions {
        for axis in 0..3 {
            min[axis] = min[axis].min(p[axis]);
            max[axis] = max[axis].max(p[axis]);
        }
    }
    let centre_x = (min[0] + max[0]) * 0.5;
    let centre_z = (min[2] + max[2]) * 0.5;
    for p in &mut positions {
        p[0] -= centre_x;
        p[1] -= min[1];
        p[2] -= centre_z;
    }
    // Clamp epsilon-out-of-range UVs (a common exporter artifact); real wrapping still fails
    // validation loudly.
    for uv in &mut uvs {
        uv[0] = uv[0].clamp(-0.001, 1.001).clamp(0.0, 1.0);
        uv[1] = uv[1].clamp(-0.001, 1.001).clamp(0.0, 1.0);
    }

    let texture_file = format!("{}.flora.png", manifest.name);
    let asset = world_forge::flora::FloraAsset {
        name: manifest.name.clone(),
        license: world_forge::flora::FloraLicense {
            spdx: manifest.spdx,
            author: manifest.author,
            source_url: manifest.source_url,
        },
        texture_file: texture_file.clone(),
        texture_width: image.width,
        texture_height: image.height,
        height_m: max[1] - min[1],
        positions,
        normals,
        uvs,
        indices,
    };
    asset.validate().map_err(|reason| anyhow::anyhow!("{} refused: {reason}", input.display()))?;

    std::fs::create_dir_all(out_dir)?;
    let png_path = out_dir.join(&texture_file);
    let file = std::fs::File::create(&png_path)
        .with_context(|| format!("failed to create {}", png_path.display()))?;
    let mut encoder = png::Encoder::new(std::io::BufWriter::new(file), image.width, image.height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.write_header()?.write_image_data(&rgba)?;
    let json_path = out_dir.join(format!("{}.flora.json", asset.name));
    write_json(json_path.clone(), &asset)?;
    println!(
        "imported {}: {} tris, {}x{} texture, {:.2} m tall -> {}",
        asset.name,
        asset.triangle_count(),
        asset.texture_width,
        asset.texture_height,
        asset.height_m,
        json_path.display()
    );
    Ok(())
}

/// Convert a decoded glTF image to tight RGBA8 (the only wire format the atlas takes).
fn image_to_rgba8(image: &gltf::image::Data) -> anyhow::Result<Vec<u8>> {
    use gltf::image::Format;
    let pixel_count = (image.width * image.height) as usize;
    Ok(match image.format {
        Format::R8G8B8A8 => image.pixels.clone(),
        Format::R8G8B8 => {
            let mut out = Vec::with_capacity(pixel_count * 4);
            for rgb in image.pixels.chunks_exact(3) {
                out.extend_from_slice(&[rgb[0], rgb[1], rgb[2], 255]);
            }
            out
        }
        other => anyhow::bail!(
            "texture format {other:?} unsupported: export the source as 8-bit RGB/RGBA"
        ),
    })
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

fn vehicle_spec(slug: &str) -> anyhow::Result<TankSpec> {
    Ok(match slug {
        "t54-1951" => TankSpec::t54_1951(),
        "tiger-i-ausf-e" => TankSpec::tiger_i_ausf_e(),
        "tiger-ii-ausf-b" => TankSpec::tiger_ii_ausf_b(),
        "jagdtiger" => TankSpec::jagdtiger(),
        "panther-ii" => TankSpec::panther_ii(),
        "is3" | "is-3" => TankSpec::is3(),
        "t34-85" | "t34_85" => VehicleKind::T34_85.spec(),
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
        "is3" | "is-3" => Ok(VehicleKind::IS3),
        "centurion-mk3" | "centurion_mk3" => Ok(VehicleKind::Centurion),
        "t34-85" | "t34_85" => Ok(VehicleKind::T34_85),
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
