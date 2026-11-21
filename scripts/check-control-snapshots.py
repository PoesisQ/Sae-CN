"""Check the mode wheel silhouette and zoom disc in native Windows snapshots."""
import argparse
import json
import math
from pathlib import Path
from statistics import median

from PIL import Image


def profile(image, center, radius, scale):
    deviations = []
    # Look for the meniscus against the neighbouring backdrop on a radial
    # scan. Transmitted page colour varies across the lens and between pages.
    for fraction in (-.85,-.6,-.3,.3,.6,.85):
        ny = math.sqrt(1-fraction*fraction)
        def pixel(r):
            return image.getpixel((round(center+fraction*r),round(ny*r)))
        def gradient(d):
            a, b = pixel(radius+d-scale), pixel(radius+d+scale)
            return max(abs(x-y) for x,y in zip(a,b))
        candidates = range(-math.ceil(2.5*scale),math.ceil(2.5*scale)+1)
        d = max(candidates,key=gradient)
        assert gradient(d) >= 8, (fraction,d,gradient(d))
        deviations.append(d)
    return deviations


def tick_span(folder):
    data = json.loads((folder / "gestures.json").read_text(encoding="utf-8"))
    count = 2 if "epub-end" in data["captures"] else 3
    samples = {}
    for name, pose in data["view_poses"].items():
        with Image.open(folder / data["captures"][name]) as source:
            image = source.convert("RGB")
        scale = pose["dpi"]
        radius = pose["depth"] + pose["header"]
        center = pose["mode_x"]
        ink = (213,223,215) if "dark" in name else (61,80,77)
        found = []
        for tooth in range(-16, 33):
            degrees = tooth * 6 - pose["mode_position"] * 48
            if abs(degrees) > 85:
                continue
            angle = math.radians(degrees)
            x = round((center + (radius - 8) * math.sin(angle)) * scale)
            y = round((radius - 8) * math.cos(angle) * scale)
            delta = max(1, round(1.25 * scale))
            crop = image.crop((x - delta, y - delta, x + delta + 1, y + delta + 1))
            painted = any(max(abs(c-target) for c,target in zip(pixel,ink)) < 56 for pixel in crop.getdata())
            assert painted == (0 <= tooth <= (count - 1) * 8), (name, tooth, degrees, painted)
            if painted:
                found.append(tooth)
        samples[name] = len(found)
    assert "drum" in samples and "drum-dark" in samples, "Missing light or dark scale capture"
    return samples


def disc(image, cx, cy, scale):
    radius = 34 * scale
    radii = []
    # Sample the lip on all sides, including the diagonals where a square
    # has no edge. Three neighbouring rays keep a tick or the index from
    # dominating, and a narrow radial band excludes the soft contact shadow.
    tolerance = math.ceil(2 * scale)
    distances = range(-tolerance, tolerance + 1)
    for ray in range(16):
        angle = ray * math.tau / 16
        rays = [(math.cos(angle + off), math.sin(angle + off)) for off in (-.055, 0, .055)]
        def color(dx, dy, offset):
            return image.getpixel((round(cx + dx * (radius + offset) - .5),
                                   round(cy + dy * (radius + offset) - .5)))
        strengths = []
        for offset in distances:
            gradients = []
            for dx, dy in rays:
                a, b = color(dx, dy, offset - .6 * scale), color(dx, dy, offset + .6 * scale)
                gradients.append(max(abs(x - y) for x, y in zip(a, b)))
            strengths.append(median(gradients))
        peak = max(strengths)
        assert peak >= 8, ("Missing circular rim", ray, peak)
        edges = [(d, g) for d, g in zip(distances, strengths) if g >= .7 * peak]
        radii.append(radius + sum(d*g for d, g in edges) / sum(g for _, g in edges))
    width, height = radii[0] + radii[8], radii[4] + radii[12]
    assert max(abs(width - height), abs(width - 2*radius), abs(height - 2*radius)) <= 2.5 * scale
    return [round(width), round(height)]


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("report", type=Path, help="instruments/fixture-00.json")
    parser.add_argument("--wheel-samples", type=Path, nargs="+", help="check-wheel-input.py output directories")
    args = parser.parse_args()
    report = json.loads(args.report.read_text(encoding="utf-8"))
    assert report["scenario"] == "instruments" and not report["errors"]
    scale = report["dpi"]
    images = args.report.with_name(args.report.stem + "-screenshots")
    with Image.open(images / "instrument-04.png") as source:
        image = source.convert("RGB")
    pose = next(s for s in report["samples"] if s["step"] == 4)
    assert abs(pose["mode_radius"] - 78) < .1
    outline = profile(image, pose["mode_x"] * scale, pose["mode_radius"] * scale, scale)
    with Image.open(images / "instrument-19.png") as source:
        image = source.convert("RGB")
    dock_right = (image.width + 516 * scale) / 2
    dimensions = disc(image, dock_right - 50 * scale, image.height - 60 * scale, scale)
    counts = [tick_span(folder) for folder in args.wheel_samples or []]
    print(json.dumps({"scale": scale, "circular_edge_error_px": outline,
                      "zoom_face_physical_px": dimensions, "ticks_on_visible_arc": counts}))


if __name__ == "__main__":
    main()
