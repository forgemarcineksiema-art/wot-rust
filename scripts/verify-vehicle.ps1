param(
    # The vehicle slug the PR is about (t54_1951, tiger_i_ausf_e, ...).
    [Parameter(Mandatory = $true)]
    [string] $Vehicle
)

$ErrorActionPreference = "Stop"

# THE VEHICLE-DATA GATE (acceleration step 1, 2026-09-05). For a PR that changes ONE vehicle's
# data — its blueprint / visual / outline / reference RON, its inventory row, its dossier, its
# goldens — and no Rust. It runs what such a change can break, in minutes:
#   - rustfmt (cheap, always);
#   - clippy over the vehicle crates only (all targets, check mode);
#   - the vehicle crates' tests, the armour/hitbox tests in `sim`, the ratchet;
#   - the studio tiles and the asset parity in `tools`;
#   - the K0 outline scores of THIS vehicle, printed to be looked at.
# A PR that touches Rust runs `verify-pr.ps1 -Crates ...`; the full gate still owes the day its run.
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

$vehicleCrates = @("game_core", "vehicle_geometry", "vehicle_build", "vehicle_recipes", "vehicle_forge")
$clippyPackages = @()
foreach ($crate in $vehicleCrates) { $clippyPackages += @("-p", $crate) }

Invoke-Checked "rustfmt" { cargo fmt --all -- --check }
Invoke-Checked "clippy (vehicle crates, all targets, check mode)" {
    cargo clippy @clippyPackages --all-targets -- -D warnings
}
$testPackages = @()
foreach ($crate in ($vehicleCrates + @("sim", "quality"))) { $testPackages += @("-p", $crate) }
Invoke-Checked "tests (vehicle crates, sim, quality)" { cargo test @testPackages }
Invoke-Checked "studio tiles + asset parity (tools)" {
    cargo test -p tools --test studio_goldens --test vehicle_asset_parity
}
Invoke-Checked "K0 outline scores ($Vehicle)" {
    cargo run -p tools -- outline-overlay --vehicle $Vehicle
}
Write-Host "Vehicle gate green for $Vehicle. Rust changes take verify-pr.ps1; the full gate still owes the day its run."
