# VEGA MCP Server — Agent Integration Guide

This document is addressed to an AI agent integrating with the VEGA MCP server. It covers
connection, all four tools with exact I/O shapes, error handling, and chaining patterns.

---

## Connection

Transport: **stdio**. The client spawns the server as a subprocess and communicates over
stdin/stdout using the MCP protocol.

```bash
python mcp-server/server.py
```

Required services must be running before the MCP server receives tool calls:

| Service | Default address | How to start |
|---|---|---|
| Intent Router (gRPC) | `localhost:50051` | `cd llm-orchestration && python src/intent_router_service.py` |
| Rust quant backend | `http://localhost:3001` | `cd fin-analysis-backend && cargo run --release` |
| FastAPI strategy backend | `http://localhost:8000` | `cd backend && python api.py` |

Override addresses via environment variables: `INTENT_ROUTER_ADDR`, `RUST_BACKEND_URL`,
`FASTAPI_URL`.

---

## Error Contract

All tools return a `TextContent` block containing JSON. On failure, the JSON is:

```json
{ "error": "<human-readable description>" }
```

Tools never raise exceptions. Check for the `"error"` key before consuming output.

---

## Tool Reference

### `route_intent`

Classify a natural language query into a structured intent using the on-device LLM
(Llama 3.2 3B via Ollama). Falls back to a keyword classifier if Ollama is unavailable —
the fallback still returns a valid result but with lower confidence (~0.60–0.75).

**Input**
```json
{ "query": "string" }
```

**Output (success)**
```json
{
  "action": "analyze_risk | execute_strategy | check_position | get_price",
  "scope":  "portfolio | specific_asset | specific_pool",
  "confidence": 0.95,
  "parameters": [
    { "key": "pool",  "value": "XRP/USD" },
    { "key": "focus", "value": "impermanent_loss" }
  ]
}
```

> `confidence` is hardcoded to `0.95` when the LLM response passes validation. The raw
> LLM confidence is discarded. The keyword fallback produces `0.60`–`0.75`. Treat values
> below `0.65` as uncertain.

**Action semantics**

| `action` | Meaning |
|---|---|
| `analyze_risk` | Full portfolio or pool risk analysis |
| `execute_strategy` | User wants to confirm/run a strategy |
| `check_position` | Single-pool deep-dive (requires `pool` parameter) |
| `get_price` | Spot price lookup for an asset |

---

### `analyze_portfolio`

Fetch live XRPL AMM data and compute risk metrics (IL, VaR, CVaR, Sharpe, delta, gamma,
fee APR, net carry) for a wallet. Calls the Rust quant backend directly.

**Input**
```json
{
  "wallet_id": "rXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX",
  "pool": "XRP/USD"
}
```

`pool` is optional. When omitted the backend analyses the full portfolio. When provided
it performs a single-pool deep-dive — this is the `check_position` pipeline, which
**requires both** `wallet_id` and `pool`; omitting `pool` here falls back to the full
portfolio pipeline.

**Output (success)**

The Rust backend returns the rendered `PortfolioRiskSummary` as a **plain-text prompt
string** for the `analyze_risk` action. For `check_position` with a pool specified, the
structure is the same. The tool wraps it as:

```json
{ "raw": "<plain-text risk summary>" }
```

Key fields in the plain-text summary (for parsing if needed):
- `Total Value: $NNN USD`
- `Impermanent Loss: N.N% (-$NNN)`
- `Delta Exposure: NNN XRP (~$NNN if XRP drops 10%)`
- `7-Day Fee Income: $NNN`
- `Current XRP Price: $N.NNNN USD`
- `Fee APR: N.N%`
- `Sharpe Ratio: N.NN`
- `VaR (95%, 1-day): $NNN`
- `CVaR (95%, 1-day expected shortfall): $NNN`
- `Gamma: $N.NN`
- `Net Carry: N.NN%`
- `Break-even Range: $N.NNNN - $N.NNNN`
- Per-pool lines under `AMM Pool Details:`
- Lending vault lines under `Lending Context:` (if XLS-66d positions exist)

---

### `generate_strategies`

Run the full pipeline — intent classification → quant analysis → Claude Sonnet strategy
generation → validation — and return 3 risk-ranked strategies. This is the highest-level
tool; prefer it for end-to-end user queries.

**Input**
```json
{
  "user_query": "Should I hedge my XRP impermanent loss exposure?",
  "wallet_id":  "rXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX"
}
```

**Output (success)**
```json
{
  "strategies": [
    {
      "id": "option_a",
      "title": "string",
      "description": "string (≤50 words)",
      "risk_score": 2,
      "projected_return_7d": {
        "best_case": "$NNN",
        "expected": "$NNN",
        "worst_case": "-$NNN"
      },
      "trade_actions": [
        {
          "action": "swap | deposit | withdraw | lend | borrow",
          "asset_in":  "XRP | USD | BTC | ETH | USDC | USDT",
          "asset_out": "XRP | USD | BTC | ETH | USDC | USDT",
          "amount": 100.0,
          "amount2": null,
          "estimated_slippage": 0.2,
          "pool": "XRP/USD",
          "deposit_mode": "single_asset | two_asset | null",
          "interest_rate": null,
          "term_days": null
        }
      ],
      "pros": ["string", "string"],
      "cons": ["string", "string"]
    }
  ]
}
```

Strategy IDs are always `option_a` (conservative, risk 1–3), `option_b` (yield-focused,
risk 4–8), `option_c` (do nothing, empty `trade_actions`, risk 1–4).

When the `user_query` maps to `execute_strategy` intent, the backend returns UI
confirmation strategies instead of analysis-backed strategies — `option_a` is "Confirm
Execution", `option_c` is "Cancel". This is expected behaviour, not an error.

**Validation guardrails enforced before output:**
- `estimated_slippage` ≤ 1.0 on all trade actions
- `risk_score` ≤ 8
- `projected_return_7d` has `best_case`, `expected`, `worst_case`
- `trade_actions` only use allowed assets and action types

If fewer than 2 strategies pass validation, a hardcoded fallback set is returned.

---

### `get_lending_context`

Return XLS-66d vault APYs, utilization, and the wallet's open loans for a specific asset.
Useful for answering "what rate can I earn on XRP?" without triggering a full portfolio
analysis.

**Input**
```json
{
  "asset":     "XRP",
  "wallet_id": "rXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX"
}
```

**Output (success)**
```json
{
  "lending_vaults": [
    {
      "asset": "XRP",
      "total_supply_usd": 500000.0,
      "total_borrow_usd": 350000.0,
      "utilization_rate": 0.70,
      "kink_utilization": 0.80,
      "available_liquidity_usd": 150000.0,
      "supply_apy": 0.042,
      "borrow_apy": 0.068
    }
  ],
  "open_loans": [
    {
      "asset_borrowed": "XRP",
      "amount_borrowed_usd": 1200.0,
      "collateral_asset": "USD",
      "collateral_usd": 2000.0,
      "health_factor": 1.6,
      "liquidation_price": 0.31,
      "liquidation_penalty_pct": 0.08,
      "borrow_apy": 0.068,
      "term_days": 30
    }
  ]
}
```

`lending_vaults` and `open_loans` may both be empty arrays if no XLS-66d data exists for
the asset or the wallet has no open loans.

**Important:** `utilization_rate` and APY values are decimals (0.70 = 70%). Multiply by
100 before displaying. If `utilization_rate > kink_utilization`, borrow APY may spike
further — flag this in any recommendation.

---

## Chaining Patterns

### Full advisory flow (recommended)

```
generate_strategies(user_query, wallet_id)
  → returns strategies array
```

Use this for any natural language query where the user wants actionable advice. It
encapsulates the entire pipeline.

### Staged flow (when you need intermediate data)

```
route_intent(query)
  → check action and confidence
  → if confidence < 0.65: ask user to clarify
  → if action == "get_price": call a price API directly (not in this MCP server)
  → if action == "check_position": call analyze_portfolio(wallet_id, pool=<from parameters>)
  → if action in ["analyze_risk", "execute_strategy"]: call generate_strategies(query, wallet_id)
```

### Lending-aware advisory

```
get_lending_context(asset, wallet_id)
  → inspect health_factor on open loans
  → if any health_factor < 1.2: prepend "repayment urgency" to user_query
generate_strategies(modified_query, wallet_id)
```

### Risk-only snapshot (no strategy generation)

```
analyze_portfolio(wallet_id, pool=null)
  → parse the plain-text summary for current metrics
  → present to user or use to answer a specific question
```

---

## Demo Wallet

The lending devnet has a pre-funded demo wallet for testing:

```
wallet_id: rprsHgtUT6r1ZuWQXAVSaAzDtxtH7q8VBF
network:   lend-devnet (https://lend.devnet.rippletest.net:51234/)
```

The Rust backend must be configured with `XRPL_ENDPOINT=https://lend.devnet.rippletest.net:51234/`
for lending vault data to be populated.
