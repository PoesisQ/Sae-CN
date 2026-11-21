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
"""
        sheet += "".join(f"0.8 w 0.15 0.27 0.31 RG {x} 615 m {x+119} 800 l S\n"
                         for x in range(582, 1015, 13))
        page = len(objects)
        pages.append(page)
        payload = sheet.encode("ascii")
        objects.extend([f"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 1024 800] /Resources << /Font << /F1 3 0 R >> >> /Contents {page+1} 0 R >>".encode(),
                        f"<< /Length {len(payload)} >>\nstream\n".encode() + payload + b"endstream"])
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
    assert 72 <= samples[2]["radius"] < samples[3]["radius"] <= 100.1
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
    return {"rest_radius": samples[2]["radius"], "expanded_radius": r,
            "region_median_rgb": colors, "bare_edge_energy": round(bare,3),
            "glass_edge_energy": round(frosted,3), "builds": samples[6]["builds"],
            "mean_build_us": round(samples[6]["total_us"] / max(1,samples[6]["builds"]),1),
            "idle_builds": samples[6]["builds"] - samples[5]["builds"]}


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
    (args.output / "optics.json").write_bytes((json.dumps(data,indent=2) + "\n").encode())
    print(json.dumps(data))
