# 更新记录

## v0.1.0-preview.1 · 2026-10-26

首个可下载的 Windows x64 预览版，支持 EPUB / PDF。

### 本次更新

- 修复部分 EPUB 缺失出版方字体时反复扫描系统字库、长时间等待的问题。
- 优先显示当前页面，再逐章补齐分页和目录，减少打开后的等待。
- 滚轮改为整页切换；提供单页 / 双页模式，双页时一次切换整组页面。
- 重做暖纸、白纸和深色界面，使用细分隔线及按需展开的目录、排版侧栏。
- EPUB 字体、字号、行距、段距、缩进、宽度及四边距实际参与重排；PDF 使用独立缩放。
- 正常关闭或切书时保存排版、阅读模式和每本书的位置。
- 加入新的应用图标，提供 PNG 原稿和多尺寸 Windows ICO。

### 下载文件

- `SaeReader-0.1.0-preview.1-windows-x64.zip`：推荐下载，含程序、说明、更新记录及许可。
- `SaeReader.exe`：独立 Windows x64 程序，与压缩包内 EXE 完全相同。
- `SaeReader-0.1.0-preview.1-source.zip`：对应应用和修改过的原生引擎源码、构建脚本，以及全部 580 个锁定 registry 依赖；安装开发工具后可离线构建。
- `SaeReader-0.1.0-preview.1-licenses.zip` 和 `LICENSE.txt`：AGPL 应用许可、第三方许可正文、通知及依赖清单。
- `SaeReader-icon.png`、`SaeReader.ico`：应用图标原稿及 Windows 图标资源。
- `SHA256SUMS.txt`、`release-manifest.json`：下载完整性与来源信息。

GitHub 自动附带的 Source code ZIP/TAR 仅为标签源码快照；完整离线依赖源码请下载上面明确命名的 `*-source.zip`。

### 使用与验证

适用于 Windows 10 / 11 x64。完整解压 Windows 包后运行 `SaeReader.exe`；
`--portable` 将设置与阅读记录保存到程序旁的 `data/`。
若缺少 VC++ DLL，可安装 [Microsoft 官方 x64 运行库](https://aka.ms/vc14/vc_redist.x64.exe)。

发布沿用已验证的 0.1.0 应用二进制，补充公开发行文档和附件。
已有 9 项回归及 730 次窗口操作测试证据，完整 ZIP CRC 和 EXE 图标资源另经校验。
性能测量的版本、口径和限制见 [性能报告](https://github.com/PoesisQ/Sae-CN/blob/v0.1.0-preview.1/docs/performance-report.md)。

### 已知限制

当前仅开放 EPUB / PDF，尚无 TXT / Markdown、拖入打开或跨章节全文搜索。
Ruby、MathML、竖排、固定版式等复杂 EPUB 3 内容仍存在缺陷或未完整验证。
PDF 放大后平移、部分控件中的 Escape、异常退出恢复仍待完善。
Windows 其他系统 DPI、跨屏与干净虚拟机测试尚未全部完成；不将预览版标为全部验收通过。

应用采用 AGPL-3.0-or-later；依赖各自的许可与源码随本次发布提供。
