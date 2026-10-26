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

The owner selected AGPL-compliant open-source distribution in this chat on 2026-10-25. Application source is AGPL-3.0-or-later. Preserve upstream component terms and include corresponding source/build changes and notices with releases. The owner subsequently authorized pushes and GitHub Releases to PoesisQ/Sae-CN; no commercial purchase is required. See `license-memo.md`. Toolchain and dependency caches stay under `.tools` and are not part of a deliverable.

## ADR 007 — Feedback: page turns and staged EPUB pagination

The owner's 2026-10-26 feedback requests whole-page wheel navigation and selectable single/double pages. A spread lays out two true native pages at column width, then keeps both images and their own glyph/link/search coordinates. It is not a scaled image or one page copied twice. Narrow canvases temporarily show one page. Both modes originally shared a 24 MiB cache budget; the 0.2 iteration reduces it to 16 MiB with a shared UI/worker pixel allocation.

The sole engine worker publishes the first correct page before completing chapter-by-chapter page counts. It checks cancellation between chapters, resolves outline destinations only when activated, and prefetches at most the next page/spread. Native layout cannot be interrupted inside a C call. Initial Winit geometry/DPI notifications settle once before CLI document layout; fixed canvas geometry and shared metadata updates avoid image-size feedback and redundant bitmap copies.

## ADR 008 — Diagnostics and source delivery

Production builds have no periodic metrics timer. Explicit optional `qa` builds can run visible-window scenarios and renderer snapshots; an external Win32 sampler measures the uninstrumented process. Counter reports are anonymous and exclude book text/images/paths. Packaging includes corresponding application/native source, all locked registry source packages, build scripts and original notices. Library books, private profiles, tools and third-party comparison binaries are excluded.


## ADR 009 — Permanent native efficiency principle (owner, 2026-10-26)

Native, simple and efficient operation is the first principle for subsequent development. Rust remains the application language; the approved native C MuPDF engine is retained. This rule is also recorded in the repository's AGENTS.md. No services, browser runtime, network font fetches, persistent search index or format worker pool are added.

One RGB allocation is shared between page cache and Slint. Text coordinates use native iterators without first cloning a complete structured-text tree. Application bitmap cache is 16 MiB; native evictable resources remain 32 MiB. Visible pages can exceed the cache on a high-DPI screen, so the budget is not a process-memory promise. Skip prefetch when visible pages already require over half that cache. Font files map read-only; their storage lifetime follows the final native buffer reference, including retained fonts. Report total working set as well as Private Bytes; mapped pages still consume physical memory.

PDF zoom uses a device-pixel viewport rectangle, independent scales for different page sizes, and normalized pan positions. Zoomed tiles are generated on demand with a replaceable request slot and a short one-shot pan debounce. Native invert/tint run only for changed tiles when selected; original mode has no color transform. Theme/color and tile bounds are part of the cache key.

TXT streams through BOM/Chinese encoding detection and a strict incremental decoder into small native chapters. Blank lines and leading spaces are preserved. Markdown initializes its parser only when used, sanitizes raw HTML as text, and permits raster images only inside the canonical source directory. Converted EPUBs are temporary and owned by the book, dropped after the engine closes; originals stay read-only. Markdown currently holds decoded source/AST input for parsing, so huge Markdown files have a different peak-memory profile from streamed TXT.

Whole-book search scans native text pages only when requested, with cancellation checks between pages and no resident corpus/index. A cursor records the actual matched page even in a spread. Page highlights are bounded; the selected region remains highlighted. Chapter totals are accumulated incrementally instead of rescanning all chapter counters.

## ADR 010 — Targeted EPUB compatibility

A book is inspected before native open. Only content actually containing supported Ruby/MathML starts the Rust vector adapter. Unchanged ZIP resources are copied in compressed form; originals are never rewritten. A completed SVG display list is replayed directly into the HTML page device: vector text retains coordinates, search and selection, and no intermediate SVG bitmap is needed. Ordinary raster images keep their original path; non-opaque image opacity retains native compositing. Unsupported math and malformed complex HTML remain intact and are reported.

Validated pre-paginated books use the declared fixed viewport, publisher CSS and one canvas per spine item. Native layout suppresses internal reflow page breaks while preserving viewport bounds for positioning; positioned inline images are blockified before layout. Uniform viewport is covered; per-item overrides and mixed canvas sizes remain an explicit limitation. Vertical writing is detected and reported; no alternative layout engine is silently loaded.

## ADR 011 — Immersive controls without a new rendering layer

Use Slint rectangles, thin borders and six tiny source SVG icons. No blur, bitmap skin, animation loop or new dependency package is needed. The canvas occupies the complete client area; menus, settings, search and progress overlay it. Visibility changes do not alter native viewport dimensions or trigger reflow. The existing pinned Winit backend supplies cursor/keyboard events through its accessor feature; the locked dependency graph gains one edge to its already-present crate.

The top/bottom edge reveals each floating control independently. F10 pins both, and F11/Esc are handled before focused controls consume keys. A progress scrubber commits on release; PDF zoom accepts wheel/drag and coalesces native rendering after changes. Panning keeps at most one pending 40ms refresh while continuously updating the latest normalized position. The original left-button selection and link hit-testing remain; middle/right drag pans magnified pages. No idle timer is created for these controls.

Fixed-page viewport/scale/theme changes clear obsolete cached images. A pan miss replaces the same page's preceding tile rather than accumulating adjacent viewport-sized bitmaps. The shared Slint frame may keep the previous tile until its replacement is displayed, so transient active-frame storage still exists outside the cache budget.

## ADR 012 - Owner-authorized Kindle/FB2 expansion (2026-10-27)

The owner's new request supersedes the original four-format non-goal for AZW3
and one or two common alternatives. Add AZW3/KF8, MOBI and FB2 without changing
the Rust + Slint + MuPDF architecture. Pin libmobi 0.12 maintenance commit
906274205c11944b628da1c553b255acb1af7c55; statically compile parsing and the
internal OPF writer only. Encryption, file-writing tools and libxml2 are unbuilt.
Reuse existing zlib. Rust streams rebuilt parts into a temporary EPUB and frees
the entire native parse before MuPDF opens it. A per-thread bounded allocator
reclaims even abandoned malformed-input allocations on scope exit. No parser
object, child converter, conversion cache or idle task exists for other formats.

FB2 uses the existing native HTML handler. Rust validates/decode-checks XML and
builds its TOC, adding missing IDs to a temporary copy only when necessary.
Native HTML/FB2 anchors reuse source-flow ordinals, never pointer bookmarks.
MuPDF HTML bakes CSS at parse time: initial CSS is installed before open, and
style changes reparse on the existing document worker. Replacement can transiently
hold both trees; neither a source buffer nor a second XML tree remains afterward.
Resize reflows the current tree. Font/image/text semantics remain native.

## ADR 013 - Damped zoom with no perpetual simulation

Use a small log-space input state with two geometry-derived stops: contain the
page and cover the column. Each PDF spread column computes its own native scale.
The UI shows the first column's ratio; mixed page sizes can have different crops.
A hysteresis band catches the stop and requires further input to leave it; it
does not run a spring integrator. The vector needle has a 110ms ease-out after
wheel events and direct drag updates. Native rendering uses an 80ms one-shot
throttle with the latest replaceable request; no queued work or idle animation.
TOC rows stay virtualized and use system fonts, not redistributed font assets.

## ADR 014 - Check native startup geometry instead of always sleeping

The 0.2 startup path waited 100ms before every CLI book open to avoid a temporary
viewport/DPI. In 0.3, dispatch once to the event loop, then compare Slint's canvas
with Winit's actual physical client area using Slint's effective scale factor.
Matching geometry starts the document immediately. A mismatch retains the
existing single 100ms fallback; no idle polling or initial low-resolution render
is introduced. Effective Slint scale is deliberate: controlled scale tests can
differ from Windows monitor DPI. First-frame/double-page/DPI checks and final
binary measurements must be repeated after this change. Earlier measurements
and exact binaries remain available for comparison.
