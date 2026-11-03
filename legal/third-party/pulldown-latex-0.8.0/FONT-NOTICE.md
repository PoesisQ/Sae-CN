# Source-only upstream example fonts

The unmodified pulldown-latex 0.8.0 registry source includes these optional
browser-demo files in `font/`:

- latinmodern-math.woff2
- lmroman12-regular.woff2
- lmroman12-italic.woff2
- lmroman12-bold.woff2

They are Latin Modern / Latin Modern Math fonts, distributed separately under
the GUST Font License (legally based on LPPL 1.3c or later), rather than the MIT
license for the Rust parser. Embedded font notices remain in the original files.
The upstream font projects and unmodified originals are available from:

- https://www.gust.org.pl/projects/e-foundry/latin-modern
- https://www.gust.org.pl/projects/e-foundry/lm-math
- https://ctan.org/pkg/lm
- https://ctan.org/pkg/lm-math

Our corresponding-source ZIP retains these registry files verbatim, with their
original Cargo file checksums. GUST-FONT-LICENSE.txt and LPPL-1.3c.txt are included
here as supplements. Download provenance and hashes are in sources.json.
This notice makes no claim of support by the original font authors/maintainers.

These WOFF2 assets are NOT placed in the Windows runtime or used by Sae Reader.
Native formulas use mapped Cambria Math / Times New Roman from Windows. The
runtime ships only the applicable notices alongside the parser's MIT LICENSE.
