# EPUB compatibility matrix

Engine/binding versions will be recorded from the actual build. `待测` means no acceptance evidence. A successful open or extracted text alone is insufficient. Screenshots and comparison evidence belong in `docs/evidence`/`artifacts`, with run metadata.

| ID | Priority / baseline | Sample | Status | Evidence / missing work |
|---|---|---|---|---|
| E01 | P0 EPUB2 OPF/manifest/spine/metadata/cover | core-epub2.epub | 原生生成样本通过；互操作待补 | g0-epub2、实书 3 本首屏；外部基准与完整元数据对照仍待补 |
| E02 | P0 EPUB3 package/nav/multilevel TOC | core-epub3.epub | 部分验证 | 最终 Windows 窗口 Chapter 10 跳转至 74/241、返回 1/241 实测；完整多层待补 |
| E03 | P0 CJK/English/mixed punctuation/wrapping | core-epub3.epub | 部分验证 | feedback/ui-paper-single.png；20 页实际 glyph/选择断言；严格禁则待补 |
| E04 | P0 emphasis/sup/sub/headings/quotes/lists/alignment | core-epub3.epub | 部分验证 | feedback/ui-paper-double.png、ui-night.png 保留强调和层级 |
| E05 | P0 cover/inline/standalone images/captions | core-epub3.epub, inline-svg-cover.epub | 普通图像通过；缩放交互待补 | 原生 20 页检图像；inline SVG 图片回归及三本实书非空封面像素；未从零文本块误判丢图 |
| E06 | P0 tables/header/rowspan/colspan/wide table | core-epub3.epub | 部分验证，有分页差异待审 | ui-image-table.png；普通合并表格可见，跨页表格表头/边框需要更严对照，宽表交互未完成 |
| E07 | P0 intra/cross chapter/internal links | core-epub3.epub | 当前原型通过；更多书待补 | 最终窗口目录 Chapter 10 跳转/返回；书页 Next chapter anchor 命中跳到 Chapter 02（13–14/333），返回 3–4/333 |
| E08 | P0 external links with confirmation | core-epub3.epub | 待测 | No automatic browser launch |
| E09 | P0 footnotes/endnotes/backlinks | core-epub3.epub | 待测 | Navigate/return exact source |
| E10 | P0 embedded fonts/weight/fallback | fonts.epub | 嵌入字体与 CJK fallback 已测；字重待补 | 原创 CC0 EMBED 字体可见；Windows 字体仅动态加载，无再分发 |
| E11 | P0 logical location after layout/restart | core-epub3.epub | 当前原型通过；更多书/结构变更待补 | G0 五种真实重排+重开文字校验；g1_native 后段锚点；最终普通窗口正常关闭/重开恢复双页 3–4/333 同一内容，ui-reopened-position.png |
| E12 | P0 global/book style precedence, original immutable | core-epub3.epub | 待测 | Render changes and file hash |
| E13 | P1 SVG and mixed graphics | advanced.epub, inline-svg-cover.epub | 基本形状/图片通过；复杂对照待补 | g0-advanced、inline SVG 原生回归与实际窗口图片；不是 SVG 全规范声明 |
| E14 | P1 float/nested tables/lists/positioning | advanced.epub | 待测 | Compare and classify differences |
| E15 | P1 MathML | advanced.epub | 已确认 P1 缺陷 | 分数/幂被展平；g0-advanced 证据，不能称兼容 |
| E16 | P1 SVG/image formulas | advanced.epub | 待测 | Verify elements and baseline |
| E17 | P1 Ruby/emphasis marks | advanced.epub | 已确认 P1 缺陷 | Ruby 成普通行内文字；未实现可靠注音回退 |
| E18 | P1 CJK line-break constraints | advanced.epub | 待测 | Punctuation at line edges |
| E19 | P1 vertical-rl/Japanese/reading direction | vertical.epub | 待测 | Direction and navigation |
| E20 | P1 RTL/bidi | advanced.epub | 待测 | Text order and navigation |
| E21 | P1 EPUB3 fixed layout/spreads | fixed.epub | 待测 | Respect fixed layout, no text-only substitute |
| E22 | P1 cross chapter search/highlight | core-epub3.epub | 当前页高亮通过；跨章未完成 | 最终窗口 Quiet 两处高亮坐标一致，ui-search-native.png；不宣称全文搜索 |
| E23 | P2 audio/video/media overlay/script | not supplied | 不支持（产品范围） | Must surface limitation; never execute scripts |
| E24 | P2 DRM | not supplied | 不支持（产品范围） | No bypass |
| S01 | Security ZIP bomb/traversal/XXE/depth | generated hostile fixtures | 当前自动回归通过 | test-feedback-final.log；容错 HTML 仍拒绝实体/脚本/深度炸弹，容器 XML 仍严格 |

All P0 rows require real render **and interactive** evidence, and reference-reader comparison. Generated minimal EPUB files are real ZIP/OPF/XHTML books with source and content assertions; they supplement, rather than replace, external interoperability testing. No current row is an EPUB 3 completeness claim.
