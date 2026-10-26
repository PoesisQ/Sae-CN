# License investigation — 2026-10-25

Status: owner selected **AGPL-compliant open-source distribution**, with corresponding source and licenses, in this chat on 2026-10-25. The owner subsequently authorized pushing this source project to https://github.com/PoesisQ/Sae-CN on 2026-10-26. No commercial license was purchased. The application is AGPL-3.0-or-later; upstream components retain their actual licenses. Source publication does not mean that the staged product/release acceptance gates have all passed.

| Component | Primary source checked | Finding / required action |
|---|---|---|
| MuPDF C | https://artifex.com/licensing ; https://mupdf.readthedocs.io/en/latest/guide/license.html | AGPL/commercial dual licensing. Distribution must follow the selected route. Retain notices and corresponding source/build changes as applicable. |
| mupdf-rs | https://github.com/messense/mupdf-rs/blob/main/LICENSE ; Cargo.toml | AGPL-3.0 metadata. A commercial MuPDF agreement must not be presumed to also relicense the independently authored binding; clarify separately if choosing proprietary distribution. |
| Slint | https://slint.dev/pricing ; https://slint.dev/faqs | GPLv3, royalty-free desktop terms, or commercial terms. Review the actual license files of the pinned crate and compatibility with the project's selected license before release. |
| Rust and Cargo | https://www.rust-lang.org/policies/licenses | Development tools; capture installed toolchain and license inventory. Not bundled in application. |
| Fonts and samples | `tests/fixtures/SOURCES.md` | Prefer Windows system fonts and book embedded fonts. Never redistribute Windows font files. Self-authored fixtures use CC0. External fixtures require explicit source/license records. |

## Options recorded and owner decision

1. **Selected by owner:** AGPL-compatible source distribution: ship corresponding application/engine/binding source, patches, build scripts and notices with the binary in a compliant manner.
2. Proprietary/commercial distribution: obtain appropriate Artifex rights, separately resolve binding rights and Slint terms. No fees or contracts are authorized by this project.

Do not label a local evaluation build a legally cleared final release. Before packaging, inventory every Cargo dependency and native third-party component, include license texts, and check DLL/font/assets separately. This memo does not replace review of the actual terms. The initial source push was authorized separately; the owner then explicitly authorized a GitHub Release containing the Windows program, corresponding source, licenses, changelog and application icon on 2026-10-26.

2026-10-26 packaging: cargo vendor prepared all 580 registry packages in Cargo.lock, including optional/platform source. The local feedback source ZIP also includes the modified native MuPDF/binding source, original upstream notices, application/build scripts and offline directory-source configuration. The runtime inventory distinguishes default Windows build dependencies from the complete locked-source superset. Slint uses its GPLv3 open-source option. No Microsoft runtime DLL or Windows font is redistributed in this package; VC++ x64 runtime installation and a clean-machine validation remain external prerequisites/work. No public upload was performed.

2026-10-26 icon addition: the owner supplied an AI-generated PNG for use in this
application. Its exact original, SHA256, proportional size-conversion script and
outputs are retained under assets/ in the corresponding source. See assets/README.md;
this is separate from the self-authored CC0 fixtures. No additional font, artwork
library, Rust crate or runtime component was introduced.

Final feedback packaging also checks all 293 dependencies in the default Windows
build graph (including build-time components). Eighteen published Cargo packages
omit standalone license files. Upstream notice/text supplements, URLs, commits,
SHA256 and selected branches are retained in `legal/third-party/sources.json`,
copied into runtime licenses, and checked during packaging. Original registry
sources/checksums are not changed. The dwrote canonical-text and unavailable
pathfinder_geometry VCS-commit exceptions are recorded explicitly in that manifest
and README rather than being silently described as exact-commit files.

## 2026-10-27 - libmobi native parser

New statically linked native component: libmobi 0.12 maintenance snapshot,
commit 906274205c11944b628da1c553b255acb1af7c55, LGPL-3.0-or-later. Upstream
source/provenance is in vendor/libmobi; no DRM code, tools or libxml2 is built.
The combined application remains AGPL-3.0-or-later and upstream library sources
retain their notices. The runtime delivers both LGPLv3 and GPLv3 texts, upstream
commit/archive checksum and a native-component inventory. The corresponding
source ZIP includes the library, our C bridge/allocator, all Rust sources and
locked registry inputs, plus complete rebuild scripts, so the library can be
modified and the combined application rebuilt. No anti-relink restriction is
added. Source packaging explicitly includes native/; it must not be omitted.

The only new Cargo edge is a build dependency on cc 1.10.0, already in Cargo.lock.
No new registry package, redistributed font or executable converter is added.
Upstream ebook test payloads are local opt-in inputs, excluded from the public
source/runtime archives and distinct from the original CC0 fixtures.
