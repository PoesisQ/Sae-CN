param(
    [Parameter(Mandatory=$true)][string]$Books,
    [string]$Output='artifacts/next/verified'
)
$ErrorActionPreference='Stop'
$readerRoot=Split-Path -Parent $PSScriptRoot
Set-Location -LiteralPath $readerRoot
function Invoke-ReaderMeasurement {
    param([string[]]$Arguments)
    python @Arguments
    if ($LASTEXITCODE -ne 0) { throw 'Measurement failed' }
}
# Compare uninstrumented production memory/counters with an untouched old EXE.
# Each profiler owns only its own child; this is not a graceful-close test.
Invoke-ReaderMeasurement -Arguments @('scripts/profile-window.py','--exe','artifacts/production/SaeReader.exe','--runs','3','--output',"$Output/blank")
Invoke-ReaderMeasurement -Arguments @('scripts/profile-window.py','--exe','artifacts/production/SaeReader.exe','--books',$Books,'--runs','3','--output',"$Output/single")
Invoke-ReaderMeasurement -Arguments @('scripts/profile-window.py','--exe','artifacts/production/SaeReader.exe','--books',$Books,'--spread','--runs','3','--output',"$Output/double")
Invoke-ReaderMeasurement -Arguments @('scripts/profile-window.py','--exe','artifacts/production/SaeReader.exe','--fixture','tests/fixtures/core.pdf','--runs','3','--output',"$Output/pdf")
# The separate QA feature reports native software-renderer readiness from main,
# not pre-main OS startup, a cold launch, or compositor/interaction readiness.
Invoke-ReaderMeasurement -Arguments @('scripts/qa-windows.py','--blank','--runs','20','--output',"$Output/startup-blank")
Invoke-ReaderMeasurement -Arguments @('scripts/qa-windows.py','--books',$Books,'--runs','20','--output',"$Output/startup-epub")
Invoke-ReaderMeasurement -Arguments @('scripts/qa-windows.py','--books',$Books,'--spread','--runs','20','--output',"$Output/startup-spread")
foreach ($readerFormat in @('core.pdf','text-gb18030.txt','reading.md')) {
    Invoke-ReaderMeasurement -Arguments @('scripts/qa-windows.py','--fixture',"tests/fixtures/$readerFormat",'--runs','20','--output',"$Output/startup-$readerFormat")
}
