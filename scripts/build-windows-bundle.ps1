param(
    [string]$Version = "0.1.0-alpha.1",
    [string]$Target = "x86_64-pc-windows-msvc",
    [string]$RepoRoot = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path,
    [string]$OutputDir = "",
    [string]$CacheDir = (Join-Path $env:USERPROFILE ".cache/scene-core/toolchains"),
    [string]$EngineBinary = "",
    [string]$EngineCommit = $env:GITHUB_SHA
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

if (-not $OutputDir) { $OutputDir = Join-Path $RepoRoot "packaging/bundles/windows-x86_64" }
if (-not $EngineBinary) { $EngineBinary = Join-Path $RepoRoot "target/release/scene-core.exe" }
if (-not $EngineCommit) { $EngineCommit = "0000000000000000000000000000000000000000" }

$lockPath = Join-Path $RepoRoot "packaging/toolchains/$Target/toolchain.lock.json"
$lockBytes = [System.IO.File]::ReadAllBytes($lockPath)
$lock = [System.Text.Json.JsonSerializer]::Deserialize(
    [System.Text.Encoding]::UTF8.GetString($lockBytes),
    [System.Text.Json.JsonElement])

function Get-Sha256([byte[]]$Bytes) {
    $sha = [System.Security.Cryptography.SHA256]::Create()
    try { return "sha256:" + [System.BitConverter]::ToString($sha.ComputeHash($Bytes)).Replace("-", "").ToLowerInvariant() }
    finally { $sha.Dispose() }
}

function Get-FileSha256([string]$Path) {
    return Get-Sha256 ([System.IO.File]::ReadAllBytes($Path))
}

function Write-JsonFile([string]$Path, $Value) {
    $json = $Value | ConvertTo-Json -Depth 8
    $encoding = New-Object System.Text.UTF8Encoding($false)
    [System.IO.File]::WriteAllText($Path, $json, $encoding)
}

$archive = $lock.GetProperty("archive")
$source = $lock.GetProperty("source")
$build = $lock.GetProperty("build")
$url = $archive.GetProperty("url").GetString()
$expectedSize = $archive.GetProperty("byteSize").GetInt64()
$expectedSha = $archive.GetProperty("sha256").GetString()
$layoutRoot = $archive.GetProperty("layoutRoot").GetString()

New-Item -ItemType Directory -Force -Path $CacheDir | Out-Null
$archivePath = Join-Path $CacheDir ([System.IO.Path]::GetFileName($url))
if (-not (Test-Path $archivePath)) {
    Write-Host "downloading toolchain archive"
    Invoke-WebRequest -Uri $url -OutFile "$archivePath.part"
    Move-Item "$archivePath.part" $archivePath
}
if ((Get-Item $archivePath).Length -ne $expectedSize) { throw "toolchain archive size mismatch" }
if ((Get-FileSha256 $archivePath) -ne $expectedSha) { throw "toolchain archive sha256 mismatch" }

$extractDir = Join-Path $env:TEMP ("scene-core-toolchain-" + [System.Guid]::NewGuid().ToString("N"))
Expand-Archive -Path $archivePath -DestinationPath $extractDir
$toolchainRoot = Join-Path $extractDir $layoutRoot

$bundle = Join-Path $OutputDir "bundle"
if (Test-Path $bundle) { Remove-Item -Recurse -Force $bundle }
New-Item -ItemType Directory -Force -Path (Join-Path $bundle "bin") | Out-Null
New-Item -ItemType Directory -Force -Path (Join-Path $bundle "THIRD_PARTY_LICENSES") | Out-Null
New-Item -ItemType Directory -Force -Path (Join-Path $bundle "schemas/0.1") | Out-Null

Copy-Item $EngineBinary (Join-Path $bundle "bin/scene-core.exe")
$toolBin = $archive.GetProperty("binaries")
Copy-Item (Join-Path $toolchainRoot $toolBin.GetProperty("ffmpeg").GetString()) (Join-Path $bundle "bin/ffmpeg.exe")
Copy-Item (Join-Path $toolchainRoot $toolBin.GetProperty("ffprobe").GetString()) (Join-Path $bundle "bin/ffprobe.exe")
foreach ($library in $archive.GetProperty("sharedLibraries").EnumerateArray()) {
    $relative = $library.GetString()
    Copy-Item (Join-Path $toolchainRoot $relative) (Join-Path $bundle $relative)
}
Copy-Item (Join-Path $toolchainRoot $lock.GetProperty("license").GetProperty("licenseFile").GetString()) `
    (Join-Path $bundle "THIRD_PARTY_LICENSES/COPYING.LGPLv3")
Copy-Item (Join-Path $RepoRoot "schemas/0.1/*.json") (Join-Path $bundle "schemas/0.1")

function Invoke-Tool([string]$Program, [string[]]$Arguments) {
    $output = & $Program @Arguments 2>&1
    if ($LASTEXITCODE -ne 0) { throw "tool failed: $Program" }
    return $output
}

function Get-CapabilityList([string[]]$Lines) {
    $names = New-Object System.Collections.Generic.HashSet[string]
    foreach ($line in $Lines) {
        if ($line -match "^\s*[A-Z.]{1,6}\s+([A-Za-z0-9_.,-]+)") {
            [void]$names.Add($matches[1].ToLowerInvariant())
        }
    }
    $sorted = [string[]]$names
    [Array]::Sort($sorted, [System.StringComparer]::Ordinal)
    return $sorted
}

$ffmpeg = Join-Path $bundle "bin/ffmpeg.exe"
$ffprobe = Join-Path $bundle "bin/ffprobe.exe"

$protocolLines = Invoke-Tool $ffmpeg @("-hide_banner", "-protocols")
$protocolNames = New-Object System.Collections.Generic.HashSet[string]
foreach ($line in $protocolLines) {
    if ($line -match "^\s*([A-Za-z0-9_.,-]+)\s*$") { [void]$protocolNames.Add($matches[1].ToLowerInvariant()) }
}
$protocols = [string[]]$protocolNames
[Array]::Sort($protocols, [System.StringComparer]::Ordinal)

$capabilities = [ordered]@{
    capabilitiesVersion = "1"
    target = $Target
    demuxers = Get-CapabilityList (Invoke-Tool $ffmpeg @("-hide_banner", "-demuxers"))
    decoders = Get-CapabilityList (Invoke-Tool $ffmpeg @("-hide_banner", "-decoders"))
    encoders = Get-CapabilityList (Invoke-Tool $ffmpeg @("-hide_banner", "-encoders"))
    filters = Get-CapabilityList (Invoke-Tool $ffmpeg @("-hide_banner", "-filters"))
    protocols = $protocols
}
$capabilitiesPath = Join-Path $bundle "capabilities.json"
Write-JsonFile $capabilitiesPath $capabilities
$capabilitySetFingerprint = Get-FileSha256 $capabilitiesPath

$baselinePath = Join-Path $RepoRoot "packaging/toolchains/$Target/capabilities.json"
if (Test-Path $baselinePath) {
    $generatedBytes = [System.IO.File]::ReadAllBytes($capabilitiesPath)
    $baselineBytes = [System.IO.File]::ReadAllBytes($baselinePath)
    $same = [System.Linq.Enumerable]::SequenceEqual([byte[]]$generatedBytes, [byte[]]$baselineBytes)
    if (-not $same) {
        throw "capabilities.json drifted from the committed baseline; review the capability diff and update packaging/toolchains/$Target/capabilities.json in an explicit upgrade PR"
    }
}

$sharedLibraries = New-Object System.Collections.Generic.List[object]
foreach ($library in $archive.GetProperty("sharedLibraries").EnumerateArray()) {
    $relative = $library.GetString()
    $sharedLibraries.Add([ordered]@{
        path = $relative
        sha256 = Get-FileSha256 (Join-Path $bundle $relative)
    })
}

$configureFlags = New-Object System.Collections.Generic.List[string]
foreach ($flag in $build.GetProperty("configureFlags").EnumerateArray()) { $configureFlags.Add($flag.GetString()) }

$descriptor = [ordered]@{
    descriptorVersion = "1"
    target = $Target
    ffmpegVersion = $lock.GetProperty("ffmpegVersion").GetString()
    ffprobeVersion = $lock.GetProperty("ffprobeVersion").GetString()
    toolchainLockSha256 = Get-Sha256 $lockBytes
    configureFlags = $configureFlags
    capabilitySetFingerprint = $capabilitySetFingerprint
    sharedLibraries = $sharedLibraries
}
$descriptorPath = Join-Path $bundle "toolchain-descriptor.json"
Write-JsonFile $descriptorPath $descriptor
$toolchainFingerprint = Get-FileSha256 $descriptorPath

$licenseProfile = $lock.GetProperty("license").GetProperty("profile").GetString()
$toolVersion = $lock.GetProperty("ffmpegVersion").GetString()
$spdxPackages = New-Object System.Collections.Generic.List[object]
$spdxPackages.Add([ordered]@{
    SPDXID = "SPDXRef-Package-scene-core"
    name = "scene-core"
    versionInfo = $Version
    downloadLocation = "NOASSERTION"
    licenseConcluded = "NOASSERTION"
    filesAnalyzed = $false
})
foreach ($component in @(
        [ordered]@{ id = "ffmpeg"; name = "ffmpeg"; version = $lock.GetProperty("ffmpegVersion").GetString(); location = $archive.GetProperty("url").GetString() },
        [ordered]@{ id = "ffprobe"; name = "ffprobe"; version = $lock.GetProperty("ffprobeVersion").GetString(); location = $archive.GetProperty("url").GetString() }
    )) {
    $spdxPackages.Add([ordered]@{
        SPDXID = "SPDXRef-Package-$($component["id"])"
        name = $component["name"]
        versionInfo = $component["version"]
        downloadLocation = $component["location"]
        licenseConcluded = $licenseProfile
        filesAnalyzed = $false
    })
}
foreach ($library in $archive.GetProperty("sharedLibraries").EnumerateArray()) {
    $file = [System.IO.Path]::GetFileName($library.GetString())
    $componentId = [System.IO.Path]::GetFileNameWithoutExtension($file)
    $spdxPackages.Add([ordered]@{
        SPDXID = "SPDXRef-Package-$componentId"
        name = $componentId
        versionInfo = $toolVersion
        downloadLocation = $archive.GetProperty("url").GetString()
        licenseConcluded = $licenseProfile
        filesAnalyzed = $false
    })
}
$spdx = [ordered]@{
    spdxVersion = "SPDX-2.3"
    dataLicense = "CC0-1.0"
    SPDXID = "SPDXRef-DOCUMENT"
    name = "scene-core-$Version-$Target"
    documentNamespace = "https://github.com/Axwen/scene-core/spdx/$Version/$Target"
    creationInfo = [ordered]@{
        created = $build.GetProperty("publishedAt").GetString()
        creators = @("Tool: scene-core build-windows-bundle.ps1")
    }
    packages = $spdxPackages.ToArray()
    relationships = @(
        foreach ($package in $spdxPackages) {
            [ordered]@{
                spdxElementId = "SPDXRef-DOCUMENT"
                relationshipType = "DESCRIBES"
                relatedSpdxElement = $package["SPDXID"]
            }
        }
    )
}
Write-JsonFile (Join-Path $bundle "SBOM.spdx.json") $spdx

$files = New-Object System.Collections.Generic.List[object]
$prefixLength = $bundle.Length + 1
Get-ChildItem -Recurse -File -Path $bundle | ForEach-Object {
    $relative = $_.FullName.Substring($prefixLength).Replace("\", "/")
    if ($relative -ne "package-manifest.json") {
        $files.Add([ordered]@{
            path = $relative
            byteSize = $_.Length
            sha256 = Get-FileSha256 $_.FullName
        })
    }
}
$fileArray = [object[]]$files.ToArray()
$fileArray = [object[]][System.Linq.Enumerable]::OrderBy(
    [object[]]$fileArray,
    [System.Func[object, string]] { param($file) return $file["path"] },
    [System.StringComparer]::Ordinal)

$tools = @(
    [ordered]@{
        name = "ffmpeg"
        path = "bin/ffmpeg.exe"
        version = $lock.GetProperty("ffmpegVersion").GetString()
        sourceUrl = $archive.GetProperty("url").GetString()
        sourceSha256 = $expectedSha
        licenseProfile = $lock.GetProperty("license").GetProperty("profile").GetString()
    },
    [ordered]@{
        name = "ffprobe"
        path = "bin/ffprobe.exe"
        version = $lock.GetProperty("ffprobeVersion").GetString()
        sourceUrl = $archive.GetProperty("url").GetString()
        sourceSha256 = $expectedSha
        licenseProfile = $lock.GetProperty("license").GetProperty("profile").GetString()
    }
)

$manifest = [ordered]@{
    packageManifestVersion = "1"
    name = "scene-core"
    packageVersion = $Version
    target = $Target
    distributionProfile = "lgpl"
    toolchainFingerprint = $toolchainFingerprint
    toolchainDescriptorRef = "toolchain-descriptor.json"
    capabilitiesRef = "capabilities.json"
    engine = [ordered]@{
        path = "bin/scene-core.exe"
        version = $Version
        commit = $EngineCommit
        engineCacheCompatibilityId = "scene-core-output-v1"
        supportedProtocolVersions = @("0.1")
        implementedOperations = @()
    }
    tools = $tools
    files = $fileArray
    sbomRef = "SBOM.spdx.json"
    thirdPartyLicensesRef = "THIRD_PARTY_LICENSES"
}
$manifestPath = Join-Path $bundle "package-manifest.json"
Write-JsonFile $manifestPath $manifest
$manifestSha = Get-FileSha256 $manifestPath
[System.IO.File]::WriteAllText(
    (Join-Path $OutputDir "package-manifest.json.sha256"),
    "$manifestSha  package-manifest.json`n",
    (New-Object System.Text.UTF8Encoding($false)))

function New-DeterministicZip([string]$SourceDir, [string]$Destination) {
    if (Test-Path $Destination) { Remove-Item -Force $Destination }
    $relative = New-Object System.Collections.Generic.List[string]
    Get-ChildItem -Recurse -File -Path $SourceDir | ForEach-Object {
        $relative.Add($_.FullName.Substring($SourceDir.Length + 1).Replace("\", "/"))
    }
    $ordered = [string[]]$relative.ToArray()
    [Array]::Sort($ordered, [System.StringComparer]::Ordinal)
    $timestamp = [System.DateTimeOffset]::Parse("2026-01-01T00:00:00Z")
    $stream = [System.IO.File]::Open($Destination, [System.IO.FileMode]::CreateNew)
    try {
        $archive = New-Object System.IO.Compression.ZipArchive($stream, [System.IO.Compression.ZipArchiveMode]::Create, $true)
        try {
            foreach ($entryName in $ordered) {
                $entry = $archive.CreateEntry($entryName, [System.IO.Compression.CompressionLevel]::Optimal)
                $entry.LastWriteTime = $timestamp
                $entryStream = $entry.Open()
                try {
                    $source = [System.IO.File]::OpenRead((Join-Path $SourceDir $entryName))
                    try { $source.CopyTo($entryStream) } finally { $source.Dispose() }
                }
                finally { $entryStream.Dispose() }
            }
        }
        finally { $archive.Dispose() }
    }
    finally { $stream.Dispose() }
}

$zipName = "scene-core-$Version-windows-x86_64.zip"
$zipPath = Join-Path $OutputDir $zipName
New-DeterministicZip $bundle $zipPath
$zipSha = Get-FileSha256 $zipPath
$verifyPath = Join-Path $env:TEMP ("scene-core-zip-verify-" + [System.Guid]::NewGuid().ToString("N") + ".zip")
New-DeterministicZip $bundle $verifyPath
if ((Get-FileSha256 $verifyPath) -ne $zipSha) { throw "ZIP output is not reproducible" }
Remove-Item -Force $verifyPath
[System.IO.File]::WriteAllText(
    (Join-Path $OutputDir "$zipName.sha256"),
    "$zipSha  $zipName`n",
    (New-Object System.Text.UTF8Encoding($false)))

Remove-Item -Recurse -Force $extractDir

$result = [ordered]@{
    bundle = $bundle
    zip = $zipPath
    manifestSha256 = $manifestSha
    zipSha256 = $zipSha
    toolchainFingerprint = $toolchainFingerprint
    capabilitySetFingerprint = $capabilitySetFingerprint
}
Write-Output ($result | ConvertTo-Json -Compress)
