#!/usr/bin/env bash
# Runs nyx-crypto hybrid-handshake tests with an optional name filter (Linux/macOS)
# Usage examples:
#   ./scripts/run-hybrid-tests.sh
#   ./scripts/run-hybrid-tests.sh test_key_pair_generation
#   ./scripts/run-hybrid-tests.sh test_complete_handshake_protocol

set -euo pipefail

FILTER="${1:-}"

echo "Running nyx-crypto tests with 'hybrid-handshake' feature..."

args=(test -p nyx-crypto --features hybrid-handshake)
if [[ -n "$FILTER" ]]; then
  args+=("$FILTER")
fi
args+=(-- --nocapture)

echo "> cargo ${args[*]}"
cargo "${args[@]}"

echo "Done."
