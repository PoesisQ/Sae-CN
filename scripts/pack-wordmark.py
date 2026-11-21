"""Compile Nimbus outlines into subpixel scan intersections for the native cover.

Authoring only (fontTools). Regenerate after changing the path in wordmark.slint.
The reader samples this small immutable table directly without loading a font.
"""
import re
import struct
from pathlib import Path

from fontTools.pens.basePen import BasePen
from fontTools.svgLib.path import parse_path

ROOT = Path(__file__).resolve().parents[1]
BANDS = 1024
WIDTH, HEIGHT = 1395, 753


class Contours(BasePen):
    def __init__(self):
        super().__init__(None)
        self.lines = []
        self.start = None

    def _moveTo(self, point):
        self.start = point

    def _lineTo(self, point):
        self.lines.append((self._getCurrentPoint(), point))

    def _curveToOne(self, b, c, d):
        a = self._getCurrentPoint()
        previous = a
        for i in range(1, 49):
            t = i / 48
            u = 1 - t
            point = tuple(u**3*a[k] + 3*u*u*t*b[k] + 3*u*t*t*c[k] + t**3*d[k] for k in (0, 1))
            self.lines.append((previous, point))
            previous = point

    def _closePath(self):
        self._lineTo(self.start)


if __name__ == "__main__":
    source = (ROOT / "ui/wordmark.slint").read_text(encoding="utf-8")
    pen = Contours()
    parse_path(re.search(r'commands:\s*"([^"]+)"', source)[1], pen)
    rows = [[] for _ in range(BANDS)]
    for a, b in pen.lines:
        if a[1] > b[1]:
            a, b = b, a
        if b[1] == a[1]:
            continue
        for row in range(max(0, int(a[1] * BANDS / HEIGHT) - 1), min(BANDS, int(b[1] * BANDS / HEIGHT) + 2)):
            y = (row + .5) * HEIGHT / BANDS
            if a[1] <= y < b[1]:
                x = a[0] + (b[0] - a[0]) * (y - a[1]) / (b[1] - a[1])
                rows[row].append(round(x / WIDTH * 65535))
    offsets, edges = [], []
    for row in rows:
        assert len(row) % 2 == 0
        offsets.append(len(edges))
        edges.extend(sorted(row))
    words = offsets + [len(edges)] + edges
    (ROOT / "assets/home/wordmark.spans").write_bytes(struct.pack(f"<{len(words)}H", *words))
