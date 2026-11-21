"""Check full bleed and wordmark contrast against the exact native animation frame."""
import argparse
import json
from pathlib import Path

from PIL import Image, ImageFilter
import numpy as np


def measure(path, kind, dpi):
    with Image.open(path) as source:
        shot = source.convert("RGB")
    w, h = shot.size
    frame = path.with_suffix(".field.png")
    if not frame.exists():
        frame = Path(__file__).resolve().parents[1] / "assets/home" / f"{kind}-still.png"
    with Image.open(frame) as source:
        field = source.convert("RGB")
    small = (48, 32)
    actual = np.asarray(shot.resize(small, Image.Resampling.BOX), dtype=float)
    # Slint's Windows software renderer advances texture coordinates in 8.8
    # fixed point from the top left. The last source texel can repeat at high DPI.
    tx = (np.arange(w) * max(1, field.width*256//w) // 256) % field.width
    ty = (np.arange(h) * max(1, field.height*256//h) // 256) % field.height
    underlying = Image.fromarray(np.asarray(field)[ty[:,None],tx[None,:]])
    expected = np.asarray(underlying.resize(small, Image.Resampling.BOX), dtype=float)
    # Each cell is much larger than a grain, so different subpixel filters in
    # Pillow and Slint average out. Exclude only cells touched by type or the CTA.
    inset = min(max(min(w,h)/16,24*dpi),64*dpi)
    logo_w = min(.30*w,.40*h,356*dpi)
    logo_h = logo_w*753/1395
    x = inset - logo_w*.032
    action_y = h-inset-38*dpi
    y = action_y-inset*.32-logo_h
    boxes = [(x,y,logo_w,logo_h),(inset,action_y,116*dpi,38*dpi)]
    free = np.ones((small[1],small[0]), dtype=bool)
    for bx,by,bw,bh in boxes:
        free[int(by/h*small[1]):int((by+bh)/h*small[1])+1,
             int(bx/w*small[0]):int((bx+bw)/w*small[0])+1] = False
    err = abs(actual-expected).max(axis=2)[free]
    assert np.quantile(err,.99) < 6.5 and err.max() < 15, (path, err.max(), np.quantile(err,.99))
    # Read the canonical glyph crossings independently of the Rust rasterizer.
    # Erode the test region so antialiasing at the letter edges is excluded.
    words = np.fromfile(Path(__file__).resolve().parents[1] / "assets/home/wordmark.spans", dtype="<u2")
    offsets, edges = words[:1025], words[1025:]
    lw, lh = round(logo_w), round(logo_h)
    gx, gy = round(x), round(y)
    mask = np.zeros((lh, lw), dtype=np.uint8)
    for row in range(lh):
        band = min(1023, (2*row+1)*1024//(2*lh))
        for left, right in edges[offsets[band]:offsets[band+1]].reshape(-1, 2):
            a, b = (round(int(v)*lw/65535) for v in (left, right))
            mask[row, a:b] = 255
    interior = np.asarray(Image.fromarray(mask).filter(ImageFilter.MinFilter(7))) == 255
    assert interior.sum() > 180, (path, "missing glyph samples", interior.sum())
    actual_mark = np.asarray(shot, dtype=np.int16)[gy:gy+lh,gx:gx+lw]
    backdrop = np.asarray(underlying, dtype=np.int16)[gy:gy+lh,gx:gx+lw]
    shown, base = actual_mark[interior], backdrop[interior]
    def luminance(values):
        rgb = values / 255.
        return np.where(rgb <= .04045, rgb/12.92, ((rgb+.055)/1.055)**2.4) @ [.2126,.7152,.0722]
    if kind == "blue":
        light = 1. - luminance(backdrop)
        gray = np.rint(255 * np.where(light <= .0031308, 12.92*light, 1.055*light**(1/2.4)-.055))
        ink = gray[:, :, None]
    else:
        shift = 255 - backdrop.max(axis=2) - backdrop.min(axis=2)
        ink = backdrop + shift[:, :, None]
    negative = ink[interior]
    # Antialiasing can only blend this frame's background and its own ink.
    edge = (np.asarray(Image.fromarray(mask).filter(ImageFilter.MaxFilter(5))) > 0) & ~interior
    low, high = np.minimum(ink[edge],backdrop[edge]), np.maximum(ink[edge],backdrop[edge])
    error = np.maximum(low-actual_mark[edge],actual_mark[edge]-high).max(axis=1)
    edge_match = round(float(np.mean(error <= 3)),5)
    assert edge_match > .97, (path, "unexpected ink on the contour", edge_match)
    delta = np.abs(shown-negative).max(axis=1)
    fg, bg = luminance(shown), luminance(base)
    ratios = (np.maximum(fg,bg)+.05)/(np.minimum(fg,bg)+.05)
    cream = np.all(np.asarray(shot) == (243,239,228),axis=2).mean()*100
    assert cream < .1, (path,cream)
    match = float(np.mean(delta <= 2))
    assert match > .96, (path, "inverse pixel agreement", match, np.quantile(delta,[.5,.90,.99]))
    return {"capture":path.stem,"pixels":[w,h],"old_paper_percent":round(float(cream),4),
            "field_99pct_error_rgb_levels":round(float(np.quantile(err,.99)),2),
            "compared_stroke_pixels":int(interior.sum()),"inverse_match_within_2_levels":round(match,5),
            "median_stroke_contrast":round(float(np.median(ratios)),2),
            "contour_blend_agreement":edge_match}


if __name__ == "__main__":
    parser=argparse.ArgumentParser()
    parser.add_argument("captures", type=Path, nargs="+")
    parser.add_argument("--kind",choices=["blue","plasma"],default="plasma",help="Cover ink and static frame")
    parser.add_argument("--dpi",type=float,default=1.5,help="Physical pixels per logical pixel")
    args=parser.parse_args()
    print(json.dumps([measure(path,args.kind,args.dpi) for path in args.captures]))
