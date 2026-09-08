# A COLD measurement, gated by the GPU's own thermometer (the one program's Q7; the owner's
# condition of 2026-09-06: "only on a free, cool machine - two sessions heat it"). The MX330
# throttles inside the first 100 s of a probe; a number taken warm measures the heat, not the
# code. This waits for the GPU to cool under the threshold, logs its temperature and SM clock
# before and after, samples the client's working set and the GPU's used memory while the run
# lasts, and runs the command it is given.
#
#   scripts/perf/cold-run.ps1 -Label baseline -Run "cargo run --release -p client --example probe -- perf_capture"
#   scripts/perf/cold-run.ps1 -Label drive -MemoryProcess client -Run "target/release/client.exe"
#
# Writes `output/perf/<label>-<timestamp>.thermal.txt` beside whatever the command writes.
param(
    [string]$Label = "run",
    [int]$CoolBelowC = 60,
    [int]$MaxWaitS = 900,
    [string]$MemoryProcess = "",
    [Parameter(Mandatory = $true)][string]$Run
)
$ErrorActionPreference = "Stop"
$root = Resolve-Path (Join-Path $PSScriptRoot "..\..")
$outDir = Join-Path $root "output\perf"
New-Item -ItemType Directory -Force $outDir | Out-Null
$stamp = Get-Date -Format "yyyyMMdd-HHmmss"
$log = Join-Path $outDir "$Label-$stamp.thermal.txt"

function Gpu-Line {
    try {
        $q = & nvidia-smi --query-gpu=temperature.gpu,clocks.sm,clocks.max.sm,memory.used,memory.total,clocks_throttle_reasons.active --format=csv,noheader,nounits 2>$null
        return ($q -replace "\s+", "")
    } catch { return "nvidia-smi unavailable" }
}
function Gpu-TempC {
    $line = Gpu-Line
    if ($line -match "^(\d+),") { return [int]$Matches[1] }
    return -1
}

"# cold run: $Label - $(Get-Date -Format s)" | Out-File -Encoding utf8 $log
"command: $Run" | Out-File -Encoding utf8 -Append $log
$waited = 0
$temp = Gpu-TempC
while ($temp -gt $CoolBelowC -and $waited -lt $MaxWaitS) {
    "waiting: GPU $temp C > $CoolBelowC C ($waited s)" | Out-File -Encoding utf8 -Append $log
    Start-Sleep -Seconds 15
    $waited += 15
    $temp = Gpu-TempC
}
"before: temp,sm_mhz,sm_max_mhz,mem_used_mib,mem_total_mib,throttle_mask = $(Gpu-Line) (waited $waited s)" | Out-File -Encoding utf8 -Append $log

$env:WOT_COLD_RUN_LABEL = $Label
$proc = Start-Process -FilePath "cmd.exe" -ArgumentList "/c", $Run -NoNewWindow -PassThru
$samples = 0
while (-not $proc.HasExited) {
    Start-Sleep -Seconds 5
    $samples += 1
    $mem = ""
    if ($MemoryProcess -ne "") {
        $p = Get-Process -Name $MemoryProcess -ErrorAction SilentlyContinue | Sort-Object WorkingSet64 -Descending | Select-Object -First 1
        if ($p) { $mem = " working_set_mib=$([math]::Round($p.WorkingSet64 / 1MB)) private_mib=$([math]::Round($p.PrivateMemorySize64 / 1MB))" }
    }
    "t=$($samples * 5)s gpu=$(Gpu-Line)$mem" | Out-File -Encoding utf8 -Append $log
}
$proc.WaitForExit()
"after: temp,sm_mhz,sm_max_mhz,mem_used_mib,mem_total_mib,throttle_mask = $(Gpu-Line)" | Out-File -Encoding utf8 -Append $log
"exit code: $($proc.ExitCode)" | Out-File -Encoding utf8 -Append $log
Write-Host "thermal log: $log"
exit $proc.ExitCode
