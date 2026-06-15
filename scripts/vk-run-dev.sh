#!/usr/bin/env bash
# Background wrapper — env must be exported by vk-start before invocation.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "${ROOT}"
bash "${ROOT}/scripts/dev-full-local.sh"
