#!/bin/bash
# Advance ledger every 1 second via ledger_accept RPC

while true; do
    curl -s -X POST http://localhost:5005 \
        -H "Content-Type: application/json" \
        -d '{"method": "ledger_accept", "params": [{}]}' \
        | jq -r '.result | "Ledger: \(.ledger_current_index // .error)"' 2>/dev/null \
        || echo "RPC call failed"
    sleep 1
done
