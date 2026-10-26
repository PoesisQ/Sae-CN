param(
    [string]$Name='SaeReader-0.2.0-preview.1',
    [string]$ReuseRegistryFrom
)
$ErrorActionPreference='Stop'
. "$PSScriptRoot\env.ps1"
cargo build --release --locked --offline --bin sae-reader
if ($LASTEXITCODE -ne 0) { throw 'Release build failed' }
$readerProduction=Join-Path "$PSScriptRoot\..\artifacts\production" $Name
New-Item -ItemType Directory -Path $readerProduction -Force | Out-Null
Copy-Item -LiteralPath "$PSScriptRoot\..\target\x86_64-pc-windows-msvc\release\sae-reader.exe" -Destination (Join-Path $readerProduction 'SaeReader.exe')
if (-not $ReuseRegistryFrom) {
    cargo vendor --locked --versioned-dirs "$PSScriptRoot\..\artifacts\distribution-dependencies" | Out-Null
    if ($LASTEXITCODE -ne 0) { throw 'Dependency source preparation failed' }
}
cargo metadata --locked --offline --format-version 1 | Set-Content -Encoding UTF8 "$PSScriptRoot\..\artifacts\cargo-metadata.json"
cargo tree --locked --offline --target x86_64-pc-windows-msvc --edges normal,build --prefix none --format '{p}|{l}' | Set-Content -Encoding UTF8 "$PSScriptRoot\..\artifacts\used-dependencies.txt"
$readerPackageArgs=@("$PSScriptRoot\package.py",'--name',$Name,'--exe',(Join-Path $readerProduction 'SaeReader.exe'))
if ($ReuseRegistryFrom) { $readerPackageArgs+=@('--reuse-registry-from',$ReuseRegistryFrom) }
python @readerPackageArgs
if ($LASTEXITCODE -ne 0) { throw 'Packaging failed' }
