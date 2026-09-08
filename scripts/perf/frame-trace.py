"""Read a `WOT_FRAME_LOG` trace (`<log>.frames.csv`) and say what the session's FPS actually did.

The report beside the trace summarises the run; this reads the RAW rows — one per presented
frame — and answers the questions a summary cannot: which second did the frame rate fall in,
how long did it stay down, and what was the frame doing while it was down.

    python scripts/perf/frame-trace.py output/perf/play-3min.frames.csv [--chart out.png]

Prints: the run's percentiles, the share of frames over the 60 Hz budget, the per-second FPS
timeline (min/mean), and the worst seconds with the phase that owned them. With --chart it
writes a PNG: instantaneous FPS per frame, the 1 s minimum under it, and the 60/30 FPS lines.
"""

import argparse
import csv
import sys
from collections import defaultdict

BUDGET_MS = 1000.0 / 60.0

PHASES = [
    "fixed_ticks_ms",
    "bookkeeping_ms",
    "camera_ms",
    "scene_assembly_ms",
    "hud_ms",
    "upload_ms",
    "render_ms",
    "wait_ms",
]


def read(path):
    with open(path, newline="", encoding="utf-8") as handle:
        rows = list(csv.DictReader(handle))
    if not rows:
        sys.exit(f"{path}: no frames")
    return rows


def percentile(values, p):
    ordered = sorted(values)
    index = round((len(ordered) - 1) * p)
    return ordered[index]


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("csv_path")
    parser.add_argument("--chart", default=None, help="write a PNG of the FPS timeline")
    parser.add_argument("--from-s", type=float, default=0.0, help="ignore frames before this second")
    args = parser.parse_args()

    rows = [r for r in read(args.csv_path) if float(r["at_s"]) >= args.from_s]
    total_ms = [float(r["total_ms"]) for r in rows]
    at_s = [float(r["at_s"]) for r in rows]
    fps = [1000.0 / ms if ms > 0 else 0.0 for ms in total_ms]
    span = at_s[-1] - at_s[0]

    print(f"{args.csv_path}: {len(rows)} frames over {span:.1f} s")
    print(f"  mean FPS      {len(rows) / max(span, 1e-6):>7.1f}")
    for label, p in (("p50", 0.50), ("p95", 0.95), ("p99", 0.99), ("max", 1.0)):
        ms = percentile(total_ms, p)
        print(f"  {label} frame     {ms:>7.2f} ms  ({1000.0 / ms if ms else 0:.1f} FPS)")
    over = sum(1 for ms in total_ms if ms > BUDGET_MS + 0.5)
    print(f"  over 17.2 ms  {over:>7} frames ({100.0 * over / len(rows):.1f} %)")
    for limit in (25.0, 33.3, 50.0):
        n = sum(1 for ms in total_ms if ms > limit)
        print(f"  over {limit:>5.1f} ms {n:>7} frames ({100.0 * n / len(rows):.1f} %)")

    # Per-second buckets: the timeline a player would describe as "it dropped around 0:40".
    buckets = defaultdict(list)
    for second, frame_fps in zip((int(t) for t in at_s), fps):
        buckets[second].append(frame_fps)
    seconds = sorted(buckets)
    print("\nper second (frames, mean FPS, worst frame FPS):")
    for second in seconds:
        values = buckets[second]
        bar = "#" * int(min(sum(values) / len(values), 90) / 2)
        print(
            f"  {second // 60:d}:{second % 60:02d}  {len(values):>3}  "
            f"{sum(values) / len(values):>5.1f}  {min(values):>5.1f}  {bar}"
        )

    worst = sorted(rows, key=lambda r: -float(r["total_ms"]))[:12]
    print("\nthe longest frames and what owned them:")
    for row in worst:
        owner = max(PHASES, key=lambda phase: float(row[phase]))
        print(
            f"  t {float(row['at_s']):>7.1f} s  {float(row['total_ms']):>8.2f} ms  "
            f"{owner[:-3]} {float(row[owner]):.2f} ms  ticks {row['fixed_ticks']}"
        )

    means = {phase: sum(float(r[phase]) for r in rows) / len(rows) for phase in PHASES}
    print("\nmean CPU phase over every traced frame (ms):")
    for phase, mean in sorted(means.items(), key=lambda kv: -kv[1]):
        print(f"  {phase[:-3]:<16} {mean:>7.3f}")

    if args.chart:
        import matplotlib

        matplotlib.use("Agg")
        import matplotlib.pyplot as plt

        floor = [min(buckets[s]) for s in seconds]
        mean = [sum(buckets[s]) / len(buckets[s]) for s in seconds]
        centre = [s + 0.5 for s in seconds]
        figure, axes = plt.subplots(figsize=(14, 5))
        # Per-frame FPS is the evidence but it is spiky: a 3 ms frame paired with a 60 ms one
        # reads as 300 FPS and would own the axis. Draw it faint, and put the two lines a player
        # would actually describe on top — the second's mean and the second's WORST frame.
        axes.plot(at_s, fps, linewidth=0.3, color="#b8cdf0", label="frame FPS")
        axes.plot(centre, mean, linewidth=1.4, color="#2f6fd0", label="mean FPS each second")
        axes.plot(centre, floor, linewidth=1.6, color="#c0392b",
                  label="worst frame each second")
        axes.axhline(60, color="#2c3e50", linewidth=1.0, linestyle="--", label="60 FPS")
        axes.axhline(30, color="#95a5a6", linewidth=1.0, linestyle=":", label="30 FPS")
        axes.set_ylim(0, 80)
        axes.set_xlabel("seconds of play")
        axes.set_ylabel("FPS")
        axes.set_title(args.csv_path)
        axes.legend(loc="lower right")
        axes.grid(alpha=0.25)
        figure.tight_layout()
        figure.savefig(args.chart, dpi=110)
        print(f"\nchart: {args.chart}")


if __name__ == "__main__":
    main()
