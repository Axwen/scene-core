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
if (-not $versionJson.toolchainFingerprint) { throw "toolchainFingerprint is missing" }
$manifestJson = Get-Content (Join-Path $BundleRoot "package-manifest.json") -Raw | ConvertFrom-Json
$operations = @($versionJson.implementedOperations)
foreach ($operation in $operations) {
    if ($operation -notin @("probe", "extract_preview", "extract_audio_pcm")) {
        throw "unknown implemented operation: $operation"
    }
}
if (($operations -join ",") -ne (@($manifestJson.engine.implementedOperations) -join ",")) {
    throw "implementedOperations differs from the package manifest"
}
if ($versionJson.engineVersion -ne $manifestJson.engine.version) { throw "engine version differs from the package manifest" }
if ($versionJson.engineCommit -ne $manifestJson.engine.commit) { throw "engine commit differs from the package manifest" }

$doctor = Invoke-Cli @(
    "doctor", "--json",
    "--bundle-root", $BundleRoot,
    "--trusted-manifest-sha256", $TrustedManifestSha256
)
if ($doctor.Exit -ne 0) { throw "healthy doctor exited $($doctor.Exit): $($doctor.Text)" }
$doctorJson = $doctor.Text | ConvertFrom-Json
if ($doctorJson.status -ne "ok") { throw "healthy doctor status is $($doctorJson.status)" }
if ($doctorJson.checks.Count -lt 10) { throw "doctor is missing checks" }

$descriptor = Get-Content (Join-Path $BundleRoot "toolchain-descriptor.json") -Raw | ConvertFrom-Json
$buildconf = ((& (Join-Path $BundleRoot "bin/ffmpeg.exe") -hide_banner -buildconf 2>&1) -join "`n")
foreach ($flag in $descriptor.configureFlags) {
    if (-not $buildconf.Contains($flag)) { throw "ffmpeg buildconf is missing $flag" }
}
if ($buildconf -match "--enable-gpl" -or $buildconf -match "--enable-nonfree") {
    throw "ffmpeg buildconf contains GPL or nonfree flags"
}

$runRoot = Join-Path $WorkRoot "run"
if (Test-Path $runRoot) { Remove-Item -Recurse -Force $runRoot }
New-Item -ItemType Directory -Force -Path (Join-Path $runRoot "input") | Out-Null
$sample = Join-Path $runRoot "input/source.media"
& (Join-Path $BundleRoot "bin/ffmpeg.exe") -hide_banner -loglevel error -nostdin -f lavfi `
    -i "testsrc=duration=1:size=320x240:rate=10" -c:v mjpeg -f matroska -y $sample 2>$null
if ($LASTEXITCODE -ne 0) { throw "could not generate the in-bundle sample" }

foreach ($operation in @("probe", "extract_preview")) {
    $request = & python (Join-Path $PSScriptRoot "make-run-request.py") $operation $sample $versionJson.toolchainFingerprint
    if ($LASTEXITCODE -ne 0) { throw "could not build the $operation request" }
    $events = $request | & (Join-Path $BundleRoot "bin/scene-core.exe") run --staging-root $runRoot
    if ($LASTEXITCODE -ne 0) { throw "$operation failed inside the bundle" }
    if (-not (($events -join "`n") -match '"eventType":"completed"')) { throw "$operation did not complete" }
}
if (-not (Test-Path (Join-Path $runRoot "output/preview/opening.jpg"))) {
    throw "extract_preview did not produce the opening artifact"
}

$audioRoot = Join-Path $WorkRoot "run-audio"
if (Test-Path $audioRoot) { Remove-Item -Recurse -Force $audioRoot }
New-Item -ItemType Directory -Force -Path (Join-Path $audioRoot "input") | Out-Null
$audioSample = Join-Path $audioRoot "input/source.media"
& (Join-Path $BundleRoot "bin/ffmpeg.exe") -hide_banner -loglevel error -nostdin -f lavfi `
    -i "sine=frequency=440:duration=1:sample_rate=48000" -ac 2 -c:a pcm_s16le -f matroska -y $audioSample 2>$null
if ($LASTEXITCODE -ne 0) { throw "could not generate the in-bundle audio sample" }
$request = & python (Join-Path $PSScriptRoot "make-run-request.py") extract_audio_pcm $audioSample $versionJson.toolchainFingerprint
if ($LASTEXITCODE -ne 0) { throw "could not build the extract_audio_pcm request" }
$events = $request | & $exe run --staging-root $audioRoot
if ($LASTEXITCODE -ne 0) { throw "extract_audio_pcm failed inside the bundle" }
$eventsText = ($events -join "`n")
if ($eventsText -notmatch '"eventType":"completed"') { throw "extract_audio_pcm did not complete" }
if ($eventsText -notmatch '"audioPcm"') { throw "extract_audio_pcm did not report the audio mapping" }
if (-not (Test-Path (Join-Path $audioRoot "output/audio/track.wav"))) {
    throw "extract_audio_pcm did not produce the track artifact"
}
Write-Output "in-bundle run: probe, extract_preview and extract_audio_pcm completed"

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

Write-Output "bundle smoke passed: version, doctor, buildconf flags and tamper handling are healthy"
exit 0
