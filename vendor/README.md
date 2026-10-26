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
