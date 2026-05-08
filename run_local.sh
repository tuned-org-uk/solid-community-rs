#!/usr/bin/env bash
# ---------------------------------------------------------------------------
# run_local.sh
#
# Run solid-contrib/specification-tests against a Solid server already
# listening on this machine, using Podman to execute the official
# solidproject/conformance-test-harness container image.
#
# Mirrors the approach of:
#   https://github.com/solid-contrib/specification-tests/blob/main/run.sh
# but replaces `docker` with `podman` and skips the CSS Docker setup
# (your server is already running).
#
# Prerequisites: podman, curl, git
#
# Usage:
#   ./run_local.sh [OPTIONS]
#
# Options:
#   -p <port>   Port your server listens on  (default: 3000)
#   -d <dir>    Path to an existing specification-tests checkout
#               (default: auto-cloned into ./specification-tests/)
#   -e <file>   Env file for CTH  (default: ./solid-community-rs.env)
#   -t <file>   Tolerable-failures file
#   -h          Show this help
#
# Quick start (server already running on port 3000):
#   chmod +x run_local.sh && ./run_local.sh
# ---------------------------------------------------------------------------
set -euo pipefail

# ── defaults ───────────────────────────────────────────────────────────────
PORT="3000"
SPEC_DIR=""
ENV_FILE=""
TOLERABLE_FILE=""

CTH_IMAGE="solidproject/conformance-test-harness"
CWD="$(pwd)"

log() { echo "==> $*"; }

while getopts ":p:d:e:t:h" opt; do
  case $opt in
    p) PORT="$OPTARG" ;;
    d) SPEC_DIR="$(cd "$OPTARG" && pwd)" ;;
    e) ENV_FILE="$(cd "$(dirname "$OPTARG")" && pwd)/$(basename "$OPTARG")" ;;
    t) TOLERABLE_FILE="$(cd "$(dirname "$OPTARG")" && pwd)/$(basename "$OPTARG")" ;;
    h) sed -n '/^# Usage/,/^# Quick/p' "$0" | sed 's/^# \?//'; exit 0 ;;
    :) echo "ERROR: -$OPTARG requires an argument"; exit 1 ;;
   \?) echo "ERROR: unknown option -$OPTARG"; exit 1 ;;
  esac
done
shift $((OPTIND - 1))

# The URL the *container* uses to reach the host server.
# host.containers.internal is the host gateway in Podman 4+ (rootless).
HOST_URL="http://host.containers.internal:${PORT}/"
# The URL used from *this machine* for the readiness probe.
PROBE_URL="http://localhost:${PORT}/"

# Resolve env file
if [[ -z "$ENV_FILE" ]]; then
  ENV_FILE="$CWD/solid-community-rs.env"
fi

# ── preflight ─────────────────────────────────────────────────────────────
for cmd in podman curl git; do
  command -v "$cmd" &>/dev/null || { echo "ERROR: '$cmd' not found on PATH"; exit 1; }
done
[[ -f "$ENV_FILE" ]] || { echo "ERROR: env file not found: $ENV_FILE"; exit 1; }

# ── server readiness probe ────────────────────────────────────────────────
log "Probing server at $PROBE_URL ..."
for i in $(seq 1 15); do
  curl --silent --fail --output /dev/null "$PROBE_URL" && break
  [[ $i -eq 15 ]] && { echo "ERROR: server not reachable at $PROBE_URL after 15 s"; exit 1; }
  sleep 1
done
log "Server is up."

# ── specification-tests checkout ──────────────────────────────────────────────
if [[ -z "$SPEC_DIR" ]]; then
  SPEC_DIR="$CWD/specification-tests"
  if [[ -d "$SPEC_DIR/.git" ]]; then
    log "Updating specification-tests ..."
    git -C "$SPEC_DIR" pull --quiet
  else
    log "Cloning specification-tests ..."
    git clone --depth=1 --quiet \
      https://github.com/solid-contrib/specification-tests.git \
      "$SPEC_DIR"
  fi
fi

# ── config: application.yaml + test-subjects.ttl ────────────────────────────
CONFIG_DIR="$CWD/config"
mkdir -p "$CONFIG_DIR"

# application.yaml: CTH reads this from /app/config inside the container.
# The mappings section rewrites GitHub blob URLs to the /data volume.
cat > "$CONFIG_DIR/application.yaml" <<YAML
subjects: /app/config/test-subjects.ttl
sources:
  - https://solidproject.org/TR/protocol
  - https://github.com/solid-contrib/specification-tests/blob/main/protocol/solid-protocol-test-manifest.ttl
  - https://github.com/solid-contrib/specification-tests/blob/main/protocol/requirement-comments.ttl
mappings:
  - prefix: https://github.com/solid-contrib/specification-tests/blob/main
    path: /data
YAML

# test-subjects.ttl: declares solid-data-governance-rs as the test subject.
# WAC, ACP and authentication suites are skipped (not implemented yet).
cat > "$CONFIG_DIR/test-subjects.ttl" <<TTL
@prefix solid-test: <https://github.com/solid/conformance-test-harness/vocab#> .
@prefix earl: <http://www.w3.org/ns/earl#> .
@prefix doap: <http://usefulinc.com/ns/doap#> .
@prefix rdfs: <http://www.w3.org/2000/01/rdf-schema#> .

<solid-data-governance-rs>
    a earl:Software, earl:TestSubject ;
    doap:name "solid-data-governance-rs" ;
    doap:description "A Rust implementation of the Solid protocol." ;
    doap:programming-language "Rust" ;
    doap:homepage <https://github.com/Genefold/solid-data-governance-rs> ;
    solid-test:skip "acp", "wac", "authentication" ;
    solid-test:serverRoot <${HOST_URL}> .
TTL

# ── output dirs ────────────────────────────────────────────────────────────────
REPORTS_DIR="$CWD/reports"
mkdir -p "$REPORTS_DIR" "$CWD/target"

# ── pull the CTH image ────────────────────────────────────────────────────────
log "Pulling $CTH_IMAGE ..."
podman pull "$CTH_IMAGE"

# ── run ───────────────────────────────────────────────────────────────────────
PODMAN_ARGS=(
  --rm
  --interactive
  --user "$(id -u):$(id -g)"
  # Env file: supplies users.alice.webid, users.bob.webid etc.
  --env-file="$ENV_FILE"
  # specification-tests checkout → /data  (matches mappings in application.yaml)
  --volume "${SPEC_DIR}:/data:ro,z"
  # Config files → /app/config
  --volume "${CONFIG_DIR}:/app/config:ro,z"
  # Reports output
  --volume "${REPORTS_DIR}:/reports:z"
  # Karate artefacts
  --volume "${CWD}/target:/app/target:z"
  # host.containers.internal → host gateway (Podman 4+ rootless, macOS & Linux)
  --add-host="host.containers.internal:host-gateway"
)

HARNESS_ARGS=(
  --output=/reports
  --target="${HOST_URL}"
  --skip-teardown
)

if [[ -n "$TOLERABLE_FILE" ]]; then
  cp "$TOLERABLE_FILE" "$CWD/target/tolerable-failures.txt"
  HARNESS_ARGS+=("--tolerable-failures=/app/target/tolerable-failures.txt")
fi

log "Launching CTH container ..."
log "  image:      $CTH_IMAGE"
log "  server:     $HOST_URL  (probe: $PROBE_URL)"
log "  env file:   $ENV_FILE"
log "  tests:      $SPEC_DIR"
log "  reports:    $REPORTS_DIR"
echo ""
echo "podman run ${PODMAN_ARGS[*]} $CTH_IMAGE ${HARNESS_ARGS[*]}"
echo ""

set +e
podman run "${PODMAN_ARGS[@]}" "$CTH_IMAGE" "${HARNESS_ARGS[@]}"
exit_code=$?
set -e

echo ""
[[ $exit_code -eq 0 ]] \
  && log "All tests passed." \
  || log "Tests finished with failures (exit $exit_code) — see: $REPORTS_DIR"

exit $exit_code
# ---------------------------------------------------------------------------
# TROUBLESHOOTING
#
# "connection refused" from inside the container:
#   Your server may only bind to 127.0.0.1.  Try adding --network=host to
#   PODMAN_ARGS and changing PORT to use http://localhost:${PORT}/.
#
# SELinux denials on volume mounts:
#   The :z flag re-labels volumes for shared access.  If you see permission
#   errors try replacing :z with :Z (private label) or run:
#     chcon -Rt svirt_sandbox_file_t <directory>
#
# Platform warning (linux/amd64 on arm64):
#   Expected on Apple Silicon — Rosetta/QEMU emulation handles this.
#   Add --platform=linux/arm64 to PODMAN_ARGS if a native arm64 image
#   becomes available.
# ---------------------------------------------------------------------------
