"""Measure the actual paper footprint and detect kinks or gaps in a native cover capture."""
import argparse
import json
from pathlib import Path

from PIL import Image, ImageChops


def measure(path):
    with Image.open(path) as source:
        image = source.convert("RGB")
    def paper_mask(tolerance):
        channels = [ch.point([255 if abs(i - value) <= tolerance else 0 for i in range(256)])
                    for ch, value in zip(image.split(), (243, 239, 228))]
        return ImageChops.darker(ImageChops.darker(channels[0], channels[1]), channels[2])

    w, h = image.size
    coverage = paper_mask(0).histogram()[255] / (w * h)
    assert 0 < coverage < .25, (path, coverage)
    # At fractional scale the last antialiased row can round one RGB level
    # below the opaque fill. Include it when tracking the geometric boundary.
    paper = paper_mask(1)
    run = bytes([255]) * 5
    # On each scanline find the last contiguous paper run. Individual grain
    # samples can match the colour; the wordmark and action are holes inside it.
    def edge(y):
        row = paper.crop((0, y, w, y + 1)).tobytes()
        end = row.rfind(run)
        return end + len(run) - 1 if end >= 0 else -1

    left = paper.crop((2, 0, 3, h)).tobytes()
    start = left.find(run)
    assert start > h * .1 and all(left[start:]), (path, "left edge broken", start)
    bottom = edge(h - 1)
    slope = (bottom - 2) / (h - 1 - start)
    assert 1.47 < slope < 1.53, (path, "unexpected diagonal", slope)
    for y in range(start + 8, h, 3):
        assert abs(edge(y) - (2 + slope * (y - start))) < 3, (path, "displaced paper", y)
    return {"pixels": [w, h], "paper_percent": round(coverage * 100, 2)}


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("captures", type=Path, nargs="+", help="Full-size Plasma or Blue Grain client images")
    args = parser.parse_args()
    print(json.dumps([measure(path) for path in args.captures]))
