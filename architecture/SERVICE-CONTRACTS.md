# Service Contracts

Exact request/response shapes for every internal service. Read this when you need to call
a service directly rather than through the MCP server, or when debugging a pipeline step.

---

## 1. Intent Router — gRPC `:50051`

**Proto definition:** `llm-orchestration/proto/intent_router.proto`

### Request

```protobuf
message IntentRequest {
  string user_query = 1;
  int64  timestamp  = 2;  // unix seconds; use current time
}
```

### Response

```protobuf
message IntentResponse {
  string            action     = 1;  // "analyze_risk" | "execute_strategy" | "check_position" | "get_price"
  string            scope      = 2;  // "portfolio" | "specific_asset" | "specific_pool"
  repeated Parameter parameters = 3;
  float             confidence = 4;  // always 0.95 when is_valid=true
  bool              is_valid   = 5;
}

message Parameter {
  string key   = 1;
  string value = 2;
}
```

### Python call (async)

```python
import grpc, intent_router_pb2, intent_router_pb2_grpc

channel = grpc.aio.insecure_channel("localhost:50051")
stub    = intent_router_pb2_grpc.IntentRouterStub(channel)
req     = intent_router_pb2.IntentRequest(user_query="...", timestamp=int(time.time()))
resp    = await stub.ClassifyIntent(req)
await channel.close()
# resp.action, resp.scope, resp.confidence, resp.is_valid, resp.parameters
```

Generated stubs are at `llm-orchestration/src/intent_router_pb2.py` and
`intent_router_pb2_grpc.py`. Add that directory to `sys.path` before importing.

### Behaviour notes

- The LLM runs locally via Ollama (`llama3.2:3b` by default). If Ollama is down the
  service falls back to a keyword classifier. Both paths return valid `IntentResponse`
  objects; the fallback just has lower implicit reliability.
- `is_valid` is `True` whenever the classification passes schema validation. The FastAPI
  backend rejects requests with `is_valid=False`.
- `parameters` is a flat list of `{key, value}` pairs. Common keys: `pool` (e.g.
  `"XRP/USD"`), `focus` (e.g. `"impermanent_loss"`), `asset` (e.g. `"XRP"`).

---

## 2. Rust Quant Backend — HTTP `:3001`

Single endpoint. The `action` field in the request body determines which pipeline runs.

### `POST /analyze`

**Request body — `IntentRouterOutput`**

```json
{
  "action":     "analyze_risk | check_position | get_price",
  "scope":      "portfolio | specific_asset | specific_pool",
  "parameters": {
    "wallet_address": "rXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX",
    "pool":           "XRP/USD",
    "focus":          "impermanent_loss"
  },
  "confidence": 0.95
}
```

> `execute_strategy` is **not** handled by Rust — the FastAPI layer intercepts it and
> returns UI confirmation strategies directly. Sending `action: "execute_strategy"` to
> the Rust backend will produce a 500 error.

**Parameter requirements by action**

| `action` | Required parameters | Optional parameters |
|---|---|---|
| `analyze_risk` | `wallet_address` | `pool`, `focus` |
| `check_position` | `wallet_address`, `pool` | `focus` |
| `get_price` | — | `asset` |

**Response**

`200 OK` with `Content-Type: text/plain`. The body is a rendered `PortfolioRiskSummary`
prompt string intended as the user message for Claude Sonnet. Example structure:

```
Portfolio Risk Summary:
- Total Value: $4821 USD
- Impermanent Loss: -2.3% (-$111)
- Delta Exposure: 1200 XRP (~$60 if XRP drops 10%)
- 7-Day Fee Income: $34
- Current XRP Price: $0.5012 USD
- Fee APR: 8.4%
- Sharpe Ratio: 1.23
- VaR (95%, 1-day): $142
- CVaR (95%, 1-day expected shortfall): $198
- Gamma: $-3.12
- Net Carry: 0.41%
- Break-even Range: $0.4411 - $0.5691

AMM Pool Details:
- Pool: XRP/USD, Value: $4821, LP Share: 0.8%, IL: -2.3% (-$111), Fee APR: 8.4%, 7d Fees: $34, Delta: 1200 XRP

Lending Context:
- Vault XRP: supply APY 4.2%, borrow APY 6.8%, utilization 70%
Open Loans:
- Borrowed 1200 XRP against 2000 USD collateral, health factor 1.6, borrow APY 6.8%

Task: Generate 3 strategies to manage this position.
```

**Error responses**

| Status | Meaning |
|---|---|
| `400` | Missing required parameter (e.g. `check_position` called without `pool`) |
| `500` | XRPL RPC error, unsupported action, or internal computation failure |

### `GET /health`

Returns `200 OK` with `{"status":"ok"}` when the service is running.

### Environment variables

| Variable | Default | Description |
|---|---|---|
| `XRPL_ENDPOINT` | `https://s.altnet.rippletest.net:51234` | XRPL node RPC URL |
| `PORT` | `3001` | HTTP listen port |
| `RUST_LOG` | — | Set to `fin_analysis_backend=info` for structured logs |

For lending vault data, use `XRPL_ENDPOINT=https://lend.devnet.rippletest.net:51234/`.

---

## 3. FastAPI Strategy Backend — HTTP `:8000`

### `POST /strategies/generate`

The highest-level endpoint. Orchestrates the full pipeline internally.

**Request**
```json
{
  "user_query": "string",
  "wallet_id":  "rXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX"
}
```

**Response**
```json
{
  "intent": {
    "action":     "analyze_risk",
    "scope":      "portfolio",
    "confidence": 0.95,
    "is_valid":   true,
    "parameters": [{ "key": "pool", "value": "XRP/USD" }]
  },
  "strategies": [ /* array of Strategy objects — see shape below */ ],
  "wallet_id": "rXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX"
}
```

**Strategy object shape**
```json
{
  "id":          "option_a | option_b | option_c",
  "title":       "string",
  "description": "string (≤50 words)",
  "risk_score":  3,
  "projected_return_7d": {
    "best_case":  "$NNN",
    "expected":   "$NNN",
    "worst_case": "-$NNN"
  },
  "trade_actions": [
    {
      "action":             "swap | deposit | withdraw | lend | borrow",
      "asset_in":           "XRP | USD | BTC | ETH | USDC | USDT",
      "asset_out":          "XRP | USD | BTC | ETH | USDC | USDT",
      "amount":             100.0,
      "amount2":            null,
      "estimated_slippage": 0.2,
      "pool":               "XRP/USD",
      "deposit_mode":       "single_asset | two_asset | null",
      "interest_rate":      null,
      "term_days":          null
    }
  ],
  "pros": ["string", "string"],
  "cons": ["string", "string"]
}
```

**Invariants guaranteed by the validation layer**
- `estimated_slippage` ≤ 1.0 (1%) on every trade action
- `risk_score` ≤ 8
- `option_c` always has empty `trade_actions`
- Minimum 2 strategies always returned (fallback set used if Claude output fails validation)

**Error responses**

| Status | Meaning |
|---|---|
| `400` | `user_query` empty, or intent `is_valid=false` (confidence too low) |
| `500` | gRPC failure, Rust backend error, Claude API error, or Claude returned invalid JSON |

---

### `POST /strategy/execute`

Simulated execution. Builds a human-readable summary of what trades would run. Does not
submit XRPL transactions yet.

**Request**
```json
{
  "strategy_id": "option_a",
  "wallet_id":   "rXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX",
  "strategy":    { /* full strategy object from generate response */ }
}
```

**Response**
```json
{
  "tx_hash": "ABCDEF1234...",
  "status":  "confirmed",
  "wallet_id":    "rXXX...",
  "strategy_id":  "option_a",
  "execution_summary": {
    "simulated":     true,
    "summary_lines": ["Swapped 100 XRP → USD via XRP/USD pool"],
    "il_estimate":   "Estimated IL at ±10% price move: ~-0.5%",
    "fee_estimate":  "Estimated Fee APR: 5-15% (depends on pool volume)",
    "net_cost":      "Est. network fee: 0.000012 XRP per transaction"
  }
}
```

`tx_hash` is a SHA-256 digest of `strategy_id + wallet_id + timestamp`. It is not a real
XRPL transaction hash.

---

### `POST /wallet/connect`

Validates an XRPL wallet address format. Does not verify on-ledger balance.

**Request**
```json
{ "address": "rXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX", "network": "testnet" }
```

**Response**
```json
{
  "wallet_id": "rXXX...",
  "verified":  true,
  "network":   "testnet",
  "balance":   "5000 XRP"
}
```

Validation rule: address must start with `r` and be at least 20 characters. Balance is
a placeholder.

---

### `POST /query/classify`

Classify a query via the Intent Router without running the full strategy pipeline. Useful
for routing decisions.

**Request**
```json
{ "user_query": "string", "wallet_id": "rXXX..." }
```

**Response**
```json
{
  "intent": { "action": "...", "scope": "...", "confidence": 0.95, "is_valid": true, "parameters": [] },
  "wallet_id": "rXXX..."
}
```

---

## 4. Environment Variable Reference (all services)

```
# Intent Router
OLLAMA_HOST=127.0.0.1:11434   # Ollama inference server

# Rust Quant Backend
XRPL_ENDPOINT=https://lend.devnet.rippletest.net:51234/
PORT=3001
RUST_LOG=fin_analysis_backend=info

# FastAPI Backend
RUST_BACKEND_URL=http://localhost:3001
ANTHROPIC_API_KEY=sk-ant-...   # Required for Claude Sonnet strategy generation

# MCP Server
INTENT_ROUTER_ADDR=localhost:50051
RUST_BACKEND_URL=http://localhost:3001
FASTAPI_URL=http://localhost:8000

# Frontend
NEXT_PUBLIC_API_URL=http://localhost:8000
NEXT_PUBLIC_XRPL_NETWORK=lend-devnet
NEXT_PUBLIC_DEMO_WALLET_ADDRESS=rprsHgtUT6r1ZuWQXAVSaAzDtxtH7q8VBF
```
