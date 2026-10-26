# Application icon

The owner supplied `Textured gouache reader emblem.png` on 2026-10-26 for this
application, identifying it as AI-generated. `icon-original.png` is an exact copy;
its SHA256 is recorded in `icon-manifest.json`. No third-party artwork, font or
icon library was added. Keep this provenance with the application's AGPL source
distribution; this asset is not one of the CC0 test fixtures.

`scripts/generate-icon.py` makes only proportional size conversions and transparent
square padding. It produces a 128×128 window PNG (64 KiB decoded RGBA) and an ICO
with 16/20/24/32/48/64/128/256 pixel entries. The large original is included in
source only, not decoded by the running program. No animation or refresh timer.

The Slint window embeds `window-icon.png`; Windows SDK RC compiles `app-icon.ico`
and version metadata into the executable. Normal builds use these checked-in
outputs; Pillow is required only to regenerate them (11.3.0 used here).

Resource format references:
[Microsoft ICON resource](https://learn.microsoft.com/en-us/windows/win32/menurc/icon-resource),
[RC command line](https://learn.microsoft.com/en-us/windows/win32/menurc/using-rc-the-rc-command-line-).
The small controls in `icons/*.svg` are source-authored geometric UI assets supplied under the application's AGPL license. They are separate from the owner's AI-generated application artwork; no image-generation service or external icon package is used for them.
