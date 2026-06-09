$ErrorActionPreference = "Stop"

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
Invoke-Checked "clippy" { cargo clippy --workspace --all-targets -- -D warnings }
Invoke-Checked "tests" { cargo test --workspace --all-targets }
Invoke-Checked "check" { cargo check --workspace --all-targets }
Invoke-Checked "bench compile" { cargo bench --workspace --no-run }
