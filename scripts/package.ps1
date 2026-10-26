$ErrorActionPreference='Stop'
. "$PSScriptRoot\env.ps1"
cargo build --release --locked --offline --bin sae-reader
if ($LASTEXITCODE -ne 0) { throw 'Release build failed' }
New-Item -ItemType Directory -Path "$PSScriptRoot\..\artifacts\production" -Force | Out-Null
Copy-Item -LiteralPath "$PSScriptRoot\..\target\x86_64-pc-windows-msvc\release\sae-reader.exe" -Destination "$PSScriptRoot\..\artifacts\production\SaeReader.exe"
cargo vendor --locked --versioned-dirs "$PSScriptRoot\..\artifacts\distribution-dependencies" | Out-Null
if ($LASTEXITCODE -ne 0) { throw 'Dependency source preparation failed' }
cargo metadata --locked --offline --format-version 1 | Set-Content -Encoding UTF8 "$PSScriptRoot\..\artifacts\cargo-metadata.json"
cargo tree --locked --offline --target x86_64-pc-windows-msvc --edges normal,build --prefix none --format '{p}|{l}' | Set-Content -Encoding UTF8 "$PSScriptRoot\..\artifacts\used-dependencies.txt"
python "$PSScriptRoot\package.py"
if ($LASTEXITCODE -ne 0) { throw 'Packaging failed' }
