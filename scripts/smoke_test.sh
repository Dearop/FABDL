#!/bin/bash
# Smoke test: verify all services respond
echo "Testing Rust backend..."
curl -sf http://localhost:3001/health || echo "FAIL: Rust backend"
echo "Testing FastAPI bridge..."
curl -sf http://localhost:8000/health || echo "FAIL: FastAPI bridge"
echo "Testing strategy generation..."
curl -sf -X POST http://localhost:8000/strategies/generate \
  -H "Content-Type: application/json" \
  -d '{"user_query": "analyze my portfolio risk", "wallet_id": "rN7n7otQDd6FczFgLdlqtyMVrn3NnrcVcN"}' \
  | python3 -c "import sys,json; d=json.load(sys.stdin); print(f'OK: {len(d[\"strategies\"])} strategies')" \
  || echo "FAIL: strategy generation"
echo "Done."
