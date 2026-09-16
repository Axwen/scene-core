#!/usr/bin/env bash
set -euo pipefail

schema_dir="schemas/0.1"

if [[ ! -d "$schema_dir" ]]; then
    printf 'missing schema directory: %s\n' "$schema_dir" >&2
    exit 1
fi

schema_files="$(find "$schema_dir" -type f ! -name .gitkeep -print)"
if [[ -n "$schema_files" ]]; then
    printf '%s\n' \
        "schema artifacts exist, but the generator/check command is not implemented" \
        "SC-P0-03 must replace this preflight with the generated-schema drift check" >&2
    exit 1
fi

printf '%s\n' 'schema drift preflight: no generated schemas yet; SC-P0-03 owns the generator'

