"""Bake a species' authored skeletons in Blender with Sapling Tree Gen (route 2), in VARIANTS.

Run headless (Sapling Tree Gen installed as an extension):

    blender --background --python scripts/flora/bake_tree.py -- --species oak --out assets/flora/oak

One species, four variants (young / mature / old / sparse — the owner's "from small to big,
thin to thick, low to tall, dense or not"), two rungs each, from the species' Sapling table
with per-variant deltas. Sapling grows the skeleton and places the leaves; this script keeps
the thick wood (trunk, limbs, twigs over a radius) as a bevelled mesh, turns every Sapling
leaf quad into a CROSS PAIR of cluster cards, pulls a card whose stem hangs too far from
exported wood onto it, and fits the Mid deck under the Near tip.

Output per variant, in ENGINE coordinates (Y up, grounded, centred in XZ; Blender's Z-up
frame is mapped (x, y, z) -> (x, z, -y), which preserves handedness and winding):

    v<k>/tree_near.bin, v<k>/tree_mid.bin   "WOTTREE1", u32 nverts, nverts x (pos, normal) f32,
                                            u32 nidx, nidx x u32, u32 ncards, ncards x (center,
                                            half_right, half_up, normal) f32 + u8 sprite
    tree.json                               the tables, seeds, measured tips/trunks, licence
"""

import argparse
import json
import math
import os
import random
import struct
import sys

import bpy

# ------------------------------------------------------------------------------ the tables
# Per-level tuples are (trunk, limbs, twigs, spare). `card_half_m` is the cluster card size,
# `hanging` points the cards' half_up DOWN (willow curtains), `sprites` is the species' cluster
# block size (4 slots; mirrored cards double it), the radii decide which wood is exported.
SPECIES = {
    "oak": dict(
        sapling=dict(seed=7, levels=3, length=(1.0, 0.55, 0.45, 0.4), lengthV=(0.0, 0.08, 0.12, 0.0),
                     branches=(0, 24, 12, 0), curveRes=(8, 5, 3, 1), curve=(0.0, -30.0, -25.0, 0.0),
                     curveV=(30.0, 90.0, 110.0, 0.0), baseSplits=2, segSplits=(0.0, 0.25, 0.2, 0.0),
                     splitAngle=(0.0, 30.0, 25.0, 0.0), splitAngleV=(0.0, 10.0, 10.0, 0.0), scale=15.0,
                     scaleV=0.0, attractUp=(0.0, -0.3, 0.2, 0.0), attractOut=(0.0, 0.2, 0.3, 0.0),
                     shape="2", shapeS="4", branchDist=1.2, baseSize=0.38, baseSize_s=0.2, splitHeight=0.3,
                     ratio=0.032, minRadius=0.004, rootFlare=1.25, ratioPower=1.25,
                     downAngle=(90.0, 60.0, 50.0, 45.0), downAngleV=(0.0, -30.0, 15.0, 10.0),
                     rotate=(137.5, 137.5, 137.5, 137.5), rotateV=(0.0, 20.0, 20.0, 0.0),
                     leaves=4, leafDownAngle=40.0, leafDownAngleV=15.0, leafRotate=137.5, leafRotateV=20.0,
                     leafScale=1.3, leafShape="rect", horzLeaves=False, leafDist="6"),
        card_half_m=0.85, hanging=False, limb_min_radius_m=0.10, twig_min_radius_m=0.045, max_pairs=300,
    ),
    "poplar": dict(  # Lombardy: one bole, a column of short upswept limbs
        sapling=dict(seed=11, levels=3, length=(1.0, 0.28, 0.35, 0.4), lengthV=(0.0, 0.05, 0.1, 0.0),
                     branches=(0, 42, 8, 0), curveRes=(10, 4, 3, 1), curve=(0.0, 10.0, 10.0, 0.0),
                     curveV=(10.0, 30.0, 40.0, 0.0), baseSplits=0, segSplits=(0.0, 0.0, 0.0, 0.0),
                     splitAngle=(0.0, 0.0, 0.0, 0.0), splitAngleV=(0.0, 0.0, 0.0, 0.0), scale=21.0,
                     scaleV=0.0, attractUp=(0.0, 1.4, 1.2, 0.8), attractOut=(0.0, 0.0, 0.0, 0.0),
                     shape="3", shapeS="4", branchDist=1.0, baseSize=0.12, baseSize_s=0.2, splitHeight=0.2,
                     ratio=0.02, minRadius=0.003, rootFlare=1.1, ratioPower=1.3,
                     downAngle=(90.0, 30.0, 35.0, 30.0), downAngleV=(0.0, -10.0, 10.0, 10.0),
                     rotate=(137.5, 137.5, 137.5, 137.5), rotateV=(0.0, 15.0, 15.0, 0.0),
                     leaves=4, leafDownAngle=35.0, leafDownAngleV=15.0, leafRotate=137.5, leafRotateV=20.0,
                     leafScale=1.0, leafShape="rect", horzLeaves=False, leafDist="6"),
        card_half_m=0.62, hanging=False, limb_min_radius_m=0.07, twig_min_radius_m=0.035, max_pairs=260,
    ),
    "willow": dict(  # weeping: a broad crown of limbs rising then bowing gently, hung with curtains
        sapling=dict(seed=5, levels=3, length=(0.85, 0.7, 0.45, 0.4), lengthV=(0.0, 0.1, 0.15, 0.0),
                     branches=(0, 22, 18, 0), curveRes=(8, 6, 4, 1), curve=(0.0, 25.0, 45.0, 0.0),
                     curveV=(20.0, 50.0, 70.0, 0.0), baseSplits=3, segSplits=(0.0, 0.3, 0.1, 0.0),
                     splitAngle=(0.0, 35.0, 20.0, 0.0), splitAngleV=(0.0, 10.0, 10.0, 0.0), scale=16.0,
                     scaleV=0.0, attractUp=(0.0, -0.4, -1.1, 0.0), attractOut=(0.0, 0.5, 0.2, 0.0),
                     shape="2", shapeS="4", branchDist=1.1, baseSize=0.22, baseSize_s=0.2, splitHeight=0.3,
                     ratio=0.03, minRadius=0.003, rootFlare=1.2, ratioPower=1.2,
                     downAngle=(90.0, 50.0, 60.0, 60.0), downAngleV=(0.0, -20.0, 15.0, 10.0),
                     rotate=(137.5, 137.5, 137.5, 137.5), rotateV=(0.0, 20.0, 20.0, 0.0),
                     leaves=5, leafDownAngle=60.0, leafDownAngleV=15.0, leafRotate=137.5, leafRotateV=20.0,
                     leafScale=1.4, leafShape="rect", horzLeaves=False, leafDist="6"),
        card_half_m=1.3, hanging=True, limb_min_radius_m=0.15, twig_min_radius_m=0.09, max_pairs=360,
    ),
    "fruit": dict(  # an orchard apple: short bole, low spreading dome
        sapling=dict(seed=3, levels=3, length=(1.0, 0.6, 0.5, 0.4), lengthV=(0.0, 0.1, 0.1, 0.0),
                     branches=(0, 10, 9, 0), curveRes=(6, 5, 3, 1), curve=(0.0, -20.0, -20.0, 0.0),
                     curveV=(20.0, 70.0, 90.0, 0.0), baseSplits=3, segSplits=(0.0, 0.3, 0.2, 0.0),
                     splitAngle=(0.0, 40.0, 30.0, 0.0), splitAngleV=(0.0, 10.0, 10.0, 0.0), scale=6.0,
                     scaleV=0.0, attractUp=(0.0, -0.2, 0.1, 0.0), attractOut=(0.0, 0.4, 0.3, 0.0),
                     shape="1", shapeS="4", branchDist=1.0, baseSize=0.25, baseSize_s=0.2, splitHeight=0.2,
                     ratio=0.04, minRadius=0.004, rootFlare=1.2, ratioPower=1.2,
                     downAngle=(90.0, 70.0, 55.0, 45.0), downAngleV=(0.0, -20.0, 15.0, 10.0),
                     rotate=(137.5, 137.5, 137.5, 137.5), rotateV=(0.0, 20.0, 20.0, 0.0),
                     leaves=4, leafDownAngle=45.0, leafDownAngleV=15.0, leafRotate=137.5, leafRotateV=20.0,
                     leafScale=0.9, leafShape="rect", horzLeaves=False, leafDist="6"),
        card_half_m=0.55, hanging=False, limb_min_radius_m=0.06, twig_min_radius_m=0.03, max_pairs=200,
    ),
    "pine": dict(  # a monopodial pole, whorls of near-horizontal branches, conical crown
        sapling=dict(seed=13, levels=3, length=(1.0, 0.36, 0.3, 0.4), lengthV=(0.0, 0.05, 0.1, 0.0),
                     branches=(0, 48, 14, 0), curveRes=(10, 5, 3, 1), curve=(0.0, 0.0, 0.0, 0.0),
                     curveV=(10.0, 30.0, 40.0, 0.0), baseSplits=0, segSplits=(0.0, 0.0, 0.0, 0.0),
                     splitAngle=(0.0, 0.0, 0.0, 0.0), splitAngleV=(0.0, 0.0, 0.0, 0.0), scale=21.0,
                     scaleV=0.0, attractUp=(0.0, 0.25, 0.35, 0.0), attractOut=(0.0, 0.0, 0.0, 0.0),
                     shape="0", shapeS="4", branchDist=1.0, baseSize=0.3, baseSize_s=0.2, splitHeight=0.2,
                     ratio=0.022, minRadius=0.003, rootFlare=1.1, ratioPower=1.3,
                     downAngle=(90.0, 80.0, 65.0, 45.0), downAngleV=(0.0, -10.0, 10.0, 10.0),
                     rotate=(137.5, 137.5, 137.5, 137.5), rotateV=(0.0, 15.0, 15.0, 0.0),
                     leaves=8, leafDownAngle=30.0, leafDownAngleV=10.0, leafRotate=137.5, leafRotateV=20.0,
                     leafScale=1.0, leafShape="rect", horzLeaves=False, leafDist="6"),
        # The sparse crown (the owner, 2026-09-03): twice the cluster anchors per twig, more
        # twigs, bigger cards, a fill budget to match — a pine is a dense dark cone, not a rack.
        card_half_m=0.7, hanging=False, limb_min_radius_m=0.07, twig_min_radius_m=0.035, max_pairs=360,
    ),
    "bush": dict(  # a multi-stem shrub fanning from the ground
        sapling=dict(seed=17, levels=3, length=(1.0, 0.7, 0.55, 0.4), lengthV=(0.0, 0.1, 0.1, 0.0),
                     branches=(0, 9, 6, 0), curveRes=(5, 4, 3, 1), curve=(0.0, -10.0, -15.0, 0.0),
                     curveV=(30.0, 60.0, 80.0, 0.0), baseSplits=4, segSplits=(0.0, 0.3, 0.2, 0.0),
                     splitAngle=(0.0, 35.0, 30.0, 0.0), splitAngleV=(0.0, 10.0, 10.0, 0.0), scale=2.4,
                     scaleV=0.0, attractUp=(0.0, 0.1, 0.1, 0.0), attractOut=(0.0, 0.5, 0.3, 0.0),
                     shape="1", shapeS="4", branchDist=1.0, baseSize=0.05, baseSize_s=0.2, splitHeight=0.1,
                     ratio=0.05, minRadius=0.004, rootFlare=1.0, ratioPower=1.2,
                     downAngle=(90.0, 60.0, 50.0, 45.0), downAngleV=(0.0, -20.0, 15.0, 10.0),
                     rotate=(137.5, 137.5, 137.5, 137.5), rotateV=(0.0, 20.0, 20.0, 0.0),
                     leaves=5, leafDownAngle=45.0, leafDownAngleV=15.0, leafRotate=137.5, leafRotateV=20.0,
                     leafScale=0.7, leafShape="rect", horzLeaves=False, leafDist="6"),
        card_half_m=0.42, hanging=False, limb_min_radius_m=0.03, twig_min_radius_m=0.015, max_pairs=160,
    ),
}

# The variants: multipliers on the species table. "young" small and thin, "mature" the table,
# "old" big, thick and crooked, "sparse" thin-crowned and crooked.
VARIANTS = [
    ("young", dict(scale=0.62, ratio=0.85, branches=0.8, leaves=-1, curveV=1.0, seed=101)),
    ("mature", dict(scale=1.0, ratio=1.0, branches=1.0, leaves=0, curveV=1.0, seed=0)),
    ("old", dict(scale=1.15, ratio=1.35, branches=1.15, leaves=1, curveV=1.35, seed=211)),
    ("sparse", dict(scale=0.9, ratio=0.95, branches=0.75, leaves=-2, curveV=1.6, seed=307)),
]
COMMON = dict(showLeaves=True, bevel=True, prune=False, useArm=False, makeMesh=False,
              closeTip=False, autoTaper=True, taper=(1.0, 1.0, 1.0, 1.0),
              radiusTweak=(1.0, 1.0, 1.0, 1.0), useParentAngle=True, scale0=1.0, scaleV0=0.0,
              leafScaleX=1.0, leafScaleT=0.0, leafScaleV=0.15, curveBack=(0.0, 0.0, 0.0, 0.0))
RUNGS = {"near": dict(bevelRes=1, resU=2), "mid": dict(bevelRes=0, resU=1)}
CARD_REACH_M = 0.35
SPRITES = 4


def parse_args():
    argv = sys.argv[sys.argv.index("--") + 1 :] if "--" in sys.argv else []
    parser = argparse.ArgumentParser()
    parser.add_argument("--species", required=True, choices=sorted(SPECIES))
    parser.add_argument("--out", required=True)
    return parser.parse_args(argv)


def to_engine(v):
    return (v.x, v.z, -v.y)


def variant_params(table, deltas):
    p = dict(COMMON)
    p.update(table["sapling"])
    p["scale"] = p["scale"] * deltas["scale"]
    p["ratio"] = p["ratio"] * deltas["ratio"]
    p["branches"] = tuple(int(round(b * deltas["branches"])) if i > 0 else b for i, b in enumerate(p["branches"]))
    p["leaves"] = max(1, p["leaves"] + deltas["leaves"])
    p["curveV"] = tuple(c * deltas["curveV"] for c in p["curveV"])
    p["seed"] = p["seed"] + deltas["seed"]
    return p


def grow(params, rung):
    bpy.ops.wm.read_homefile(use_empty=True)
    if "CURVE_OT_tree_add" not in dir(bpy.types):
        bpy.ops.preferences.addon_enable(module="bl_ext.blender_org.sapling_tree_gen")
    p = dict(params)
    p.update(RUNGS[rung])
    bpy.ops.curve.tree_add(do_update=True, **p)
    curve = next(o for o in bpy.context.scene.objects if o.type == "CURVE")
    leaves = next((o for o in bpy.context.scene.objects if o.type == "MESH" and o is not curve), None)
    return curve, leaves


def spline_radius_m(curve, spline):
    return spline.bezier_points[0].radius * curve.data.bevel_depth


def crown_top_splines(curve):
    """The splines that carry the tree's highest wood — every one whose control points reach
    within a metre of the top, plus the five highest: kept on every rung whatever their
    radius, so a rung swap never lowers the tip (the ladder's invariant)."""
    tops = sorted(
        ((max(bp.co.z for bp in spline.bezier_points), i) for i, spline in enumerate(curve.data.splines)),
        reverse=True,
    )
    if not tops:
        return ()
    highest = tops[0][0]
    keep = {i for z, i in tops[:5]}
    keep |= {i for z, i in tops if z >= highest - 1.0}
    return tuple(keep)


def convert_kept(curve, keep, bevel_res, res_u, always=()):
    copy = curve.copy()
    copy.data = curve.data.copy()
    bpy.context.scene.collection.objects.link(copy)
    doomed = [s for i, s in enumerate(copy.data.splines) if not keep(spline_radius_m(copy, s)) and i not in always]
    for spline in doomed:
        copy.data.splines.remove(spline)
    copy.data.bevel_resolution = bevel_res
    copy.data.resolution_u = res_u
    depsgraph = bpy.context.evaluated_depsgraph_get()
    evaluated = copy.evaluated_get(depsgraph)
    mesh = bpy.data.meshes.new_from_object(evaluated, preserve_all_data_layers=False, depsgraph=depsgraph)
    mesh.transform(copy.matrix_world)
    mesh.calc_loop_triangles()
    return mesh, len(copy.data.splines)


def wood_mesh(curve, table, rung, variant_ratio):
    # Mid keeps the thick wood only (four sides, one segment) — plus whatever carries the
    # top, so a rung swap never moves the tip (the tip of a willow or a pine is wood).
    limb_min = table["limb_min_radius_m"] * variant_ratio
    twig_min = table["twig_min_radius_m"] * variant_ratio
    if rung == "mid":
        twig_min = limb_min = max(limb_min, twig_min) * 1.4
    crown_top = crown_top_splines(curve)
    limbs, kept_limbs = convert_kept(curve, lambda r: r >= limb_min, RUNGS[rung]["bevelRes"], RUNGS[rung]["resU"], always=crown_top)
    twigs, kept_twigs = convert_kept(curve, lambda r: twig_min <= r < limb_min, 0, 1)
    positions, normals, indices = [], [], []
    for mesh in (limbs, twigs):
        base = len(positions)
        for vertex in mesh.vertices:
            positions.append(to_engine(vertex.co))
            normals.append(to_engine(vertex.normal))
        for tri in mesh.loop_triangles:
            indices.extend(base + i for i in tri.vertices)
    return positions, normals, indices, kept_limbs + kept_twigs, twig_min


def kept_polylines(curve, min_radius_m):
    lines = []
    for spline in curve.data.splines:
        if spline_radius_m(curve, spline) < min_radius_m:
            continue
        points = [curve.matrix_world @ bp.co for bp in spline.bezier_points]
        dense = []
        for a, b in zip(points, points[1:]):
            for i in range(6):
                dense.append(a.lerp(b, i / 6))
        dense.append(points[-1])
        lines.append(dense)
    return lines


def nearest_on_wood(point, lines):
    best, best_d = None, float("inf")
    for line in lines:
        for a, b in zip(line, line[1:]):
            ab = b - a
            t = 0.0 if ab.length_squared < 1e-9 else max(0.0, min(1.0, (point - a).dot(ab) / ab.length_squared))
            q = a + ab * t
            d = (point - q).length
            if d < best_d:
                best, best_d = q, d
    return best, best_d


def leaf_cards(leaves, table, seed, wood_lines, scale_mul):
    """Every Sapling leaf quad -> a cross pair of cluster cards (engine space)."""
    from mathutils import Vector

    cards = []
    if leaves is None:
        return cards, 0
    mesh = leaves.data
    world = leaves.matrix_world
    faces = [f for f in mesh.polygons if len(f.vertices) == 4]
    rng = random.Random(seed)
    pulled = 0
    half_base = table["card_half_m"] * (0.85 + 0.15 * scale_mul)
    for face in faces:
        v = [world @ mesh.vertices[i].co for i in face.vertices]
        stem = (v[0] + v[1]) * 0.5
        far = (v[2] + v[3]) * 0.5
        up = (far - stem) * 0.5
        right = (v[1] - v[0]) * 0.5
        half = half_base * rng.uniform(0.85, 1.2)
        up = up.normalized() * half
        right = right.normalized() * half
        if table["hanging"]:
            # The curtain hangs: the stem stays on the twig, the card falls straight down.
            up = Vector((0.0, 0.0, -half))
            right = Vector((right.x, right.y, 0.0)).normalized() * half if abs(right.x) + abs(right.y) > 1e-4 else Vector((half, 0.0, 0.0))
        anchor, distance = nearest_on_wood(stem, wood_lines)
        if anchor is not None and distance > CARD_REACH_M:
            stem = anchor + (stem - anchor).normalized() * CARD_REACH_M
            pulled += 1
        center = stem + up
        normal = right.cross(up).normalized()
        sprite = rng.randrange(SPRITES)
        cards.append((center, right, up, normal, sprite))
        right2 = normal * half
        normal2 = right2.cross(up).normalized()
        cards.append((center, right2, up, normal2, sprite))
    return cards, pulled


def cap_pairs(cards, max_pairs):
    """The fill budget: keep at most `max_pairs` cross pairs, evenly, each survivor scaled by
    sqrt(kept ratio) so the crown's covered AREA survives — the Mid thinning's own rule."""
    pairs = len(cards) // 2
    if pairs <= max_pairs:
        return cards
    keep_every = pairs / max_pairs
    scale = math.sqrt(keep_every)
    kept = []
    next_keep = 0.0
    for pair_index in range(pairs):
        if pair_index >= next_keep:
            next_keep += keep_every
            for card in cards[pair_index * 2 : pair_index * 2 + 2]:
                center, right, up, normal, sprite = card
                kept.append((center, right * scale, up * scale, normal, sprite))
    return kept


def card_top(card):
    center, right, up, _normal, _sprite = card
    return center.z + abs(right.z) + abs(up.z)


def thin_for_mid(cards):
    """The Mid rung draws the SAME deck as Near (the owner's verdict of 2026-09-03: a tree
    that changes its crown on approach is a different tree; only the wood coarsens). Kept as
    a function so a future thinning has one place to live — today it is the identity."""
    return list(cards)


def thin_for_mid_halved(cards):
    """The old halving (every second cross pair, scaled by sqrt(2), fitted under the Near tip,
    top pair kept) — retired 2026-09-03, kept for the record."""
    tip = max(card_top(card) for card in cards) if cards else 0.0
    top_pair = max(range(len(cards) // 2), key=lambda i: card_top(cards[i * 2])) if cards else -1
    kept = []
    for pair_index in range(0, len(cards), 2):
        if (pair_index // 2) % 2 == 0 or pair_index // 2 == top_pair:
            for card in cards[pair_index : pair_index + 2]:
                center, right, up, normal, sprite = card
                scale = math.sqrt(2.0)
                span = abs(right.z) + abs(up.z)
                if span > 1e-6:
                    scale = min(scale, max((tip - center.z) / span, 0.5))
                kept.append((center, right * scale, up * scale, normal, sprite))
    return kept


def write_bin(path, positions, normals, indices, cards):
    with open(path, "wb") as handle:
        handle.write(b"WOTTREE1")
        handle.write(struct.pack("<I", len(positions)))
        for p, n in zip(positions, normals):
            handle.write(struct.pack("<6f", *p, *n))
        handle.write(struct.pack("<I", len(indices)))
        handle.write(struct.pack(f"<{len(indices)}I", *indices))
        handle.write(struct.pack("<I", len(cards)))
        for center, right, up, normal, sprite in cards:
            handle.write(struct.pack("<12f", *to_engine(center), *to_engine(right), *to_engine(up), *to_engine(normal)))
            handle.write(struct.pack("<B", sprite))


def main():
    args = parse_args()
    table = SPECIES[args.species]
    out = os.path.abspath(args.out)
    os.makedirs(out, exist_ok=True)
    report = {"species": args.species, "generator": "Sapling Tree Gen (Blender extension) + scripts/flora/bake_tree.py",
              "blender": bpy.app.version_string, "license": "project-owned output; Sapling is GPL tooling, not shipped",
              "table": {k: (list(v) if isinstance(v, tuple) else v) for k, v in table["sapling"].items()},
              "variants": {}}
    for index, (name, deltas) in enumerate(VARIANTS):
        params = variant_params(table, deltas)
        vdir = os.path.join(out, f"v{index}")
        os.makedirs(vdir, exist_ok=True)
        rungs = {}
        near_cards = None
        near_wood = None
        near_lift = 0.0
        for rung in ("near", "mid"):
            curve, leaves = grow(params, rung)
            wood_lines = kept_polylines(curve, table["twig_min_radius_m"] * deltas["ratio"])
            positions, normals, indices, kept, _ = wood_mesh(curve, table, rung, deltas["ratio"])
            # Pendulous wood that hangs below the ground (a young willow's curtains) lifts the
            # whole tree: the trunk sink hides the lift, the soil hides nothing. The lift is
            # the NEAR rung's and applies to both, or the rungs would stand at different heights.
            if rung == "near":
                lowest = min((p[1] for p in positions), default=0.0)
                near_lift = -lowest if lowest < -0.05 else 0.0
            lift = near_lift
            if lift > 0.0:
                positions = [(p[0], p[1] + lift, p[2]) for p in positions]
            if rung == "near":
                near_wood = (positions, normals, indices)
            else:
                # The tip cap: whatever wood the Near rung has in its top half-metre, Mid
                # carries verbatim — so a rung swap never moves the tip, whichever thin
                # twig happens to carry it (a willow's arc, a pine's leader).
                n_pos, n_nrm, n_idx = near_wood
                near_top = max(p[1] for p in n_pos) if n_pos else 0.0
                mid_top = max(p[1] for p in positions) if positions else 0.0
                if near_top - mid_top > 0.02:
                    remap = {}
                    base = len(positions)
                    for tri in range(0, len(n_idx), 3):
                        corners = n_idx[tri : tri + 3]
                        if all(n_pos[c][1] > near_top - 0.6 for c in corners):
                            for c in corners:
                                if c not in remap:
                                    remap[c] = base + len(remap)
                                    positions.append(n_pos[c])
                                    normals.append(n_nrm[c])
                                indices.append(remap[c])
            if rung == "near":
                cards, pulled = leaf_cards(leaves, table, params["seed"], wood_lines, deltas["scale"])
                cards = cap_pairs(cards, table["max_pairs"])
                if lift > 0.0:
                    from mathutils import Vector
                    cards = [(c + Vector((0.0, 0.0, lift)), r, u, n, s) for c, r, u, n, s in cards]
                near_cards = cards
            else:
                cards, pulled = thin_for_mid(near_cards), 0
            tip = max(p[1] for p in positions) if positions else 0.0
            tip_cards = max(c[0].z + abs(c[2].z) + abs(c[1].z) for c in cards) if cards else 0.0
            butt = max((math.hypot(p[0], p[2]) for p in positions if p[1] < 0.3), default=0.0)
            write_bin(os.path.join(vdir, f"tree_{rung}.bin"), positions, normals, indices, cards)
            rungs[rung] = {"wood_vertices": len(positions), "wood_triangles": len(indices) // 3, "wood_pieces": kept,
                           "cards": len(cards), "cards_pulled": pulled, "tip_wood_m": round(tip, 3),
                           "tip_cards_m": round(tip_cards, 3), "butt_radius_m": round(butt, 3)}
            print("RUNG", args.species, name, rung, rungs[rung], flush=True)
        report["variants"][f"v{index}"] = {"name": name, "deltas": deltas, "seed": params["seed"], "rungs": rungs}
    with open(os.path.join(out, "tree.json"), "w", encoding="utf-8") as handle:
        json.dump(report, handle, indent=2)
    print("TREE DONE", args.species, flush=True)


if __name__ == "__main__":
    main()
