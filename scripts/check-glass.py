"""Exercise a native reader on two original colour plates and measure the optical layer."""
import argparse
import json
import subprocess
import sys
from pathlib import Path

from PIL import Image, ImageChops, ImageStat


def plates(path):
    pages = []
    objects = [b"", b"<< /Type /Catalog /Pages 2 0 R >>", b"",
               b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>"]
    for i, (field, light, name) in enumerate([
            ("0.08 0.42 0.62", "0.40 0.82 0.86", "Cyan"),
            ("0.82 0.24 0.13", "0.98 0.62 0.26", "Vermilion")]):
        sheet = f"""0.96 0.94 0.89 rg 0 0 1024 800 re f
{field} rg 0 615 1024 185 re f
{light} rg 765 800 m 811 800 l 930 615 l 884 615 l h f
1 1 1 rg BT /F1 44 Tf 45 711 Td (Plate 0{i+1}) Tj ET
0.09 0.13 0.15 rg BT /F1 56 Tf 48 512 Td ({name}) Tj ET
BT /F1 16 Tf 49 476 Td (Colour and line / Sae Reader) Tj ET
{light} rg 0 0 1024 240 re f
{field} rg 530 0 m 805 320 l 1024 320 l 1024 0 l h f
"""
        sheet += "".join(f"0.8 w 0.15 0.27 0.31 RG {x} 615 m {x+119} 800 l S\n"
                         for x in range(582, 1015, 13))
        page = len(objects)
        pages.append(page)
        payload = sheet.encode("ascii")
        objects.extend([f"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 1024 800] /Resources << /Font << /F1 3 0 R >> >> /Contents {page+1} 0 R >>".encode(),
                        f"<< /Length {len(payload)} >>\nstream\n".encode() + payload + b"endstream"])
    outline = len(objects)
    objects[1] = f"<< /Type /Catalog /Pages 2 0 R /Outlines {outline} 0 R >>".encode()
    objects.append(f"<< /Type /Outlines /First {outline+1} 0 R /Last {outline+2} 0 R /Count 2 >>".encode())
    for i, name in enumerate(("Cyan", "Vermilion")):
        neighbor = f"/Next {outline+2} 0 R" if i == 0 else f"/Prev {outline+1} 0 R"
        objects.append(f"<< /Title ({name}) /Parent {outline} 0 R {neighbor} /Dest [{pages[i]} 0 R /Fit] >>".encode())
    objects[2] = ("<< /Type /Pages /Count 2 /Kids [" + " ".join(f"{n} 0 R" for n in pages) + "] >>").encode()
    output = bytearray(b"%PDF-1.7\n")
    offsets = [0]
    for n, obj in enumerate(objects[1:], 1):
        offsets.append(len(output))
        output.extend(f"{n} 0 obj\n".encode() + obj + b"\nendobj\n")
    xref = len(output)
    output.extend(f"xref\n0 {len(objects)}\n0000000000 65535 f \n".encode())
    for offset in offsets[1:]:
        output.extend(f"{offset:010} 00000 n \n".encode())
    output.extend(f"trailer\n<< /Size {len(objects)} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n".encode())
    path.write_bytes(output)


def measure(folder):
    report = json.loads((folder / "fixture-00.json").read_text(encoding="utf-8"))
    assert not report["errors"] and report["count"] == 2
    samples = report["samples"]
    assert samples[1]["visible"] is False and samples[2]["visible"] is True
    assert abs(samples[2]["radius"] - 72) < .1 and abs(samples[3]["radius"] - 88) < .1
    assert samples[3]["page"] == 0 and samples[4]["page"] == 1
    assert samples[5]["builds"] == samples[6]["builds"] == samples[7]["builds"]
    assert samples[4]["builds"] > samples[3]["builds"] and samples[5]["builds"] > samples[4]["builds"]
    def read(name):
        with Image.open(folder / "fixture-00-screenshots" / (name + ".png")) as im:
            return im.convert("RGB")
    blue, red = read("raised"), read("next")
    dark, settled = read("night"), read("settled")
    assert ImageChops.difference(dark, settled).getbbox() is None, "The resting glass is changing"
    dpi = report["dpi"]
    def icon_centers(im, ink):
        centers = []
        for cx in (34, im.width / dpi - 78, im.width / dpi - 34):
            crop = im.crop((round((cx-13)*dpi), 0, round((cx+13)*dpi), round(52*dpi)))
            rows = [i // crop.width for i, rgb in enumerate(crop.getdata())
                    if max(abs(c-t) for c,t in zip(rgb,ink)) <= 14]
            assert len(rows) > 10, "Missing toolbar icon"
            mid = (min(rows)+max(rows)+1)/(2*dpi)
            assert abs(mid-26) < 1.5, ("Toolbar ink is off center", cx, mid)
            centers.append(round(mid,2))
        return centers
    centers = {"light": icon_centers(read("rest"),(70,86,79)),
               "dark": icon_centers(dark,(190,203,197))}
    cx, r = samples[3]["center"], samples[3]["radius"]
    box = tuple(round(p*dpi) for p in (cx-r*.48, r*.65, cx-r*.25, r*.79))
    colors = [ImageStat.Stat(im.crop(box)).median for im in (blue, red, dark)]
    assert colors[0][2] - colors[0][0] > 20 and colors[1][0] - colors[1][2] > 20, colors
    assert sum(colors[2]) < sum(colors[1]) - 95, colors
    # The plate's fine diagonal rulings should be substantially softer through the lens.
    def fine_detail(im):
        crop = im.crop(box).convert("L")
        diffs = ImageChops.difference(crop.crop((1,0,crop.width,crop.height)),
                                      crop.crop((0,0,crop.width-1,crop.height)))
        return ImageStat.Stat(diffs).mean[0]
    bare, frosted = fine_detail(read("bare")), fine_detail(blue)
    assert frosted < .65*bare, (bare, frosted)
    return {"rest_radius": samples[2]["radius"], "expanded_radius": r, "toolbar_icon_centers": centers,
            "region_median_rgb": colors, "bare_edge_energy": round(bare,3),
            "glass_edge_energy": round(frosted,3), "builds": samples[6]["builds"],
            "mean_build_us": round(samples[6]["total_us"] / max(1,samples[6]["builds"]),1),
            "idle_builds": samples[6]["builds"] - samples[5]["builds"]}


def measure_surfaces(folder):
    report = json.loads((folder / "fixture-00.json").read_text(encoding="utf-8"))
    assert not report["errors"] and report["count"] == 2
    steps = report["samples"]
    assert len(steps) == 15 and steps[0]["maps"] == steps[1]["maps"] == 0
    assert steps[2]["maps"] > 0 and steps[2]["faces"] >= 2
    assert steps[2]["maps"] == steps[5]["maps"] and steps[6]["maps"] == steps[7]["maps"], steps
    for before, after in ((8, 9), (10, 11)):
        assert (steps[before]["maps"], steps[before]["faces"]) == (steps[after]["maps"], steps[after]["faces"])

    screens = folder / "fixture-00-screenshots"
    def patch(file, x, y):
        with Image.open(screens / (file + ".png")) as frame:
            cx, cy = round(x*frame.width), round(y*frame.height)
            return ImageStat.Stat(frame.crop((cx-5, cy-5, cx+6, cy+6)).convert("RGB")).median
    def blue(rgb):
        return rgb[2] - rgb[0]
    bare = patch("bare", .20, .19)
    rail = patch("rail", .25, .025)
    upper, middle, lower = [patch("contents", .20, y) for y in (.19, .50, .84)]
    light, dark = [patch(name, .90, .19) for name in ("settings", "night")]
    left, right = [patch("dock-open", x, .96) for x in (.40, .65)]
    assert blue(bare) > blue(rail) > blue(upper) > blue(middle) + 12
    assert lower[1] - lower[0] > middle[1] - middle[0] + 12
    assert sum(light) > sum(dark) + 160 and blue(dark) > 5
    assert right[0] < left[0] - 10 and right[1] < left[1] - 10
    for a, b in (("night", "settled"), ("hidden", "rested")):
        with Image.open(screens / (a + ".png")) as first, Image.open(screens / (b + ".png")) as second:
            assert ImageChops.difference(first.convert("RGB"), second.convert("RGB")).getbbox() is None
    # The same panel and tile model stay open while only the page pixels change.
    assert steps[12]["maps"] < steps[13]["maps"] < steps[14]["maps"]
    restored = []
    for x, y in ((.25, .025), (.20, .19)):
        before, inverse, after = [patch(name, x, y) for name in ("original", "inverted", "restored")]
        assert before == after and blue(before) > 6 and blue(inverse) < -6, (before, inverse, after)
        restored.append([before, inverse, after])
    last = steps[-1]
    return {"backdrop_builds": last["maps"], "face_builds": last["faces"],
            "mean_backdrop_us": round(last["map_us"] / last["maps"], 1),
            "mean_face_us": round(last["face_us"] / last["faces"], 1),
            "same_panel_invert_roundtrip_rgb": restored, "idle_builds": 0, "rail_panel_paper_rgb": [rail, upper, bare],
            "panel_over_three_colors": [upper, middle, lower], "settings_light_dark_rgb": [light, dark]}


def measure_neutral_paper(folder):
    report = json.loads((folder / "fixture-00.json").read_text(encoding="utf-8"))
    assert not report["errors"]
    samples = report["samples"]
    assert samples[12]["maps"] < samples[13]["maps"] < samples[14]["maps"]
    result = {}
    for surface, pos, bound in (("rail", (.34, .023), 38), ("contents", (.20, .5), 25)):
        patches = []
        for phase in ("original", "inverted", "restored"):
            with Image.open(folder / "fixture-00-screenshots" / (phase + ".png")) as im:
                x, y = (round(a*b) for a,b in zip(pos, im.size))
                patches.append(ImageStat.Stat(im.crop((x-3,y-3,x+4,y+4)).convert("RGB")).median)
        before, inverse, after = patches
        assert before == after and all(4 < a-b <= bound for a,b in zip(before,inverse)), patches
        result[surface] = patches
    return result


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--exe", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--scale", type=float)
    args = parser.parse_args()
    args.output.mkdir(exist_ok=True, parents=True)
    source = args.output / "colour-plates.pdf"
    plates(source)
    cmd = [sys.executable, str(Path(__file__).with_name("qa-windows.py")), "--scenario", "glass",
           "--no-activate", "--exe", str(args.exe), "--fixture", str(source), "--output", str(args.output)]
    if args.scale:
        cmd += ["--scale", str(args.scale)]
    subprocess.run(cmd, check=True)
    data = measure(args.output)
    surfaces = args.output / "surfaces"
    cmd[cmd.index("glass")] = "surfaces"
    cmd[cmd.index("--output") + 1] = str(surfaces)
    subprocess.run(cmd, check=True)
    data["surfaces"] = measure_surfaces(surfaces)
    paper = args.output / "paper"
    cmd[cmd.index("--output") + 1] = str(paper)
    cmd[cmd.index("--fixture") + 1] = str(Path(__file__).resolve().parents[1] / "tests/fixtures/core.pdf")
    subprocess.run(cmd, check=True)
    data["neutral_paper_roundtrip_rgb"] = measure_neutral_paper(paper)
    (args.output / "optics.json").write_bytes((json.dumps(data,indent=2) + "\n").encode())
    print(json.dumps(data))
