# Known limitations

The repository is under staged development. Unverified features must not be advertised as implemented. Current gates and blockers are in `blockers.md`; individual EPUB status is in `epub-compatibility-matrix.md`.

- G0 native feasibility passed on original synthetic EPUB2/3 and PDF. G1 native-window validation is in progress; G2–G5 are not accepted.
- EPUB3 is not assumed complete; MuPDF's official documentation explicitly describes its EPUB3 support as limited.
- DRM, embedded scripts, multimedia/media overlays are outside the product support scope.
- PDF typography cannot be reflowed by changing the app's text settings.
- The initial release flattened Ruby/MathML. Version 0.2 adds a targeted native vector fallback, retaining SVG text coordinates/search; full MathML/Ruby conformance remains unaccepted.
- Native layout cannot be preempted mid-call; cancellation currently coalesces work and suppresses stale results between calls.
- The owner chose AGPL source distribution. Final redistribution still requires the complete dependency/source/notice inventory.

2026-10-26 feedback build:

- Missing publisher-font lookups and first-window-size relayout were repaired; three local EPUBs and ten local PDFs have raw measurements. These samples do not establish universal performance/compatibility.
- The 0.1 feedback build enabled EPUB/PDF and current-page search. Version 0.2 adds TXT/MD and on-demand whole-book search. Drag/drop, full per-book/global style choices and weight/custom-color controls remain pending G2/G3.
- Mouse wheel turns whole pages; double mode turns an entire spread. A canvas narrower than 800 logical pixels temporarily uses single-page mode while retaining the preference.
- Version 0.2 adds bounded visible-tile PDF zoom, fit-page/width and pan in single/double mode. Zoomed wheel gestures pan; clicking left/right still turns pages. Native tint/invert are opt-in, cached per tile and affect illustrations as well as text.
- Preferences/location are saved on normal close or switching books, not periodically during reading. Crash recovery and invalid/future-schema backup/migration need further work.
- Actual 150% system DPI plus four controlled Slint scales were checked; actual other Windows DPI settings/multi-monitor transitions and complete screen-reader coverage remain unverified.
- Window-level F11/F10/Esc and Ctrl+O/F now bypass focused controls. Complete keyboard-only reading/editing, selection shortcuts and screen-reader coverage still need G1/G3 acceptance.
- Local feedback packaging includes full vendored source/notices. Microsoft VC runtime DLLs are external prerequisites; a clean Windows VM run remains pending.


0.2 preview scope and remaining limits:

- Auto text detection is a guess; manual UTF-8/UTF-16LE/BE/GB18030/Big5/Shift_JIS/windows-1252 selection is available and saved per book. Malformed bytes are diagnosed instead of silently replaced. TXT streams with bounded decoding/chapter buffers; a single line over 8 MiB is rejected explicitly. Markdown currently retains decoded input while parsing, so exceptionally large Markdown files can have a larger temporary peak.
- Markdown raw HTML is displayed as inert text. Network, out-of-directory and non-raster image resources are blocked; local PNG/JPEG/GIF/WebP/BMP are supported, with 32 MiB per image and 128 MiB total limits. This restriction is shown when resources are blocked. Code blocks wrap. Dynamic scripts, rich HTML and Markdown-to-other-file navigation are not implemented.
- Search needs a native text layer, checks cancellation between pages, and does not run a resident index. Each page has a 4096-quad search limit; highlights show the first 256 plus the selected quad. A wrapped phrase can occupy multiple regions; these are not an exact count of distinct words. Raster-only scans have no OCR. Unsupported MathML remains a known native rendering gap, even though the source is preserved.
- Uniform fixed viewport/publisher absolute-positioned content is covered by generated regression; mixed viewport sizes, per-item rendition overrides, automatic spine spread/page direction and full vertical writing are not accepted. Vertical requests are detected and shown as a visible limitation; see blocker options.
- Temporary text/compatibility EPUB files are deleted after a normal engine close. A forced process termination can leave its temporary folder behind; no background cleanup service runs. Originals are never rewritten or shipped in the release.
- Mapped fonts lower committed private storage, but touched file-backed pages still occupy physical RAM. The application cache budget excludes Slint's window backing store, active uncached pages, decoded native images, fonts and parser state. It is not a whole-process memory cap.
