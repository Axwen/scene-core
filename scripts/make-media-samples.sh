#!/usr/bin/env bash
set -euo pipefail

if [[ -z "${SCENE_CORE_FFMPEG_DIR:-}" ]]; then
    printf 'SCENE_CORE_FFMPEG_DIR must point at the extracted toolchain bin directory\n' >&2
    exit 1
fi

ffmpeg="${SCENE_CORE_FFMPEG_DIR}/ffmpeg"
out="${1:-fixtures/media/samples}"
mkdir -p "${out}"
cd "${out}"

"${ffmpeg}" -hide_banner -loglevel error -f lavfi -i testsrc=duration=1:size=64x48:rate=10 \
    -f lavfi -i sine=frequency=440:duration=1 -c:v mpeg4 -c:a aac -f mp4 -y baseline-cfr.mp4
"${ffmpeg}" -hide_banner -loglevel error -f lavfi -i testsrc=duration=1:size=64x48:rate=10 \
    -itsoffset 0.08 -f lavfi -i sine=frequency=440:duration=1 -c:v mjpeg -c:a pcm_s16le \
    -f matroska -y av-offset-80ms.mkv
"${ffmpeg}" -hide_banner -loglevel error -f lavfi -i testsrc=duration=1:size=64x48:rate=10 \
    -vf "select='not(mod(n,3))'" -fps_mode vfr -c:v mpeg4 -f matroska -y vfr.mkv
"${ffmpeg}" -hide_banner -loglevel error -f lavfi -i testsrc=duration=1:size=64x48:rate=10 \
    -c:v mpeg4 -bf 2 -f matroska -y bframes.mkv
"${ffmpeg}" -hide_banner -loglevel error -f lavfi -i testsrc=duration=1:size=64x48:rate=10 \
    -c:v mjpeg -f matroska -y base-rot.mkv
"${ffmpeg}" -hide_banner -loglevel error -display_rotation 90 -i base-rot.mkv -c copy \
    -f matroska -y rotation-90.mkv
rm -f base-rot.mkv
"${ffmpeg}" -hide_banner -loglevel error -f lavfi -i sine=frequency=440:duration=1 \
    -f lavfi -i testsrc=duration=1:size=32x32:rate=1 -map 0:a -map 1:v -c:a flac -c:v mjpeg \
    -disposition:v attached_pic -frames:v 1 -f flac -y attached-picture.flac
"${ffmpeg}" -hide_banner -loglevel error -f lavfi -i testsrc=duration=1:size=64x48:rate=10 \
    -c:v mjpeg -f matroska -y video-only.mkv
"${ffmpeg}" -hide_banner -loglevel error -f lavfi -i sine=frequency=440:duration=1 -c:a aac \
    -f mp4 -y audio-only.m4a
head -c 4096 baseline-cfr.mp4 > corrupt.mp4

printf 'samples written to %s\n' "$(pwd)"
