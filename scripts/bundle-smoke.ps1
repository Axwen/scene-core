param(
    [Parameter(Mandatory = $true)][string]$BundleRoot,
    [Parameter(Mandatory = $true)][string]$TrustedManifestSha256,
    [string]$WorkRoot = ""
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

if (-not $WorkRoot) { $WorkRoot = Join-Path $env:TEMP "scene-core-smoke" }
if (-not (Test-Path $BundleRoot)) { throw "bundle root does not exist" }
$BundleRoot = (Resolve-Path $BundleRoot).Path

$exe = Join-Path $BundleRoot "bin/scene-core.exe"

function Invoke-Cli([string[]]$Arguments) {
    $output = & $exe @Arguments 2>$null
    $exit = $LASTEXITCODE
    return @{ Exit = $exit; Text = ($output -join "`n") }
}

$version = Invoke-Cli @("version", "--json")
if ($version.Exit -ne 0) { throw "version --json exited $($version.Exit)" }
$versionJson = $version.Text | ConvertFrom-Json
if ($versionJson.implementedOperations.Count -ne 0) { throw "implementedOperations must be empty" }
if (-not $versionJson.toolchainFingerprint) { throw "toolchainFingerprint is missing" }

$doctor = Invoke-Cli @(
    "doctor", "--json",
    "--bundle-root", $BundleRoot,
    "--trusted-manifest-sha256", $TrustedManifestSha256
)
if ($doctor.Exit -ne 0) { throw "healthy doctor exited $($doctor.Exit): $($doctor.Text)" }
$doctorJson = $doctor.Text | ConvertFrom-Json
if ($doctorJson.status -ne "ok") { throw "healthy doctor status is $($doctorJson.status)" }
if ($doctorJson.checks.Count -lt 10) { throw "doctor is missing checks" }

function New-BundleCopy([string]$Name) {
    $target = Join-Path $WorkRoot $Name
    if (Test-Path $target) { Remove-Item -Recurse -Force $target }
    New-Item -ItemType Directory -Force -Path $WorkRoot | Out-Null
    Copy-Item -Recurse $BundleRoot $target
    return $target
}

function Assert-DoctorFails([string]$Root, [string]$Label) {
    $result = Invoke-Cli @(
        "doctor", "--json",
        "--bundle-root", $Root,
        "--trusted-manifest-sha256", $TrustedManifestSha256
    )
    if ($result.Exit -eq 0) { throw "doctor unexpectedly passed: $Label" }
    $report = $result.Text | ConvertFrom-Json
    if ($report.status -ne "failed") { throw "doctor status is not failed: $Label" }
    if (-not $report.checks[-1].code) { throw "failed doctor has no code: $Label" }
}

$tampered = New-BundleCopy "tampered"
$ffmpeg = Join-Path $tampered "bin/ffmpeg.exe"
[System.IO.File]::AppendAllText($ffmpeg, "tampered")
Assert-DoctorFails $tampered "tampered tool"

$missing = New-BundleCopy "missing-dll"
$dll = Get-ChildItem (Join-Path $missing "bin") -Filter "*.dll" | Select-Object -First 1
Remove-Item -Force $dll.FullName
Assert-DoctorFails $missing "missing shared library"

$unlisted = New-BundleCopy "unlisted"
[System.IO.File]::WriteAllText((Join-Path $unlisted "bin/extra.dll"), "extra")
Assert-DoctorFails $unlisted "unlisted file"

$doctor = Invoke-Cli @(
    "doctor", "--json",
    "--bundle-root", $BundleRoot,
    "--trusted-manifest-sha256", ("sha256:" + ("0" * 64))
)
if ($doctor.Exit -eq 0) { throw "doctor accepted a wrong trust anchor" }
$report = $doctor.Text | ConvertFrom-Json
if ($report.checks[-1].code -ne "MANIFEST_DIGEST_MISMATCH") { throw "wrong trust anchor did not fail the digest check" }

Write-Output "bundle smoke passed: version and doctor are healthy and fail closed on tampering"
exit 0
