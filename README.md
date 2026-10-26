<p align="center">
  <img src="assets/window-icon.png" alt="Sae Reader 图标" width="96" height="96">
</p>

<h1 align="center">Sae Reader</h1>

<p align="center">Windows 原生电子书阅读器。</p>

<p align="center">
  <a href="https://github.com/PoesisQ/Sae-CN/releases/download/v0.3.0-preview.1/SaeReader-0.3.0-preview.1-windows-x64.zip">下载 Windows x64 版</a> ·
  <a href="https://github.com/PoesisQ/Sae-CN/releases/tag/v0.3.0-preview.1">更新说明与源码</a> ·
  <a href="LICENSE">AGPL-3.0-or-later</a>
</p>

Sae Reader 支持暖纸、白纸与深色主题，可调排版、单页或双页阅读，以及按需展开的目录与设置。
采用原生界面，无账户、广告、云同步或常驻后台服务。

当前为 **v0.3.0-preview.1 预览版**，支持 **EPUB / PDF / TXT / Markdown / AZW3 / MOBI / FB2**，不含 DRM。

![双页 PDF 阅读与缩放圆盘](docs/evidence/iteration-0.3/ui-dial-fill.png)

## 下载与使用

1. 下载 [Windows x64 压缩包](https://github.com/PoesisQ/Sae-CN/releases/download/v0.3.0-preview.1/SaeReader-0.3.0-preview.1-windows-x64.zip)。
2. 完整解压，双击 `SaeReader.exe`，通过「打开」选择书籍。
3. 鼠标移至窗口上沿展开工具条，选择「单页 / 双页」；点击阅读区域左边上一页、右边下一页。

适用于 Windows 10 / 11 x64，无需安装开发工具。若系统提示缺少 `VCRUNTIME140.dll`
或 `VCRUNTIME140_1.dll`，请安装 [Microsoft 官方 Visual C++ v14 x64 运行库](https://aka.ms/vc14/vc_redist.x64.exe)。

[Release 页面](https://github.com/PoesisQ/Sae-CN/releases/tag/v0.3.0-preview.1) 同时提供独立 EXE、
完整依赖源码、许可证和 SHA256 校验文件；推荐使用包含使用说明与许可文件的 Windows 压缩包。

## 阅读功能

- **单页 / 双页**：点击左边上一页、右边下一页；链接优先响应，拖动仍可选字。滚轮切换整页或整组双页，窄窗口自动暂用单页。
- **真实重排**：可重排文档的字体、字号、行距、段距、缩进、对齐、正文宽度及四边距可调。
- **舒适主题**：暖纸、白纸和深色；PDF 可选原色、随主题着色或反色，使用 MuPDF 原生处理。
- **阅读工具**：章节目录、内部链接与返回、文字选择与复制、跨章节全文查找及前后定位。
- **PDF 缩放**：单页、双页均可放大，适页、适宽与平移；按可见区域渲染。放大后滚轮平移，点击左右仍翻页。
- **Kindle 与 FB2**：无 DRM 的 AZW3/KF8、MOBI 以及 FictionBook；保留文字、目录、图片、样式与链接，按需解析，共用原生渲染与搜索。
- **文本与 Markdown**：自动识别编码，可手动切换；保留 TXT 空行与缩进，Markdown 支持标题、列表、引用、代码、表格、本地图片与链接。
- **记住位置**：正常关闭或切书时保存排版、阅读模式及每本书的阅读位置及手动文本编码。
- **沉浸阅读**：上下浮层靠近边缘时出现，不占正文空间；F11 全屏。底部可拖动进度跳页；缩放圆盘随滚轮和拖动转动，在「整页 / 铺满」附近吸附，继续操作即可脱离，点击恢复整页。

## 快捷键

| 操作 | 快捷键 |
|---|---|
| 打开文件 | `Ctrl+O` |
| 上一页 / 下一页 | 方向键、`PageUp` / `PageDown`、滚轮 |
| 全文查找 | `Ctrl+F` |
| 复制所选文字 | `Ctrl+C` |
| 调整可重排文字字号 / PDF 缩放 | `Ctrl+Plus` / `Ctrl+Minus` |
| PDF 平移 | 放大后的上下键与滚轮、Shift+滚轮横移、中键或右键拖动 |
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

此时数据写入程序旁的 `data/` 文件夹。卸载时关闭程序并删除程序文件夹；
个人阅读记录可按需另行删除。

## 当前限制

- 尚未提供拖入打开、OCR 或整本书的常驻搜索索引。全文查找按需逐页扫描，扫描 PDF 需要已有文本层。每页查找上限 4096 个区域，同时最多高亮 256 个及当前区域。
- 常见 Ruby 与部分 MathML 使用按需生成的 SVG 兼容，保留上下结构与分数、上下标；矢量文字保留选择与搜索。复杂数学结构、竖排、不同画布尺寸的固定布局等仍有限制，不能宣称 EPUB 3 全兼容。
- 部分编辑控件下的全局快捷键、完整无障碍与干净 Windows 虚拟机验收仍待补齐。
- 不支持 DRM、嵌入脚本或多媒体播放。

详情见 [已知限制](docs/known-limitations.md)、[EPUB 兼容性](docs/epub-compatibility-matrix.md)
和 [性能实测](docs/performance-report.md)。

## 开发与许可

原生、简洁、高效是首要原则。主体使用 Rust，配合 Slint 原生界面和 MuPDF；格式适配、字体与全文查找按需工作，无 WebView 或浏览器运行时。[开发与构建说明](docs/development.md) 包含工具、测试和打包步骤。

应用代码采用 [AGPL-3.0-or-later](LICENSE)，依赖保留各自许可。
每个二进制 Release 同时提供对应源码及许可文件。

图标：[PNG 原稿](https://github.com/PoesisQ/Sae-CN/releases/download/v0.3.0-preview.1/SaeReader-icon.png) · [Windows ICO](https://github.com/PoesisQ/Sae-CN/releases/download/v0.3.0-preview.1/SaeReader.ico)。
