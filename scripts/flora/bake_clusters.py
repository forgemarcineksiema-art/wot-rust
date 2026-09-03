"""Bake a species' leaf-CLUSTER sprites in Blender (route 2: trees as data). Every species.

Run headless, one species per run:

    blender --background --python scripts/flora/bake_clusters.py -- --species oak --out assets/flora/oak

A sprite is a twig with a few hundred individual leaves (or needle fascicles for the pine),
modelled here from a species outline, cupped and jittered, rendered orthographically by Cycles
under a uniform white world (the colour page stores ALBEDO x local occlusion; the engine's
FOLIAGE path lights it live) and once more with a camera-space normal shader (the normal
page). Four variants tile into one 2048x512 row — a species' block in the foliage atlas
(`world_forge::tree::leaf_atlas`); mirrored cards double the variety for free. The twig's
base sits at the bottom centre of every slot (a card's stem hangs at -half_up), except for a
HANGING species (the willow), whose twig hangs from the top centre and whose cards point
their stem up.

No third-party asset is used: outlines, twigs and materials are authored in this script.
Deterministic per (species, seed).
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
GRID_W, GRID_H = 4, 1
SPRITES = GRID_W * GRID_H
SAMPLES = 96

# ------------------------------------------------------------------------------ the table
# window_m: the world metres one sprite spans (a card of that size shows the whole sprite).
# leaves: count per sprite; leaf_m: blade length band; twigs: side twigs; colour: linear albedo
# base and jitter; hanging: the twig hangs from the top (willow curtains); needles: the pine.
SPECIES = {
    "oak": dict(window_m=1.5, leaves=(380, 460), leaf_m=(0.15, 0.23), twigs=(13, 16),
                albedo=(0.11, 0.26, 0.09), jitter=(0.04, 0.06, 0.03), cup=(0.10, 0.28),
                outline="oak", hanging=False, needles=False, twig_len=(0.35, 0.62), aspect=1.0),
    "poplar": dict(window_m=1.3, leaves=(460, 540), leaf_m=(0.09, 0.13), twigs=(15, 18),
                   albedo=(0.15, 0.30, 0.08), jitter=(0.04, 0.05, 0.03), cup=(0.02, 0.10),
                   outline="deltoid", hanging=False, needles=False, twig_len=(0.35, 0.6), aspect=1.0),
    # The willow is a CURTAIN: a 2.6 m window of long parallel streamers, leaves close along
    # them — the card hangs the whole window from its twig (the owner, 2026-09-03: the short
    # brushes read as "retarded").
    "willow": dict(window_m=2.6, leaves=(1000, 1200), leaf_m=(0.13, 0.19), twigs=(9, 12),
                   albedo=(0.19, 0.32, 0.14), jitter=(0.04, 0.05, 0.03), cup=(0.02, 0.08),
                   outline="lanceolate", hanging=True, needles=False, twig_len=(1.6, 2.3), aspect=0.28),
    "fruit": dict(window_m=1.1, leaves=(340, 420), leaf_m=(0.06, 0.09), twigs=(12, 15),
                  albedo=(0.12, 0.28, 0.10), jitter=(0.04, 0.05, 0.03), cup=(0.05, 0.15),
                  outline="oval", hanging=False, needles=False, twig_len=(0.3, 0.5), aspect=1.0),
    "pine": dict(window_m=1.2, leaves=(280, 340), leaf_m=(0.08, 0.11), twigs=(10, 13),
                 albedo=(0.07, 0.17, 0.10), jitter=(0.02, 0.04, 0.03), cup=(0.0, 0.0),
                 outline="needle", hanging=False, needles=True, twig_len=(0.3, 0.5), aspect=1.0),
    "bush": dict(window_m=0.9, leaves=(420, 520), leaf_m=(0.035, 0.055), twigs=(14, 18),
                 albedo=(0.10, 0.22, 0.08), jitter=(0.03, 0.05, 0.03), cup=(0.05, 0.15),
                 outline="oval", hanging=False, needles=False, twig_len=(0.25, 0.45), aspect=1.0),
}


def parse_args():
    argv = sys.argv[sys.argv.index("--") + 1 :] if "--" in sys.argv else []
    parser = argparse.ArgumentParser()
    parser.add_argument("--species", required=True, choices=sorted(SPECIES))
    parser.add_argument("--out", required=True)
    parser.add_argument("--seed", type=int, default=1)
    parser.add_argument("--samples", type=int, default=SAMPLES)
    return parser.parse_args(argv)


def reset_scene():
    bpy.ops.wm.read_homefile(use_empty=True)
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


def ortho_camera(window_m):
    camera_data = bpy.data.cameras.new("cluster_cam")
    camera_data.type = "ORTHO"
    camera_data.ortho_scale = window_m
    camera_data.clip_start = 0.01
    camera_data.clip_end = 50.0
    camera = bpy.data.objects.new("cluster_cam", camera_data)
    bpy.context.scene.collection.objects.link(camera)
    # Looking down +Y from -Y: the sprite plane is XZ, +Z up, +X right (camera space x/y).
    camera.location = Vector((0.0, -6.0, 0.0))
    camera.rotation_euler = Euler((math.radians(90.0), 0.0, 0.0), "XYZ")
    bpy.context.scene.camera = camera
    return camera


def half_width(outline, t):
    """The blade's half-width as a fraction of its length at `t` along the midrib."""
    bell = max(math.sin(math.pi * t), 0.12)
    if outline == "oak":
        return 0.30 * bell * (1.0 + 0.32 * math.sin(t * 14.5))
    if outline == "deltoid":
        # Broad, rounded shoulders low on the blade, a drip tip: a poplar, not a paper dart.
        shoulder = math.sin(math.pi * min(t / 0.45, 1.0) * 0.5) if t < 0.45 else 1.0
        return 0.40 * shoulder * (1.0 - t) ** 0.85 * (1.0 + 0.05 * math.sin(t * 62.0)) + 0.02
    if outline == "lanceolate":
        return 0.10 * bell ** 0.7
    if outline == "oval":
        return 0.28 * math.sin(math.pi * t ** 0.9) * (1.0 + 0.05 * math.sin(t * 44.0)) + 0.01
    if outline == "needle":
        return 0.05
    return 0.25 * bell


def leaf_mesh(name, outline, length, cup, rng):
    """One cupped blade, midrib along +Y from the petiole at the origin, blade in the XY plane."""
    mesh = bpy.data.meshes.new(name)
    bm = bmesh.new()
    stations = 6 if outline == "needle" else 14
    cup_amount = rng.uniform(*cup)
    twist = rng.uniform(-0.15, 0.15)
    left, mid, right = [], [], []
    for i in range(stations + 1):
        t = i / stations
        y = t * length
        w = max(half_width(outline, t) * length, 0.0005)
        droop = -0.12 * length * t * t
        z_edge = cup_amount * w
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


def leaf_material(name, spec, rng):
    material = bpy.data.materials.new(name)
    material.use_nodes = True
    nodes = material.node_tree.nodes
    links = material.node_tree.links
    principled = nodes["Principled BSDF"]
    base = spec["albedo"]
    jit = spec["jitter"]
    r = base[0] + rng.uniform(-jit[0], jit[0] * 1.3)
    g = base[1] + rng.uniform(-jit[1], jit[1] * 1.2)
    b = base[2] + rng.uniform(-jit[2], jit[2])
    principled.inputs["Base Color"].default_value = (max(r, 0.01), max(g, 0.02), max(b, 0.01), 1.0)
    principled.inputs["Roughness"].default_value = 0.62
    try:
        principled.inputs["Specular IOR Level"].default_value = 0.25
    except KeyError:
        pass
    if not spec["needles"]:
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
    return material


def bark_material():
    material = bpy.data.materials.new("twig_bark")
    material.use_nodes = True
    principled = material.node_tree.nodes["Principled BSDF"]
    principled.inputs["Base Color"].default_value = (0.13, 0.09, 0.05, 1.0)
    principled.inputs["Roughness"].default_value = 0.9
    return material


def twig_curve(points, radius, name):
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
    n = len(points) - 1
    f = min(max(t * n, 0.0), n - 1e-6)
    i = int(f)
    u = f - i
    p = points[i].lerp(points[i + 1], u)
    tangent = (points[i + 1] - points[i]).normalized()
    return p, tangent


def build_cluster(spec, seed, bark, rng):
    """Twig + side twigs + leaves. Upright: base at the bottom centre, growing up and out.
    Hanging: base at the top centre, streamers falling."""
    # A HANGING species is drawn upright too (base at the bottom, streamers rising nearly
    # parallel to the main twig): the tree exporter points the card's half_up DOWN, which turns
    # the sprite over in the world — base at the twig, streamers falling.
    window = spec["window_m"]
    up_sign = 1.0
    base = Vector((0.0, 0.0, -up_sign * (window * 0.5 - 0.04)))
    lean = rng.uniform(-0.25, 0.25)
    reach = window * 0.87
    main = [
        base,
        base + Vector((lean * 0.3, 0.0, up_sign * reach * 0.30)),
        base + Vector((lean * 0.8, rng.uniform(-0.08, 0.08), up_sign * reach * 0.65)),
        base + Vector((lean * 1.4, rng.uniform(-0.1, 0.1), up_sign * reach)),
    ]
    main_obj, _ = twig_curve(main, 0.012 * window / 1.5, f"twig_main_{seed}")
    main_obj.data.materials.append(bark)
    twigs = [main]
    for side in range(rng.randint(*spec["twigs"])):
        t0 = rng.uniform(0.12, 0.9)
        p, tangent = spline_frame(main, t0)
        yaw = rng.uniform(0.0, 2 * math.pi)
        out = Vector((math.cos(yaw), math.sin(yaw) * 0.35, 0.0)).normalized()
        if spec["hanging"]:
            # Streamers: nearly parallel to the main, fanned a little, the whole window long.
            # A curtain hangs nearly straight: the streamers barely fan.
            direction = (out * 0.12 + Vector((0.0, 0.0, 0.99))).normalized()
        else:
            direction = (out * 0.8 + Vector((0.0, 0.0, 0.45)) + tangent * 0.15).normalized()
        length = rng.uniform(*spec["twig_len"]) * window / 1.5
        pts = [p, p + direction * length * 0.5 + Vector((0, 0, 0.03 * up_sign)), p + direction * length]
        obj, _ = twig_curve(pts, 0.006 * window / 1.5, f"twig_side_{seed}_{side}")
        obj.data.materials.append(bark)
        twigs.append(pts)

    leaves = []
    count = rng.randint(*spec["leaves"])
    for i in range(count):
        twig = twigs[rng.randrange(len(twigs))]
        t = rng.uniform(0.1, 1.0) if twig is not main else rng.uniform(0.3, 1.0)
        p, tangent = spline_frame(twig, t)
        length = rng.uniform(*spec["leaf_m"])
        if spec["needles"]:
            # A fascicle: five needles fanned around the shoot.
            blades = 5
        else:
            blades = 1
        for blade in range(blades):
            mesh = leaf_mesh(f"leaf_{seed}_{i}_{blade}", spec["outline"], length, spec["cup"], rng)
            leaf = bpy.data.objects.new(f"leaf_{seed}_{i}_{blade}", mesh)
            leaf.data.materials.append(leaf_material(f"leaf_mat_{seed}_{i}_{blade}", spec, rng))
            bpy.context.scene.collection.objects.link(leaf)
            side = 1.0 if (i + blade) % 2 == 0 else -1.0
            around = rng.uniform(0.0, 2 * math.pi)
            radial = Vector((math.cos(around), math.sin(around), 0.0))
            if spec["needles"]:
                spread = Vector((math.cos(around + blade * 1.25), math.sin(around + blade * 1.25), 0.0))
                forward = (tangent * 0.55 + spread * 0.75 + Vector((0.0, -0.25, 0.1))).normalized()
            elif spec["hanging"]:
                # Leaves lie ALONG the streamer, alternating sides, hugging it.
                forward = (tangent * 0.85 + radial * side * 0.25 + Vector((0.0, -0.2, 0.0))).normalized()
            else:
                forward = (tangent * 0.45 + radial * side * 0.7 + Vector((0.0, -0.35, 0.25))).normalized()
            up_hint = Vector((0.0, -1.0, 0.6)).normalized()
            right = forward.cross(up_hint).normalized()
            normal = right.cross(forward).normalized()
            rotation = Matrix((right, forward, normal)).transposed().to_4x4()
            leaf.matrix_world = Matrix.Translation(p + normal * 0.004) @ rotation
            leaves.append(leaf)
    return leaves, count


def render(path):
    bpy.context.scene.render.filepath = path
    bpy.ops.render.render(write_still=True)


def normal_shader_material():
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


def bake_sprite(spec, index, seed, out_dir, samples):
    scene = reset_scene()
    scene.cycles.samples = samples
    scene.cycles.seed = seed
    uniform_white_world()
    ortho_camera(spec["window_m"])
    rng = random.Random(seed)
    bark = bark_material()
    _leaves, count = build_cluster(spec, seed, bark, rng)
    color_path = os.path.join(out_dir, f"sprite_{index}_color.png")
    render(color_path)
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
    return {"index": index, "seed": seed, "leaves": count}


def tile(out_dir, kind, sprite_paths):
    """Tile the sprites into one row (row 0 at the TOP)."""
    width, height = GRID_W * SPRITE_PX, GRID_H * SPRITE_PX
    page = bpy.data.images.new(f"page_{kind}", width, height, alpha=True)
    pixels = [0.0] * (width * height * 4)
    for index, path in enumerate(sprite_paths):
        image = bpy.data.images.load(path)
        src = list(image.pixels)
        gx, gy = index % GRID_W, index // GRID_W
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
    spec = SPECIES[args.species]
    out_dir = os.path.abspath(args.out)
    os.makedirs(out_dir, exist_ok=True)
    tmp = os.path.join(out_dir, "sprites")
    os.makedirs(tmp, exist_ok=True)
    records = []
    species_salt = sum(ord(c) for c in args.species) * 101
    for index in range(SPRITES):
        records.append(bake_sprite(spec, index, args.seed * 1000 + species_salt + index, tmp, args.samples))
        print("sprite", index, "done", flush=True)
    color = tile(out_dir, "color", [os.path.join(tmp, f"sprite_{i}_color.png") for i in range(SPRITES)])
    normal = tile(out_dir, "normal", [os.path.join(tmp, f"sprite_{i}_normal.png") for i in range(SPRITES)])
    manifest = {
        "species": args.species,
        "kind": "leaf clusters",
        "authoring": "in-house, Blender " + bpy.app.version_string + ", scripts/flora/bake_clusters.py",
        "license": "project-owned (no third-party assets)",
        "sprite_px": SPRITE_PX,
        "grid": [GRID_W, GRID_H],
        "window_m": spec["window_m"],
        "hanging": spec["hanging"],
        "seed": args.seed,
        "samples": args.samples,
        "sprites": records,
        "color": os.path.basename(color),
        "normal": os.path.basename(normal),
        "convention": "colour = albedo x local occlusion under a uniform white world, sRGB; "
        "normal = camera-space (n*0.5+0.5), raw; twig base at the bottom centre of each slot "
        "(top centre for a hanging species)",
    }
    with open(os.path.join(out_dir, "clusters.json"), "w", encoding="utf-8") as handle:
        json.dump(manifest, handle, indent=2)
    print("BAKE DONE", args.species, color, normal, flush=True)


if __name__ == "__main__":
    main()
