param(
    [Parameter(ValueFromRemainingArguments = $true)]
    [string[]]$RemainingArgs
)

$scriptDir = Split-Path -Parent $PSCommandPath
$bashCandidates = @(
    'C:\Program Files\Git\bin\bash.exe',
    'C:\Program Files\Git\usr\bin\bash.exe',
    'C:\Program Files (x86)\Git\bin\bash.exe',
    'C:\Program Files (x86)\Git\usr\bin\bash.exe'
)

$bashCommand = Get-Command bash -ErrorAction SilentlyContinue
if ($bashCommand -and $bashCommand.Source -notlike '*WindowsApps*') {
    $bashCandidates += $bashCommand.Source
}

$bash = $null
foreach ($candidate in $bashCandidates) {
    if ([string]::IsNullOrWhiteSpace($candidate)) {
        continue
    }
    if (Test-Path -LiteralPath $candidate) {
        $bash = $candidate
        break
    }
}

if (-not $bash) {
    Write-Error "bash.exe not found. Install Git for Windows or expose bash in PATH."
    exit 2
}

& $bash "$scriptDir/admission.sh" @RemainingArgs
exit $LASTEXITCODE
