# Sae Reader

Windows 10/11 x64 native reader under staged development against the complete
`Windows_Reader_Codex_Engineering_Brief_v1_20261025.md` baseline.

**Rust + Slint + MuPDF.** EPUB/PDF/TXT/Markdown only. No browser runtime or resident
helper. The initial repository is a feasibility implementation; consult the gate
table in [docs/blockers.md](docs/blockers.md) before assuming any acceptance item
has passed.

源代码仓库：[PoesisQ/Sae-CN](https://github.com/PoesisQ/Sae-CN)。仓库包含应用、修改过的
原生引擎源码、合法测试样本、性能/视觉证据及许可补充文件；用户书籍、个人配置、
本机构建缓存、工具链和 `dist/` 发行文件不入库。完整离线依赖源码包由打包脚本生成，
普通源码检出则按 `Cargo.lock` 获取 registry 依赖。

本次反馈版的实际可用格式为 **EPUB / PDF**；TXT / Markdown 适配器尚未通过 G2。
2026-10-26 修复实书字体查找导致的长时间等待，支持滚轮整页翻阅及单页/双页选择，
采用暖纸、细分隔线和按需展开侧栏的轻量界面。数据和截图见
[本轮报告](docs/feedback-20261026.md)，不据此宣称 Phase 0–5 全部验收完成。

## 使用

运行 `dist/SaeReader-0.1.0-feedback3/SaeReader.exe`，或向程序传入 EPUB/PDF 路径。
正常关闭后保存全局排版和每本书的逻辑位置；默认保存到
`%LOCALAPPDATA%/SaeReader/state.json`，`--portable` 改为程序旁 `data/`。
无常驻服务。卸载时关闭程序、删除程序目录，个人历史可按需另行删除。

已接入用户提供的 AI PNG 图标；窗口和 EXE 使用有界尺寸资源，原稿及来源记录
随源码交付，见 `assets/README.md`。

- 滚轮、方向键、PageUp/PageDown：整页切换；双页时整跨页切换。
- 顶部「单页 / 双页」：切换模式；阅读画布不足 800 逻辑像素时暂用单页。
- Ctrl+O：打开；Ctrl+F：当前页查找；Ctrl+C：复制所选文字。
- Ctrl+Plus/Minus：EPUB 字号或 PDF 缩放；F10：工具栏；F11：全屏。
- 「排版」：字体、字号、行距、段前后距、缩进、对齐、正文宽度、四边距和主题。

当前不含拖入文件、跨章节全文搜索、完整单书排版配置、PDF 放大后的平移等 G2/G3 功能。
Windows 系统 DLL 和 VC++ 2015–2022 x64 运行库仍为外部运行依赖；干净虚拟机验收待做。

## Build

VS 2022 C++ v143 + Windows SDK and Python are required for development. Bootstrap
installs Rust 1.99.1 and build-only libclang into `.tools` without changing the
system PATH:

```powershell
./scripts/bootstrap.ps1
. ./scripts/env.ps1
cargo build --release --locked
cargo test --release --locked --no-default-features
cargo run --release --locked --no-default-features --bin engine-probe -- tests/fixtures/core-epub3.epub artifacts/probe
```

`Cargo.lock` pins the resolved dependency graph. The exact native library version,
feature audit, test results and build caveats must be recorded before a release.
Fixtures can be regenerated with Python and `fonttools`:

```powershell
python scripts/generate-fixtures.py
```

本地实书回归（只读，不会把书复制进源码或发行包）：

```powershell
$env:SAE_READER_TEST_BOOKS='S:\书'
cargo test --release --locked --offline --no-default-features --test local_books -- --nocapture
```

原生窗口诊断需显式 `--features qa` 和 `SAE_READER_QA_REPORT`；普通发行构建不含
诊断定时器。复测脚本为 `scripts/qa-windows.py`、`profile-window.py`、
`profile-local-books.py`。窗口必须在可见的 Windows 桌面运行。

打包脚本为 `scripts/package.ps1`。发行目录旁交付完整 `*-source.zip`，包含修改过的
MuPDF 源码和锁定依赖的全部 580 个 registry 源码目录；配置使用本地 vendor，
已安装开发前提后可离线构建。不会附带 Windows 字体、用户书籍或私有配置。

## Validation and limits

- [Architecture](docs/architecture-decisions.md)
- [EPUB matrix](docs/epub-compatibility-matrix.md)
- [Performance](docs/performance-report.md)
- [Visual QA](docs/visual-qa.md)
- [Known limitations](docs/known-limitations.md)
- [License memo](docs/license-memo.md)

EPUB3 is evaluated feature by feature. Multimedia, scripts and DRM are unsupported.
PDF typography is fixed. Application files must never overwrite original books.

## License

The owner selected AGPL-compliant open-source distribution. Application source:
AGPL-3.0-or-later; dependencies retain their licenses. Distribute matching source,
build scripts, native changes and notices with binary releases. See `LICENSE` and
`docs/license-memo.md`. The repository does not yet constitute a cleared final
release package.
