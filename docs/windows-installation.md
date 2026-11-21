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
.\scripts\install-local.ps1 -PackageDirectory '.\dist\SaeReader-0.6.5-preview.3' -OpenDefaults
```

包含 `Install.ps1` 的后续 Windows 发布包也可直接在包目录运行：

```powershell
.\Install.ps1 -OpenDefaults
```

运行后仍需在 Windows 界面确认所需格式的默认应用。若当前脚本执行策略不允许运行，
按上面的手动方式使用固定目录即可。此工具不更改系统执行策略。

更新时先在安装目录校验待替换 EXE，再通过 Windows 文件替换 API 换入新版。已经打开的窗口可继续阅读，新开的窗口使用新版。若 Windows 报告文件占用，正常退出后重试。同一安装目录的兼容链接仍有效。 [Windows 文件替换](https://learn.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-replacefilew)
它保留个人配置，创建开始菜单快捷方式，并随程序保留相应的许可。

Windows 的注册和默认选择方式参见微软文档：[应用注册](https://learn.microsoft.com/en-us/windows/win32/shell/app-registration)、
[默认应用平台](https://learn.microsoft.com/en-us/windows/apps/develop/windows-integration/default-apps-platform)。

## 系统报告 4551

安装器在复制文件前请 Windows 校验源 EXE 能否创建进程。校验过程不执行阅读器代码。替换后还会检查固定路径。目标位置的检查失败时自动恢复原有 EXE。本版在一台启用智能应用控制的 Windows 11 上被拦截，代码完整性日志为事件 3077，策略名为 VerifiedAndReputableDesktop。该机的现有版本仍可启动。

智能应用控制根据文件信誉或受信任的代码签名决定是否运行。遇到 4551 时可继续使用已安装的版本，等待受信任签名的构建。发行者也可以将同一文件提交微软复核。[微软的智能应用控制开发说明](https://learn.microsoft.com/windows/apps/develop/smart-app-control/overview)
