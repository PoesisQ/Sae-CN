$ErrorActionPreference = 'Stop'
$dir='artifacts/iteration-0.6.4'
$qa='artifacts/qa/SaeReader-0.6.4-preview.1-qa.exe'
$prod='artifacts/production/SaeReader-0.6.4-preview.1/SaeReader.exe'
$env:SAE_MATH_BENCH_REPORT=(Join-Path (Resolve-Path $dir) 'math.json')
$env:PYTHONUTF8='1'
$failed = [Collections.Generic.List[string]]::new()
function Check([string]$name,[scriptblock]$action){
    $global:LASTEXITCODE=0
    Write-Output "::group::$name"
    try {
        & $action *> "$dir/$name.log"
        if($LASTEXITCODE){throw "exit $LASTEXITCODE"}
        Get-Content "$dir/$name.log" -Tail 3
    } catch {
        $failed.Add($name)
        Get-Content "$dir/$name.log" -Tail 12 -ErrorAction SilentlyContinue
        Write-Output "::error::$name failed: $_"
    }
    Write-Output '::endgroup::'
}
# Disposable GitHub runner session. Exercise the Windows motion preference
# both disabled and enabled; the user's workstation is not changed.
Add-Type -TypeDefinition @"
using System;
using System.Runtime.InteropServices;
public class RunnerDesktop {
 [DllImport("user32.dll",EntryPoint="SystemParametersInfoW",SetLastError=true)] public static extern bool Set(uint action,uint param,IntPtr value,uint flags);
 [DllImport("user32.dll",EntryPoint="SystemParametersInfoW",SetLastError=true)] public static extern bool Get(uint action,uint param,out int value,uint flags);
 [DllImport("user32.dll")] public static extern int GetSystemMetrics(int n);
}
"@
$motion=0
$null=[RunnerDesktop]::Get(0x1042,0,[ref]$motion,0)
Write-Output ("Desktop {0}x{1}, original motion {2}" -f [RunnerDesktop]::GetSystemMetrics(0),[RunnerDesktop]::GetSystemMetrics(1),$motion)
if(-not [RunnerDesktop]::Set(0x1043,0,[IntPtr]1,3)){throw 'Could not enable client animations on test runner'}
$null=[RunnerDesktop]::Get(0x1042,0,[ref]$motion,0)
if($motion -ne 1){throw 'Test user has disabled client area animation'}
# Fit the entire test window and the captured pointer travel on the runner's
# 1024x768 desktop. The geometry tests above independently exercise 100–200% DPI.
if(Get-Command Set-DisplayResolution -ErrorAction SilentlyContinue) {
    try { Set-DisplayResolution -Width 1920 -Height 1080 -Force -ErrorAction Stop | Out-Null } catch { Write-Output $_ }
}
$screenW=[RunnerDesktop]::GetSystemMetrics(0)
$screenH=[RunnerDesktop]::GetSystemMetrics(1)
$env:SLINT_SCALE_FACTOR=if($screenW -ge 1440 -and $screenH -ge 1000){'1'}else{'0.7'}
Write-Output ("Pointer desktop {0}x{1} at {2}" -f $screenW,$screenH,$env:SLINT_SCALE_FACTOR)
Check 'pdf-gestures' {python scripts/check-wheel-input.py --exe $qa --output "$dir/wheels"}
Check 'epub-gestures' {python scripts/check-wheel-input.py --exe $qa --epub --output "$dir/epub-wheel"}
Check 'instrument' {python scripts/qa-windows.py --exe $qa --scenario instruments --output "$dir/instruments" --fixture tests/fixtures/core.pdf}
Check 'instrument-pixels-final' {python scripts/check-control-snapshots.py "$dir/instruments/fixture-00.json" --wheel-samples "$dir/wheels" "$dir/epub-wheel"}
if($failed.Count){throw ('Failed cases: '+($failed -join ', '))}
