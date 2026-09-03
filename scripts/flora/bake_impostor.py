"""Render a species' IMPOSTOR sprite pair in Blender from its authored tree (route 2, LOD honesty).

    blender --background --python scripts/flora/bake_impostor.py -- --species oak --out assets/flora/oak

The far rung used to be two crossed quads over a sprite splatted on the CPU from the thinned
card deck — "from afar they look tragic" (the owner, 2026-09-03). Now the sprite IS the tree:
the reference (mature) variant's wood and its cluster cards, textured with the species' own
cluster sprites, rendered orthographically by Cycles from two azimuths (0°: the sprite plane
spans X; 90°: spans Y — the two the crossed quads show) under a uniform white world (albedo ×
occlusion, like every card) plus a camera-space normal pass. Two 512×1024 views side by side
in `impostor_color.png` / `impostor_normal.png`, and `impostor.json` with the world window
the quads must span (`half_width_m`, `top_m`) — shared constants, not tuning.
"""

import argparse
import json
import math
import os
import sys

import bpy
from mathutils import Euler, Vector

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import bake_tree  # noqa: E402

VIEW_W, VIEW_H = 512, 1024
SAMPLES = 96


def parse_args():
    argv = sys.argv[sys.argv.index("--") + 1 :] if "--" in sys.argv else []
    parser = argparse.ArgumentParser()
    parser.add_argument("--species", required=True, choices=sorted(bake_tree.SPECIES))
    parser.add_argument("--out", required=True)
    parser.add_argument("--samples", type=int, default=SAMPLES)
    return parser.parse_args(argv)


def to_blender(p):
    """Engine (x, y up, z) -> Blender (x, -z, y up)... the exporter maps (x, y, z)_b -> (x, z, -y)_e,
    so the inverse is (x, y, z)_e -> (x, -z, y)_b."""
    return Vector((p[0], -p[2], p[1]))


def scene_setup(samples):
    bpy.ops.wm.read_homefile(use_empty=True)
    scene = bpy.context.scene
    scene.render.engine = "CYCLES"
    scene.render.resolution_x = VIEW_W
    scene.render.resolution_y = VIEW_H
    scene.render.resolution_percentage = 100
    scene.render.film_transparent = True
    scene.render.image_settings.file_format = "PNG"
    scene.render.image_settings.color_mode = "RGBA"
    scene.render.image_settings.color_depth = "8"
    scene.view_settings.view_transform = "Standard"
    scene.view_settings.look = "None"
    try:
        prefs = bpy.context.preferences.addons["cycles"].preferences
        prefs.compute_device_type = "OPTIX"
        prefs.get_devices()
        for device in prefs.devices:
            device.use = device.type in ("OPTIX", "CUDA")
        scene.cycles.device = "GPU"
    except Exception as error:  # noqa: BLE001
        print("cycles: CPU fallback:", error)
        scene.cycles.device = "CPU"
    scene.cycles.samples = samples
    scene.cycles.use_denoising = True
    scene.cycles.transparent_max_bounces = 16
    world = bpy.data.worlds.new("impostor_world")
    world.use_nodes = True
    world.node_tree.nodes["Background"].inputs[0].default_value = (1.0, 1.0, 1.0, 1.0)
    world.node_tree.nodes["Background"].inputs[1].default_value = 1.0
    scene.world = world
    return scene


def wood_object(positions, normals, indices, bark_path):
    mesh = bpy.data.meshes.new("impostor_wood")
    verts = [to_blender(p) for p in positions]
    faces = [tuple(indices[i : i + 3]) for i in range(0, len(indices), 3)]
    mesh.from_pydata(verts, [], faces)
    mesh.validate(clean_customdata=False)
    mesh.update()
    obj = bpy.data.objects.new("impostor_wood", mesh)
    bpy.context.scene.collection.objects.link(obj)
    material = bpy.data.materials.new("impostor_bark")
    material.use_nodes = True
    nodes = material.node_tree.nodes
    links = material.node_tree.links
    principled = nodes["Principled BSDF"]
    principled.inputs["Roughness"].default_value = 0.9
    tex = nodes.new("ShaderNodeTexImage")
    tex.image = bpy.data.images.load(bark_path)
    tex.projection = "BOX"
    tex.projection_blend = 0.3
    coords = nodes.new("ShaderNodeTexCoord")
    mapping = nodes.new("ShaderNodeMapping")
    mapping.inputs["Scale"].default_value = (1.0, 1.0, 0.5)
    links.new(coords.outputs["Object"], mapping.inputs["Vector"])
    links.new(mapping.outputs["Vector"], tex.inputs["Vector"])
    links.new(tex.outputs["Color"], principled.inputs["Base Color"])
    obj.data.materials.append(material)
    for polygon in mesh.polygons:
        polygon.use_smooth = True
    return obj


def card_objects(cards, cluster_path):
    """Every cross-pair card as a double-sided plane cutting the species' cluster sprite."""
    image = bpy.data.images.load(cluster_path)
    material = bpy.data.materials.new("impostor_cards")
    material.use_nodes = True
    material.use_backface_culling = False
    nodes = material.node_tree.nodes
    links = material.node_tree.links
    principled = nodes["Principled BSDF"]
    principled.inputs["Roughness"].default_value = 0.65
    tex = nodes.new("ShaderNodeTexImage")
    tex.image = image
    tex.interpolation = "Linear"
    links.new(tex.outputs["Color"], principled.inputs["Base Color"])
    links.new(tex.outputs["Alpha"], principled.inputs["Alpha"])
    try:
        material.blend_method = "CLIP"
        material.alpha_threshold = 0.5
    except Exception:  # noqa: BLE001 - EEVEE-only settings on newer builds
        pass
    mesh = bpy.data.meshes.new("impostor_cards")
    verts, faces, uvs = [], [], []
    sprites = bake_tree.SPRITES
    for center, right, up, _normal, sprite in cards:
        c, r, u = to_blender(center), to_blender(right), to_blender(up)
        base = len(verts)
        verts += [c - r - u, c + r - u, c + r + u, c - r + u]
        faces.append((base, base + 1, base + 2, base + 3))
        u0, u1 = sprite / sprites, (sprite + 1) / sprites
        # The stem (-up) sits at the sprite's bottom (Blender v = 0).
        uvs += [(u0, 0.0), (u1, 0.0), (u1, 1.0), (u0, 1.0)]
    mesh.from_pydata(verts, [], faces)
    uv_layer = mesh.uv_layers.new(name="UVMap")
    for loop in mesh.loops:
        uv_layer.data[loop.index].uv = uvs[loop.vertex_index]
    mesh.update()
    obj = bpy.data.objects.new("impostor_cards", mesh)
    bpy.context.scene.collection.objects.link(obj)
    obj.data.materials.append(material)
    return obj


def read_tree(path):
    import struct

    with open(path, "rb") as handle:
        data = handle.read()
    assert data[:8] == b"WOTTREE1"
    at = 8
    (nverts,) = struct.unpack_from("<I", data, at)
    at += 4
    positions, normals = [], []
    for _ in range(nverts):
        x, y, z, nx, ny, nz = struct.unpack_from("<6f", data, at)
        at += 24
        positions.append((x, y, z))
        normals.append((nx, ny, nz))
    (nidx,) = struct.unpack_from("<I", data, at)
    at += 4
    indices = list(struct.unpack_from(f"<{nidx}I", data, at))
    at += 4 * nidx
    (ncards,) = struct.unpack_from("<I", data, at)
    at += 4
    cards = []
    for _ in range(ncards):
        vals = struct.unpack_from("<12f", data, at)
        at += 48
        (sprite,) = struct.unpack_from("<B", data, at)
        at += 1
        cards.append((vals[0:3], vals[3:6], vals[6:9], vals[9:12], sprite))
    return positions, normals, indices, cards


def normal_material():
    material = bpy.data.materials.new("normal_viz")
    material.use_nodes = True
    nodes = material.node_tree.nodes
    links = material.node_tree.links
    for node in list(nodes):
        nodes.remove(node)
    output = nodes.new("ShaderNodeOutputMaterial")
    emission = nodes.new("ShaderNodeEmission")
    geometry = nodes.new("ShaderNodeNewGeometry")
    transform = nodes.new("ShaderNodeVectorTransform")
    transform.vector_type = "NORMAL"
    transform.convert_from = "WORLD"
    transform.convert_to = "CAMERA"
    scale = nodes.new("ShaderNodeVectorMath")
    scale.operation = "MULTIPLY_ADD"
    scale.inputs[1].default_value = (0.5, 0.5, 0.5)
    scale.inputs[2].default_value = (0.5, 0.5, 0.5)
    links.new(geometry.outputs["Normal"], transform.inputs[0])
    links.new(transform.outputs[0], scale.inputs[0])
    links.new(scale.outputs[0], emission.inputs["Color"])
    links.new(emission.outputs[0], output.inputs["Surface"])
    return material


def normal_material_cards(cluster_path):
    """The cards' normal shader keeps the alpha cutout: transparent where the sprite is cut."""
    material = normal_material()
    nodes = material.node_tree.nodes
    links = material.node_tree.links
    output = next(n for n in nodes if n.type == "OUTPUT_MATERIAL")
    emission = next(n for n in nodes if n.type == "EMISSION")
    tex = nodes.new("ShaderNodeTexImage")
    tex.image = bpy.data.images.load(cluster_path)
    transparent = nodes.new("ShaderNodeBsdfTransparent")
    mix = nodes.new("ShaderNodeMixShader")
    links.new(tex.outputs["Alpha"], mix.inputs["Fac"])
    links.new(transparent.outputs[0], mix.inputs[1])
    links.new(emission.outputs[0], mix.inputs[2])
    links.new(mix.outputs[0], output.inputs["Surface"])
    return material


def render_view(scene, azimuth, width_px, top, path):
    # Square pixels at render time: the window is `top` tall (vertical sensor fit) and
    # `top * width_px / VIEW_H` wide; `tile_pair` squashes the view to VIEW_W afterwards.
    scene.render.resolution_x = width_px
    scene.render.resolution_y = VIEW_H
    camera_data = bpy.data.cameras.new("impostor_cam")
    camera_data.type = "ORTHO"
    camera_data.sensor_fit = "VERTICAL"
    camera_data.ortho_scale = top
    camera_data.clip_start = 0.01
    camera_data.clip_end = 400.0
    camera = bpy.data.objects.new("impostor_cam", camera_data)
    scene.collection.objects.link(camera)
    # Azimuth 0 looks along +Y from -Y (the sprite plane spans X); azimuth 1 looks along -X
    # from +X (the plane spans Y) — the two views `foliage::push_impostor_quads` shows.
    if azimuth == 0:
        camera.location = Vector((0.0, -120.0, top * 0.5))
        camera.rotation_euler = Euler((math.radians(90.0), 0.0, 0.0), "XYZ")
    else:
        camera.location = Vector((120.0, 0.0, top * 0.5))
        camera.rotation_euler = Euler((math.radians(90.0), 0.0, math.radians(90.0)), "XYZ")
    scene.camera = camera
    scene.render.filepath = path
    bpy.ops.render.render(write_still=True)
    bpy.data.objects.remove(camera)


def tile_pair(paths, out_path):
    page = bpy.data.images.new("impostor_page", VIEW_W * 2, VIEW_H, alpha=True)
    pixels = [0.0] * (VIEW_W * 2 * VIEW_H * 4)
    for index, path in enumerate(paths):
        image = bpy.data.images.load(path)
        # The view was rendered at the crown's own aspect; the atlas slot is 1:2, and the quad
        # spans the manifest window, so the squash is undone on the GPU.
        if image.size[0] != VIEW_W or image.size[1] != VIEW_H:
            image.scale(VIEW_W, VIEW_H)
        src = list(image.pixels)
        for y in range(VIEW_H):
            dst = (y * VIEW_W * 2 + index * VIEW_W) * 4
            srow = y * VIEW_W * 4
            pixels[dst : dst + VIEW_W * 4] = src[srow : srow + VIEW_W * 4]
        bpy.data.images.remove(image)
    page.pixels = pixels
    page.filepath_raw = out_path
    page.file_format = "PNG"
    page.save()


def main():
    args = parse_args()
    table = bake_tree.SPECIES[args.species]
    out = os.path.abspath(args.out)
    reference = 1  # the mature variant, the ladder's representative individual
    positions, normals, indices, cards = read_tree(os.path.join(out, f"v{reference}", "tree_near.bin"))
    bark_dir = {
        "oak": "jolcham_oak_bark_01", "poplar": "bark_brown_02", "willow": "bark_willow_02",
        "fruit": "sakura_bark", "pine": "pine_bark", "bush": "tree_bark_03",
    }[args.species]
    bark_path = os.path.join(os.path.dirname(out), "bark", bark_dir, "diff_1k.png")
    cluster_path = os.path.join(out, "clusters_color.png")
    top = max([p[1] for p in positions] + [c[0][1] + abs(c[1][1]) + abs(c[2][1]) for c in cards])
    reach = max(math.hypot(c[0][0], c[0][2]) + math.hypot(c[1][0], c[1][2]) + math.hypot(c[2][0], c[2][2]) for c in cards)
    reach = max(reach, max(math.hypot(p[0], p[2]) for p in positions))
    # The window: the WHOLE crown. The first bake derived the width from the height (a 1:2
    # sprite, `half_width = top / 4`) and clipped every broad crown flat at both sides — the
    # owner saw square trees at 300 m. The view now renders at the crown's own aspect
    # (square pixels, `width_px` wide) and is squashed into the 1:2 slot afterwards; the quad
    # spans `half_width_m` x `top_m`, so the tree keeps its shape and nothing is cut.
    top = top * 1.02
    width_px = max(64, int(math.ceil(VIEW_H * 2.0 * reach * 1.04 / top / 2.0)) * 2)
    half_width = top * width_px / (2.0 * VIEW_H)
    tmp = os.path.join(out, "sprites")
    os.makedirs(tmp, exist_ok=True)
    scene = scene_setup(args.samples)
    wood = wood_object(positions, normals, indices, bark_path)
    card_obj = card_objects(cards, cluster_path)
    colors = []
    for azimuth in (0, 1):
        path = os.path.join(tmp, f"impostor_{azimuth}_color.png")
        render_view(scene, azimuth, width_px, top, path)
        colors.append(path)
    # The normal pass.
    wood.data.materials.clear()
    wood.data.materials.append(normal_material())
    card_obj.data.materials.clear()
    card_obj.data.materials.append(normal_material_cards(cluster_path))
    scene.world.node_tree.nodes["Background"].inputs[0].default_value = (0.0, 0.0, 0.0, 1.0)
    scene.view_settings.view_transform = "Raw"
    scene.cycles.samples = 16
    scene.cycles.use_denoising = False
    normals_out = []
    for azimuth in (0, 1):
        path = os.path.join(tmp, f"impostor_{azimuth}_normal.png")
        render_view(scene, azimuth, width_px, top, path)
        normals_out.append(path)
    tile_pair(colors, os.path.join(out, "impostor_color.png"))
    tile_pair(normals_out, os.path.join(out, "impostor_normal.png"))
    manifest = {"species": args.species, "variant": reference, "half_width_m": round(half_width, 4),
                "top_m": round(top, 4), "view_px": [VIEW_W, VIEW_H], "views": ["azimuth 0: plane spans X", "azimuth 1: plane spans Z"],
                "convention": "colour = albedo x occlusion under a white world, sRGB; normal = camera-space raw; "
                              "the window is the quad `push_impostor_quads` spans: half_width_m across, 0..top_m up"}
    with open(os.path.join(out, "impostor.json"), "w", encoding="utf-8") as handle:
        json.dump(manifest, handle, indent=2)
    print("IMPOSTOR DONE", args.species, manifest["half_width_m"], manifest["top_m"], flush=True)


if __name__ == "__main__":
    main()
