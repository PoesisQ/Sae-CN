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
Get-ComputerInfo | Select-Object WindowsProductName,WindowsVersion,OsBuildNumber,CsNumberOfLogicalProcessors | ConvertTo-Json | Set-Content "$dir/machine.json"
foreach($t in Get-ChildItem artifacts/unit-tests/*.exe){Check "unit-$($t.BaseName)" {& $t.FullName --test-threads=1}}
Check 'formula-time' {& ./artifacts/unit-tests/math_performance.exe --ignored --test-threads=1}
foreach($scale in '1','1.5','2'){Check "glass-$scale" {python scripts/check-glass.py --exe $qa --output "$dir/glass-$scale" --scale $scale}}
Check 'native-suite' {& "$dir/press-suite.ps1"}
Check 'home-motion' {python scripts/qa-windows.py --exe $qa --blank --scenario home-motion --output "$dir/home"}
Check 'pdf-gestures' {python scripts/check-wheel-input.py --exe $qa --output "$dir/wheels"}
Check 'epub-gestures' {python scripts/check-wheel-input.py --exe $qa --epub --output "$dir/epub-wheel"}
Check 'instrument-pixels' {python scripts/check-control-snapshots.py "$dir/press-ui/instruments/fixture-00.json" --wheel-samples "$dir/wheels" "$dir/epub-wheel"}
Check 'wheel-cost' {python "$dir/wheel-cost.py"}
Check 'home-cost' {python "$dir/profile-home.py" --exe $prod --runs 2 --modes plasma blue static --out "$dir/home-cost.json"}
Check 'startup-and-memory' {python "$dir/compare-perf.py"}
if($failed.Count){throw ('Failed cases: '+($failed -join ', '))}
