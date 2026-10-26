# MuPDF / Rust API feasibility — measured G0

Pinned binding: mupdf/mupdf-sys 0.8.0. Actual native source: **1.27.2**.
Slint: 1.18.1. Rust: 1.99.1 MSVC. See `evidence/toolchain.json` and `vendor/README.md`.

| Capability | Published safe binding | Actual implementation / evidence |
|---|---|---|
| EPUB/PDF native open + RGB raster | Document::open, Page::to_pixmap | Both EPUB2/3 and PDF rendered for 20 consecutive pages; `g0-*.json` |
| Window-dependent reflow | Document::layout | Width 600→420 native points: 123→213 pages |
| Font size | layout em + user CSS | 18→26 pt: 123→301 pages; actual new glyph layout |
| Line height | Context user CSS only | Exception-safe document-style wrapper + HTML-store purge: 1.55→2.05 produces 123→181 pages |
| Paragraph spacing | Context user CSS only | 0.6→1.8 em produces 123→151 pages |
| Page margins | Context user CSS only | 28/40/28/40→72/64/72/64 pt: 123→181 pages |
| fz_style_document | Absent from native 1.27.2 | Do not call nonexistent 1.28 API. Targeted cache purge + true layout on sole worker. No full source extraction/rewrite |
| Bookmark across layout | Native API not exposed; HTML bookmark holds pointer | New exception-safe chapter/flow ordinal API; original native bookmark APIs retained. All five variants preserve source text, including reopen |
| Text/glyph geometry | TextPage::structured, character quads | Real glyph counts, font sizes and positions collected with each raster |
| Page search | Page::search, quads | Real search rectangles in evidence JSON. Full-book search still G2 work |
| Links / destinations / outline | Page::links, Document::resolve_link/outlines | Footnote, backlink, cross chapter and external bounds recorded; actual UI activation remains G1 |
| Embedded fonts | Native EPUB resource parser | Original geometric EMBED face visible in `g0-embedded-font.png` |
| CJK fallback | SystemFontLoader lacks generic script fallback | App FontLoader uses lazy Windows fonts, no redistributed Windows fonts. Font payload cache 32 MiB |
| Cancellation | Cookie exists, but EPUB draw ignores it and layout has no cookie | Coalesce requests and discard stale generations between calls; individual native layout is not preemptible |
| Complex EPUB3 | Engine limited | SVG renders. Ruby is inline, MathML fraction/power flatten incorrectly. Not accepted; see matrix and limitations |

The line/paragraph/width/size results above are synthetic fixture evidence, not a
claim that all publisher CSS, fonts or all EPUB3 are supported. All P0 rows still
need application interaction/reference comparison before G2.

Upstream C APIs: https://mupdf.readthedocs.io/en/1.28.0/_static/generated/c/html/fitz_2document_8h_source.html
(version difference explicitly verified against local 1.27.2 headers).
Official EPUB3 limitation: https://mupdf.readthedocs.io/en/latest/guide/what-is-mupdf.html .
