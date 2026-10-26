# Supplemental upstream license texts

The locked registry sources are delivered unchanged in `dependencies/` in the
source ZIP. Eighteen packages in the default Windows build omit standalone
license files from their published Cargo package. This directory supplies those
texts without changing any crate or its `.cargo-checksum.json`.

`sources.json` records each exact URL, commit when available, SHA256 and chosen
license branch. Original copyright/SPDX headers and all source remain in the
corresponding source archive. `AUTHORS-AND-SOURCE.txt` preserves the leading source
headers and identifies Cargo author metadata as such, without substituting it
for a copyright statement. Runtime copies are placed in `licenses/<crate-version>/`.

Two provenance exceptions are explicit:

- dwrote's pinned repository tree contains no LICENSE file; its original source
  header points to Mozilla MPL 2.0. The canonical text comes from Mozilla.
- pathfinder_geometry's published VCS commit returns 404 upstream. Shared upstream
  license texts come from the reachable commit used by the locked pathfinder_simd;
  geometry's own SPDX and 2019 copyright headers remain intact.

These are notice supplements, not additional software dependencies or changes to
the application's AGPL route. No network access is required to build or package
from these retained files once development tools are installed.
