$ErrorActionPreference = "Stop"

# Fast inner-loop verify: fmt + clippy + tests over lib/bins/tests only — no examples, no benches,
# no release bench compile. Use this while iterating; run the full scripts/verify.ps1 before pushing.
if (-not $env:CARGO_BUILD_JOBS) {
    $cores = [Environment]::ProcessorCount
    $env:CARGO_BUILD_JOBS = [string]([Math]::Max(1, $cores - 2))
}
Write-Host "Using CARGO_BUILD_JOBS=$($env:CARGO_BUILD_JOBS)"

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
Invoke-Checked "clippy" { cargo clippy --workspace --tests -- -D warnings }
Invoke-Checked "tests" { cargo test --workspace }
