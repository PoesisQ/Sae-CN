# 安装、更新与文件关联

Windows 会记住默认阅读器的程序路径。同名的 `SaeReader.exe` 放在不同版本目录时，
「打开方式」仍可能使用旧路径；新格式也可能被交给旧版。

## 使用固定目录

将完整的 Windows 发布包解压到一个固定目录，例如：

```text
%LOCALAPPDATA%\Programs\Sae Reader
```

右键书籍，选择「打开方式 → 选择其他应用」，选择 Sae Reader 并点「始终」。
如果列表中的项目仍不正确，浏览到这个固定目录中的 `SaeReader.exe`。
EPUB、PDF、AZW3 等格式分别选择一次即可。也可以在 Windows「设置 → 应用 → 默认应用」中选择。

以后更新时正常关闭阅读器，用新版完整包覆盖这个目录。保留同一程序路径，
无需把每个版本目录分别添加为默认应用。保留包内的 `LICENSE`、`licenses/` 和第三方许可文件。

默认阅读设置和位置在 `%LOCALAPPDATA%\SaeReader\state.json`，与程序目录分开，
更新程序无需删除这个目录。便携模式用户应保留程序旁的 `data/`。

## 关联修复工具

[install-local.ps1](../scripts/install-local.ps1) 将完整发布包安装到上述固定目录，
修复已经指向 Sae 的旧打开命令，并为支持的七种格式注册「打开方式」和默认应用入口。
它只处理当前 Windows 用户，无常驻服务，不修改其他应用的打开命令，
也不删除、伪造或重写 Windows 保护的默认选择。

在仓库目录的 PowerShell 中运行（把包路径替换成实际解压目录）：

```powershell
.\scripts\install-local.ps1 -PackageDirectory '.\dist\SaeReader-0.3.0-preview.1' -OpenDefaults
```

包含 `Install.ps1` 的后续 Windows 发布包也可直接在包目录运行：

```powershell
.\Install.ps1 -OpenDefaults
```

运行后仍需在 Windows 界面确认所需格式的默认应用。若当前脚本执行策略不允许运行，
按上面的手动方式使用固定目录即可。此工具不更改系统执行策略。

程序复制后会校验 EXE 的 SHA256；更新时若固定目录内的程序仍在运行，工具会停止并提示先正常关闭。
通过旧目录的兼容链接启动时也能识别运行中的程序，避免更新占用中的 EXE；从同一安装的链接入口运行工具不会重复复制自身。
它保留个人配置，创建开始菜单快捷方式，并随程序保留相应的许可。

Windows 的注册和默认选择方式参见微软文档：[应用注册](https://learn.microsoft.com/en-us/windows/win32/shell/app-registration)、
[默认应用平台](https://learn.microsoft.com/en-us/windows/apps/develop/windows-integration/default-apps-platform)。
