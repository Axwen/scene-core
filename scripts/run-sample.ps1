param(
    [Parameter(Mandatory = $true)][string]$BundleRoot,
    [Parameter(Mandatory = $true)][string]$Sample,
    [ValidateSet("probe", "extract_preview")][string]$Operation = "probe"
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$BundleRoot = (Resolve-Path $BundleRoot).Path
$sample = (Resolve-Path $Sample).Path
$engine = Join-Path $BundleRoot "bin/scene-core.exe"
$work = Join-Path $env:TEMP ("scene-core-manual-" + [System.Guid]::NewGuid().ToString("N"))
New-Item -ItemType Directory -Force -Path (Join-Path $work "input") | Out-Null
$staged = Join-Path $work "input/source.media"
Copy-Item $sample $staged

function Get-Sha256Hex([string]$Text) {
    $sha = [System.Security.Cryptography.SHA256]::Create()
    try {
        return [System.BitConverter]::ToString($sha.ComputeHash([System.Text.Encoding]::UTF8.GetBytes($Text))).Replace("-", "").ToLowerInvariant()
    }
    finally { $sha.Dispose() }
}

$size = (Get-Item $staged).Length
$content = "sha256:" + (Get-FileHash -Algorithm SHA256 -Path $staged).Hash.ToLowerInvariant()
$descriptor = '{"inputSetVersion":"1","inputs":[{"byteSize":' + $size + ',"contentHash":"' + $content + '","role":"source_media"}]}'
$inputFingerprint = "sha256:" + (Get-Sha256Hex $descriptor)
$operationConfigHash = "sha256:" + (Get-Sha256Hex "{}")
$contract = if ($Operation -eq "probe") { "probe-result/1" } else { "extract-preview-result/1" }

$version = & $engine version --json | ConvertFrom-Json
$derivation = '{"derivationDescriptorVersion":"1","engineCacheCompatibilityId":"scene-core-output-v1","inputFingerprint":"' +
    $inputFingerprint + '","operation":"' + $Operation + '","operationConfigHash":"' + $operationConfigHash +
    '","outputContractVersion":"' + $contract + '","toolchainFingerprint":"' + $version.toolchainFingerprint + '"}'
$derivationKey = "sha256:" + (Get-Sha256Hex $derivation)

$request = [ordered]@{
    engineProtocolVersion = "0.1"
    messageType = "start"
    requestId = "manual_01"
    operation = $Operation
    sourceVersionId = "sourcev_manual"
    inputs = @([ordered]@{ role = "source_media"; ref = "input/source.media"; contentHash = $content; byteSize = $size })
    inputFingerprint = $inputFingerprint
    operationConfigHash = $operationConfigHash
    outputContractVersion = $contract
    derivationKey = $derivationKey
    executionContext = [ordered]@{ runId = "run_manual"; generation = 0; attempt = 1; scope = "asset" }
    deadlineMs = 120000
    options = @{}
} | ConvertTo-Json -Depth 6 -Compress

Write-Output "== $Operation"
$request | & $engine run --staging-root $work
$exit = $LASTEXITCODE
Write-Output "exit=$exit"
Get-ChildItem -Recurse -File -Path (Join-Path $work "output") -ErrorAction SilentlyContinue |
    ForEach-Object { Write-Output ("artifact {0} {1} bytes" -f $_.FullName.Substring($work.Length + 1), $_.Length) }
Write-Output "staging=$work"
exit $exit
