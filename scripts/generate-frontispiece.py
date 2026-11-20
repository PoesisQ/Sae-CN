"""Draw the folded opening print in homogeneous coordinates using three inks.

The crosswise ruling has one vanishing point. Adjacent planes share their
crease and the same ruling, including when the reverse face falls into shade.
This is an authoring tool. The reader embeds only the completed SVG.
"""
from pathlib import Path

ASSETS = Path(__file__).resolve().parents[1] / "assets"
PAPER = "#f3efe4"
VANISHING_POINT = (2620, -370)
WIDTH_DEPTH = 0.83
# x, y and distance to the viewer along the near edge of the print
FOLDS = [(-420, -330, .85), (780, 380, 1.15), (560, 554, 1.0), (1060, 1240, .90)]


def project(index, across, along):
    x0, y0, z0 = FOLDS[index]
    x1, y1, z1 = FOLDS[index + 1]
    depth = z0 + (z1 - z0) * along + WIDTH_DEPTH * across
    wx, wy = VANISHING_POINT
    x = ((1 - along) * x0 * z0 + along * x1 * z1 + across * WIDTH_DEPTH * wx) / depth
    y = ((1 - along) * y0 * z0 + along * y1 * z1 + across * WIDTH_DEPTH * wy) / depth
    return f"{x:.2f} {y:.2f}"


def strip(index, u0, u1, v0, v1):
    points = [(u0, v0), (u1, v0), (u1, v1), (u0, v1)]
    return "M" + "L".join(project(index, u, v) for u, v in points) + "Z"


def filled(shape, ink):
    return f'<path d="{shape}" fill="{ink}"/>'


def ruling(index, across_range, along_lines, across_lines, ink, weight, opacity):
    left, right = across_range
    lines = []
    for u in across_lines:
        if left < u < right:
            lines.append("M" + project(index, u, 0) + "L" + project(index, u, 1))
    for v in along_lines:
        lines.append("M" + project(index, left, v) + "L" + project(index, right, v))
    return f'<path d="{"".join(lines)}" fill="none" stroke="{ink}" stroke-width="{weight}" opacity="{opacity}"/>'


def build():
    art = ['<svg xmlns="http://www.w3.org/2000/svg" width="1600" height="1000" viewBox="0 0 1600 1000">',
           f'<rect width="1600" height="1000" fill="{PAPER}"/>',
           '<defs><linearGradient id="top" x2=".86" y2="1"><stop stop-color="#f55c30"/><stop offset=".57" stop-color="#ef4c24"/><stop offset="1" stop-color="#c7341d"/></linearGradient>',
           '<linearGradient id="back" x2=".12" y2="1"><stop stop-color="#62271f"/><stop offset="1" stop-color="#8b2d1e"/></linearGradient>',
           '<linearGradient id="base" x2=".34" y2="1"><stop stop-color="#bb331e"/><stop offset=".32" stop-color="#ef4623"/><stop offset="1" stop-color="#fc6032"/></linearGradient></defs>']
    colors = [('url(#top)', '#e4dfd1', '#222c2a', '#fff2d7'),
              ('url(#back)', '#797c70', '#25302e', '#c6bba2'),
              ('url(#base)', '#ddd9cd', '#24302d', '#fff1dc')]
    for face, divisions in enumerate((21, 4, 24)):
        red, stock, black, light = colors[face]
        art.append(filled(strip(face, 0, 1, 0, 1), stock))
        art.append(filled(strip(face, 0, .49, 0, 1), red))
        art.append(filled(strip(face, .80, 1, 0, 1), black))
        across = [i / 22 for i in range(1, 22)]
        along = [i / divisions for i in range(1, divisions)]
        # A sequence of skew cuts opens into solid ink near the near edge.
        # All cuts follow the same longitudinal and transverse projections.
        count = (8, 2, 11)[face]
        for i in range(count):
            v = i / count
            taper = (i + 1) / count if face == 0 else (count - i) / count
            thickness = (.22 + .46 * taper) / count
            a, b = project(face, 0, v), project(face, .49, v)
            ax, ay = a.split()
            bx, by = b.split()
            key = f"slit-{face}-{i}"
            art.append(f'<defs><linearGradient id="{key}" gradientUnits="userSpaceOnUse" x1="{ax}" y1="{ay}" x2="{bx}" y2="{by}"><stop stop-color="#202925"/><stop offset=".26" stop-color="#362722"/><stop offset="1" stop-color="#a33221"/></linearGradient></defs>')
            cut = 'M' + a + 'L' + b + 'L' + project(face, .49, v + thickness*.32) + 'L' + project(face, 0, v + thickness) + 'Z'
            art.append(filled(cut, f'url(#{key})'))
        art.append(ruling(face, (0, .49), along, across, light, .8, .24))
        art.append(ruling(face, (.49, .80), along, across, black, 1.75, .84))
        art.append(ruling(face, (.80, 1), along, across, light, 1.1, .57))
        # One run of dark modules crosses the print. The clipped stair-step
        # reads as a second plane even in a monochrome reproduction.
        for i in range(divisions):
            edge = .49 + ((i // max(1, divisions // 5)) % 4) / 22
            if i % 7 in (0, 1):
                art.append(filled(strip(face, edge, min(edge + 2/22, .79),
                                        i/divisions, (i+1)/divisions), black))
    art.append('</svg>')
    (ASSETS / 'frontispiece.svg').write_text("\n".join(art) + "\n", encoding="utf-8")


if __name__ == '__main__':
    build()
