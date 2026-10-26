# Known limitations

The repository is under staged development. Unverified features must not be advertised as implemented. Current gates and blockers are in `blockers.md`; individual EPUB status is in `epub-compatibility-matrix.md`.

- G0 native feasibility passed on original synthetic EPUB2/3 and PDF. G1 native-window validation is in progress; G2–G5 are not accepted.
- EPUB3 is not assumed complete; MuPDF's official documentation explicitly describes its EPUB3 support as limited.
- DRM, embedded scripts, multimedia/media overlays are outside the product support scope.
- PDF typography cannot be reflowed by changing the app's text settings.
- Ruby currently appears inline; MathML fractions/powers flatten incorrectly. These are P1 defects with actual render evidence, not compatible output.
- Native layout cannot be preempted mid-call; cancellation currently coalesces work and suppresses stale results between calls.
- The owner chose AGPL source distribution. Final redistribution still requires the complete dependency/source/notice inventory.

2026-10-26 feedback build:

- Missing publisher-font lookups and first-window-size relayout were repaired; three local EPUBs and ten local PDFs have raw measurements. These samples do not establish universal performance/compatibility.
- Actual enabled formats remain EPUB/PDF. TXT/MD, drag/drop, cross-chapter search, full per-book/global style choices, weight/custom-color controls remain pending G2/G3. Search in this build is explicitly current-page only.
- Mouse wheel turns whole pages; double mode turns an entire spread. A canvas narrower than 800 logical pixels temporarily uses single-page mode while retaining the preference.
- PDF fits the page initially. Zoom exists, but panning when enlarged beyond the fixed canvas remains incomplete.
- Preferences/location are saved on normal close or switching books, not periodically during reading. Crash recovery and invalid/future-schema backup/migration need further work.
- Actual 150% system DPI plus four controlled Slint scales were checked; actual other Windows DPI settings/multi-monitor transitions and complete screen-reader coverage remain unverified.
- Final manual QA found Escape can be consumed by a focused settings/edit control instead of closing all panels; visible close/collapse buttons work. Keyboard shortcut propagation still needs G1/G3 regression work.
- Local feedback packaging includes full vendored source/notices. Microsoft VC runtime DLLs are external prerequisites; a clean Windows VM run remains pending.
