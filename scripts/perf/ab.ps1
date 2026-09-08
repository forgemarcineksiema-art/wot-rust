# Cold GPU measurements on the MX330's terms (the one program's Q7). The card is 6 C warmer and
# throttling by the end of an eight-second capture, so a number is only comparable to another
# number taken FIRST in its own cold process. Two modes:
#
#   A/B of two probe binaries (one row, alternated A, B, B, A, A, B ...):
#     scripts/perf/ab.ps1 -A <probe-A.exe> -B <probe-B.exe> -Rounds 3 -Map bystra-valley
#   attribution with one binary (each row first and cold in its own process, rounds interleaved):
#     scripts/perf/ab.ps1 -A <probe.exe> -Rows "full scene","no card meadow","no near ring" -Rounds 3
#
# Every run waits for the GPU to cool under -CoolBelowC, runs the probe's quick capture
# (`WOT_PERF_QUICK=1`, and `WOT_PERF_ONLY=<row>`), and reads back the GPU p50 of the frame and
# of the scene pass. Build a probe with `cargo build --release -p client --example probe` and copy
# `target\release\examples\probe.exe` aside before building the other. Full outputs land in
# `output/perf/ab-<stamp>/`; the table and the medians in its `summary.txt`.
param(
    [Parameter(Mandatory = $true)][string]$A,
    [string]$B = "",
    [string[]]$Rows = @("full scene"),
    [int]$Rounds = 3,
    [int]$CoolBelowC = 68,
    [int]$MaxWaitS = 400,
    [string]$Map = "bystra-valley"
)
$ErrorActionPreference = "Stop"
$root = Resolve-Path (Join-Path $PSScriptRoot "..\..")
$stamp = Get-Date -Format "yyyyMMdd-HHmmss"
$outDir = Join-Path $root "output\perf\ab-$stamp"
New-Item -ItemType Directory -Force $outDir | Out-Null

function Gpu-TempC {
    try {
        $q = & nvidia-smi --query-gpu=temperature.gpu --format=csv,noheader,nounits 2>$null
        return [int]($q.Trim())
    } catch { return -1 }
}
function Wait-Cool {
    $waited = 0
    $temp = Gpu-TempC
    while ($temp -gt $CoolBelowC -and $waited -lt $MaxWaitS) {
        Start-Sleep -Seconds 10
        $waited += 10
        $temp = Gpu-TempC
    }
    return $temp
}
function Read-P50 {
    param([string]$Path, [string]$RowName)
    $text = Get-Content $Path -Raw
    $frame = [regex]::Match($text, "\[" + [regex]::Escape($RowName) + "\] frame p50\s+([0-9.]+)")
    $scene = [regex]::Match($text, "\[" + [regex]::Escape($RowName) + "\] frame p50[^\n]*\n(?:[^\n]*\n)*?\s+scene_pass\s+p50\s+([0-9.]+)")
    $f = if ($frame.Success) { [double]$frame.Groups[1].Value } else { [double]::NaN }
    $s = if ($scene.Success) { [double]$scene.Groups[1].Value } else { [double]::NaN }
    return @($f, $s)
}
function Median { param([double[]]$xs) $s = @($xs | Sort-Object); $n = $s.Count; if ($n -eq 0) { return [double]::NaN }; if ($n % 2 -eq 1) { return $s[[math]::Floor($n / 2)] } else { return ($s[$n / 2 - 1] + $s[$n / 2]) / 2 } }

$env:WOT_MAP = $Map
$env:WOT_PERF_QUICK = "1"
$env:RUST_LOG = "warn"
# `powershell -File` hands a comma list to a string[] as ONE string: split it back.
$Rows = @($Rows | ForEach-Object { $_ -split "," } | ForEach-Object { $_.Trim() } | Where-Object { $_ -ne "" })

# The plan: a list of (label, exe, row). A/B alternates the binaries on one row; attribution
# alternates the rows on one binary. Either way every measurement is the first row of a cold run.
$plan = @()
if ($B -ne "") {
    for ($r = 0; $r -lt $Rounds; $r++) {
        $pair = if ($r % 2 -eq 0) { @("A", "B") } else { @("B", "A") }
        foreach ($which in $pair) {
            $exe = if ($which -eq "A") { $A } else { $B }
            $plan += [pscustomobject]@{ label = $which; exe = $exe; row = $Rows[0] }
        }
    }
} else {
    for ($r = 0; $r -lt $Rounds; $r++) {
        $ordered = @($Rows)
        if ($r % 2 -eq 1) { [array]::Reverse($ordered) }
        foreach ($row in $ordered) {
            $plan += [pscustomobject]@{ label = $row; exe = $A; row = $row }
        }
    }
}

$results = @()
$i = 0
foreach ($step in $plan) {
    $env:WOT_PERF_ONLY = $step.row
    $temp = Wait-Cool
    $safe = ($step.label -replace "[^A-Za-z0-9]+", "-")
    $log = Join-Path $outDir ("{0:d2}-{1}.txt" -f $i, $safe)
    $sw = [Diagnostics.Stopwatch]::StartNew()
    # Start-Process, not `& exe 2>&1`: PowerShell 5.1 turns a native stderr line into an error record.
    $proc = Start-Process -FilePath $step.exe -ArgumentList "perf_capture" -RedirectStandardOutput $log -RedirectStandardError ($log + ".err") -NoNewWindow -PassThru -Wait
    $sw.Stop()
    if ($proc.ExitCode -ne 0) { Write-Host ("run {0} exited with {1} - see {2}.err" -f $i, $proc.ExitCode, $log) }
    $p = Read-P50 -Path $log -RowName $step.row
    $after = Gpu-TempC
    $results += [pscustomobject]@{ run = $i; label = $step.label; before_c = $temp; after_c = $after; secs = [math]::Round($sw.Elapsed.TotalSeconds, 1); frame_p50 = $p[0]; scene_p50 = $p[1] }
    Write-Host ("run {0} [{1}]: GPU {2} C -> {3} C, {4} s, frame p50 {5:n2} ms, scene_pass p50 {6:n2} ms" -f $i, $step.label, $temp, $after, $results[-1].secs, $p[0], $p[1])
    $i += 1
}

$lines = @()
foreach ($label in ($plan | ForEach-Object { $_.label } | Select-Object -Unique)) {
    $rs = @($results | Where-Object label -eq $label)
    $scene = @($rs | ForEach-Object { $_.scene_p50 })
    $frame = @($rs | ForEach-Object { $_.frame_p50 })
    $lines += ("[{0}] scene_pass p50 median {1:n2} ms (spread {2:n2}..{3:n2}), GPU frame p50 median {4:n2} ms, {5} runs" -f $label, (Median $scene), ($scene | Measure-Object -Minimum).Minimum, ($scene | Measure-Object -Maximum).Maximum, (Median $frame), $rs.Count)
}
$header = "cold runs on $Map, $Rounds rounds, GPU cooled under $CoolBelowC C before each, quick capture ($($Rows -join ', '))"
Write-Host $header
$lines | ForEach-Object { Write-Host $_ }
($results | Format-Table -AutoSize | Out-String) | Write-Host
($header + "`n" + ($lines -join "`n") + "`n" + ($results | Format-Table -AutoSize | Out-String)) | Out-File -Encoding utf8 (Join-Path $outDir "summary.txt")
Write-Host "written: $outDir"
