"""Check a standalone cover giving way to the native page. Run on a dedicated Windows desktop."""
import argparse
import json
import os
from pathlib import Path
import subprocess
import sys
from statistics import median

from PIL import Image, ImageChops, ImageDraw


ROOT = Path(__file__).resolve().parents[1]


def read_rgb(path):
    with Image.open(path) as image:
        return image.convert("RGB")


def page_region(path):
    with Image.open(path) as image:
        w, h = image.size
        # Inside the portrait page, clear of the masthead, page controls and cover wordmark.
        return image.crop((round(.31*w), round(.14*h), round(.68*w), round(.68*h))).convert("RGB")


def distance(a, b):
    red, green, blue = ImageChops.difference(a, b).split()
    return ImageChops.lighter(red, ImageChops.lighter(green, blue))


def count(mask):
    return mask.histogram()[255]


def measure_mix(folder, samples):
    frozen = page_region(folder / "frozen.png")
    book = page_region(folder / "book.png")
    # Pale paper and a pale cover can be indistinguishable. Compare only places
    # where the actual cover and the final page differ substantially.
    distinct = distance(frozen, book).point(lambda p: 255 if p >= 72 else 0)
    ink = ImageChops.darker(distinct, book.convert("L").point(lambda p: 255 if p <= 150 else 0))
    total, text = count(distinct), count(ink)
    assert total > 1000 and text > 25, ("The test page offers too little evidence", total, text)
    readings = []
    for sample in (s for s in samples if "mix" in s):
        image = page_region(folder / f"mix-{sample['mix']}.png")
        paper_match = distance(image, book).point(lambda p: 255 if p <= 13 else 0)
        cover_match = distance(image, frozen).point(lambda p: 255 if p <= 18 else 0)
        readings.append({
            "progress": round(sample["progress"], 3),
            "page": round(count(ImageChops.darker(distinct, paper_match)) / total, 3),
            "ink": round(count(ImageChops.darker(ink, paper_match)) / text, 3),
            "cover": round(count(ImageChops.darker(distinct, cover_match)) / total, 3),
        })
    assert len(readings) >= 2, ("No sampled reveal", readings)
    first, last = readings[0], readings[-1]
    assert last["page"] > first["page"] + .20 and last["cover"] < first["cover"] - .20, readings
    assert any(r["ink"] > .06 and r["cover"] > .06 for r in readings), (
        "No intermediate frame showed both real page ink and cover grains", readings)
    return readings


def measure_distribution(folder, samples):
    cover = read_rgb(folder / "frozen.png")
    page = read_rgb(folder / "book.png")
    w, h = page.size
    eligible = distance(cover, page).point(lambda p: 255 if p >= 72 else 0)
    # The logo and action fade independently of the grains. Keep the actual
    # margins around them so that the lower-left corner can still contribute.
    mask = ImageDraw.Draw(eligible)
    for x0, y0, x1, y1 in ((.025, .55, .355, .85), (.025, .845, .26, .95)):
        mask.rectangle((round(x0*w), round(y0*h), round(x1*w), round(y1*h)), fill=0)
    cells = {}
    for row in range(3):
        for col in range(3):
            box = (round(w*(.02+col*.32)), round(h*(.04+row*.30)),
                   round(w*(.02+(col+1)*.32)), round(h*(.04+(row+1)*.30)))
            available = eligible.crop(box)
            pixels = count(available)
            if pixels >= max(80, .02 * available.width * available.height):
                cells[row*3+col] = (box, pixels)
    assert 4 in cells and len(cells) >= 6, ("Too little distinct cover/page detail across the window", list(cells))
    shots = [s for s in samples if "mix" in s]
    early = min(shots, key=lambda s: abs(s["progress"]-.14))
    middle = min(shots, key=lambda s: abs(s["progress"]-.36))
    assert early["progress"] < middle["progress"] < .67, (early, middle)
    result = []
    for sample, floor in ((early, .008), (middle, .05)):
        frame = read_rgb(folder / f"mix-{sample['mix']}.png")
        exposed = ImageChops.darker(eligible, distance(frame, page).point(lambda p: 255 if p <= 13 else 0))
        grains = ImageChops.darker(eligible, distance(frame, cover).point(lambda p: 255 if p <= 34 else 0))
        spread = {i: [round(count(exposed.crop(box))/n, 3), round(count(grains.crop(box))/n, 3)]
                  for i, (box, n) in cells.items()}
        mixed = {i for i, (clear, remaining) in spread.items() if clear >= floor and remaining >= .04}
        assert (4 in mixed and len(mixed) >= len(cells)-1 and len(mixed & {0,2,6,8}) >= 2
                and len(mixed & {1,3,5,7}) >= 2), ("Reveal is confined to part of the page", sample, spread)
        center = spread[4][0]
        outside = median(values[0] for i, values in spread.items() if i != 4)
        assert center - outside < .28, ("Center-first reveal", sample, spread)
        result.append({"progress": round(sample["progress"], 3), "tiles_page_and_grain": spread})
    return result


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--exe", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--scale", type=float, default=1)
    args = parser.parse_args()
    args.output = args.output.resolve()
    results = {}
    reference = None
    resumed_reference = None
    for case in ("direct", "picker", "drop", "static", "reduced", "print", "replace"):
        dest = args.output / case
        cmd = [sys.executable, str(ROOT / "scripts/qa-windows.py"), "--scenario", "opening",
               "--exe", str(args.exe.resolve()), "--output", str(dest), "--scale", str(args.scale)]
        if case == "direct":
            cmd += ["--fixture", str(ROOT / "tests/fixtures/core.pdf")]
            alias = "fixture-00"
        else:
            cmd += ["--blank", "--cover", {"drop": "blue", "static": "blue", "print": "print"}.get(case, "plasma")]
            alias = "blank-00"
        if case == "static":
            cmd.append("--static")
        if case == "drop":
            cmd += ["--initial-state", str(args.output / "direct/fixture-00-profile/state.json")]
        marker = dest / (alias + "-profile") / "next-cover-blue"
        alternation_before = marker.exists()
        subprocess.run(cmd, env=dict(os.environ, SAE_READER_QA_OPEN_CASE=case), check=True)
        report = json.loads((dest / (alias + ".json")).read_text(encoding="utf-8"))
        assert report["exit_code"] == 0 and not report["errors"] and report["document"], (case, report["errors"])
        source = dest / (alias + "-screenshots")
        book = read_rgb(source / "book.png")
        if case == "direct":
            assert marker.exists() == alternation_before, "File launch advanced the standalone cover sequence"
            reference = book
            resumed_reference = read_rgb(source / "resumed-page.png")
            assert distance(book, resumed_reference).getbbox(), "Reference navigation did not change the page"
            different = distance(book, read_rgb(source / "reading-drop.png"))
            assert sum(different.histogram()[40:]) > 500, "A drag while reading left the previous page visible"
        else:
            expected = resumed_reference if case == "drop" else reference
            assert book.size == expected.size and not distance(book, expected).getbbox(), (
                "Final frame differs from the same book opened directly", case)
        samples = report["samples"]
        result = next(s for s in samples if "resources" in s)
        if case in ("picker", "drop", "print"):
            result["exposed_page"] = measure_mix(source, samples)
            result["distribution"] = measure_distribution(source, samples)
        if case == "drop":
            result["resumed_page"] = next(s["resumed_page"] for s in samples if "resumed_page" in s)
        results[case] = result
    (args.output / "opening.json").write_text(json.dumps(results, indent=2) + "\n", encoding="utf-8")
    print(json.dumps(results))


if __name__ == "__main__":
    main()
