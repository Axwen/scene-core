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

for operation in probe extract_preview; do
    printf '== %s\n' "${operation}"
    python3 scripts/make-run-request.py "${operation}" "${staging}/input/source.media" \
        "${SCENE_CORE_TOOLCHAIN_FINGERPRINT}" > "${staging}/request.json"
    set +e
    /usr/bin/time -v target/release/scene-core run --staging-root "${staging}" \
        < "${staging}/request.json" > "${staging}/events.jsonl" 2> "${staging}/time.txt"
    status=$?
    set -e
    printf 'exit=%s events=%s\n' "${status}" "$(wc -l < "${staging}/events.jsonl")"
    grep -E "Maximum resident set size|Elapsed \(wall clock\)" "${staging}/time.txt" || true
done
