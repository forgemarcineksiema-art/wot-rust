#!/usr/bin/env bash
# Build and stage the dedicated server for a VPS (see docs/ops/dedicated-server.md).
# Usage: scripts/deploy-server.sh user@host [/opt/wot]
set -euo pipefail
TARGET="${1:?usage: deploy-server.sh user@host [/opt/wot]}"
DEST="${2:-/opt/wot}"

cargo build --release -p server
scp target/release/server "$TARGET:$DEST/server.new"
ssh "$TARGET" "mv '$DEST/server.new' '$DEST/server' && systemctl restart wot-server || true"
echo "deployed to $TARGET:$DEST (unit restart attempted; see docs/ops/dedicated-server.md)"
