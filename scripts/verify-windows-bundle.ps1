$ErrorActionPreference = "Stop"

$bundleDirectory = Join-Path $PSScriptRoot "..\packaging\bundles\windows-x86_64"
if (-not (Test-Path -LiteralPath $bundleDirectory -PathType Container)) {
    throw "missing Windows bundle directory: $bundleDirectory"
}

$bundleEntries = @(Get-ChildItem -LiteralPath $bundleDirectory -Force | Where-Object { $_.Name -ne ".gitkeep" })
if ($bundleEntries.Count -gt 0) {
    $manifestPath = Join-Path $bundleDirectory "manifest.json"
    if (-not (Test-Path -LiteralPath $manifestPath -PathType Leaf)) {
        throw "bundle artifacts require manifest.json: $bundleDirectory"
    }
}

Write-Output "Windows bundle preflight passed: $bundleDirectory"

