"""Bake the oak's leaf-CLUSTER sprites in Blender (Inny Poziom F7c / route 2: trees as data).

Run headless:

    blender --background --python scripts/flora/bake_oak_clusters.py -- --out assets/flora/oak

Every sprite is a twig with a few dozen individual oak leaves, modelled here from a lobed
outline, cupped and jittered, rendered orthographically by Cycles under a uniform white world
(so the colour page stores ALBEDO x local occlusion - the engine's FOLIAGE path lights it live,
exactly as the procedural cards were stored) and once more with a camera-space normal shader
(the normal page). Eight variants tile into one 2048x1024 block: a 4x2 grid of 512x512 slots,
which `world_forge::tree::leaf_atlas` pastes into the bottom half of its page. The twig's base
sits at the bottom centre of every slot, because a card's stem hangs at -half_up (v1).

No third-party asset is used: leaf outline, twig and materials are authored in this script.
Deterministic per seed (Python's `random` seeded per sprite; Cycles seeded per sprite).
"""

import argparse
import json
import math
import os
import random
import sys

import bmesh
import bpy
from mathutils import Euler, Matrix, Vector

SPRITE_PX = 512
GRID_W, GRID_H = 4, 2
SPRITES = GRID_W * GRID_H
# The world window one sprite covers, metres: an oak cluster card in the engine is ~1.4 m.
WINDOW_M = 1.5
SAMPLES = 96


def parse_args():
    argv = sys.argv[sys.argv.index("--") + 1 :] if "--" in sys.argv else []
    parser = argparse.ArgumentParser()
    parser.add_argument("--out", required=True)
    parser.add_argument("--seed", type=int, default=1)
    parser.add_argument("--samples", type=int, default=SAMPLES)
    return parser.parse_args(argv)


def reset_scene():
    bpy.ops.wm.read_factory_settings(use_empty=True)
    scene = bpy.context.scene
    scene.render.engine = "CYCLES"
    scene.render.resolution_x = SPRITE_PX
    scene.render.resolution_y = SPRITE_PX
    scene.render.resolution_percentage = 100
    scene.render.film_transparent = True
    scene.render.image_settings.file_format = "PNG"
    scene.render.image_settings.color_mode = "RGBA"
    scene.render.image_settings.color_depth = "8"
    scene.render.image_settings.compression = 50
    scene.view_settings.view_transform = "Standard"
    scene.view_settings.look = "None"
    scene.view_settings.exposure = 0.0
    scene.view_settings.gamma = 1.0
    try:
        prefs = bpy.context.preferences.addons["cycles"].preferences
        prefs.compute_device_type = "OPTIX"
        prefs.get_devices()
        for device in prefs.devices:
            device.use = device.type in ("OPTIX", "CUDA")
        scene.cycles.device = "GPU"
    except Exception as error:  # noqa: BLE001 - CPU is the honest fallback
        print("cycles: GPU unavailable, CPU fallback:", error)
        scene.cycles.device = "CPU"
    scene.cycles.use_denoising = True
    scene.cycles.max_bounces = 4
    scene.cycles.diffuse_bounces = 2
    scene.cycles.glossy_bounces = 1
    scene.cycles.transparent_max_bounces = 8
    return scene


def uniform_white_world(strength=1.0, color=(1.0, 1.0, 1.0)):
    world = bpy.data.worlds.new("cluster_world")
    world.use_nodes = True
    background = world.node_tree.nodes["Background"]
    background.inputs[0].default_value = (*color, 1.0)
    background.inputs[1].default_value = strength
    bpy.context.scene.world = world
    return world


def ortho_camera():
    camera_data = bpy.data.cameras.new("cluster_cam")
    camera_data.type = "ORTHO"
    camera_data.ortho_scale = WINDOW_M
    camera_data.clip_start = 0.01
    camera_data.clip_end = 50.0
    camera = bpy.data.objects.new("cluster_cam", camera_data)
    bpy.context.scene.collection.objects.link(camera)
    # Looking down +Y from -Y: the sprite plane is XZ, +Z up, +X right (camera space x/y).
    camera.location = Vector((0.0, -6.0, 0.0))
    camera.rotation_euler = Euler((math.radians(90.0), 0.0, 0.0), "XYZ")
    bpy.context.scene.camera = camera
    return camera


def leaf_half_width(t):
    """The oak blade: a lobed bell, half-width as a fraction of length at `t` along the midrib."""
    bell = max(math.sin(math.pi * t), 0.12)
    return 0.30 * bell * (1.0 + 0.32 * math.sin(t * 14.5))


def leaf_mesh(name, length, rng):
    """One cupped oak leaf, midrib along +Y from the petiole at the origin, blade in the XY plane."""
    mesh = bpy.data.meshes.new(name)
    bm = bmesh.new()
    stations = 14
    cup = rng.uniform(0.10, 0.28)
    twist = rng.uniform(-0.15, 0.15)
    left, mid, right = [], [], []
    for i in range(stations + 1):
        t = i / stations
        y = t * length
        w = leaf_half_width(t) * length
        # The blade cups toward its midrib and droops toward the tip.
        droop = -0.12 * length * t * t
        z_edge = cup * w
        mid.append(bm.verts.new((0.0, y, droop)))
        left.append(bm.verts.new((-w, y, droop + z_edge + twist * w)))
        right.append(bm.verts.new((w, y, droop + z_edge - twist * w)))
    for i in range(stations):
        bm.faces.new((mid[i], left[i], left[i + 1], mid[i + 1]))
        bm.faces.new((right[i], mid[i], mid[i + 1], right[i + 1]))
    bm.normal_update()
    bm.to_mesh(mesh)
    bm.free()
    return mesh


def leaf_material(name, rng):
    material = bpy.data.materials.new(name)
    material.use_nodes = True
    nodes = material.node_tree.nodes
    links = material.node_tree.links
    principled = nodes["Principled BSDF"]
    # Albedo in LINEAR terms, the engine's canopy band (its authored dark oak tone sits near
    # (0.13, 0.27, 0.12)); every leaf jitters hue and value so a cluster is not one green.
    r = 0.11 + rng.uniform(-0.03, 0.05)
    g = 0.26 + rng.uniform(-0.05, 0.06)
    b = 0.09 + rng.uniform(-0.02, 0.03)
    principled.inputs["Base Color"].default_value = (r, g, b, 1.0)
    principled.inputs["Roughness"].default_value = 0.62
    try:
        principled.inputs["Specular IOR Level"].default_value = 0.25
    except KeyError:
        pass
    # Veins as a bump: a fine wave texture along the midrib, feathered by noise.
    wave = nodes.new("ShaderNodeTexWave")
    wave.wave_type = "BANDS"
    wave.bands_direction = "Y"
    wave.inputs["Scale"].default_value = 60.0
    wave.inputs["Distortion"].default_value = 1.2
    bump = nodes.new("ShaderNodeBump")
    bump.inputs["Strength"].default_value = 0.25
    bump.inputs["Distance"].default_value = 0.002
    links.new(wave.outputs["Fac"], bump.inputs["Height"])
    links.new(bump.outputs["Normal"], principled.inputs["Normal"])
    material.blend_method = "OPAQUE" if hasattr(material, "blend_method") else None
    return material


def bark_material():
    material = bpy.data.materials.new("twig_bark")
    material.use_nodes = True
    principled = material.node_tree.nodes["Principled BSDF"]
    principled.inputs["Base Color"].default_value = (0.13, 0.09, 0.05, 1.0)
    principled.inputs["Roughness"].default_value = 0.9
    return material


def twig_curve(points, radius, name):
    """A tapered twig along `points` (world metres) as a bevelled curve object."""
    curve = bpy.data.curves.new(name, "CURVE")
    curve.dimensions = "3D"
    curve.bevel_depth = radius
    curve.bevel_resolution = 3
    curve.fill_mode = "FULL"
    spline = curve.splines.new("BEZIER")
    spline.bezier_points.add(len(points) - 1)
    for i, point in enumerate(points):
        bp = spline.bezier_points[i]
        bp.co = point
        bp.handle_left_type = "AUTO"
        bp.handle_right_type = "AUTO"
        bp.radius = 1.0 - 0.75 * i / max(len(points) - 1, 1)
    obj = bpy.data.objects.new(name, curve)
    bpy.context.scene.collection.objects.link(obj)
    return obj, spline


def spline_frame(points, t):
    """Position and tangent at fraction `t` along a polyline of `points`."""
    n = len(points) - 1
    f = min(max(t * n, 0.0), n - 1e-6)
    i = int(f)
    u = f - i
    p = points[i].lerp(points[i + 1], u)
    tangent = (points[i + 1] - points[i]).normalized()
    return p, tangent


def build_cluster(seed, leaf_count, bark, rng):
    """Twig + side twigs + leaves. Base at (0, 0, -WINDOW/2 + margin), growing up and out."""
    # A DENSE cluster (the first bake covered 6-12 % of its slot and read as a sparse sprig
    # at card range): a short bare stem, side twigs from low on the main, leaves along every
    # twig from a third of the way up, overlapping.
    base = Vector((0.0, 0.0, -WINDOW_M * 0.5 + 0.04))
    lean = rng.uniform(-0.25, 0.25)
    main = [
        base,
        base + Vector((lean * 0.3, 0.0, 0.40)),
        base + Vector((lean * 0.8, rng.uniform(-0.08, 0.08), 0.85)),
        base + Vector((lean * 1.4, rng.uniform(-0.1, 0.1), 1.30)),
    ]
    main_obj, _ = twig_curve(main, 0.014, f"twig_main_{seed}")
    main_obj.data.materials.append(bark)
    twigs = [main]
    for side in range(rng.randint(13, 16)):
        t0 = rng.uniform(0.12, 0.9)
        p, tangent = spline_frame(main, t0)
        yaw = rng.uniform(0.0, 2 * math.pi)
        out = Vector((math.cos(yaw), math.sin(yaw) * 0.35, 0.0)).normalized()
        direction = (out * 0.8 + Vector((0.0, 0.0, 0.45)) + tangent * 0.15).normalized()
        length = rng.uniform(0.35, 0.62)
        pts = [p, p + direction * length * 0.5 + Vector((0, 0, 0.03)), p + direction * length]
        obj, _ = twig_curve(pts, 0.007, f"twig_side_{seed}_{side}")
        obj.data.materials.append(bark)
        twigs.append(pts)

    leaves = []
    for i in range(leaf_count):
        twig = twigs[rng.randrange(len(twigs))]
        t = rng.uniform(0.1, 1.0) if twig is not main else rng.uniform(0.3, 1.0)
        p, tangent = spline_frame(twig, t)
        length = rng.uniform(0.15, 0.23)
        mesh = leaf_mesh(f"leaf_{seed}_{i}", length, rng)
        leaf = bpy.data.objects.new(f"leaf_{seed}_{i}", mesh)
        leaf.data.materials.append(leaf_material(f"leaf_mat_{seed}_{i}", rng))
        bpy.context.scene.collection.objects.link(leaf)
        # Alternate phyllotaxis: leaves fan out around the twig, tilted toward the light
        # (+Z, and toward the camera at -Y so the sprite reads as a lit cluster face).
        side = 1.0 if i % 2 == 0 else -1.0
        around = rng.uniform(0.0, 2 * math.pi)
        radial = Vector((math.cos(around), math.sin(around), 0.0))
        forward = (tangent * 0.45 + radial * side * 0.7 + Vector((0.0, -0.35, 0.25))).normalized()
        up_hint = Vector((0.0, -1.0, 0.6)).normalized()
        right = forward.cross(up_hint).normalized()
        normal = right.cross(forward).normalized()
        rotation = Matrix((right, forward, normal)).transposed().to_4x4()
        leaf.matrix_world = Matrix.Translation(p + normal * 0.004) @ rotation
        leaves.append(leaf)
    return leaves


def render(path):
    bpy.context.scene.render.filepath = path
    bpy.ops.render.render(write_still=True)


def normal_shader_material():
    """Camera-space normal as emission: (n * 0.5 + 0.5), the dome convention of the atlas."""
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
    emission.inputs["Strength"].default_value = 1.0
    links.new(emission.outputs[0], output.inputs["Surface"])
    return material


def bake_sprite(index, seed, out_dir, samples):
    scene = reset_scene()
    scene.cycles.samples = samples
    scene.cycles.seed = seed
    uniform_white_world()
    ortho_camera()
    rng = random.Random(seed)
    bark = bark_material()
    leaf_count = rng.randint(380, 460)
    leaves = build_cluster(seed, leaf_count, bark, rng)
    color_path = os.path.join(out_dir, f"sprite_{index}_color.png")
    render(color_path)

    # The normal pass: every surface an emission of its camera-space normal, black world, raw.
    normal_material = normal_shader_material()
    for obj in bpy.context.scene.objects:
        if obj.type in ("MESH", "CURVE"):
            obj.data.materials.clear()
            obj.data.materials.append(normal_material)
    scene.world.node_tree.nodes["Background"].inputs[0].default_value = (0.0, 0.0, 0.0, 1.0)
    scene.view_settings.view_transform = "Raw"
    scene.cycles.samples = 16
    scene.cycles.use_denoising = False
    normal_path = os.path.join(out_dir, f"sprite_{index}_normal.png")
    render(normal_path)
    return {"index": index, "seed": seed, "leaves": leaf_count}


def tile(out_dir, kind, sprite_paths):
    """Tile the eight sprites into one 2048x1024 RGBA block (row-major, row 0 at the TOP)."""
    width, height = GRID_W * SPRITE_PX, GRID_H * SPRITE_PX
    page = bpy.data.images.new(f"page_{kind}", width, height, alpha=True)
    pixels = [0.0] * (width * height * 4)
    for index, path in enumerate(sprite_paths):
        image = bpy.data.images.load(path)
        src = list(image.pixels)
        gx, gy = index % GRID_W, index // GRID_W
        # Blender images are bottom-up; the atlas is top-down, so row 0 goes to the top.
        for y in range(SPRITE_PX):
            dst_y = height - 1 - (gy * SPRITE_PX + y)
            src_row = (SPRITE_PX - 1 - y) * SPRITE_PX * 4
            dst = (dst_y * width + gx * SPRITE_PX) * 4
            pixels[dst : dst + SPRITE_PX * 4] = src[src_row : src_row + SPRITE_PX * 4]
        bpy.data.images.remove(image)
    page.pixels = pixels
    page.filepath_raw = os.path.join(out_dir, f"clusters_{kind}.png")
    page.file_format = "PNG"
    page.save()
    return page.filepath_raw


def main():
    args = parse_args()
    out_dir = os.path.abspath(args.out)
    os.makedirs(out_dir, exist_ok=True)
    tmp = os.path.join(out_dir, "sprites")
    os.makedirs(tmp, exist_ok=True)
    records = []
    for index in range(SPRITES):
        records.append(bake_sprite(index, args.seed * 1000 + index, tmp, args.samples))
        print("sprite", index, "done", flush=True)
    color = tile(out_dir, "color", [os.path.join(tmp, f"sprite_{i}_color.png") for i in range(SPRITES)])
    normal = tile(out_dir, "normal", [os.path.join(tmp, f"sprite_{i}_normal.png") for i in range(SPRITES)])
    manifest = {
        "species": "oak",
        "kind": "leaf clusters",
        "authoring": "in-house, Blender " + bpy.app.version_string + ", scripts/flora/bake_oak_clusters.py",
        "license": "project-owned (no third-party assets)",
        "sprite_px": SPRITE_PX,
        "grid": [GRID_W, GRID_H],
        "window_m": WINDOW_M,
        "seed": args.seed,
        "samples": args.samples,
        "sprites": records,
        "color": os.path.basename(color),
        "normal": os.path.basename(normal),
        "convention": "colour = albedo x local occlusion under a uniform white world, sRGB; "
        "normal = camera-space (n*0.5+0.5), raw; twig base at bottom centre of each slot",
    }
    with open(os.path.join(out_dir, "clusters.json"), "w", encoding="utf-8") as handle:
        json.dump(manifest, handle, indent=2)
    print("BAKE DONE", color, normal, flush=True)


if __name__ == "__main__":
    main()
