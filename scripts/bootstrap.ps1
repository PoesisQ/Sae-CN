param([string]$Python = 'python')
$ErrorActionPreference = 'Stop'
$readerRoot = Split-Path -Parent $PSScriptRoot
$readerTools = Join-Path $readerRoot '.tools'
New-Item -ItemType Directory -Force -Path $readerTools | Out-Null
$readerInstaller = Join-Path $readerTools 'rustup-init.exe'
$readerChecksum = $readerInstaller + '.sha256'
& curl.exe -fsSL --retry 3 --max-time 300 'https://static.rust-lang.org/rustup/dist/x86_64-pc-windows-msvc/rustup-init.exe' -o $readerInstaller
if ($LASTEXITCODE -ne 0) { throw 'Cannot download official rustup installer' }
& curl.exe -fsSL --retry 3 --max-time 60 'https://static.rust-lang.org/rustup/dist/x86_64-pc-windows-msvc/rustup-init.exe.sha256' -o $readerChecksum
if ($LASTEXITCODE -ne 0) { throw 'Cannot download official rustup checksum' }
$readerExpected = ((Get-Content -LiteralPath $readerChecksum -Raw) -split '\s+')[0]
if ((Get-FileHash -LiteralPath $readerInstaller -Algorithm SHA256).Hash -ne $readerExpected) { throw 'Rustup SHA256 mismatch' }
. (Join-Path $PSScriptRoot 'env.ps1')
& $readerInstaller -y --no-modify-path --default-host x86_64-pc-windows-msvc --default-toolchain 1.99.1 --profile minimal
if ($LASTEXITCODE -ne 0) { throw 'Private Rust installation failed' }
& $Python -m pip install --disable-pip-version-check --no-warn-script-location --target (Join-Path $readerTools 'python') 'libclang==18.1.1'
if ($LASTEXITCODE -ne 0) { throw 'Build-only libclang installation failed' }
& rustc -Vv
& cargo -V
Write-Host 'MSVC v143 and Windows SDK must be installed separately. No system PATH changes were made.'
