"""Bake the oak's authored skeleton in Blender with Sapling Tree Gen (route 2: trees as data).

Run headless (Sapling Tree Gen must be installed as an extension):

    blender --background --python scripts/flora/bake_oak_tree.py -- --out assets/flora/oak

Sapling grows the Weber-Penn skeleton (trunk, limbs, twigs) and places the leaves; this script
keeps the thick wood (trunk + limbs) as a bevelled mesh, drops the twig-level tubes (the
cluster sprites carry their own twigs), and turns every Sapling leaf quad into a CROSS PAIR of
cluster cards (the engine's card convention: two perpendicular quads through one centre, stem
at -half_up). Two rungs are exported from the same seed: Near (finer bevel) and Mid (coarser
bevel, every second cross pair kept and scaled, exactly as the procedural Mid thins).

Output, in ENGINE coordinates (Y up, grounded at y = 0, centred in XZ; Blender's Z-up frame is
mapped (x, y, z) -> (x, z, -y), which preserves handedness and winding):

    tree_near.bin, tree_mid.bin   "WOTTREE1", u32 nverts, nverts x (pos xyz, normal xyz) f32,
                                  u32 nidx, nidx x u32, u32 ncards, ncards x (center xyz,
                                  half_right xyz, half_up xyz, normal xyz) f32 + u8 sprite
    tree.json                     the Sapling parameters, seed, measured tip/trunk, licence
"""

import argparse
import json
import math
import os
import struct
import sys

import bpy
from mathutils import Vector

# The oak, as Sapling parameters. Per-level tuples are (trunk, limbs, twigs, spare).
OAK = dict(
    seed=7,
    levels=3,
    length=(1.0, 0.55, 0.45, 0.4),
    lengthV=(0.0, 0.08, 0.12, 0.0),
    branches=(0, 22, 9, 0),
    curveRes=(8, 5, 3, 1),
    curve=(0.0, -30.0, -25.0, 0.0),
    curveV=(30.0, 90.0, 110.0, 0.0),
    curveBack=(0.0, 0.0, 0.0, 0.0),
    baseSplits=2,
    segSplits=(0.0, 0.25, 0.2, 0.0),
    splitAngle=(0.0, 30.0, 25.0, 0.0),
    splitAngleV=(0.0, 10.0, 10.0, 0.0),
    scale=15.0,
    scaleV=0.0,
    attractUp=(0.0, -0.3, 0.2, 0.0),
    attractOut=(0.0, 0.2, 0.3, 0.0),
    shape="2",  # hemispherical crown
    shapeS="4",
    branchDist=1.2,
    baseSize=0.38,
    baseSize_s=0.2,
    splitHeight=0.3,
    ratio=0.032,
    minRadius=0.004,
    closeTip=False,
    rootFlare=1.25,
    autoTaper=True,
    taper=(1.0, 1.0, 1.0, 1.0),
    radiusTweak=(1.0, 1.0, 1.0, 1.0),
    ratioPower=1.25,
    downAngle=(90.0, 60.0, 50.0, 45.0),
    downAngleV=(0.0, -30.0, 15.0, 10.0),
    useParentAngle=True,
    rotate=(137.5, 137.5, 137.5, 137.5),
    rotateV=(0.0, 20.0, 20.0, 0.0),
    scale0=1.0,
    scaleV0=0.0,
    leaves=2,
    leafDownAngle=40.0,
    leafDownAngleV=15.0,
    leafRotate=137.5,
    leafRotateV=20.0,
    leafScale=1.3,
    leafScaleX=1.0,
    leafScaleT=0.0,
    leafScaleV=0.15,
    leafShape="rect",
    horzLeaves=False,
    leafDist="6",
    showLeaves=True,
    bevel=True,
    prune=False,
    useArm=False,
    makeMesh=False,
)
# Bevel and curve resolution per rung.
RUNGS = {
    "near": dict(bevelRes=1, resU=2, limb_min_radius_m=0.10, twig_min_radius_m=0.065),
    "mid": dict(bevelRes=0, resU=1, limb_min_radius_m=0.14, twig_min_radius_m=0.14),
}
# A cluster's stem may hang at most this far from the wood that carries it; farther cards
# are pulled onto the nearest kept spline (the twig they grew on was not exported).
CARD_REACH_M = 0.35
CARD_HALF_M = 0.72


def parse_args():
    argv = sys.argv[sys.argv.index("--") + 1 :] if "--" in sys.argv else []
    parser = argparse.ArgumentParser()
    parser.add_argument("--out", required=True)
    return parser.parse_args(argv)


def to_engine(v):
    return (v.x, v.z, -v.y)


def grow(rung):
    # `read_factory_settings` would also reset the preferences and drop the extension;
    # `read_homefile` keeps them. Enable Sapling explicitly all the same.
    bpy.ops.wm.read_homefile(use_empty=True)
    if "CURVE_OT_tree_add" not in dir(bpy.types):
        bpy.ops.preferences.addon_enable(module="bl_ext.blender_org.sapling_tree_gen")
    params = dict(OAK)
    params.update({k: v for k, v in RUNGS[rung].items() if not k.endswith("_radius_m")})
    bpy.ops.curve.tree_add(do_update=True, **params)
    curve = next(o for o in bpy.context.scene.objects if o.type == "CURVE")
    leaves = next((o for o in bpy.context.scene.objects if o.type == "MESH" and o is not curve), None)
    return curve, leaves


def spline_radius_m(curve, spline):
    return spline.bezier_points[0].radius * curve.data.bevel_depth


def convert_kept(curve, keep, bevel_res, res_u):
    """A copy of `curve` with only the splines `keep` accepts, converted to a mesh."""
    copy = curve.copy()
    copy.data = curve.data.copy()
    bpy.context.scene.collection.objects.link(copy)
    for spline in [s for s in copy.data.splines if not keep(spline_radius_m(copy, s))]:
        copy.data.splines.remove(spline)
    copy.data.bevel_resolution = bevel_res
    copy.data.resolution_u = res_u
    depsgraph = bpy.context.evaluated_depsgraph_get()
    evaluated = copy.evaluated_get(depsgraph)
    mesh = bpy.data.meshes.new_from_object(evaluated, preserve_all_data_layers=False, depsgraph=depsgraph)
    mesh.transform(copy.matrix_world)
    mesh.calc_loop_triangles()
    return mesh, len(copy.data.splines)


def wood_mesh(curve, rung):
    """Limbs at the rung's bevel, twigs 4-sided at one segment; return engine-space arrays."""
    limb_min = rung["limb_min_radius_m"]
    twig_min = rung["twig_min_radius_m"]
    limbs, kept_limbs = convert_kept(curve, lambda r: r >= limb_min, rung["bevelRes"], rung["resU"])
    twigs, kept_twigs = convert_kept(curve, lambda r: twig_min <= r < limb_min, 0, 1)
    positions, normals, indices = [], [], []
    for mesh in (limbs, twigs):
        base = len(positions)
        for vertex in mesh.vertices:
            positions.append(to_engine(vertex.co))
            normals.append(to_engine(vertex.normal))
        for tri in mesh.loop_triangles:
            indices.extend(base + i for i in tri.vertices)
    return positions, normals, indices, kept_limbs + kept_twigs


def kept_polylines(curve, min_radius_m):
    """The kept splines as dense polylines (Blender space) for the card pull."""
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


def leaf_cards(leaves, rng_seed, wood_lines):
    """Every Sapling leaf quad -> a cross pair of cluster cards (engine space), each hanging
    its stem within CARD_REACH_M of the exported wood."""
    cards = []
    if leaves is None:
        return cards
    mesh = leaves.data
    world = leaves.matrix_world
    faces = [f for f in mesh.polygons if len(f.vertices) == 4]
    import random

    rng = random.Random(rng_seed)
    pulled = 0
    for face in faces:
        v = [world @ mesh.vertices[i].co for i in face.vertices]
        stem = (v[0] + v[1]) * 0.5
        far = (v[2] + v[3]) * 0.5
        up = (far - stem) * 0.5
        right = (v[1] - v[0]) * 0.5
        # Re-scale to the engine's cluster card size (the sprite window is ~1.5 m).
        half = CARD_HALF_M * rng.uniform(0.85, 1.2)
        up = up.normalized() * half
        right = right.normalized() * half
        # The stem hangs from wood: a card whose twig was not exported slides to the nearest
        # kept spline, keeping its facing.
        anchor, distance = nearest_on_wood(stem, wood_lines)
        if anchor is not None and distance > CARD_REACH_M:
            stem = anchor + (stem - anchor).normalized() * CARD_REACH_M
            pulled += 1
        center = stem + up
        normal = right.cross(up).normalized()
        sprite = rng.randrange(8)
        cards.append((center, right, up, normal, sprite))
        # The second quad of the cross: spun 90 degrees about the stem axis.
        right2 = normal * half
        normal2 = right2.cross(up).normalized()
        cards.append((center, right2, up, normal2, sprite))
    print("cards pulled onto wood:", pulled, "of", len(faces), flush=True)
    return cards


def card_top(card):
    center, right, up, _normal, _sprite = card
    return center.z + abs(right.z) + abs(up.z)


def thin_for_mid(cards):
    """Keep every second cross pair, scaled by sqrt(2) so the covered area survives — but
    never above the Near deck's tip: the tip height is the ladder's invariant (a rung swap
    moves triangles, never metres), so a scaled card that would poke over it is shrunk back."""
    tip = max(card_top(card) for card in cards) if cards else 0.0
    kept = []
    for pair_index in range(0, len(cards), 2):
        if (pair_index // 2) % 2 == 0:
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
    out = os.path.abspath(args.out)
    os.makedirs(out, exist_ok=True)
    report = {"species": "oak", "generator": "Sapling Tree Gen (Blender extension) + scripts/flora/bake_oak_tree.py",
              "blender": bpy.app.version_string, "license": "project-owned output; Sapling is GPL tooling, not shipped",
              "params": {k: (list(v) if isinstance(v, tuple) else v) for k, v in OAK.items()}, "rungs": {}}
    near_cards = None
    for rung in ("near", "mid"):
        curve, leaves = grow(rung)
        wood_lines = kept_polylines(curve, RUNGS[rung]["twig_min_radius_m"])
        positions, normals, indices, kept = wood_mesh(curve, RUNGS[rung])
        cards = leaf_cards(leaves, OAK["seed"], wood_lines)
        if rung == "near":
            near_cards = cards
        else:
            cards = thin_for_mid(near_cards)
        tip = max(p[1] for p in positions)
        tip_cards = max(c[0].z + abs(c[2].z) + abs(c[1].z) for c in cards) if cards else 0.0
        base_radius = max(math.hypot(p[0], p[2]) for p in positions if p[1] < 0.3)
        write_bin(os.path.join(out, f"tree_{rung}.bin"), positions, normals, indices, cards)
        report["rungs"][rung] = {
            "wood_vertices": len(positions), "wood_triangles": len(indices) // 3, "limbs_kept": kept,
            "cards": len(cards), "tip_wood_m": round(tip, 3), "tip_cards_m": round(tip_cards, 3),
            "butt_radius_m": round(base_radius, 3),
        }
        print("RUNG", rung, report["rungs"][rung], flush=True)
    with open(os.path.join(out, "tree.json"), "w", encoding="utf-8") as handle:
        json.dump(report, handle, indent=2)
    print("TREE DONE", flush=True)


if __name__ == "__main__":
    main()
