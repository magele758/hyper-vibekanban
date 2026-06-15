#!/usr/bin/env bash
# Pre-pull Docker base images via registry mirrors, then retag for compose builds.
set -euo pipefail

PLATFORM="${DOCKER_PLATFORM:-linux/arm64}"

# Override with DOCKER_MIRRORS="mirror1 mirror2" (space-separated, no /library prefix).
DEFAULT_MIRRORS=(
  docker.1ms.run
  docker.m.daocloud.io
  dockerproxy.com
  hub.rat.dev
  docker.rainbond.cc
)

if [[ -n "${DOCKER_MIRRORS:-}" ]]; then
  read -r -a MIRRORS <<< "${DOCKER_MIRRORS}"
else
  MIRRORS=("${DEFAULT_MIRRORS[@]}")
fi

pull_library_image() {
  local official="$1"
  local name="${official%%:*}"
  local tag="${official#*:}"
  if [[ "$name" == "$tag" ]]; then
    tag="latest"
  fi

  for mirror in "${MIRRORS[@]}"; do
    local ref="${mirror}/library/${name}:${tag}"
    echo "==> Trying ${ref} (${PLATFORM})"
    if docker pull --platform "${PLATFORM}" "${ref}"; then
      docker tag "${ref}" "${official}"
      echo "    tagged as ${official} (via ${mirror})"
      return 0
    fi
    echo "    failed on ${mirror}, trying next mirror..."
  done

  echo "ERROR: could not pull ${official} from any mirror" >&2
  return 1
}

echo "Mirrors: ${MIRRORS[*]}"
echo "Platform: ${PLATFORM}"

pull_library_image "rust:1.93-slim-bookworm"
pull_library_image "node:22-bookworm-slim"
pull_library_image "debian:bookworm-slim"
pull_library_image "postgres:16-alpine"

echo "==> Pulling electricsql/electric:1.4.13"
docker pull --platform "${PLATFORM}" "electricsql/electric:1.4.13"

echo "==> Base images ready:"
docker images --format '  {{.Repository}}:{{.Tag}}  {{.Size}}' \
  | grep -E '(^|/)rust:1.93|^node:20|^debian:bookworm|^postgres:16|^electricsql/electric' || true
