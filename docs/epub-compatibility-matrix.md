# EPUB compatibility matrix

Engine/binding versions will be recorded from the actual build. `待测` means no acceptance evidence. A successful open or extracted text alone is insufficient. Screenshots and comparison evidence belong in `docs/evidence`/`artifacts`, with run metadata.

| ID | Priority / baseline | Sample | Status | Evidence / missing work |
|---|---|---|---|---|
| E01 | P0 EPUB2 OPF/manifest/spine/metadata/cover | core-epub2.epub | 原生生成样本通过；互操作待补 | g0-epub2、实书 3 本首屏；外部基准与完整元数据对照仍待补 |
| E02 | P0 EPUB3 package/nav/multilevel TOC | core-epub3.epub, catalog.epub | 原创 nav-only 多层回归通过；外部互操作待补 | 0.3 回归不依赖 NCX fallback，保留中英标签与层级，实际 URI 可定位；55 项长目录在窄窗口/受控缩放中换行显示。0.1 Chapter 10 跳转/返回证据仍保留 |
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
| E15 | P1 MathML | advanced.epub | 基础子集兼容；复杂结构未完成 | 0.2 原生截图及 reader_features：分组、分数、根号、上下标矢量转换；不支持的节点保留并提示 |
| E16 | P1 SVG/image formulas | advanced.epub | 基础矢量图文已测；全面对照待补 | 原生显示列表直接绘制，保留文字层；暖纸/夜读截图，不将位图外观当作选择证据 |
| E17 | P1 Ruby/emphasis marks | advanced.epub | 常见 Ruby 上下注音已测；着重号待补 | reader_features 断言字、注音与连续基文搜索；ui-ruby-math.png |
| E18 | P1 CJK line-break constraints | advanced.epub | 待测 | Punctuation at line edges |
| E19 | P1 vertical-rl/Japanese/reading direction | vertical.epub | 已复现缺陷，未兼容 | 原生仍横排；书内明确提示。证据与原生扩展选项见 blockers.md |
| E20 | P1 RTL/bidi | advanced.epub | 待测 | Text order and navigation |
| E21 | P1 EPUB3 fixed layout/spreads | fixed.epub | 同尺寸画布已测；混合画布与出版方跨页待补 | 600×800 固定画布、绝对定位图像和底部文字断言，字号/窗口变化不重排；支持阅读器单双页与缩放 |
| E22 | P1 cross chapter search/highlight | core-epub3.epub | 跨章与双页查找回归通过 | reader_features 前后循环、右页命中、缩放视区定位；ui-whole-book-search.png；无常驻索引 |
| E23 | P2 audio/video/media overlay/script | not supplied | 不支持（产品范围） | Must surface limitation; never execute scripts |
| E24 | P2 DRM | not supplied | 不支持（产品范围） | No bypass |
| S01 | Security ZIP bomb/traversal/XXE/depth | generated hostile fixtures | 当前自动回归通过 | test-feedback-final.log；容错 HTML 仍拒绝实体/脚本/深度炸弹，容器 XML 仍严格 |

All P0 rows require real render **and interactive** evidence, and reference-reader comparison. Generated minimal EPUB files are real ZIP/OPF/XHTML books with source and content assertions; they supplement, rather than replace, external interoperability testing. No current row is an EPUB 3 completeness claim.
