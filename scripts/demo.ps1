param(
    [switch]$VerboseEvidence
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

function Invoke-Step {
    param(
        [string]$Name,
        [scriptblock]$Command
    )

    Write-Host ""
    Write-Host "==> $Name"
    & $Command
    if ($LASTEXITCODE -ne 0) {
        throw "Step failed: $Name with exit code $LASTEXITCODE"
    }
}

if (-not (Test-Path -LiteralPath ".git")) {
    throw "Run this script from the repository root."
}

Write-Host "CCL demo"
Write-Host "Repository root: $(Get-Location)"

Invoke-Step "CCL version" {
    cargo run -p ccl-cli -- --version
}

Invoke-Step "Contract check" {
    cargo run -p ccl-cli -- contract check examples/ccl-admission-task-contract.json
}

Invoke-Step "Repository preflight" {
    cargo run -p ccl-cli -- preflight --repo .
}

if ($VerboseEvidence) {
    Invoke-Step "Validation runner" {
        cargo run -p ccl-cli -- validate run --contract examples/ccl-admission-task-contract.json --repo .
    }

    Invoke-Step "Scope check" {
        cargo run -p ccl-cli -- scope check --contract examples/ccl-admission-task-contract.json --repo .
    }

    Invoke-Step "Ledger verification" {
        cargo run -p ccl-cli -- ledger verify --contract examples/ccl-admission-task-contract.json --repo .
    }
}

Invoke-Step "Gate run" {
    cargo run -p ccl-cli -- gate run --contract examples/ccl-admission-task-contract.json --repo .
}

Write-Host ""
Write-Host "CCL demo completed."
Write-Host "Expected result: gate PASS."
Write-Host "Generated evidence artifacts are under .ccl/runs/."
