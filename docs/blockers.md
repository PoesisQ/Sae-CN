# Blockers and decision log

## B001 — Build prerequisites initially absent (resolved)

2026-10-25: `rustc -Vv`, `cargo -V`, `rustup show` all fail with CommandNotFoundException. VS 2022 Community MSVC 14.41.34120 and Windows SDK 10.0.22621 exist. libclang was not found in the inspected tool directories. Install a private toolchain under `.tools`; no change of stack is justified.

The first sandbox HTTP attempts were refused. An elevated request using .NET HTTP encountered unexpected EOF; system `curl.exe` successfully received HTTP 200 from static.rust-lang.org. Use official downloads via curl with hashes. One automatic approval review timed out; the allowed retry ran. This is a tooling problem, not evidence of engine infeasibility.

Rust 1.99.1 + libclang 18.1.1 installed privately. Native Release probe builds and runs. SHA256-verified HTTPS Range download recovered truncated mupdf-sys source. Offline builds work after bootstrap/fetch.

## B003 — Native API version and Windows binding defects (G0 resolved)

mupdf-rs 0.8.0 contains MuPDF 1.27.2, which has no fz_style_document. Its cached HTML trees also do not automatically reparse on changed user CSS. Add exception-safe style/cache invalidation and chapter/flow-ordinal anchors; all five real layout variants preserve the same source text after reopen. Windows max_align_t binding and MSBuild disabled-feature handling repaired with documented local source changes. See `engine-api-feasibility.md` and `vendor/README.md`.

## B004 — CJK fallback severe latency (G0 resolved on supplied fixtures)

The initial core EPUB exceeded five minutes without a correct first page. It was stopped; process evidence: `evidence/initial-cjk-slow.json`. Font loader inspection found system scans for generic names and missing script fallback. Lazy bounded system-font loading and generic-name bypass reduced native open/layout/page count to 83.30 ms on this host (one run, not a startup percentile). CJK screenshots are readable. Broader fonts/languages remain to test.

## B005 — Complex EPUB3 correctness (open P1)

`advanced.epub` native screenshot shows Ruby inline and MathML fraction/power flattened as ordinary text. SVG circle/triangle remain visible. This is a real correctness limitation, not support. Native fallback/repair research and user-visible diagnostics are required in G4. No browser substitution is authorized or introduced.

## B002 — License route (resolved by owner)

The owner selected AGPL-compatible open-source distribution in this chat on 2026-10-25. Include corresponding application/engine/binding source, patches, build scripts and notices. Final dependency audit remains required. The owner subsequently authorized publishing this repository and Windows/source/license Releases on GitHub.

## Gate status

| Gate | Status | Reason |
|---|---|---|
| G0 | 通过（生成样本范围） | Native EPUB2/3/PDF, five true layout changes, reopened logical anchors; raw evidence preserved |
| G1 | 大部分原型证据已补，尚未全项通过 | 20 页 EPUB/PDF、坐标/锚点、双页、取消、句柄和窗口压力通过；实际多档系统 DPI/跨屏仍待补 |
| G2 | 四格式与全文搜索已实施，未全项通过 | 真实文本/图文回归已补；P0 外链确认、脚注、宽表与完整互操作仍待补 |
| G3 | 沉浸浮层、旋钮与全屏已实施，未全项通过 | 最终截图、主题/窄屏/受控 DPI 检查；完整样式入口、独立美术与全键盘验收仍待补 |
| G4 | 部分实施，未通过 | 基础 MathML/Ruby、同尺寸固定画布有原生回归；竖排等仍有已复现缺口 |
| G5 | 部分实施，未通过 | 首屏/内存/CPU/窗口压力/初步 Sumatra 对照已有数据；冷启动、完整对照、干净 VM 等待补 |

## B006 — 真实 EPUB 缺失字体导致大量系统字库扫描（本轮修复）

2026-10-26：用户提供本地目录后复现。epub-02 原生全书分页 7403.99ms，epub-03 超过 30 秒停止。font-kit DirectWrite 的 PostScript 名查找会加载所有字体面；原始通用字体修复未覆盖真实出版方字体名 miss。改为直接 family 匹配、惰性名称索引和有界正负缓存，单次同尺寸分页降至 125.67/174.92ms。三本实书各 20 次可见窗口首屏完成；详见 performance-report.md。该证据不意味着所有大书均达到相同耗时。

## B007 — 合法容错范围内的非规范 HTML 被拒绝（本轮修复）

epub-01 XHTML 中存在 HTML 空元素未闭合。容器/OPF/NCX/SVG 继续使用严格 XML；内容 HTML 通过大小、节点、深度、实体/脚本检查后交给原生恢复解析器，UI 提示兼容读取，不改写原书。实体、深度、压缩炸弹与路径穿越回归仍通过。

## B008 — 当前交付范围与后续验收

本轮优先落实用户明确要求的性能、整页/双页和轻量美术修复；未改变 Rust + Slint + MuPDF 路线。本地反馈版提供独立程序和对应源码；G5 有初步实测/对照/打包证据，仍未通过。真实冷启动、干净 VM/DLL、全格式与复杂 EPUB3 等事项仍按原总纲逐项推进。


## 0.2 iteration: current technical obstacles and options

The previous sections are dated history. This iteration implements four-format opening, on-demand whole-book search, click turns, bounded PDF zoom/pan, native tint/invert, mapped fonts and shared pixels. Gates remain unaccepted until their full baseline checks are met; completion of a feature is not completion of all Phase 0–5 gates.

- **Vertical EPUB** remains an engine gap. `vertical.epub` explicitly requests vertical-rl, but the actual native window still lays it horizontally. The chapter retains images and Japanese text; it is not a successful vertical layout. A visible notice identifies the mismatch. Recommended next option: extend MuPDF's native flow/shaping/line-break model, preserving one engine and existing coordinates, but with substantial implementation and regression cost. Alternative: a separate on-demand Rust vertical layout module using existing native font/image rendering; this adds a second pagination/selection model and requires an owner architecture decision before adoption. A browser runtime is outside the approved route.
- **MathML/Ruby breadth**: targeted vector conversion covers basic groups, fraction, root, sub/superscript and common base/annotation Ruby. Unsupported node types are left in the document with a notice; this is not full MathML/Ruby conformance. Extend the bounded Rust subset with fixtures first. A new full mathematical layout dependency would require size, memory and license evidence before an owner decision.
- **Fixed layout breadth**: uniform viewport/publisher absolute positioning is under native regression. Per-spine rendition overrides, mixed viewports and automatic publisher spread rules still need separate native work and examples. Do not label these as fully compatible.

The selected final screenshots, build identities and measurements are linked in the 0.2 iteration report. Full clean-VM, actual multi-monitor DPI, accessibility, comprehensive second-reader comparisons and cold-start acceptance are still outstanding. Public release remains a preview.

## B009 — Cloned native contexts shared mutable CSS (resolved in 0.2)

Default parallel native regression terminated with Windows STATUS_HEAP_CORRUPTION (0xc0000374). Inspection found `fz_clone_context` shares `fz_style_context`, while `fz_set_user_css` freed/replaced its string without isolation. The native patch detaches style storage on first mutation and allocates the replacement before freeing the old string. Shared resource/font stores remain bounded and shared. An eight-thread, 500-write/read-per-thread regression checks independent CSS; ordinary document tests remain parallel. Direct Book callers now install the same lazy Windows font loader deterministically. Handle-release regression uses its own fixture copies so an intentionally open reader cannot invalidate an exclusive-open assertion.
