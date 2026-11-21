"""Check full bleed and wordmark contrast against the exact native animation frame."""
import argparse
import json
from pathlib import Path

from PIL import Image, ImageFilter
import numpy as np


def ink_ranges(field, backdrop, tx, ty, gx, gy, kind):
    """Each possible local exposure is within six encoded levels of the sampled field."""
    h, w, _ = backdrop.shape
    x = np.arange(w)
    y = np.arange(h)
    x_sample = tx[gx + np.minimum(np.arange((w-1)//16 + 2)*16, w-1)]
    y_sample = ty[gy + np.minimum(np.arange((h-1)//16 + 2)*16, h-1)]
    field = np.asarray(field, dtype=np.int32)

    def luma(rgb):
        return (rgb.astype(np.int32) @ [54,183,19] + 128) // 256

    total = np.zeros((len(y_sample),len(x_sample)), dtype=np.int32)
    for dy in (-5,-2,2,5):
        for dx in (-5,-2,2,5):
            total += luma(field[np.clip(y_sample+dy,0,field.shape[0]-1)[:,None],
                                np.clip(x_sample+dx,0,field.shape[1]-1)[None,:]])
    sampled = (total+8)//16
    surround = np.pad(sampled,1,mode="edge")
    rows, cols = sampled.shape
    weights = (1,2,1)
    sampled = (sum(wy*wx*surround[j:j+rows,i:i+cols] for j,wy in enumerate(weights)
                   for i,wx in enumerate(weights)) + 8)//16
    rows = ((16-y%16)[:,None]*sampled[y//16] + (y%16)[:,None]*sampled[y//16+1])
    guide = (rows[:,x//16]*(16-x%16) + rows[:,x//16+1]*(x%16) + 128)//256
    base = luma(backdrop)
    ranges = []
    for pale in (False,True):
        lo = np.full_like(backdrop, 255)
        hi = np.zeros_like(backdrop)
        possible = np.zeros_like(base,dtype=bool)
        for d in range(-6,7):
            g = np.clip(guide+d,0,255)
            use = (g < 117) == pale
            possible |= use
            grain = np.trunc((g-base)*3/8).astype(int)
            if pale:
                v = np.clip(252 - g//12 + grain,195,255)
                tint = -((backdrop.max(axis=2)[:,:,None] - backdrop)//3)
            else:
                v = np.clip(5 + (255-g)//11 + grain,0,58)
                tint = (backdrop-backdrop.min(axis=2)[:,:,None])//3
            rgb = v[:,:,None] + (0 if kind == "blue" else tint)
            lo = np.where(use[:,:,None], np.minimum(lo,rgb), lo)
            hi = np.where(use[:,:,None], np.maximum(hi,rgb), hi)
        ranges.append((possible,lo,hi))
    return ranges


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
    logo_w = int(min(.30*w,.40*h,356*dpi)+.5)
    logo_h = int(logo_w*753/1395+.5)
    x = int(inset - logo_w*.032+.5)
    action_y = h-inset-38*dpi
    y = int(action_y-inset*.32-logo_h+.5)
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
    possibilities = ink_ranges(field, backdrop, tx, ty, gx, gy, kind)
    # A complete stroke selects one of two possible inks. Its antialiased
    # contour may only mix that same ink with the actual texel behind it.
    edge = (np.asarray(Image.fromarray(mask).filter(ImageFilter.MaxFilter(5))) > 0) & ~interior
    def error(lo,hi):
        return np.maximum(lo-actual_mark,actual_mark-hi).max(axis=2)
    solid_error = np.full(interior.shape, 255)
    edge_error = solid_error.copy()
    for possible,lo,hi in possibilities:
        solid_error = np.minimum(solid_error,np.where(possible,error(lo,hi),255))
        edge_error = np.minimum(edge_error,np.where(possible,error(np.minimum(lo,backdrop),np.maximum(hi,backdrop)),255))
    edge_match = round(float(np.mean(edge_error[edge] <= 3)),5)
    assert edge_match > .97, (path, "unexpected ink on the contour", edge_match)
    fg, bg = luminance(shown), luminance(base)
    ratios = (np.maximum(fg,bg)+.05)/(np.minimum(fg,bg)+.05)
    cream = np.all(np.asarray(shot) == (243,239,228),axis=2).mean()*100
    assert cream < .1, (path,cream)
    match = float(np.mean(solid_error[interior] <= 2))
    assert match > .96, (path, "same-frame ink agreement", match, np.quantile(solid_error[interior],[.5,.90,.99]))
    if kind == "blue":
        assert np.mean(shown.max(axis=1)-shown.min(axis=1) <= 2) > .98, (path,"Blue ink is not neutral")
    else:
        chroma = base.max(axis=1)-base.min(axis=1)
        vivid = chroma > 36
        if vivid.sum() > 200:
            assert np.mean((shown.max(axis=1)-shown.min(axis=1))[vivid] > 8) > .95, (path,"Plasma chroma lost")
    # Flat and textured grey are common in both covers. A disappearing stroke
    # can be missed by a high median contrast over only the bright/dark extremes.
    grey = (bg >= .085) & (bg <= .63)
    strong = float(np.mean(ratios[grey] >= 3)) if grey.any() else None
    if grey.sum() >= 100:
        assert strong > .78 and np.median(ratios[grey]) >= 3.6, (path,"weak middle-grey lettering",strong)
    pale = actual_mark.max(axis=2) > 161
    boundaries = pairs = 0
    for axis in (0,1):
        neighbors = np.diff(interior.astype(int),axis=axis) == 0
        common = neighbors & np.take(interior,np.arange(interior.shape[axis]-1),axis=axis)
        boundaries += np.sum((np.diff(pale.astype(int),axis=axis) != 0) & common)
        pairs += common.sum()
    fraction = float(boundaries)/max(1,int(pairs))
    assert fraction < .065, (path,"individual grain switched polarity",fraction)
    return {"capture":path.stem,"pixels":[w,h],"old_paper_percent":round(float(cream),4),
            "field_99pct_error_rgb_levels":round(float(np.quantile(err,.99)),2),
            "compared_stroke_pixels":int(interior.sum()),"same_frame_ink_match":round(match,5),
            "median_stroke_contrast":round(float(np.median(ratios)),2),
            "midgray_pixels":int(grey.sum()),"midgray_contrast_p50":round(float(np.median(ratios[grey])),2) if grey.any() else None,
            "midgray_at_least_3":round(strong,4) if strong is not None else None,
            "adjacent_polarity_boundaries":round(fraction,5),"contour_blend_agreement":edge_match}, pale, interior


if __name__ == "__main__":
    parser=argparse.ArgumentParser()
    parser.add_argument("captures", type=Path, nargs="+")
    parser.add_argument("--kind",choices=["blue","plasma"],default="plasma",help="Cover ink and static frame")
    parser.add_argument("--dpi",type=float,default=1.5,help="Physical pixels per logical pixel")
    parser.add_argument("--sequence",action="store_true",help="Require three successive wordmark samples with stable grain polarity")
    args=parser.parse_args()
    values = [measure(path,args.kind,args.dpi) for path in args.captures]
    if args.sequence:
        assert len(values) == 3 and all(v[0]["midgray_pixels"] >= 100 for v in values)
        first, middle, last = values
        mask = first[2] & middle[2] & last[2]
        flicker = np.mean(((first[1] == last[1]) & (middle[1] != first[1]))[mask])
        assert flicker < .002, ("three-frame polarity reversals",flicker)
        middle[0]["three_frame_reversal_fraction"] = round(float(flicker),6)
    print(json.dumps([v[0] for v in values]))
