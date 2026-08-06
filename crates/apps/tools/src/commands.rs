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
        Command::ImportFlora { input, manifest, out } => import_flora(&input, &manifest, &out)?,
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

/// The RON manifest the importer requires next to every source model: provenance is part of
/// the asset (Flora 2.0, doctrine decision 10).
#[derive(Debug, serde::Deserialize)]
struct FloraManifest {
    name: String,
    spdx: String,
    author: String,
    source_url: String,
    /// Optional per-asset triangle budget (hero flora). Bounded by
    /// `world_forge::flora::FLORA_TRI_CEILING`; raising it is a measured, per-item
    /// decision — cite a `flora_frame_probe` before/after in the PR body.
    #[serde(default)]
    tri_budget: Option<usize>,
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
    let mut colors: Vec<[f32; 3]> = Vec::new();
    let mut indices: Vec<u32> = Vec::new();
    // Which glTF image each vertex run samples: real models split bark and leaves across
    // MATERIALS — taking only the first texture dresses the canopy in bark (the red-pine
    // lesson). Every primitive's texture is composited into ONE per-asset sheet below.
    let mut vertex_runs: Vec<(usize, usize, usize)> = Vec::new();
    // Walk the SCENE, not the raw mesh list: authored node transforms (Quaternius models
    // carry real scales) are part of the geometry's truth — reading bare accessors imports
    // a 2 cm bush.
    let mut stack: Vec<(gltf::Node, glam::Mat4)> = document
        .scenes()
        .flat_map(|scene| scene.nodes())
        .map(|node| (node, glam::Mat4::IDENTITY))
        .collect();
    while let Some((node, parent)) = stack.pop() {
        let world = parent * glam::Mat4::from_cols_array_2d(&node.transform().matrix());
        for child in node.children() {
            stack.push((child, world));
        }
        let Some(mesh) = node.mesh() else {
            continue;
        };
        let normal_matrix = glam::Mat3::from_mat4(world).inverse().transpose();
        for primitive in mesh.primitives() {
            anyhow::ensure!(
                primitive.mode() == gltf::mesh::Mode::Triangles,
                "primitive mode {:?} unsupported: the pipeline takes triangles only",
                primitive.mode()
            );
            let reader = primitive.reader(|buffer| buffers.get(buffer.index()).map(|b| &b.0[..]));
            let base = positions.len() as u32;
            let prim_positions = reader
                .read_positions()
                .ok_or_else(|| anyhow::anyhow!("primitive without POSITION"))?;
            for p in prim_positions {
                positions.push(world.transform_point3(glam::Vec3::from_array(p)).to_array());
            }
            let prim_normals =
                reader.read_normals().ok_or_else(|| anyhow::anyhow!("primitive without NORMAL"))?;
            for n in prim_normals {
                normals.push(
                    (normal_matrix * glam::Vec3::from_array(n)).normalize_or_zero().to_array(),
                );
            }
            let prim_uvs: Vec<[f32; 2]> = reader
                .read_tex_coords(0)
                .ok_or_else(|| anyhow::anyhow!("primitive without TEXCOORD_0"))?
                .into_f32()
                .collect();
            let uv_count = prim_uvs.len();
            uvs.extend(prim_uvs);
            // The material's base_color_factor is half the palette in these packs: a shared
            // gradient texture times a PER-PRIMITIVE factor makes leaves green and bark
            // brown. Bake the factor into vertex colors — the runtime multiplies
            // vertex color x texel, so the product reconstructs the authored look
            // (the red-pine lesson, FL-4 look gate).
            //
            // Hero flora goes further: the source bakes per-vertex COLOR_0 — crown-interior
            // ambient occlusion, a vertical light gradient and per-frond hue jitter — which
            // is exactly the depth cue the flat-lit scene pass cannot compute at runtime.
            // When the attribute exists, multiply it into the factor; when it does not,
            // the old factor-only path is unchanged.
            let factor = primitive.material().pbr_metallic_roughness().base_color_factor();
            match reader.read_colors(0) {
                Some(vertex_colors) => {
                    let mut appended = 0usize;
                    colors.extend(vertex_colors.into_rgb_f32().map(|c| {
                        appended += 1;
                        [
                            (c[0] * factor[0]).clamp(0.0, 1.0),
                            (c[1] * factor[1]).clamp(0.0, 1.0),
                            (c[2] * factor[2]).clamp(0.0, 1.0),
                        ]
                    }));
                    anyhow::ensure!(
                        appended == uv_count,
                        "COLOR_0 stream disagrees with TEXCOORD_0 ({appended} vs {uv_count})"
                    );
                }
                None => {
                    colors.extend(std::iter::repeat_n([factor[0], factor[1], factor[2]], uv_count));
                }
            }
            let prim_indices = reader
                .read_indices()
                .ok_or_else(|| anyhow::anyhow!("primitive without indices"))?
                .into_u32();
            indices.extend(prim_indices.map(|index| index + base));
            let texture = primitive
                .material()
                .pbr_metallic_roughness()
                .base_color_texture()
                .map(|info| info.texture().source().index())
                .ok_or_else(|| anyhow::anyhow!("primitive without a base-color texture"))?;
            vertex_runs.push((base as usize, uv_count, texture));
        }
    }
    // Composite every referenced texture into one vertical sheet and remap each run's UVs
    // into its texture's band. Downstream (validation, the runtime atlas packer) stays
    // single-texture per asset.
    let mut used: Vec<usize> = vertex_runs.iter().map(|run| run.2).collect();
    used.sort_unstable();
    used.dedup();
    anyhow::ensure!(!used.is_empty(), "no textured primitives in the source");
    let mut decoded: Vec<(usize, Vec<u8>, u32, u32)> = Vec::new();
    for &index in &used {
        let image =
            images.get(index).ok_or_else(|| anyhow::anyhow!("texture image {index} missing"))?;
        decoded.push((index, image_to_rgba8(image)?, image.width, image.height));
    }
    let sheet_width = decoded.iter().map(|(_, _, w, _)| *w).max().unwrap_or(1);
    let sheet_height: u32 = decoded.iter().map(|(_, _, _, h)| *h).sum();
    let mut rgba = vec![0u8; (sheet_width * sheet_height * 4) as usize];
    // Per source image: [v_offset, v_scale, u_scale] of its band in the sheet.
    let mut bands: Vec<(usize, [f32; 3])> = Vec::new();
    let mut cursor_y = 0u32;
    for (index, pixels, w, h) in &decoded {
        for row in 0..*h as usize {
            let src = row * *w as usize * 4;
            let dst = ((cursor_y as usize + row) * sheet_width as usize) * 4;
            rgba[dst..dst + *w as usize * 4].copy_from_slice(&pixels[src..src + *w as usize * 4]);
        }
        bands.push((
            *index,
            [
                cursor_y as f32 / sheet_height as f32,
                *h as f32 / sheet_height as f32,
                *w as f32 / sheet_width as f32,
            ],
        ));
        cursor_y += h;
    }
    for (start, count, texture) in &vertex_runs {
        let [v_offset, v_scale, u_scale] =
            bands.iter().find(|(index, _)| index == texture).expect("band exists").1;
        for uv in &mut uvs[*start..*start + *count] {
            uv[0] = (uv[0].clamp(0.0, 1.0)) * u_scale;
            uv[1] = v_offset + (uv[1].clamp(0.0, 1.0)) * v_scale;
        }
    }
    // Halve the sheet until it fits a sane per-asset footprint: many assets share ONE 2048
    // runtime atlas page (min-spec VRAM is the budget), and a stylized texture loses nothing
    // a battle camera can see at 512-1024. UVs are normalized, so they never change.
    let (mut sheet_width, mut sheet_height, mut rgba) = (sheet_width, sheet_height, rgba);
    while sheet_width.max(sheet_height) > 1024 {
        let (half_w, half_h) = (sheet_width / 2, sheet_height / 2);
        let mut halved = vec![0u8; (half_w * half_h * 4) as usize];
        for y in 0..half_h as usize {
            for x in 0..half_w as usize {
                for channel in 0..4 {
                    let mut sum = 0u32;
                    for (dy, dx) in [(0, 0), (0, 1), (1, 0), (1, 1)] {
                        let src =
                            ((y * 2 + dy) * sheet_width as usize + (x * 2 + dx)) * 4 + channel;
                        sum += u32::from(rgba[src]);
                    }
                    halved[(y * half_w as usize + x) * 4 + channel] = (sum / 4) as u8;
                }
            }
        }
        sheet_width = half_w;
        sheet_height = half_h;
        rgba = halved;
    }

    // Over-budget sources get DECIMATED here, loudly — the validate gate's "decimate the
    // source" is a step of this pipeline, never a raised ceiling. meshopt preserves the
    // silhouette as far as the budget allows; how it looks is the FL-4 look gate's verdict.
    let tri_budget = manifest
        .tri_budget
        .unwrap_or(world_forge::flora::FLORA_MAX_TRIS)
        .min(world_forge::flora::FLORA_TRI_CEILING);
    let source_tris = indices.len() / 3;
    if source_tris > tri_budget {
        let vertex_bytes: &[u8] = bytemuck::cast_slice(&positions);
        let adapter =
            meshopt::VertexDataAdapter::new(vertex_bytes, std::mem::size_of::<[f32; 3]>(), 0)
                .map_err(|error| anyhow::anyhow!("meshopt adapter: {error}"))?;
        // Loosen the error bound until the budget is met: a stylized canopy often needs a
        // coarser pass than the default tolerance allows. The LAST resort is refusal, and
        // the look gate judges whatever survives.
        let mut simplified = Vec::new();
        for target_error in [0.05_f32, 0.15, 0.3, 0.6, 1.0] {
            simplified = meshopt::simplify(
                &indices,
                &adapter,
                tri_budget * 3,
                target_error,
                meshopt::SimplifyOptions::None,
                None,
            );
            if simplified.len() / 3 <= tri_budget {
                break;
            }
        }
        anyhow::ensure!(
            !simplified.is_empty() && simplified.len().is_multiple_of(3),
            "simplification collapsed the mesh"
        );
        // Compact: keep only the vertices the simplified index stream still references.
        let mut remap = vec![u32::MAX; positions.len()];
        let (mut new_positions, mut new_normals, mut new_uvs, mut new_colors) =
            (Vec::new(), Vec::new(), Vec::new(), Vec::new());
        let mut new_indices = Vec::with_capacity(simplified.len());
        for &index in &simplified {
            let slot = &mut remap[index as usize];
            if *slot == u32::MAX {
                *slot = new_positions.len() as u32;
                new_positions.push(positions[index as usize]);
                new_normals.push(normals[index as usize]);
                new_uvs.push(uvs[index as usize]);
                new_colors.push(colors[index as usize]);
            }
            new_indices.push(*slot);
        }
        println!(
            "decimated {}: {source_tris} -> {} tris ({} -> {} vertices)",
            manifest.name,
            new_indices.len() / 3,
            positions.len(),
            new_positions.len()
        );
        positions = new_positions;
        normals = new_normals;
        uvs = new_uvs;
        colors = new_colors;
        indices = new_indices;
    }
    // Factors are spec-bounded [0, 1]; clamp exporter drift so validation never trips on
    // a 1.0000001.
    for color in &mut colors {
        for channel in color.iter_mut() {
            *channel = channel.clamp(0.0, 1.0);
        }
    }

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
        texture_width: sheet_width,
        texture_height: sheet_height,
        height_m: max[1] - min[1],
        positions,
        normals,
        uvs,
        colors,
        indices,
        tri_budget: manifest.tri_budget,
    };
    asset.validate().map_err(|reason| anyhow::anyhow!("{} refused: {reason}", input.display()))?;

    std::fs::create_dir_all(out_dir)?;
    let png_path = out_dir.join(&texture_file);
    let file = std::fs::File::create(&png_path)
        .with_context(|| format!("failed to create {}", png_path.display()))?;
    let mut encoder = png::Encoder::new(std::io::BufWriter::new(file), sheet_width, sheet_height);
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
