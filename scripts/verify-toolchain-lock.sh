#!/usr/bin/env bash
set -euo pipefail

target="${1:-x86_64-pc-windows-msvc}"
lock="packaging/toolchains/${target}/toolchain.lock.json"
cache="${SCENE_CORE_TOOLCHAIN_CACHE:-${HOME}/.cache/scene-core/toolchains}"
mkdir -p "${cache}"

fail() {
    printf 'toolchain lock verification failed: %s\n' "$1" >&2
    exit 1
}

for command in curl python3 sha256sum; do
    command -v "${command}" >/dev/null || fail "missing command ${command}"
done

verify() {
    local label="$1" url="$2" size="$3" sha="$4"
    local file="${cache}/$(basename "${url}")"
    if [[ ! -f "${file}" ]]; then
        printf 'downloading %s\n' "${label}" >&2
        curl -fsSL -o "${file}.part" "${url}" || fail "cannot download ${label}"
        mv "${file}.part" "${file}"
    fi
    [[ "$(stat -c%s "${file}")" == "${size}" ]] || fail "${label} byte size mismatch"
    [[ "sha256:$(sha256sum "${file}" | cut -d' ' -f1)" == "${sha}" ]] ||
        fail "${label} sha256 mismatch"
    printf 'verified %s: %s\n' "${label}" "${sha}"
}

read -r archive_url archive_size archive_sha < <(
    python3 - "${lock}" <<'PY'
import json, sys
entry = json.load(open(sys.argv[1], encoding="utf-8"))["archive"]
print(entry["url"], entry["byteSize"], entry["sha256"])
PY
)
read -r source_url source_size source_sha < <(
    python3 - "${lock}" <<'PY'
import json, sys
entry = json.load(open(sys.argv[1], encoding="utf-8"))["source"]
print(entry["archiveUrl"], entry["archiveByteSize"], entry["archiveSha256"])
PY
)

verify "toolchain archive" "${archive_url}" "${archive_size}" "${archive_sha}"
verify "source archive" "${source_url}" "${source_size}" "${source_sha}"

if command -v unzip >/dev/null; then
    archive_file="${cache}/$(basename "${archive_url}")"
    read -r layout_root < <(
        python3 - "${lock}" <<'PY'
import json, sys
print(json.load(open(sys.argv[1], encoding="utf-8"))["archive"]["layoutRoot"])
PY
    )
    actual="$(unzip -Z1 "${archive_file}" | grep -E "^${layout_root}/bin/.*\.dll$" |
        sed "s|^${layout_root}/||" | sort)"
    expected="$(python3 - "${lock}" <<'PY'
import json, sys
libraries = json.load(open(sys.argv[1], encoding="utf-8"))["archive"]["sharedLibraries"]
print("\n".join(sorted(libraries)))
PY
)"
    [[ "${actual}" == "${expected}" ]] || fail "shared library closure mismatch"
    printf 'verified shared library closure: %s files\n' "$(printf '%s\n' "${expected}" | wc -l)"
fi

printf 'toolchain lock verified for %s\n' "${target}"
