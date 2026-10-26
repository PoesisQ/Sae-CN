# Native dependency provenance and local changes

Upstream crates from https://static.crates.io/crates/ (2026-10-25):

- mupdf 0.8.0, SHA256 `185fb74927a40c569b7152c01c73f4c0206059b8b3d37c7eb007cc3f63453b64`.
- mupdf-sys 0.8.0, SHA256 recorded in `docs/evidence/toolchain.json`; checked against the crates.io lockfile before extraction. Native MuPDF **1.27.2**, not the 1.28 documentation examples.

Preserve all original notices. These copies are required for reproducible builds
and complete AGPL source delivery. No opaque binary MuPDF replacement is used.

Local changes (2026-10-25, AGPL-3.0-or-later):

- `mupdf-sys/msbuild.rs`: honor disabled OCR/DOCX/barcode Cargo features on MSBuild;
  bound native compiler parallelism to four.
- `mupdf-sys/wrapper/document.c`: exception-safe CSS + document HTML-cache purge,
  logical EPUB anchor and chapter APIs.
- `mupdf-sys/wrapper/context.c`: native evictable store budget 32 MiB.
- `mupdf-sys/mupdf/source/html/html-outline.c`, `html-imp.h`, `epub-doc.c`:
  add durable chapter/flow-ordinal anchor APIs. Original pointer bookmarks remain
  unchanged; an application never persists pointer addresses.
- `mupdf/src/document.rs`: safe Rust methods calling those C wrappers; expose chapter counts and an outline traversal that does not eagerly resolve every target page.
- `mupdf/src/device/native.rs`: use documented Windows x64 malloc alignment
  (16 bytes) when MSVC headers do not expose `max_align_t` to bindgen. Source:
  https://learn.microsoft.com/en-us/cpp/c-runtime-library/reference/malloc .
- `mupdf/src/font.rs`: native reference-counted Clone to share loaded font payloads.
- `mupdf/src/system_font.rs`: bypass font-kit scans for CSS generic family aliases;
  replace DirectWrite's brute-force PostScript-name scan with direct family matching,
  a bounded 256-key positive/negative handle cache and a lazy normalized family-name
  index. Load only a matched face; never enumerate every font's file data on a miss.
  The app supplies bounded, lazy Windows fallback loading through FontLoader.
- `mupdf-sys/build.rs`: track changes to native implementation and wrapper sources
  so Cargo cannot silently reuse an old native library after a C-source edit.

Anchor schema version 1 applies only to unchanged content. Record chapter and
nearby text as relocation context; CSS display/structure changes may require
fallback. Ordinary typography changes must be tested, not assumed stable.


Local changes (2026-10-26, native performance and compatibility):

- `wrapper/font.c`, `mupdf/fitz/buffer.h`, `source/fitz/buffer.c`: read-only Windows font mappings, owned by a zero-initialized optional storage finalizer on `fz_buffer`. The last native reference unmaps storage; retained Rust/native font clones remain valid. UTF-8 paths convert to wide Win32 paths and all failure branches release handles/views. All native consumers are rebuilt together after the internal structure extension.
- `mupdf/src/font.rs`, `system_font.rs` and application font loader: map matched path fonts without `fs::read` and `copy_font_data` heap copies; in-memory font handles retain the existing owned-buffer route.
- `wrapper/page.c`, `mupdf/src/page.rs`: exception-safe rendering of a bounded device-pixel rectangle, including annotations. PDF zoom allocates the visible tile rather than a whole magnified page. The application shares its resulting RGB buffer with Slint.
- `source/html/html-parse.c`: blockify absolutely/fixed positioned inline boxes before dispatching images, following [CSS2.1 section 9.7](https://www.w3.org/TR/CSS2/visuren.html#dis-pos-flo). This is required for fixed-canvas EPUB absolute images.
- `source/fitz/image.c`: render display-list images with an alpha channel, preserving transparent SVG backgrounds instead of clearing an opaque RGB image to white. Final reader page output remains RGB.

Targeted Ruby/MathML SVG conversion and fixed-canvas selection live in application Rust, not a browser. Unchanged EPUB resources use ZIP raw-copy; only books containing relevant elements initialize this adapter. Unsupported cases remain visible and are reported. See the compatibility matrix for measured coverage and limitations.

- `html-imp.h`, `epub-doc.c`, `html-layout.c`, safe `Document::set_fixed_layout`: validated fixed canvases keep their viewport bounds while disabling internal reflow page breaks and returning one page per spine item. This pinned HTML engine treats CSS px as one native layout unit, so the viewport uses the same units; applying an extra 72/96 factor crops the source canvas.
- `image.c` and the HTML image drawing dispatch: replay completed SVG display lists directly for opaque content, preserving vector text and searchable/selected coordinates with no intermediate image. A verified native callback identity guards the derived-image cast; ordinary rasters and image opacity retain their existing drawing path.
# 0.2 context isolation

`mupdf/source/fitz/context.c`: copy-on-write style storage for cloned contexts, used by both CSS setters. This fixes concurrent CSS replacement without duplicating the native resource store. An allocation failure keeps the previous CSS valid. Upstream copyright and AGPL notices remain intact.

# 0.3 native format adapters

`html-doc.c` and `wrapper/document.c` extend the existing source-flow ordinal
anchor helpers to native HTML/FB2, guarded by the concrete handler callback.
FB2 CSS is installed before parse; the application reparses it on style changes
and sets the native page background as well as body colors. No pixel recoloring
is used for its theme.

`libmobi/` is a separately pinned LGPL-3.0-or-later parser. Its provenance and
archive checksum are in `libmobi/UPSTREAM.txt`. The application statically builds
only reading/reconstruction and the internal OPF writer, with a streaming Rust
bridge and bounded thread-local allocator in `native/`; existing zlib is reused.
Upstream ebook test payloads are excluded. See the root license memo and build
scripts for the exact source list and corresponding-source distribution.
