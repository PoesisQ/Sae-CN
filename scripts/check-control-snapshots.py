"""Check the mode wheel silhouette and zoom disc in native Windows snapshots."""
import argparse
import json
import math
from pathlib import Path

from PIL import Image, ImageChops


def profile(image, center, radius, scale):
    face = (225, 227, 217)
    deviations = []
    for fraction in (-.85, -.6, -.3, 0, .3, .6, .85):
        x = round(center + fraction * radius)
        edge = math.sqrt(radius**2 - (x - center)**2)
        # The fixed index and teeth interrupt the last few pixels of the face.
        last = max(y for y in range(round(radius + 1)) if image.getpixel((x, y)) == face)
        deviations.append(round(edge - last, 2))
    assert all(-1 <= value <= 13 * scale for value in deviations), deviations
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
        face = image.getpixel((round(center * scale), round(radius * .25 * scale)))
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
            painted = any(max(f - lo, hi - f) > 25 for (lo, hi), f in zip(crop.getextrema(), face))
            assert painted == (0 <= tooth <= (count - 1) * 8), (name, tooth, degrees, painted)
            if painted:
                found.append(tooth)
        samples[name] = len(found)
    assert "drum" in samples and "drum-dark" in samples, "Missing light or dark scale capture"
    return samples


def disc(image, cx, cy, scale):
    radius = 34 * scale
    left, top = round(cx - radius - 2), round(cy - radius - 2)
    crop = image.crop((left, top, round(cx + radius + 3), round(cy + radius + 3)))
    face = (244, 242, 234)
    red, green, blue = ImageChops.difference(crop, Image.new("RGB", crop.size, face)).split()
    mask = ImageChops.lighter(ImageChops.lighter(red, green), blue).point([255] + [0] * 255)
    x0, y0, x1, y1 = mask.getbbox()
    width, height = x1 - x0, y1 - y0
    assert abs(width - height) <= 1 and abs(width - 2 * radius) <= 4, (width, height)
    assert abs(left + (x0 + x1) / 2 - cx) < 2 and abs(top + (y0 + y1) / 2 - cy) < 2
    # The unpainted corners and four radial extremes distinguish a disc from a square.
    assert all(not mask.getpixel(p) for p in ((x0, y0), (x0, y1 - 1), (x1 - 1, y0), (x1 - 1, y1 - 1)))
    return [width, height]


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
    span = min(max(158, image.width / scale * .27), 214)
    wheel_center = image.width - (16 + 88 + span / 2) * scale
    outline = profile(image, wheel_center, min(100, span / 2 - 4) * scale, scale)
    with Image.open(images / "instrument-19.png") as source:
        image = source.convert("RGB")
    dock_right = (image.width + 516 * scale) / 2
    dimensions = disc(image, dock_right - 50 * scale, image.height - 60 * scale, scale)
    counts = [tick_span(folder) for folder in args.wheel_samples or []]
    print(json.dumps({"scale": scale, "circular_edge_error_px": outline,
                      "zoom_face_physical_px": dimensions, "ticks_on_visible_arc": counts}))


if __name__ == "__main__":
    main()
