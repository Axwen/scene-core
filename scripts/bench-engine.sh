#!/usr/bin/env bash
set -euo pipefail

for variable in SCENE_CORE_FFMPEG_DIR SCENE_CORE_TOOLCHAIN_FINGERPRINT; do
    if [[ -z "${!variable:-}" ]]; then
        printf '%s must be set\n' "${variable}" >&2
        exit 1
    fi
done

cargo build --release -q -p scene-core-engine
staging="$(mktemp -d)"
trap 'rm -rf "${staging}"' EXIT
mkdir -p "${staging}/input"
"${SCENE_CORE_FFMPEG_DIR}/ffmpeg" -hide_banner -loglevel error -f lavfi \
    -i testsrc=duration=1:size=640x480:rate=30 -c:v mjpeg -f matroska -y \
    "${staging}/input/source.media"

request() {
    python3 - "$1" "${staging}/input/source.media" <<'PY'
import hashlib, json, os, sys;

def canonical(value):
    return json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=False)

def digest(text):
    return "sha256:" + hashlib.sha256(text.encode()).hexdigest()

operation = sys.argv[1]
media = sys.argv[2]
size = os.path.getsize(media)
content = "sha256:" + hashlib.sha256(open(media, "rb").read()).hexdigest()
fingerprint = digest(canonical({
    "inputSetVersion": "1",
    "inputs": [{"role": "source_media", "contentHash": content, "byteSize": size}],
}))
contract = "probe-result/1" if operation == "probe" else "extract-preview-result/1"
key = digest(canonical({
    "derivationDescriptorVersion": "1",
    "engineCacheCompatibilityId": "scene-core-output-v1",
    "inputFingerprint": fingerprint,
    "operation": operation,
    "operationConfigHash": digest("{}"),
    "outputContractVersion": contract,
    "toolchainFingerprint": os.environ["SCENE_CORE_TOOLCHAIN_FINGERPRINT"],
}))
print(json.dumps({
    "engineProtocolVersion": "0.1",
    "messageType": "start",
    "requestId": "bench_01",
    "operation": operation,
    "sourceVersionId": "sourcev_bench",
    "inputs": [{"role": "source_media", "ref": "input/source.media", "contentHash": content, "byteSize": size}],
    "inputFingerprint": fingerprint,
    "operationConfigHash": digest("{}"),
    "outputContractVersion": contract,
    "derivationKey": key,
    "executionContext": {"runId": "run_bench", "generation": 0, "attempt": 1, "scope": "asset"},
    "deadlineMs": 60000,
    "options": {},
}))
PY
}

for operation in probe extract_preview; do
    printf '== %s\n' "${operation}"
    request "${operation}" > "${staging}/request.json"
    set +e
    /usr/bin/time -v target/release/scene-core run --staging-root "${staging}" \
        < "${staging}/request.json" > "${staging}/events.jsonl" 2> "${staging}/time.txt"
    status=$?
    set -e
    printf 'exit=%s events=%s\n' "${status}" "$(wc -l < "${staging}/events.jsonl")"
    grep -E "Maximum resident set size|Elapsed \(wall clock\)" "${staging}/time.txt" || true
done
