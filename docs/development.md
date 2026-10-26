# 开发与构建

Sae Reader 使用 Rust + Slint + 原生 MuPDF，目标为 Windows 10 / 11 x64。
当前实际可用格式为 EPUB / PDF，其余功能状态见兼容性与已知限制文档。

## 构建

需要 VS 2022 C++ v143、Windows SDK 和 Python。引导脚本将 Rust 1.99.1 与
构建所需 libclang 放在项目的 `.tools/` 内，不修改系统 PATH：

```powershell
./scripts/bootstrap.ps1
. ./scripts/env.ps1
cargo build --release --locked --bin sae-reader
```

普通仓库检出按 `Cargo.lock` 获取 registry 依赖。Release 附带的完整 `*-source.zip`
包含本地 `dependencies/` 和离线 Cargo 配置；安装开发工具后可使用：

```powershell
. ./scripts/env.ps1
cargo build --release --locked --offline --bin sae-reader
```

应用源码中的 `vendor/` 保留了修改过的 MuPDF Rust 绑定与原生 C 引擎；
不能用未修补的 registry 同名包静默替代。Windows SDK RC 编译 EXE 图标和版本资源。

## 测试与诊断

```powershell
cargo test --release --locked --no-default-features --lib --test g1_native
```

测试本地书籍时设置 `SAE_READER_TEST_BOOKS` 为自己的目录，再执行：

```powershell
cargo test --release --locked --no-default-features --test local_books -- --nocapture
```

样本可通过 `python scripts/generate-fixtures.py` 重建，需要 `fonttools`。
图标尺寸转换脚本是 `scripts/generate-icon.py`，仅重建资源时需要 Pillow；正常构建不需要它。

窗口诊断需显式启用 `--features qa` 与 `SAE_READER_QA_REPORT`；普通发行程序不含诊断定时器。
`qa-windows.py`、`profile-window.py`、`profile-local-books.py` 提供计时和进程采样。
窗口测量必须在可见的 Windows 桌面运行，并区分引擎耗时、程序入口计时与 OS 可见首帧。

## 打包

```powershell
./scripts/package.ps1 -Name SaeReader-0.1.0-preview.1
python scripts/verify-package.py --name SaeReader-0.1.0-preview.1
python scripts/prepare-release.py --name SaeReader-0.1.0-preview.1
```

打包目录必须是全新名称，避免把个人便携数据带入发行包。
输出包括 Windows 包、完整依赖源码、独立许可包、图标、更新说明和校验清单。
源码不包含用户书籍、个人配置、工具链或构建缓存，也不分发 Windows 字体。
源码 Git 检出使用 `.gitattributes` 保持编码样本、图标及原生源码的字节不被自动换行转换。

有本地已校验的完整源码 ZIP 时，可传入 `-ReuseRegistryFrom` 加快打包；
脚本先验证旧包 SHA256、Cargo.lock 与依赖覆盖，实际程序及项目源码仍取当前文件。

## 工程记录

- [架构与原生修补](architecture-decisions.md)
- [EPUB 兼容性](epub-compatibility-matrix.md)
- [性能实测](performance-report.md)
- [视觉检查](visual-qa.md)
- [已知限制](known-limitations.md)
- [依赖许可来源](license-memo.md)
