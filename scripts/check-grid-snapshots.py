"""Check native four-slide and eight-page layouts using original fixture screenshots."""
import argparse
import json
from pathlib import Path

from PIL import Image


def slide_markers(path):
    with Image.open(path) as source:
        image = source.convert("RGB")
    result = []
    for x in (int(image.width * .1), int(image.width * .6)):
        runs = []
        start = None
        for y in range(int(image.height * .1), int(image.height * .92)):
            r, g, b = image.getpixel((x, y))
            green = g > r + 15 and g > b + 5 and 60 < g < 170
            if green and start is None:
                start = y
            elif not green and start is not None:
                if y - start >= 5:
                    runs.append((start, y - 1))
                start = None
        result.append(runs)
    return result


def check_portrait(path):
    with Image.open(path) as source:
        image = source.convert("RGB")
    counts = []
    for row in range(2):
        for column in range(4):
            tile = image.crop((column * image.width // 4, row * image.height // 2,
                               (column + 1) * image.width // 4, (row + 1) * image.height // 2))
            # Every page of core.pdf has a solid green footer. Check each tile
            # individually so that seven visible pages can't mask a blank one.
            count = sum(n for n, color in tile.getcolors(tile.width * tile.height)
                        if color == (79, 107, 96))
            assert count > tile.width * tile.height * .01, (row, column, count)
            counts.append(count)
    return counts


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("folder", type=Path, help="Screenshot directory for the slides.pdf resize test")
    parser.add_argument("--portrait", type=Path, help="Screenshot of core.pdf in a 4 by 2 grid")
    args = parser.parse_args()
    before = slide_markers(args.folder / "grid-01.png")
    after = slide_markers(args.folder / "grid-18.png")
    assert all(len(column) == 2 for column in before + after), (before, after)
    # The floating controls may cover part of the lower marker. Its start
    # and unobscured centre must return to the same position after resizing.
    for old, new in zip(before, after):
        for (old_start, old_end), (new_start, new_end) in zip(old, new):
            assert abs(old_start - new_start) <= 2, (before, after)
            assert new_end >= (old_start + old_end) // 2, (before, after)
    result = {"four_page_pixels_after_resize": "passed", "before": before, "after": after}
    if args.portrait:
        result["portrait_4_by_2_footer_pixels"] = check_portrait(args.portrait)
    print(json.dumps(result))


if __name__ == "__main__":
    main()
