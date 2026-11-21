<p align="center">
  <img src="assets/window-icon.png" alt="Sae Reader 图标" width="96" height="96">
</p>

<h1 align="center">Sae Reader</h1>

<p align="center">Windows 原生电子书阅读器。</p>

<p align="center">
  <a href="https://github.com/PoesisQ/Sae-CN/releases/download/v0.6.5-preview.4/SaeReader-0.6.5-preview.4-windows-x64.zip">下载 Windows x64 版</a> ·
  <a href="https://github.com/PoesisQ/Sae-CN/releases/tag/v0.6.5-preview.4">更新说明与源码</a> ·
  <a href="LICENSE">AGPL-3.0-or-later</a>
</p>

Sae Reader 支持暖纸、白纸与夜读主题。阅读时文档占满窗口，工具栏、目录和设置随用随开。
可重排文档的排版独立可调，PDF 保留原页版式，并提供单页、双页和多页。

当前为 **v0.6.5-preview.4 预览版**，支持 **EPUB / PDF / TXT / Markdown / AZW3 / MOBI / FB2**，不含 DRM。

![蓝白色带上的黑白反色字标，与 Plasma 上的彩色反色字标](docs/evidence/iteration-0.6.5-p2/covers.png)

直接启动时，两张动画封面逐次轮换。把书拖进窗口，或点击左下角的「选择书籍」。等书页排好，四角、边缘和中央的颗粒同时开始随机散去，露出上次读到的位置。双击文件直接进入正文。在首页按左右方向键可以切换蓝白色带、Plasma 与朱红网格版画。

## 下载与使用

1. 下载 [Windows x64 压缩包](https://github.com/PoesisQ/Sae-CN/releases/download/v0.6.5-preview.4/SaeReader-0.6.5-preview.4-windows-x64.zip)。
2. 完整解压，双击 `SaeReader.exe`。从资源管理器拖一本书进来，也可以点击「选择书籍」。
3. 靠近窗口上沿展开工具栏。拖动半滚轮到下一挡，或直接点两侧的视图名称。点击阅读区域左侧上一页、右侧下一页。

适用于 Windows 10 / 11 x64，无需安装开发工具。若系统提示缺少 `VCRUNTIME140.dll`
或 `VCRUNTIME140_1.dll`，请安装 [Microsoft 官方 Visual C++ v14 x64 运行库](https://aka.ms/vc14/vc_redist.x64.exe)。

[Release 页面](https://github.com/PoesisQ/Sae-CN/releases/tag/v0.6.5-preview.4) 同时提供独立 EXE、
完整依赖源码、许可证和 SHA256 校验文件。Windows 压缩包内附使用说明与许可。

作为默认阅读器使用时，可用完整包里的 `Install.ps1` 更新固定目录中的程序。
Windows 会记住程序路径；每次换到不同的版本目录可能仍会启动旧版。
遇到这个问题可参考 [安装、更新与文件关联修复](docs/windows-installation.md)。

这个预览版尚无 Authenticode 签名。开启 Windows 智能应用控制的设备可能报告错误 4551。安装器会先检查系统是否允许启动，已有的默认阅读器会在更新被拦截时继续可用。[Windows 安装说明](docs/windows-installation.md)

顶栏的视图拨轮收起时只在栏下露出一小段圆弧，指向它就会展开。底部缩放盘的磨砂轮面透出所在位置的书页色彩，倍率和指针浮在玻璃上。目录、设置和菜单的磨砂更厚，便于阅读文字。切换 PDF 原色与反色时，玻璃会随书页重新取色。

## 阅读功能

- **单页 / 双页**：点击左边上一页、右边下一页；链接优先响应。适页时拖动可选字，放大后可抓住页面查看四边。滚轮切换整页或整组双页，窄窗口自动暂用单页。
- **真实重排**：可重排文档的字体、字号、行距、段距、缩进、对齐、正文宽度及四边距可调。
- **舒适主题**：暖纸、白纸和深色；PDF 可选原色、随主题着色或反色，使用 MuPDF 原生处理。
- **阅读工具**：目录和书内链接支持跳转。跳转后顶栏会出现「回到跳转前」，点它可回到刚才读到的地方。也可选字复制、跨章节查找和定位。
- **PDF 多页**　顶栏的磨砂玻璃拨轮透出书页上的图文，悬停时挡位字随轮面一起放大。刻度从单页延伸到多页，随挡位名称一起经过固定指针。松手时吸附到挡位，短距离试拨会回弹。鼠标滚轮连续拨两格也能换挡。可在「阅读设置 → 多页布局」中选自动、3×1、4×1、2×2、3×2 或 4×2，方向键按整组翻页。
- **收起侧栏**：点击文档或页边空白即可关闭目录、排版或查找面板；这一次点击只收起面板。
- **PDF 缩放**　靠近底部倍率，圆盘和底板一起展开。向右或向上直拖放大，反向缩小。沿盘缘顺时针放大、逆时针缩小。直拖到屏幕边缘并按住，可一直调到最低或最高倍率，使用自定义显示缩放时也适用。松手后倍率留在盘心，可接着拖。下弧三道刻线从左到右为整页（100%）、适宽、铺满，点按跳转，从刻线上拖动也能调倍率。
- **跳页**：拖细进度线能先看到目标页码，松手才跳转。点击页码或按 `Ctrl+G`，在原位置输入页数后回车。多页模式显示包含目标页的整组。
- **Kindle 与 FB2**：无 DRM 的 AZW3/KF8、MOBI 以及 FictionBook；保留文字、目录、图片、样式与链接，按需解析，共用原生渲染与搜索。
- **文本与 Markdown**：自动识别编码，可手动切换；保留 TXT 空行与缩进，Markdown 支持标题、列表、引用、代码、表格、本地图片与链接。图片未载入等情况可在阅读设置的「文档信息」查看。
- **Markdown 公式**：支持 `$...$`、`$$...$$` 和 `math` 块中的常见 LaTeX；分式、根式、上下标、矩阵和对齐公式由原生引擎绘制，保留文字坐标。普通代码保持原义，复杂命令保留源码。[语法与范围](docs/markdown-math.md) · [实测记录](docs/iteration-0.4.1.md)。
- **记住位置**：正常关闭或切书时保存排版、阅读模式及每本书的阅读位置及手动文本编码。
- **沉浸阅读**：点击系统最大化按钮、双击标题栏或按 `F11`，都会进入隐藏标题栏和任务栏的全屏。鼠标靠近边缘时显示工具栏；目录与设置覆盖在正文之上。

[上下拨轮的原生窗口实拍](docs/evidence/iteration-0.6.5-p3/wheels.png) · [顶栏和侧栏的磨砂层次](docs/evidence/iteration-0.6.5/glass.png) · [竖版 PDF 八页实图](docs/evidence/iteration-0.6.0/portrait.png)

## 快捷键

| 操作 | 快捷键 |
|---|---|
| 在首页切换封面 | 左右方向键 |
| 打开文件 | `Ctrl+O` |
| 上一页 / 下一页 | 左右方向键、`PageUp` / `PageDown` |
| 输入页码跳转 | 点击页码或 `Ctrl+G` |
| 全文查找 | `Ctrl+F` |
| 复制所选文字 | `Ctrl+C` |
| 调整可重排文字字号 / PDF 缩放 | `Ctrl+Plus` / `Ctrl+Minus` |
| PDF 平移 | 放大后按住左键拖动，或用中键、右键拖动。滚轮和上下方向键纵移，Shift+滚轮横移 |
| 选取文字 | 在文档上拖动。PDF 放大并超出窗口时，用 Shift+左键拖动 |
| PDF 缩放 | 底部旋钮、Ctrl+滚轮 |
| 恢复 PDF 适页 / 默认字号 | `Ctrl+0` |
| 固定 / 自动收起工具栏 | `F10` |
| 全屏 / 退出全屏 | `F11` / `Esc` |

## 数据与便携模式

默认设置与阅读位置保存在 `%LOCALAPPDATA%\SaeReader\state.json`。
需要便携使用时，在程序目录启动：

```powershell
.\SaeReader.exe --portable
```

可用 `--cover=plasma`、`--cover=blue` 或 `--cover=print` 指定本次封面。打开选书窗口时封面暂停，取消后可以继续选择。Windows 关闭「动画效果」或使用 `--motion=static` 时，书页就绪即显示。阅读中也可以拖入另一本书直接切换。一次拖入多个文件时，会打开其中第一本支持的书。

便携模式下数据写入程序旁的 `data/` 文件夹。卸载时关闭程序并删除程序文件夹；
个人阅读记录可按需另行删除。

## 当前限制

- 全文查找按需逐页扫描，扫描 PDF 需要已有文本层。每页查找上限 4096 个区域，同时最多高亮 256 个及当前区域。当前未提供 OCR。
- 常见 Ruby 与部分 MathML 使用按需生成的 SVG 兼容，保留上下结构与分数、上下标；矢量文字保留选择与搜索。复杂数学结构、竖排、不同画布尺寸的固定布局等仍有限制，不能宣称 EPUB 3 全兼容。
- 部分编辑控件下的全局快捷键、完整无障碍与干净 Windows 虚拟机验收仍待补齐。
- 带脚本的规范 EPUB 章节可阅读其中的静态正文、图片和目录。依赖 JavaScript 生成的内容、DRM 及多媒体播放暂未支持。

详情见 [已知限制](docs/known-limitations.md)、[EPUB 兼容性](docs/epub-compatibility-matrix.md)
和 [本版开书过渡与 Windows 实测](docs/iteration-0.6.5-p4.md)。

## 开发与许可

原生、简洁、高效是首要原则。主体使用 Rust，配合 Slint 原生界面和 MuPDF；格式适配、字体与全文查找按需工作，无 WebView 或浏览器运行时。[开发与构建说明](docs/development.md) 包含工具、测试和打包步骤。

应用代码采用 [AGPL-3.0-or-later](LICENSE)，依赖保留各自许可。
每个二进制 Release 同时提供对应源码及许可文件。

图标：[PNG 原稿](https://github.com/PoesisQ/Sae-CN/releases/download/v0.6.5-preview.4/SaeReader-icon.png) · [Windows ICO](https://github.com/PoesisQ/Sae-CN/releases/download/v0.6.5-preview.4/SaeReader.ico)。
