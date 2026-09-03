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

VIEW_W, VIEW_H = 256, 512
VARIANTS = 4
WOOD_THICKEN_M = 0.10
# The cards' normal pass carries the CROWN normal the engine lights its cards with
# (`authored::bent_card_normal`: outward from the crown centroid, lifted, 75/25 with the quad's
# facing) — so the impostor and the Mid deck answer the sun with the same normal field.
CROWN_NORMAL_LIFT = 0.3
CROWN_NORMAL_OUTWARD = 0.75
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
    scene.cycles.transparent_max_bounces = 128  # a crown is hundreds of cut-out cards deep; 8 made it a solid lid
    world = bpy.data.worlds.new("impostor_world")
    world.use_nodes = True
    world.node_tree.nodes["Background"].inputs[0].default_value = (1.0, 1.0, 1.0, 1.0)
    world.node_tree.nodes["Background"].inputs[1].default_value = 1.0
    scene.world = world
    return scene


def wood_object(positions, normals, indices, bark_path, wood_crown_base=0.0):
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
    # The far wood must SURVIVE the alpha test at 300 m and beyond (the owner, 2026-09-03: "the
    # tree itself is practically invisible from afar, only the leaves"): a limb of 0.14 m is
    # half a pixel there and the cutout drops it. Every wood surface is pushed out along its
    # normal by WOOD_THICKEN_M — a trunk of 0.5 m becomes 0.7 m (three pixels at 300 m), a limb
    # of 0.14 m becomes 0.34 m (one and a half). Invisible as an error at that range, decisive
    # for the silhouette.
    # Per vertex, along the smooth normal: the full amount below the crown (the trunk), a
    # third of it inside the crown (limbs and twigs — a twig thickened by 0.10 m would be a
    # limb, and the far crown would fill with wood; measured: 7x the wood pixels).
    crown_base = wood_crown_base
    for vertex in mesh.vertices:
        amount = WOOD_THICKEN_M if vertex.co.z < crown_base else WOOD_THICKEN_M * 0.3
        vertex.co = vertex.co + vertex.normal * amount
    mesh.update()
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


def normal_material_cards(cluster_path, centroid, reach):
    """The cards' normal shader keeps the alpha cutout: transparent where the sprite is cut,
    and writes the CROWN normal (see CROWN_NORMAL_*), not the quad's plane."""
    material = normal_material()
    nodes = material.node_tree.nodes
    links = material.node_tree.links
    output = next(n for n in nodes if n.type == "OUTPUT_MATERIAL")
    emission = next(n for n in nodes if n.type == "EMISSION")
    geometry = next(n for n in nodes if n.type == "NEW_GEOMETRY")
    transform = next(n for n in nodes if n.type == "VECT_TRANSFORM")
    # outward = normalize((position - centroid) / reach + (0, 0, lift)); crown = normalize(
    # outward * 0.75 + geometric * 0.25)
    sub = nodes.new("ShaderNodeVectorMath")
    sub.operation = "SUBTRACT"
    sub.inputs[1].default_value = (centroid.x, centroid.y, centroid.z)
    links.new(geometry.outputs["Position"], sub.inputs[0])
    scale = nodes.new("ShaderNodeVectorMath")
    scale.operation = "SCALE"
    scale.inputs["Scale"].default_value = 1.0 / max(reach, 0.01)
    links.new(sub.outputs[0], scale.inputs[0])
    lift = nodes.new("ShaderNodeVectorMath")
    lift.operation = "ADD"
    lift.inputs[1].default_value = (0.0, 0.0, CROWN_NORMAL_LIFT)
    links.new(scale.outputs[0], lift.inputs[0])
    outward = nodes.new("ShaderNodeVectorMath")
    outward.operation = "NORMALIZE"
    links.new(lift.outputs[0], outward.inputs[0])
    mixed = nodes.new("ShaderNodeMix")
    mixed.data_type = "VECTOR"
    mixed.inputs["Factor"].default_value = 1.0 - CROWN_NORMAL_OUTWARD
    links.new(outward.outputs[0], mixed.inputs[4])
    links.new(geometry.outputs["Normal"], mixed.inputs[5])
    crown = nodes.new("ShaderNodeVectorMath")
    crown.operation = "NORMALIZE"
    links.new(mixed.outputs[1], crown.inputs[0])
    links.new(crown.outputs[0], transform.inputs[0])
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


def tile_page(paths, out_path):
    """Tile the views into one row: slot k = variant * 2 + azimuth, 256 x 512 each."""
    slots = len(paths)
    page = bpy.data.images.new("impostor_page", VIEW_W * slots, VIEW_H, alpha=True)
    pixels = [0.0] * (VIEW_W * slots * VIEW_H * 4)
    for index, path in enumerate(paths):
        image = bpy.data.images.load(path)
        # The view was rendered at the crown's own aspect; the atlas slot is 1:2, and the quad
        # spans the manifest window, so the squash is undone on the GPU.
        if image.size[0] != VIEW_W or image.size[1] != VIEW_H:
            image.scale(VIEW_W, VIEW_H)
        src = list(image.pixels)
        for y in range(VIEW_H):
            dst = (y * VIEW_W * slots + index * VIEW_W) * 4
            srow = y * VIEW_W * 4
            pixels[dst : dst + VIEW_W * 4] = src[srow : srow + VIEW_W * 4]
        bpy.data.images.remove(image)
    page.pixels = pixels
    page.filepath_raw = out_path
    page.file_format = "PNG"
    page.save()


def bake_variant(args, out, variant, bark_path, cluster_path, tmp):
    """One variant's two views, colour and normal: returns (colour paths, normal paths, window)."""
    positions, normals, indices, cards = read_tree(os.path.join(out, f"v{variant}", "tree_near.bin"))
    top = max([p[1] for p in positions] + [c[0][1] + abs(c[1][1]) + abs(c[2][1]) for c in cards])
    reach = max(math.hypot(c[0][0], c[0][2]) + math.hypot(c[1][0], c[1][2]) + math.hypot(c[2][0], c[2][2]) for c in cards)
    reach = max(reach, max(math.hypot(p[0], p[2]) for p in positions))
    # The window: the WHOLE crown. The first bake derived the width from the height (a 1:2
    # sprite, `half_width = top / 4`) and clipped every broad crown flat at both sides — the
    # owner saw square trees at 300 m. The view renders at the crown's own aspect (square
    # pixels, `width_px` wide) and is squashed into the 1:2 slot afterwards; the quad spans
    # `half_width_m` x `top_m`, so the tree keeps its shape and nothing is cut.
    top = top * 1.02
    width_px = max(32, int(math.ceil(VIEW_H * 2.0 * reach * 1.04 / top / 2.0)) * 2)
    half_width = top * width_px / (2.0 * VIEW_H)
    scene = scene_setup(args.samples)
    crown_base = min(to_blender(c[0]).z for c in cards) if cards else 0.0
    wood = wood_object(positions, normals, indices, bark_path, crown_base)
    card_obj = card_objects(cards, cluster_path)
    colors = []
    for azimuth in (0, 1):
        path = os.path.join(tmp, f"impostor_v{variant}_{azimuth}_color.png")
        render_view(scene, azimuth, width_px, top, path)
        colors.append(path)
    # The normal pass.
    wood.data.materials.clear()
    wood.data.materials.append(normal_material())
    card_obj.data.materials.clear()
    centroid = sum((to_blender(c[0]) for c in cards), Vector((0.0, 0.0, 0.0))) / max(len(cards), 1)
    card_reach = max((to_blender(c[0]) - centroid).length for c in cards) if cards else 1.0
    card_obj.data.materials.append(normal_material_cards(cluster_path, centroid, card_reach))
    scene.world.node_tree.nodes["Background"].inputs[0].default_value = (0.0, 0.0, 0.0, 1.0)
    scene.view_settings.view_transform = "Raw"
    scene.cycles.samples = 16
    scene.cycles.use_denoising = False
    normals_out = []
    for azimuth in (0, 1):
        path = os.path.join(tmp, f"impostor_v{variant}_{azimuth}_normal.png")
        render_view(scene, azimuth, width_px, top, path)
        normals_out.append(path)
    return colors, normals_out, {"variant": variant, "half_width_m": round(half_width, 4), "top_m": round(top, 4)}


def main():
    args = parse_args()
    out = os.path.abspath(args.out)
    bark_dir = {
        "oak": "jolcham_oak_bark_01", "poplar": "bark_brown_02", "willow": "bark_willow_02",
        "fruit": "sakura_bark", "pine": "pine_bark", "bush": "tree_bark_03",
    }[args.species]
    bark_path = os.path.join(os.path.dirname(out), "bark", bark_dir, "diff_1k.png")
    cluster_path = os.path.join(out, "clusters_color.png")
    tmp = os.path.join(out, "sprites")
    os.makedirs(tmp, exist_ok=True)
    # One impostor PER VARIANT (LOD continuity, 2026-09-03): the ladder's Near and Mid rungs
    # are the variant's own tree, so its far sprite must be too — a young oak that became the
    # mature one at 300 m was "a tree changing its graphics" (measured: 1.9x the crown).
    colors, normals_out, windows = [], [], []
    for variant in range(VARIANTS):
        c, n, window = bake_variant(args, out, variant, bark_path, cluster_path, tmp)
        colors += c
        normals_out += n
        windows.append(window)
        print("variant", variant, "done", window, flush=True)
    tile_page(colors, os.path.join(out, "impostor_color.png"))
    tile_page(normals_out, os.path.join(out, "impostor_normal.png"))
    reference = windows[1]
    manifest = {"species": args.species, "variant": 1, "half_width_m": reference["half_width_m"],
                "top_m": reference["top_m"], "view_px": [VIEW_W, VIEW_H], "views": ["azimuth 0: plane spans X", "azimuth 1: plane spans Z"],
                "variants": windows,
                "convention": "colour = albedo x occlusion under a white world, sRGB; normal = camera-space raw (crown normals); "
                              "slot = variant * 2 + azimuth, 256 x 512 each; the window per variant is the quad "
                              "`push_impostor_quad` spans: half_width_m across, 0..top_m up"}
    with open(os.path.join(out, "impostor.json"), "w", encoding="utf-8") as handle:
        json.dump(manifest, handle, indent=2)
    print("IMPOSTOR DONE", args.species, [(w["half_width_m"], w["top_m"]) for w in windows], flush=True)


if __name__ == "__main__":
    main()
