$ErrorActionPreference = "Stop"

# THE PREFLIGHT (Inny Poziom Q6 follow-up, 2026-09-02): thirty seconds before a gate. Four of the
# day's gate reruns died on a `quality` ratchet — a duplicated free function, a contract check no
# test named, a fleet walk without a floor — each found after eight minutes of fmt and clippy when
# the ratchet itself answers in twenty seconds. Run this first; launch the gate when it is green.
if (-not $env:CARGO_BUILD_JOBS) {
    $cores = [Environment]::ProcessorCount
    $env:CARGO_BUILD_JOBS = [string]([Math]::Max(1, $cores - 2))
}

function Invoke-Checked {
    param(
        [Parameter(Mandatory = $true)]
        [string] $Name,
        [Parameter(Mandatory = $true)]
        [scriptblock] $Command
    )

    Write-Host "==> $Name"
    & $Command
    if ($LASTEXITCODE -ne 0) {
        throw "$Name failed with exit code $LASTEXITCODE"
    }
}

Invoke-Checked "rustfmt" { cargo fmt --all -- --check }
Invoke-Checked "quality ratchet" { cargo test -p quality }
Write-Host "Preflight green. Launch the gate."
