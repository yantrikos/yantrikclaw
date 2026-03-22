#!/usr/bin/env bash
# Deploy YantrikClaw to Proxmox LXC container 127.
#
# Usage:
#   ./scripts/deploy.sh                  # build release + deploy
#   ./scripts/deploy.sh --skip-build     # deploy existing binary only
#
# Requires: SSH key at ~/.ssh/id_deploy, access to 192.168.4.152

set -euo pipefail

PROXMOX_HOST="192.168.4.152"
CONTAINER_ID="127"
SSH_KEY="$HOME/.ssh/id_deploy"
PROJECT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
BINARY="$PROJECT_DIR/target/release/yantrikclaw"

if [[ "${1:-}" != "--skip-build" ]]; then
    echo "🔨 Building release..."
    "$PROJECT_DIR/scripts/build.sh" release
fi

if [[ ! -f "$BINARY" ]]; then
    echo "❌ Binary not found: $BINARY"
    exit 1
fi

echo "📦 Deploying to container $CONTAINER_ID..."
scp -i "$SSH_KEY" "$BINARY" "root@$PROXMOX_HOST:/tmp/yantrikclaw"
ssh -i "$SSH_KEY" "root@$PROXMOX_HOST" "\
    pct exec $CONTAINER_ID -- systemctl stop yantrikclaw && \
    pct push $CONTAINER_ID /tmp/yantrikclaw /usr/local/bin/yantrikclaw && \
    pct exec $CONTAINER_ID -- chmod +x /usr/local/bin/yantrikclaw && \
    pct exec $CONTAINER_ID -- systemctl start yantrikclaw"

echo "✅ Deployed and restarted"

# Show startup logs
sleep 2
ssh -i "$SSH_KEY" "root@$PROXMOX_HOST" \
    "pct exec $CONTAINER_ID -- journalctl -u yantrikclaw --no-pager -n 10 --since '5 sec ago' -o cat" 2>/dev/null || true
