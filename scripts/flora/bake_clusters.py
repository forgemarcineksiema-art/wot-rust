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
    # Leaves 2.0 (the owner, 2026-09-03: "the leaves must be improved, corrected and refined"):
    # blade sizes are the species' REAL ones (an oak leaf is 8-12 cm; the first bake's 15-23 cm
    # read as lettuce at 22 m), at twice the count so the window stays full; `under` is the
    # underside albedo (paler, matte); the outline carries lobes or teeth.
    "oak": dict(window_m=1.5, leaves=(1050, 1200), leaf_m=(0.09, 0.13), twigs=(20, 26),
                albedo=(0.085, 0.235, 0.065), under=(0.15, 0.25, 0.11), jitter=(0.03, 0.05, 0.03), cup=(0.08, 0.22),
                outline="oak", hanging=False, needles=False, twig_len=(0.35, 0.62), aspect=1.0, gloss=0.35),
    "poplar": dict(window_m=1.3, leaves=(1400, 1600), leaf_m=(0.07, 0.10), twigs=(24, 30),
                   albedo=(0.14, 0.29, 0.07), under=(0.20, 0.30, 0.13), jitter=(0.03, 0.05, 0.03), cup=(0.02, 0.08),
                   outline="deltoid", hanging=False, needles=False, twig_len=(0.35, 0.6), aspect=1.0, gloss=0.5),
    # The willow is a CURTAIN: a 2.6 m window of long parallel streamers, leaves close along
    # them — the card hangs the whole window from its twig (the owner, 2026-09-03: the short
    # brushes read as "retarded").
    "willow": dict(window_m=2.6, leaves=(1000, 1200), leaf_m=(0.13, 0.19), twigs=(9, 12),
                   albedo=(0.19, 0.32, 0.14), jitter=(0.04, 0.05, 0.03), cup=(0.02, 0.08),
                   outline="lanceolate", hanging=True, needles=False, twig_len=(1.6, 2.3), aspect=0.28),
    "fruit": dict(window_m=1.1, leaves=(1100, 1300), leaf_m=(0.055, 0.085), twigs=(22, 28),
                  albedo=(0.11, 0.27, 0.09), under=(0.19, 0.26, 0.15), jitter=(0.03, 0.05, 0.03), cup=(0.05, 0.15),
                  outline="oval", hanging=False, needles=False, twig_len=(0.3, 0.5), aspect=1.0, gloss=0.25),
    # A Scots pine: TWO needles a fascicle, 4-7 cm, blue-green, a brush of them along the shoot.
    "pine": dict(window_m=1.2, leaves=(1000, 1200), leaf_m=(0.05, 0.08), twigs=(16, 20),
                 albedo=(0.07, 0.16, 0.10), under=(0.09, 0.17, 0.12), jitter=(0.02, 0.03, 0.03), cup=(0.0, 0.0),
                 outline="needle", hanging=False, needles=True, twig_len=(0.3, 0.5), aspect=1.0, gloss=0.3),
    "bush": dict(window_m=0.9, leaves=(1300, 1500), leaf_m=(0.035, 0.055), twigs=(24, 30),
                 albedo=(0.10, 0.22, 0.08), under=(0.16, 0.24, 0.13), jitter=(0.03, 0.05, 0.03), cup=(0.05, 0.15),
                 outline="oval", hanging=False, needles=False, twig_len=(0.25, 0.45), aspect=1.0, gloss=0.3),
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
    teeth = 1.0 + 0.045 * (2.0 * abs((t * 26.0) % 1.0 - 0.5) - 0.5)  # a fine triangle-wave serration
    if outline == "oak":
        # Quercus robur: a short petiole, then 4-5 ROUNDED lobes a side with sinuses cutting
        # almost half-way to the midrib, the blade widest past its middle, a rounded tip.
        if t < 0.06:
            return 0.03
        u = (t - 0.06) / 0.94
        body = math.sin(math.pi * u ** 0.8) ** 0.7
        hump = 0.5 + 0.5 * math.cos(2.0 * math.pi * (u * 4.5 - 0.5))
        lobe = 0.42 + 0.58 * hump ** 0.9
        return 0.34 * body * lobe + 0.006
    if outline == "deltoid":
        # Broad, rounded shoulders low on the blade, a drip tip, crenate teeth: a poplar.
        shoulder = math.sin(math.pi * min(t / 0.45, 1.0) * 0.5) if t < 0.45 else 1.0
        crenate = 1.0 + 0.05 * math.sin(t * 52.0)
        return 0.42 * shoulder * (1.0 - t) ** 0.85 * crenate + 0.02
    if outline == "lanceolate":
        return 0.10 * bell ** 0.7
    if outline == "oval":
        return 0.28 * math.sin(math.pi * t ** 0.9) * teeth + 0.01
    if outline == "needle":
        return 0.06
    return 0.25 * bell


def leaf_mesh(name, outline, length, cup, rng):
    """One cupped blade, midrib along +Y from the petiole at the origin, blade in the XY plane."""
    mesh = bpy.data.meshes.new(name)
    bm = bmesh.new()
    stations = 6 if outline == "needle" else (40 if outline == "oak" else 24)
    cup_amount = rng.uniform(*cup)
    twist = rng.uniform(-0.2, 0.2)
    # The midrib is a crease: the blade rises from it to the margins (a real leaf is a shallow
    # V in section), then curls at the very edge.
    crease = rng.uniform(0.06, 0.16)
    left, mid, right = [], [], []
    for i in range(stations + 1):
        t = i / stations
        y = t * length
        w = max(half_width(outline, t) * length, 0.0005)
        droop = -0.10 * length * t * t
        z_edge = cup_amount * w + crease * w
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
    """One pooled leaf material: two-sided, translucent, per-object hue (Object Info Random)."""
    material = bpy.data.materials.new(name)
    material.use_nodes = True
    nodes = material.node_tree.nodes
    links = material.node_tree.links
    principled = nodes["Principled BSDF"]
    output = nodes["Material Output"]
    base = spec["albedo"]
    jit = spec["jitter"]
    r = base[0] + rng.uniform(-jit[0], jit[0] * 1.3)
    g = base[1] + rng.uniform(-jit[1], jit[1] * 1.2)
    b = base[2] + rng.uniform(-jit[2], jit[2])
    top = (max(r, 0.01), max(g, 0.02), max(b, 0.01), 1.0)
    ur, ug, ub = spec["under"]
    under = (max(ur + (r - base[0]) * 0.6, 0.02), max(ug + (g - base[1]) * 0.6, 0.03), max(ub + (b - base[2]) * 0.6, 0.02), 1.0)
    # A young leaf is yellower and lighter: the per-object random picks the age.
    young = (min(top[0] * 1.55 + 0.05, 1.0), min(top[1] * 1.25 + 0.03, 1.0), top[2] * 0.7, 1.0)
    info = nodes.new("ShaderNodeObjectInfo")
    age = nodes.new("ShaderNodeMath")
    age.operation = "POWER"
    age.inputs[1].default_value = 3.0  # most leaves mature, a few young
    links.new(info.outputs["Random"], age.inputs[0])
    top_mix = nodes.new("ShaderNodeMix")
    top_mix.data_type = "RGBA"
    top_mix.inputs["A"].default_value = top
    top_mix.inputs["B"].default_value = young
    links.new(age.outputs[0], top_mix.inputs["Factor"])
    side = nodes.new("ShaderNodeNewGeometry")
    side_mix = nodes.new("ShaderNodeMix")
    side_mix.data_type = "RGBA"
    links.new(top_mix.outputs["Result"], side_mix.inputs["A"])
    side_mix.inputs["B"].default_value = under
    links.new(side.outputs["Backfacing"], side_mix.inputs["Factor"])
    links.new(side_mix.outputs["Result"], principled.inputs["Base Color"])
    rough = nodes.new("ShaderNodeMix")
    rough.data_type = "FLOAT"
    rough.inputs["A"].default_value = 1.0 - spec["gloss"] * 0.9
    rough.inputs["B"].default_value = 0.85
    links.new(side.outputs["Backfacing"], rough.inputs["Factor"])
    links.new(rough.outputs["Result"], principled.inputs["Roughness"])
    try:
        principled.inputs["Specular IOR Level"].default_value = 0.3
    except KeyError:
        pass
    if not spec["needles"]:
        # Veins: a distorted band pattern across the midrib, as a bump.
        wave = nodes.new("ShaderNodeTexWave")
        wave.wave_type = "BANDS"
        wave.bands_direction = "Y"
        wave.inputs["Scale"].default_value = 140.0
        wave.inputs["Distortion"].default_value = 2.0
        wave.inputs["Detail"].default_value = 2.0
        bump = nodes.new("ShaderNodeBump")
        bump.inputs["Strength"].default_value = 0.3
        bump.inputs["Distance"].default_value = 0.0015
        links.new(wave.outputs["Fac"], bump.inputs["Height"])
        links.new(bump.outputs["Normal"], principled.inputs["Normal"])
    # Translucency: a leaf passes a share of the light through — under the white world that
    # is what keeps an occluded leaf green instead of black.
    translucent = nodes.new("ShaderNodeBsdfTranslucent")
    links.new(side_mix.outputs["Result"], translucent.inputs["Color"])
    mix = nodes.new("ShaderNodeMixShader")
    mix.inputs["Fac"].default_value = 0.0 if spec["needles"] else 0.28
    links.new(principled.outputs["BSDF"], mix.inputs[1])
    links.new(translucent.outputs["BSDF"], mix.inputs[2])
    links.new(mix.outputs[0], output.inputs["Surface"])
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
    materials = [leaf_material(f"leaf_mat_{seed}_{k}", spec, rng) for k in range(12)]
    for i in range(count):
        twig = twigs[rng.randrange(len(twigs))]
        # Leaves crowd the shoot TIPS (an oak's rosettes), thin out toward the base.
        u = rng.uniform(0.0, 1.0)
        if spec["needles"]:
            # Needles clothe the whole shoot (two or three years of them), tip to base.
            t = rng.uniform(0.1, 1.0) if twig is not main else rng.uniform(0.25, 1.0)
        else:
            t = (0.1 + 0.9 * (1.0 - u * u)) if twig is not main else (0.3 + 0.7 * (1.0 - u * u))
        p, tangent = spline_frame(twig, t)
        length = rng.uniform(*spec["leaf_m"])
        # The petiole: the blade sits OFF the twig, around it, not on its axis — a row of
        # blades on the axis reads as a bottle brush.
        if not spec["needles"] and not spec["hanging"]:
            around0 = rng.uniform(0.0, 2 * math.pi)
            off = Vector((math.cos(around0), math.sin(around0) * 0.6, rng.uniform(-0.3, 0.6))).normalized()
            p = p + off * length * rng.uniform(0.25, 0.7)
        if spec["needles"]:
            # A Scots pine fascicle: TWO needles, slightly splayed.
            blades = 2
        else:
            blades = 1
        material = materials[rng.randrange(len(materials))]
        for blade in range(blades):
            mesh = leaf_mesh(f"leaf_{seed}_{i}_{blade}", spec["outline"], length, spec["cup"], rng)
            leaf = bpy.data.objects.new(f"leaf_{seed}_{i}_{blade}", mesh)
            leaf.data.materials.append(material)
            bpy.context.scene.collection.objects.link(leaf)
            side = 1.0 if (i + blade) % 2 == 0 else -1.0
            around = rng.uniform(0.0, 2 * math.pi)
            radial = Vector((math.cos(around), math.sin(around), 0.0))
            if spec["needles"]:
                spread = Vector((math.cos(around + blade * 0.35), math.sin(around + blade * 0.35), 0.0))
                forward = (tangent * 0.6 + spread * 0.7 + Vector((0.0, -0.2, 0.15))).normalized()
            elif spec["hanging"]:
                # Leaves lie ALONG the streamer, alternating sides, hugging it.
                forward = (tangent * 0.85 + radial * side * 0.25 + Vector((0.0, -0.2, 0.0))).normalized()
            else:
                # Out from the twig, alternating sides, a little toward the eye and up.
                forward = (tangent * 0.4 + radial * side * 0.75 + Vector((0.0, -0.3, 0.2))).normalized()
            # Phototropism: the blade's face turns UP (+Z) and toward the light, with a
            # per-leaf tumble — the mass layers like shingles instead of a random scatter.
            tumble = Vector((rng.uniform(-0.6, 0.6), rng.uniform(-0.5, 0.5), rng.uniform(-0.4, 0.4)))
            up_hint = (Vector((0.0, -0.9, 0.8)) + tumble).normalized()
            right = forward.cross(up_hint).normalized()
            normal = right.cross(forward).normalized()
            rotation = Matrix((right, forward, normal)).transposed().to_4x4()
            leaf.matrix_world = Matrix.Translation(p + normal * 0.003) @ rotation
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
