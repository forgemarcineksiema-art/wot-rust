param(
    # Crates whose tests run in full (with their integration tests). Pass the crates a PR
    # touched; their dependents are covered by clippy's whole-workspace check and by the
    # daily full gate. With no crates named, every workspace lib/bin/test target runs.
    [string[]] $Crates
)

$ErrorActionPreference = "Stop"

# THE PER-PR GATE (Inny Poziom, 2026-09-02). `verify.ps1` compiles every example and bench with
# codegen and runs every test binary — 25-35 minutes a run on the MX330 laptop, most of it the
# probe binary's forty modules and the client's test targets being rebuilt for a change that
# touched neither. This gate keeps what catches a broken PR and drops what only catches a broken
# day:
#   - rustfmt, as before;
#   - clippy over the WHOLE workspace and ALL targets in check mode (no codegen): a probe or a
#     bench that no longer compiles fails here, in a fraction of the build;
#   - the tests of the crates named, plus `quality` (the ratchet) always;
#   - no example or bench codegen, no full-workspace test run.
# Run `verify.ps1` (the full gate) before a merge that touches examples, benches, the wire, the
# replay fixtures or the physics numbers — and at least once a day over what landed.
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
Invoke-Checked "clippy (whole workspace, all targets, check mode)" {
    cargo clippy --workspace --all-targets -- -D warnings
}
if ($Crates -and $Crates.Count -gt 0) {
    # The ratchet rides along whatever the PR names — once, not twice when it is named too.
    $tested = @("quality") + @($Crates | Where-Object { $_ -ne "quality" })
    $packages = @()
    foreach ($crate in $tested) { $packages += @("-p", $crate) }
    Invoke-Checked "tests ($($tested -join ', '))" { cargo test @packages }
} else {
    # `--workspace` already carries the ratchet.
    Invoke-Checked "tests (workspace lib/bin/test targets, no examples or benches)" {
        cargo test --workspace --lib --bins --tests
    }
}
Write-Host "PR gate green. The full gate (scripts/verify.ps1) still owes the day its run."
