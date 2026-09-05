"""Trace a vehicle silhouette out of a reference drawing into outline loops (metres, bake frame).

The K0 overlay gate (`vehicle_forge::outline`) compares the bake with closed loops per view. This
script produces those loops FROM A DRAWING instead of from tables: threshold the drawing, drop
thin lines (aerials, scribbles), fill the body, take the largest component, trace its contour,
simplify it, and calibrate pixels to metres on two documented lengths. Nothing is committed but
the numbers; the drawing stays under `output/refs/` (git-ignored) with its licence in
`output/refs/SOURCES.md`.

    python scripts/refs/trace_silhouette.py --image output/refs/t54_1951/T-55_schematic_1978.png \
        --view Side --region 0,340,884,614 --erase 400,352,442,404 --erase 536,348,560,364 \
        --h-extent 9.00 --h-left 5.8475 --flip-h --out output/refs/t54_1951/side.json

Frames (the bake's): Side = (z, y) with +Z the bow; Front = (x, y) with +X port; Plan = (x, z).
`--h-extent` is the metres spanned by the silhouette's horizontal extent (e.g. overall length
gun forward); `--h-left` the bake coordinate of the silhouette's LEFT edge after `--flip-h`
(e.g. the muzzle's z when the gun points left in the drawing). The vertical axis uses the same
scale, anchored at the silhouette's bottom edge (`--v-bottom`, default 0 = the ground) or, with
`--v-centre`, centred (a plan view's x). `--rotate 90` turns a drawing whose bow points left into
bow-up before tracing (plan views). The debug PNG beside the output shows the body and the loop.
"""
import argparse, json, os
import numpy as np
import cv2
from PIL import Image


def load_gray(path):
    img = Image.open(path).convert("RGBA")
    bg = Image.new("RGBA", img.size, (255, 255, 255, 255))
    return np.array(Image.alpha_composite(bg, img).convert("L"))


def body_mask(gray, region, erase, open_px, close_px, thresh):
    x0, y0, x1, y1 = region
    ink = (gray[y0:y1, x0:x1] < thresh).astype(np.uint8)
    for ex0, ey0, ex1, ey1 in erase:
        ink[max(ey0 - y0, 0):max(ey1 - y0, 0), max(ex0 - x0, 0):max(ex1 - x0, 0)] = 0
    if open_px > 1:
        ink = cv2.morphologyEx(ink, cv2.MORPH_OPEN, cv2.getStructuringElement(cv2.MORPH_ELLIPSE, (open_px, open_px)))
    if close_px > 1:
        ink = cv2.morphologyEx(ink, cv2.MORPH_CLOSE, cv2.getStructuringElement(cv2.MORPH_ELLIPSE, (close_px, close_px)))
    # Fill from EVERY border pixel (a one-pixel white frame), so a dimension line that walls off
    # a corner of the region cannot turn the space under the vehicle into "body".
    padded = np.pad(ink, 1)
    flood = np.zeros((padded.shape[0] + 2, padded.shape[1] + 2), np.uint8)
    filled = padded.copy()
    cv2.floodFill(filled, flood, (0, 0), 1)
    filled = filled[1:-1, 1:-1]
    body = ((ink == 1) | (filled == 0)).astype(np.uint8)
    n, labels, stats, _ = cv2.connectedComponentsWithStats(body, 8)
    if n <= 1:
        raise SystemExit("no body found in the region")
    largest = 1 + int(np.argmax(stats[1:, cv2.CC_STAT_AREA]))
    return (labels == largest).astype(np.uint8)


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--image", required=True)
    ap.add_argument("--view", required=True, choices=["Side", "Front", "Plan"])
    ap.add_argument("--region", required=True, help="x0,y0,x1,y1 in image pixels")
    ap.add_argument("--erase", action="append", default=[], help="x0,y0,x1,y1 to blank before tracing (repeatable)")
    ap.add_argument("--h-extent", type=float, required=True, help="metres across the silhouette's horizontal extent")
    ap.add_argument("--h-left", type=float, default=None, help="bake coordinate of the left edge (after flip); default: centred")
    ap.add_argument("--flip-h", action="store_true", help="mirror horizontally (a gun pointing left → +Z bow)")
    ap.add_argument("--rotate", type=int, default=0, choices=[0, 90, 180, 270], help="rotate the region counter-clockwise first")
    ap.add_argument("--v-bottom", type=float, default=0.0, help="bake coordinate of the bottom edge (default 0 = ground)")
    ap.add_argument("--v-centre", action="store_true", help="centre the vertical axis instead of anchoring the bottom")
    ap.add_argument("--flip-v", action="store_true")
    ap.add_argument("--thresh", type=int, default=215)
    ap.add_argument("--open", type=int, default=3)
    ap.add_argument("--close", type=int, default=7)
    ap.add_argument("--eps", type=float, default=1.5, help="contour simplification in pixels")
    ap.add_argument("--out", required=True, help="JSON path for the loop; a _debug.png lands beside it")
    a = ap.parse_args()

    gray = load_gray(a.image)
    region = tuple(int(v) for v in a.region.split(","))
    erase = [tuple(int(v) for v in e.split(",")) for e in a.erase]
    body = body_mask(gray, region, erase, a.open, a.close, a.thresh)
    if a.rotate:
        body = np.rot90(body, k=a.rotate // 90)
    if a.flip_h:
        body = body[:, ::-1]
    if a.flip_v:
        body = body[::-1, :]
    body = np.ascontiguousarray(body)

    contours, _ = cv2.findContours(body, cv2.RETR_EXTERNAL, cv2.CHAIN_APPROX_NONE)
    poly = cv2.approxPolyDP(max(contours, key=cv2.contourArea), a.eps, True).reshape(-1, 2)

    ys, xs = np.where(body == 1)
    x_min, x_max, y_min, y_max = xs.min(), xs.max(), ys.min(), ys.max()
    scale = a.h_extent / (x_max - x_min)
    h_left = a.h_left if a.h_left is not None else -a.h_extent / 2
    if a.v_centre:
        v_of = lambda py: (((y_max + y_min) / 2) - py) * scale
    else:
        v_of = lambda py: a.v_bottom + (y_max - py) * scale
    loop = [[round(h_left + (px - x_min) * scale, 4), round(v_of(py), 4)] for px, py in poly]

    h, w = body.shape
    rgb = np.full((h, w, 3), 255, np.uint8)
    rgb[body == 1] = (120, 120, 120)
    cv2.polylines(rgb, [poly.reshape(-1, 1, 2)], True, (220, 40, 40), 1)
    debug = os.path.splitext(a.out)[0] + "_debug.png"
    Image.fromarray(rgb).save(debug)

    v_extent = (y_max - y_min) * scale
    json.dump({"view": a.view, "image": a.image, "scale_cm_per_px": scale * 100, "h_extent_m": a.h_extent,
               "v_extent_m": v_extent, "points": len(loop), "loop": loop}, open(a.out, "w"), indent=0)
    print(f"{a.view}: {len(loop)} points, scale {scale*100:.3f} cm/px, horizontal {a.h_extent:.3f} m, vertical extent {v_extent:.3f} m")
    print(f"  loop h [{min(p[0] for p in loop):+.3f}, {max(p[0] for p in loop):+.3f}]  v [{min(p[1] for p in loop):+.3f}, {max(p[1] for p in loop):+.3f}]")
    print(f"  wrote {a.out} and {debug}")


if __name__ == "__main__":
    main()
