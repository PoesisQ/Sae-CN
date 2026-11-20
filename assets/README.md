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

`open-folio.svg` is the original folded-paper illustration for the frontispiece. `dial-bevel.svg` supplies the fixed light on the PDF dial. Both are distributed under the application's AGPL license.

## Homepage artwork

The native Plasma field follows the mathematical parameters of the 21st.dev Shader Builder example supplied for this project. The Blue Grain field uses the 53 colour surfaces and fixed grain supplied with the second design. Their rendering code is in `src/home/field.rs`. The reader ships the packed numerical colour samples, compressed monochrome grain and two stills. The original reference recording and shader handoff are development inputs.

`scripts/pack-home-field.py --unpack assets/home/blue.field blue.png` reconstructs the editable 8 × 7 colour sheet. Run the same script without `--unpack` to pack a replacement. The grain is a zlib stream of 640 × 360 unsigned monochrome samples. Runtime decoding uses the existing Rust inflate dependency.

The negative wordmark in `home/sae-stencil.svg` uses the S, a and e outlines of Nimbus Roman Regular, copyright 2014 by (URW)++ Design & Development. The original CFF and embedded copyright notice are in `vendor/mupdf-sys-0.8.0/mupdf/resources/fonts/urw/NimbusRoman-Regular.cff`. The MuPDF license is included in the license archive. `frontispiece.svg` remains the static perspective print.
