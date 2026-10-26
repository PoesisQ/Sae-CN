# Architecture decisions

Baseline: `Windows_Reader_Codex_Engineering_Brief_v1_20261025.md`, read in full on 2026-10-25 (Asia/Shanghai). This file tracks decisions; it does not grant acceptance.

## ADR 001 — Required native stack

Rust stable, Slint compiled UI, MuPDF native C through `mupdf-rs`. Windows x64 only. No browser runtime, tray process, telemetry, account or background service. Exactly EPUB/PDF/TXT/MD are accepted at the application boundary. MuPDF's additional parsers are not advertised as supported formats.

One crate with `document`, `rendering`, `reading`, `settings`, `storage`, `diagnostics`, `app` modules. A single document worker owns all engine objects. Only owned pixel/text/link data cross into the UI. The UI never parses, searches or rasterizes documents synchronously.

## ADR 002 — Validation gates

G0 requires native EPUB/PDF rendering and measured CSS/font/size/line/paragraph changes. G1 additionally requires logical bookmarks through layout, real text/link coordinates, cancellation, DPI/resize and real native-window evidence. No Phase 3 product-design completion claim before these pass. G2–G5 remain unaccepted until their specified evidence exists.

## ADR 003 — Reflow and position

Use MuPDF `layout` and native user CSS; preserve publisher semantics and original bytes. Test publisher/user precedence explicitly. Prefer engine bookmarks with chapter/text context for persistent relocation; a page number alone cannot represent a reflow location. If the safe binding lacks an API, extend its exception-safe C wrapper and safe Rust surface rather than allowing MuPDF longjmp across Rust frames.

## ADR 004 — Bounded work

Coalesce layout/navigation requests, attach generation IDs, discard stale results, and use MuPDF cookies where available. Cache has a byte budget including pixel data. No unbounded whole-book rasterization/index/cover loading. No idle frame timer. Keep the previous image visible while a replacement renders.

## ADR 005 — Untrusted books

Preflight EPUB central directory/entries and XML with size, ratio, depth and path limits before engine open. Do not extract archives to arbitrary filesystem paths. Disable JavaScript and unneeded engine features. No automatic external network resources; only confirmed http/https links may open a system browser. TXT/MD local images require a constrained resource policy.

## ADR 006 — Local evaluation and license gate

The owner selected AGPL-compliant open-source distribution in this chat on 2026-10-25. Application source is AGPL-3.0-or-later. Preserve upstream component terms and include corresponding source/build changes and notices with releases. No commercial purchase or public upload is authorized. See `license-memo.md`. Toolchain and dependency caches stay under `.tools` and are not part of a deliverable.

## ADR 007 — Feedback: page turns and staged EPUB pagination

The owner's 2026-10-26 feedback requests whole-page wheel navigation and selectable single/double pages. A spread lays out two true native pages at column width, then keeps both images and their own glyph/link/search coordinates. It is not a scaled image or one page copied twice. Narrow canvases temporarily show one page. Both modes share the 24 MiB cache budget.

The sole engine worker publishes the first correct page before completing chapter-by-chapter page counts. It checks cancellation between chapters, resolves outline destinations only when activated, and prefetches at most the next page/spread. Native layout cannot be interrupted inside a C call. Initial Winit geometry/DPI notifications settle once before CLI document layout; fixed canvas geometry and shared metadata updates avoid image-size feedback and redundant bitmap copies.

## ADR 008 — Diagnostics and source delivery

Production builds have no periodic metrics timer. Explicit optional `qa` builds can run visible-window scenarios and renderer snapshots; an external Win32 sampler measures the uninstrumented process. Counter reports are anonymous and exclude book text/images/paths. Packaging includes corresponding application/native source, all locked registry source packages, build scripts and original notices. Library books, private profiles, tools and third-party comparison binaries are excluded.
