"""Check full bleed and wordmark contrast against the exact native animation frame."""
import argparse
import json
from pathlib import Path

from PIL import Image
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
    rect = (int(x),int(y),int(x+logo_w),int(y+logo_h))
    glyphs = np.asarray(shot.crop(rect))
    inks = [(255,250,240),(17,30,37)]
    masks = [np.all(glyphs==ink,axis=2) for ink in inks]
    sel = int(masks[1].sum() > masks[0].sum())
    mask = masks[sel]
    assert .10 < mask.mean() < .43, (path, "missing wordmark", mask.mean())
    sample = np.asarray(underlying.crop(rect))[mask]
    def luminance(values):
        rgb = np.asarray(values)/255
        return np.where(rgb <= .04045, rgb/12.92, ((rgb+.055)/1.055)**2.4) @ [.2126,.7152,.0722]
    fg, bg = luminance(inks[sel]), luminance(sample)
    ratios = (np.maximum(fg,bg)+.05)/(np.minimum(fg,bg)+.05)
    assert np.median(ratios) >= 3.0, (path,"wordmark contrast",np.median(ratios))
    cream = np.all(np.asarray(shot) == (243,239,228),axis=2).mean()*100
    assert cream < .1, (path,cream)
    return {"pixels":[w,h],"old_paper_percent":round(float(cream),4),
            "field_99pct_error_rgb_levels":round(float(np.quantile(err,.99)),2),
            "wordmark_ink":["ivory","ink"][sel],"median_stroke_contrast":round(float(np.median(ratios)),2),
            "stroke_fraction_above_3":round(float(np.mean(ratios>=3)),3)}


if __name__ == "__main__":
    parser=argparse.ArgumentParser()
    parser.add_argument("captures", type=Path, nargs="+")
    parser.add_argument("--kind",choices=["blue","plasma"],default="plasma",help="Fallback for static frames")
    parser.add_argument("--dpi",type=float,default=1.5,help="Physical pixels per logical pixel")
    args=parser.parse_args()
    print(json.dumps([measure(path,args.kind,args.dpi) for path in args.captures]))
