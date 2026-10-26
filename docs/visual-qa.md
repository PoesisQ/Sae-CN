# 视觉 QA — 本轮反馈版

本轮按用户要求改为克制的书页界面；G3 全项仍未通过。保留真实窗口渲染和 Windows 屏幕证据，区分引擎 PNG、窗口 renderer snapshot 与桌面捕获。

## 设计选择

实际做了三套轻量候选：默认细线工具栏、居中书名/衬线标题、左侧窄工具轨。候选截图：evidence/feedback/ui-paper-double.png、ui-design-1.png、ui-design-2.png。选择默认方案：工具栏层级更稳定，省去重复工具轨，书名不与正文标题争夺注意。没有背景照片、模糊、阴影、动画或常驻刷新。

Token：暖纸 #F8F6F1、浅色界面 #F3F4EF、正文 #262A27、次要文字 #707971、分隔线 #DDE1D8、强调 #506C60；深色纸张 #171A19、文字 #E7E9E5、强调 #A4C1AA。UI 使用系统 Microsoft YaHei UI / Segoe UI，阅读字体由 MuPDF 和用户设置决定，强调/标题/代码保留语义。未捆绑 Windows 字体。

间距以 8/16/24px 为主，顶部 56px、底部 42px，侧栏按需展开。固定画布不受位图固有尺寸撑开，双页间隔 32px，中线仅 1px。状态行高度固定，避免「正在打开」消失后再触发一次重排。字体选择首次打开面板才枚举名称。

## 已检查场景

| 场景 | 证据 | 结果 / 限制 |
|---|---|---|
| 暖纸单页，中文/英文/标题 | evidence/feedback/ui-paper-single.png | 第一张文档图像直接使用最终窗口尺寸；真实文字重排 |
| 双页与插图 | ui-paper-double.png | 相邻两页、统一基线、间距与中线，完整显示图像 |
| 图片、表格、嵌入字体、脚注 | ui-image-table.png、evidence/ui-native-wheel-double.png | 原创样本能见图像/表格/EMBED；复杂分页表格仍需对照 |
| 目录 | ui-contents.png | 标题/行间距和滚动列表可见；超长标题省略 |
| 当前页搜索 | ui-search-native.png | 最终普通发行窗口搜索 Quiet，两处高亮与实际文字位置一致；全文搜索仍未实现 |
| 排版侧栏 | ui-typography.png | 字号、行距、段距、数值输入/滑块显示正常 |
| 夜读，英语斜体/段落/引用 | ui-night.png | 界面与正文一致；图片保留原色，图片自带白边不被反色 |
| 窄窗口 | ui-narrow.png | 双页偏好保留，画布不足 800px 时暂用单页，底部明确提示 |
| PDF | ui-pdf.png | 固定版式整页显示，禁止字体/行距假重排 |
| Windows 实际滚轮和焦点 | evidence/ui-native-wheel-double.png 与 .txt | 单页一次一页、双页一次一跨页；按钮有键盘焦点 |

表中除最后一行外主要使用真实可见 Winit 窗口的 renderer snapshot，非仅导出的 MuPDF 页面 PNG。桌面窗口捕获另由 Computer Use 记录，包含系统标题栏与实际输入结果。截图均为项目原创样本，未把用户书籍图像写入文档或发行包。

实际主机 150% DPI；ui-controlled-scale-1.png、-1.25.png、-1.5.png、-2.png 使用 SLINT_SCALE_FACTOR 在真实窗口进行受控缩放检查，覆盖重排、控件、画面和尺寸变化。它们**不等于**把 Windows 显示设置/显示器分别改为四档，尤其窗口逻辑尺寸和系统 DPI 不一致时不能据此认定跨显示器行为通过。

## 截图检查中修正

去掉 effective-spread 对画布布局的循环绑定；分隔线固定在面板边缘/页脚顶部；加载状态不改变画布高度；等待首次 DPI/客户区稳定，避免临时小页面后再跳版；metadata 更新共享图像，不反复复制并清除选择。

## 未验收

实际 100/125/200% 系统 DPI 与跨屏切换、全文搜索视觉、独立人工审美复核、屏幕阅读器全过程、Markdown 代码/表格、完整单书与全局样式范围、字重/自定义色/出版方样式入口。PDF 放大后的平移待实现。不能把这组截图视为 G3 所有项已通过。

最终手工测试中，部分设置/编辑控件焦点下 Escape 未关闭面板；可见关闭/收起入口可用。该快捷键传播问题列为待修，未把完整键盘路径判为通过。

## 用户图标

本日用户提供的透明 PNG 原稿为 1284×1225。按比例缩小并透明补方，生成
16/20/24/32/48/64/128/256px ICO 和 128px 窗口 PNG；没有改画或重新生成图案。
原稿、128px 派生文件已直接检查，留白和透明通道保留；缩小资源与来源见
`assets/README.md`、`assets/icon-manifest.json`。

普通 Release 已编译并包含 Windows 文件版本信息；PE 图标目录由
`scripts/verify-package.py` 检查。此次系统标题栏/任务栏视觉检查未完成：
Computer Use 刷新绑定后仍两次报告 `failed to activate captured window`。
资源检查与既有正文界面截图不能代替该系统外观检查。
