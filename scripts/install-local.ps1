<#
Install a released runtime into a stable per-user directory and register it with
Windows. Run again with a new package to update the same installation. Windows
default choices are made in Settings, never by writing protected UserChoice keys.
#>
[CmdletBinding()]
param(
    [string]$PackageDirectory,
    [string]$InstallDirectory = (Join-Path $env:LOCALAPPDATA 'Programs\Sae Reader'),
    [string]$ExpectedSHA256,
    [switch]$OpenDefaults
)
$ErrorActionPreference = 'Stop'
# Resolve script-relative defaults after binding, including PowerShell 5.1 -File.
$readerScriptPath = $PSCommandPath
if (-not $readerScriptPath) { $readerScriptPath = $MyInvocation.MyCommand.Path }
$readerScriptDirectory = Split-Path -Parent $readerScriptPath
if (-not $PackageDirectory) { $PackageDirectory = $readerScriptDirectory }

function Resolve-ReaderDirectory([string]$Directory) {
    $readerResolved = [IO.Path]::GetFullPath($Directory)
    for ($readerLinkDepth = 0; $readerLinkDepth -lt 16; $readerLinkDepth++) {
        $readerDirectoryItem = Get-Item -LiteralPath $readerResolved -Force -ErrorAction SilentlyContinue
        if (-not $readerDirectoryItem -or $readerDirectoryItem.LinkType -notin @('Junction', 'SymbolicLink')) {
            if ($readerResolved.Length -gt [IO.Path]::GetPathRoot($readerResolved).Length) {
                $readerResolved = $readerResolved.TrimEnd([IO.Path]::DirectorySeparatorChar, [IO.Path]::AltDirectorySeparatorChar)
            }
            return $readerResolved
        }
        $readerDirectoryTargets = @($readerDirectoryItem.Target)
        if ($readerDirectoryTargets.Count -ne 1 -or -not $readerDirectoryTargets[0]) {
            throw 'Cannot resolve the program directory link safely.'
        }
        $readerNextDirectory = $readerDirectoryTargets[0]
        if (-not [IO.Path]::IsPathRooted($readerNextDirectory)) {
            $readerNextDirectory = Join-Path (Split-Path -Parent $readerResolved) $readerNextDirectory
        }
        $readerResolved = [IO.Path]::GetFullPath($readerNextDirectory)
    }
    throw 'The program directory contains a link cycle or too many links.'
}

$readerSource = Resolve-ReaderDirectory ((Resolve-Path -LiteralPath $PackageDirectory).ProviderPath)
$readerTarget = Resolve-ReaderDirectory $InstallDirectory
$readerSourceExe = Join-Path $readerSource 'SaeReader.exe'
$readerTargetExe = Join-Path $readerTarget 'SaeReader.exe'
foreach ($readerRequired in @('SaeReader.exe', 'LICENSE', 'THIRD-PARTY-NOTICES.txt', 'licenses')) {
    if (-not (Test-Path -LiteralPath (Join-Path $readerSource $readerRequired))) {
        throw "Incomplete Windows package: missing $readerRequired."
    }
}
$readerVersion = [Diagnostics.FileVersionInfo]::GetVersionInfo($readerSourceExe)
if ($readerVersion.ProductName -ne 'Sae Reader') { throw 'This is not a Sae Reader executable.' }
$readerHash = (Get-FileHash -LiteralPath $readerSourceExe -Algorithm SHA256).Hash
if ($ExpectedSHA256 -and $readerHash -ne $ExpectedSHA256) { throw 'Executable checksum mismatch.' }
if (-not [string]::Equals($readerSource, $readerTarget, [StringComparison]::OrdinalIgnoreCase)) {
    $readerRunning = @(Get-Process -Name SaeReader -ErrorAction SilentlyContinue | Where-Object {
        if ($_.Path) {
            $readerRunningDirectory = Resolve-ReaderDirectory (Split-Path -Parent $_.Path)
            $readerRunningExe = Join-Path $readerRunningDirectory (Split-Path -Leaf $_.Path)
            [string]::Equals($readerRunningExe, $readerTargetExe, [StringComparison]::OrdinalIgnoreCase)
        }
    })
    if ($readerRunning.Count) { throw 'Close the installed Sae Reader normally before updating.' }
    New-Item -ItemType Directory -Path $readerTarget -Force | Out-Null
    # Only distribution files are copied. Portable data and personal books are
    # deliberately excluded; the normal LocalAppData profile is never touched.
    foreach ($readerName in @('LICENSE', 'THIRD-PARTY-NOTICES.txt', 'dependency-inventory.json', 'README.txt',
                             'README.md', 'CHANGELOG.md', 'assets', 'docs', 'licenses')) {
        $readerItem = Join-Path $readerSource $readerName
        if (Test-Path -LiteralPath $readerItem) {
            Copy-Item -LiteralPath $readerItem -Destination $readerTarget -Recurse -Force
        }
    }
    Copy-Item -LiteralPath $readerSourceExe -Destination $readerTargetExe -Force
    if ((Get-FileHash -LiteralPath $readerTargetExe -Algorithm SHA256).Hash -ne $readerHash) {
        throw 'Installed executable checksum mismatch; registration was not changed.'
    }
}
$readerInstalledScript = Join-Path $readerTarget 'Install.ps1'
$readerCurrentScript = Join-Path (Resolve-ReaderDirectory $readerScriptDirectory) (Split-Path -Leaf $readerScriptPath)
if (-not [string]::Equals($readerCurrentScript, $readerInstalledScript, [StringComparison]::OrdinalIgnoreCase)) {
    Copy-Item -LiteralPath $readerScriptPath -Destination $readerInstalledScript -Force
}

function Set-ReaderRegistryString([string]$Key, [string]$Name, [string]$Value) {
    $readerRegistryKey = [Microsoft.Win32.Registry]::CurrentUser.CreateSubKey($Key)
    try { $readerRegistryKey.SetValue($Name, $Value, [Microsoft.Win32.RegistryValueKind]::String) }
    finally { $readerRegistryKey.Dispose() }
}

$readerCommand = '"' + $readerTargetExe + '" "%1"'
$readerIcon = '"' + $readerTargetExe + '",0'
$readerAppKey = 'Software\Classes\Applications\SaeReader.exe'
$readerCapabilities = 'Software\SaeReader\Capabilities'
$readerTypes = [ordered]@{
    '.epub' = 'EPUB'; '.pdf' = 'PDF'; '.azw3' = 'AZW3'; '.mobi' = 'MOBI'
    '.fb2' = 'FB2'; '.txt' = 'TXT'; '.md' = 'Markdown'
}

# Windows may have created *_auto_file ProgIDs when the user originally chose
# a portable executable. Repair only handlers that already invoke our basename.
$readerLegacyClasses = @('Applications\SaeReader.exe', 'Applications\sae-reader.exe')
foreach ($readerExtension in $readerTypes.Keys) {
    $readerExtensionKey = [Microsoft.Win32.Registry]::ClassesRoot.OpenSubKey($readerExtension)
    if ($readerExtensionKey) {
        try { $readerLegacyClasses += [string]$readerExtensionKey.GetValue('') }
        finally { $readerExtensionKey.Dispose() }
    }
    foreach ($readerChoiceLocation in @('UserChoice', 'UserChoiceLatest\ProgId')) {
        $readerChoiceKey = [Microsoft.Win32.Registry]::CurrentUser.OpenSubKey(
            'Software\Microsoft\Windows\CurrentVersion\Explorer\FileExts\' + $readerExtension + '\' + $readerChoiceLocation)
        if ($readerChoiceKey) {
            try { $readerLegacyClasses += [string]$readerChoiceKey.GetValue('ProgId') }
            finally { $readerChoiceKey.Dispose() }
        }
    }
}
$readerRepaired = @()
foreach ($readerClass in ($readerLegacyClasses | Where-Object { $_ } | Select-Object -Unique)) {
    $readerOldKey = [Microsoft.Win32.Registry]::ClassesRoot.OpenSubKey($readerClass + '\shell\open\command')
    if (-not $readerOldKey) { continue }
    try { $readerOldCommand = [string]$readerOldKey.GetValue('') }
    finally { $readerOldKey.Dispose() }
    if ($readerOldCommand -match '^\s*(?:"(?<exe>[^"]+)"|(?<exe>.+?\.exe))(?:\s|$)' -and
        [IO.Path]::GetFileName($Matches.exe) -in @('SaeReader.exe', 'sae-reader.exe')) {
        Set-ReaderRegistryString ('Software\Classes\' + $readerClass + '\shell\open\command') '' $readerCommand
        Set-ReaderRegistryString ('Software\Classes\' + $readerClass + '\DefaultIcon') '' $readerIcon
        $readerRepaired += $readerClass
    }
}

Set-ReaderRegistryString $readerAppKey 'FriendlyAppName' 'Sae Reader'
Set-ReaderRegistryString ($readerAppKey + '\shell\open\command') '' $readerCommand
Set-ReaderRegistryString ($readerAppKey + '\DefaultIcon') '' $readerIcon
Set-ReaderRegistryString 'Software\Microsoft\Windows\CurrentVersion\App Paths\SaeReader.exe' '' $readerTargetExe
Set-ReaderRegistryString $readerCapabilities 'ApplicationName' 'Sae Reader'
Set-ReaderRegistryString $readerCapabilities 'ApplicationDescription' 'Native EPUB, PDF, AZW3, MOBI, FB2, TXT and Markdown reader.'
Set-ReaderRegistryString $readerCapabilities 'ApplicationIcon' $readerIcon
foreach ($readerExtension in $readerTypes.Keys) {
    $readerProgId = 'SaeReader.' + $readerTypes[$readerExtension]
    Set-ReaderRegistryString ('Software\Classes\' + $readerProgId) '' ('Sae Reader ' + $readerTypes[$readerExtension] + ' document')
    Set-ReaderRegistryString ('Software\Classes\' + $readerProgId + '\DefaultIcon') '' $readerIcon
    Set-ReaderRegistryString ('Software\Classes\' + $readerProgId + '\shell\open\command') '' $readerCommand
    Set-ReaderRegistryString ($readerAppKey + '\SupportedTypes') $readerExtension ''
    Set-ReaderRegistryString ($readerCapabilities + '\FileAssociations') $readerExtension $readerProgId
    Set-ReaderRegistryString ('Software\Classes\' + $readerExtension + '\OpenWithProgids') $readerProgId ''
}
Set-ReaderRegistryString 'Software\RegisteredApplications' 'Sae Reader' $readerCapabilities

$readerShell = New-Object -ComObject WScript.Shell
$readerShortcut = $readerShell.CreateShortcut((Join-Path ([Environment]::GetFolderPath('Programs')) 'Sae Reader.lnk'))
$readerShortcut.TargetPath = $readerTargetExe
$readerShortcut.WorkingDirectory = $readerTarget
$readerShortcut.IconLocation = $readerTargetExe + ',0'
$readerShortcut.Description = 'Sae Reader'
$readerShortcut.Save()

if (-not ('SaeReader.AssociationRefresh' -as [type])) {
    Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
namespace SaeReader {
    public static class AssociationRefresh {
        [DllImport("shell32.dll")]
        public static extern void SHChangeNotify(uint eventId, uint flags, IntPtr item1, IntPtr item2);
    }
}
'@
}
[SaeReader.AssociationRefresh]::SHChangeNotify(0x08000000, 0, [IntPtr]::Zero, [IntPtr]::Zero)
[PSCustomObject]@{
    InstalledExecutable = $readerTargetExe
    Version = $readerVersion.FileVersion
    SHA256 = $readerHash.ToLowerInvariant()
    RepairedHandlers = $readerRepaired
    DefaultSettings = 'ms-settings:defaultapps?registeredAppUser=Sae%20Reader'
}
if ($OpenDefaults) { Start-Process 'ms-settings:defaultapps?registeredAppUser=Sae%20Reader' }
