"""Convert the user-supplied artwork to bounded Windows icon resources.

Only proportional scaling and transparent padding; never alter the original.
Builds use the committed outputs and do not need Pillow. To regenerate:
    python scripts/generate-icon.py   (Pillow 11.3 used for these outputs)
"""
from pathlib import Path
import hashlib
import json

from PIL import Image, ImageOps

root = Path(__file__).resolve().parents[1]
assets = root / 'assets'
original = assets / 'icon-original.png'
source = Image.open(original).convert('RGBA')
square = ImageOps.pad(source, (256, 256), method=Image.Resampling.LANCZOS,
                      color=(0, 0, 0, 0), centering=(0.5, 0.5))
window = ImageOps.pad(source, (128, 128), method=Image.Resampling.LANCZOS,
                      color=(0, 0, 0, 0), centering=(0.5, 0.5))
window.save(assets / 'window-icon.png', compress_level=9)
sizes = [(s, s) for s in (16, 20, 24, 32, 48, 64, 128, 256)]
square.save(assets / 'app-icon.ico', sizes=sizes)
record = {
    'source': 'icon-original.png',
    'source_sha256': hashlib.sha256(original.read_bytes()).hexdigest(),
    'source_pixels': list(source.size),
    'window_pixels': list(window.size),
    'window_rgba_bytes': 128 * 128 * 4,
    'ico_sizes': [s[0] for s in sizes],
    'transformation': 'proportional Lanczos scaling; transparent square padding',
}
(assets / 'icon-manifest.json').write_text(json.dumps(record, indent=2) + '\n', encoding='utf-8')
print(json.dumps(record, indent=2))
