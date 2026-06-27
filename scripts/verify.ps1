param(
    # Include the release-profile bench compile (a second, optimized build of the whole workspace).
    # Off by default — it roughly doubles wall time. Run with -Release before cutting a release / tag.
    [switch] $Release
)

$ErrorActionPreference = "Stop"

# Keep the laptop responsive while verifying: leave ~2 cores free for the OS/UI instead of letting
# cargo saturate every logical CPU (which causes thermal throttling and system-wide lag).
# Override by exporting CARGO_BUILD_JOBS before calling this script.
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
# clippy --all-targets runs the full compiler front-end over every target, so a separate
# `cargo check` would be redundant work — dropped on purpose.
Invoke-Checked "clippy" { cargo clippy --workspace --all-targets -- -D warnings }
Invoke-Checked "tests" { cargo test --workspace --all-targets }

if ($Release) {
    Invoke-Checked "bench compile" { cargo bench --workspace --no-run }
}
