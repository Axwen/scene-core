#!/usr/bin/env bash
set -euo pipefail

if [[ -z "${SCENE_CORE_FFMPEG_DIR:-}" ]]; then
    printf 'SCENE_CORE_FFMPEG_DIR must point at the extracted toolchain bin directory\n' >&2
    exit 1
fi

if command -v /usr/bin/time >/dev/null 2>&1; then
    /usr/bin/time -v cargo test --release -p scene-core-media --test baselines -- --ignored --nocapture
else
    cargo test --release -p scene-core-media --test baselines -- --ignored --nocapture
fi
