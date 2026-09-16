#!/usr/bin/env bash
set -euo pipefail

cargo test -p scene-core-protocol --locked --test schema_contract
