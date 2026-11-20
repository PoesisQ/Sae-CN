"""Check the mode wheel silhouette and zoom disc in native Windows snapshots."""
import argparse
import json
from pathlib import Path

from PIL import Image


def profile(image, center, span, seam, scale):
    paper = (255, 255, 255)
    heights = []
    for fraction in (-1.05, -.75, -.5, -.25, 0, .25, .5, .75, 1.05):
        x = round(center + fraction * span / 2)
        y = seam
        while y < seam + round(40 * scale) and image.getpixel((x, y)) != paper:
            y += 1
        heights.append(y - seam)
    assert heights[0] <= 3 and heights[-1] <= 3, heights
    assert 26 * scale <= heights[4] <= 31 * scale, heights
    assert all(a < b for a, b in zip(heights[:4], heights[1:5])), heights
    assert all(abs(a - b) <= 2 for a, b in zip(heights, reversed(heights))), heights
    return heights


def disc(image, cx, cy, scale):
    radius = 43 * scale
    left, top = round(cx - radius - 2), round(cy - radius - 2)
    crop = image.crop((left, top, round(cx + radius + 3), round(cy + radius + 3)))
    face = (244, 242, 234)
    mask = crop.convert("L")
    mask.putdata([255 if color == face else 0 for color in crop.getdata()])
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
    args = parser.parse_args()
    report = json.loads(args.report.read_text(encoding="utf-8"))
    assert report["scenario"] == "instruments" and not report["errors"]
    scale = report["dpi"]
    images = args.report.with_name(args.report.stem + "-screenshots")
    with Image.open(images / "instrument-04.png") as source:
        image = source.convert("RGB")
    wheel_center = image.width - (16 + 88 + 198 / 2) * scale
    outline = profile(image, wheel_center, 198 * scale, round(52 * scale), scale)
    with Image.open(images / "instrument-19.png") as source:
        image = source.convert("RGB")
    dock_right = (image.width + 528 * scale) / 2
    dimensions = disc(image, dock_right - 60 * scale, image.height - 84 * scale, scale)
    print(json.dumps({"scale": scale, "arc_below_toolbar_physical_px": outline,
                      "zoom_face_physical_px": dimensions}))


if __name__ == "__main__":
    main()
