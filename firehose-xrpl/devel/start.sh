#!/bin/bash
# Start firehose-xrpl for local development

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"

# Default configuration
XRPL_RPC_ENDPOINT="${XRPL_RPC_ENDPOINT:-https://s1.ripple.com:51234/}"
START_BLOCK="${START_BLOCK:-80000000}"
STATE_DIR="${STATE_DIR:-$PROJECT_DIR/devel/data/poller}"
MERGED_BLOCKS_DIR="${MERGED_BLOCKS_DIR:-$PROJECT_DIR/devel/data/merged-blocks}"
ONE_BLOCKS_DIR="${ONE_BLOCKS_DIR:-$PROJECT_DIR/devel/data/one-blocks}"

# Create directories
mkdir -p "$STATE_DIR" "$MERGED_BLOCKS_DIR" "$ONE_BLOCKS_DIR"

echo "=== Firehose XRPL Development Server ==="
echo "RPC Endpoint: $XRPL_RPC_ENDPOINT"
echo "Start Block:  $START_BLOCK"
echo "State Dir:    $STATE_DIR"
echo ""

# Build if needed
if [ ! -f "$PROJECT_DIR/firexrpl" ]; then
    echo "Building firexrpl..."
    cd "$PROJECT_DIR"
    go build -o firexrpl ./cmd/firexrpl
fi

# Run the fetcher
echo "Starting block fetcher..."
"$PROJECT_DIR/firexrpl" fetch rpc "$START_BLOCK" \
    --endpoints "$XRPL_RPC_ENDPOINT" \
    --state-dir "$STATE_DIR" \
    --latest-block-retry-interval 2s \
    --max-block-fetch-duration 30s
