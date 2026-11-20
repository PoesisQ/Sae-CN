"""Pack the supplied Blue Grain atlas into 53 individually decodable RGB fields.

The packed file is also the editable source of the field: --unpack reconstructs
the stored colour samples (within two 8-bit levels of the original) in their 8 by 7 contact sheet. Pillow is authoring-only.
"""
import argparse
from pathlib import Path
import struct
import zlib
from PIL import Image

WIDTH, HEIGHT, COUNT = 160, 90, 53


def pack(atlas):
    if atlas.size != (8 * WIDTH, 7 * HEIGHT):
        raise ValueError("Expected eight 160 x 90 tiles per row")
    frames = []
    for n in range(COUNT):
        x, y = n % 8 * WIDTH, n // 8 * HEIGHT
        data = bytearray(atlas.crop((x, y, x + WIDTH, y + HEIGHT)).convert("RGB").tobytes())
        data = bytearray(min(63, (v + 2) // 4) if v < 254 else 64 for v in data)
        for row in range(HEIGHT):
            start = row * WIDTH * 3
            for i in range(start + WIDTH * 3 - 1, start + 2, -1):
                data[i] = (data[i] - data[i - 3]) & 255
        frames.append(zlib.compress(data, 9))
    offsets = [0]
    for frame in frames:
        offsets.append(offsets[-1] + len(frame))
    return struct.pack("<54I", *offsets) + b"".join(frames)


def unpack(blob):
    offsets = struct.unpack_from("<54I", blob)
    frames = memoryview(blob)[216:]
    atlas = Image.new("RGB", (8 * WIDTH, 7 * HEIGHT))
    for n, (start, end) in enumerate(zip(offsets, offsets[1:])):
        data = bytearray(zlib.decompress(frames[start:end]))
        for row in range(HEIGHT):
            first = row * WIDTH * 3
            for i in range(first + 3, first + WIDTH * 3):
                data[i] = (data[i] + data[i - 3]) & 255
        atlas.paste(Image.frombytes("RGB", (WIDTH, HEIGHT), bytes(min(255, v * 4) for v in data)), (n % 8 * WIDTH, n // 8 * HEIGHT))
    return atlas


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("input", type=Path)
    parser.add_argument("output", type=Path)
    parser.add_argument("--unpack", action="store_true")
    args = parser.parse_args()
    if args.unpack:
        unpack(args.input.read_bytes()).save(args.output)
    else:
        args.output.write_bytes(pack(Image.open(args.input)))
