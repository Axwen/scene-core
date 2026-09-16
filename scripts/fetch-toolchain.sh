#!/usr/bin/env bash
set -euo pipefail

target="${1:?usage: fetch-toolchain.sh <target> <destination>}"
destination="${2:?usage: fetch-toolchain.sh <target> <destination>}"
lock="packaging/toolchains/${target}/toolchain.lock.json"
cache="${SCENE_CORE_TOOLCHAIN_CACHE:-${HOME}/.cache/scene-core/toolchains}"

fail() {
    printf 'fetch-toolchain failed: %s\n' "$1" >&2
    exit 1
}

for command in curl python3 sha256sum; do
    command -v "${command}" >/dev/null || fail "missing command ${command}"
done

read -r archive_url archive_size archive_sha layout_root < <(
    python3 - "${lock}" <<'PY'
import json, sys
entry = json.load(open(sys.argv[1], encoding="utf-8"))["archive"]
print(entry["url"], entry["byteSize"], entry["sha256"], entry["layoutRoot"])
PY
)

mkdir -p "${cache}"
file="${cache}/$(basename "${archive_url}")"
if [[ ! -f "${file}" ]]; then
    printf 'downloading %s\n' "${archive_url}" >&2
    curl -fsSL -o "${file}.part" "${archive_url}" || fail "cannot download archive"
    mv "${file}.part" "${file}"
fi
[[ "$(stat -c%s "${file}")" == "${archive_size}" ]] || fail "archive byte size mismatch"
[[ "sha256:$(sha256sum "${file}" | cut -d' ' -f1)" == "${archive_sha}" ]] ||
    fail "archive sha256 mismatch"

if [[ -d "${destination}" ]]; then
    rm -rf "${destination}"
fi
mkdir -p "${destination}"

case "${archive_url}" in
    *.tar.xz)
        command -v tar >/dev/null || fail "tar is required for ${archive_url}"
        tar -xJf "${file}" -C "${destination}"
        ;;
    *.zip)
        command -v unzip >/dev/null || fail "unzip is required for ${archive_url}"
        unzip -q -o "${file}" -d "${destination}"
        ;;
    *)
        fail "unsupported archive type"
        ;;
esac

root="${destination}/${layout_root}"
[[ -d "${root}" ]] || fail "layout root ${layout_root} not found"
[[ -x "${root}/bin/ffmpeg" || -f "${root}/bin/ffmpeg.exe" ]] || fail "ffmpeg is missing"
[[ -x "${root}/bin/ffprobe" || -f "${root}/bin/ffprobe.exe" ]] || fail "ffprobe is missing"

printf '%s\n' "${root}/bin"
