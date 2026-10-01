<#
.SYNOPSIS
    Prints the CHANGELOG.md section of one version (without its heading).
.EXAMPLE
    ./scripts/extract-changelog.ps1 -Version 0.1.0
#>
param(
    [Parameter(Mandatory)][string]$Version,
    [string]$Path = (Join-Path $PSScriptRoot '..\CHANGELOG.md')
)

$lines = Get-Content -LiteralPath $Path
$start = -1
for ($i = 0; $i -lt $lines.Count; $i++) {
    if ($lines[$i] -match ('^## \[' + [regex]::Escape($Version) + '\]')) { $start = $i + 1; break }
}
if ($start -lt 0) { Write-Error "No section for $Version in $Path"; exit 1 }

$end = $lines.Count
for ($i = $start; $i -lt $lines.Count; $i++) {
    # Next version heading, or the link references at the bottom.
    if ($lines[$i] -match '^## \[' -or $lines[$i] -match '^\[[^\]]+\]: ') { $end = $i; break }
}
($lines[$start..($end - 1)] -join "`n").Trim()
