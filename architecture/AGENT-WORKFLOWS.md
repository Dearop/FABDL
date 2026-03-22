# Agent Workflow Patterns

Decision trees and chaining patterns for common user intents. Read this when deciding
which services to call, in what order, and how to handle edge cases.

---

## Service Map (quick reference)

```
User query
  │
  ├─ Via MCP server (recommended for agents)
  │    mcp-server/server.py — stdio transport
  │    Tools: route_intent, analyze_portfolio, generate_strategies, get_lending_context
  │
  └─ Direct HTTP/gRPC (for debugging or custom orchestration)
       Intent Router    localhost:50051  gRPC  ClassifyIntent
       Rust backend     localhost:3001   HTTP  POST /analyze
       FastAPI backend  localhost:8000   HTTP  POST /strategies/generate
                                               POST /strategy/execute
                                               POST /query/classify
```

---

## Decision Tree: Choosing the Right Entry Point

```
Received a user message about XRPL trading
│
├─ Is this a one-shot "what should I do?" query?
│   └─ YES → call generate_strategies(user_query, wallet_id)
│             This encapsulates everything. Stop here.
│
├─ Does the user want to know their current position/metrics without a recommendation?
│   └─ YES → call analyze_portfolio(wallet_id, pool=null)
│             Parse the plain-text summary for metrics.
│
├─ Is the user asking about lending rates or their loan health?
│   └─ YES → call get_lending_context(asset, wallet_id)
│             Check health_factor on open_loans — see Lending Safety section.
│
├─ Is the user asking for a price quote only?
│   └─ YES → route_intent is not the right tool; call an XRPL price feed directly.
│             The MCP server does not expose a price-only tool.
│
└─ Do you need to validate the query type before committing?
    └─ call route_intent(query) first, then branch on action.
```

---

## Pattern 1: Full Advisory (most common)

Use `generate_strategies`. It runs the entire pipeline — intent classification, quant
analysis, Claude Sonnet strategy generation, and validation — in one HTTP call.

```
Input:  { user_query, wallet_id }
Output: { intent, strategies[3], wallet_id }
```

Consume:
- `strategies[0]` → `option_a` (conservative)
- `strategies[1]` → `option_b` (yield-focused)
- `strategies[2]` → `option_c` (do nothing)

When to use fallback strategies (auto): if the backend returns strategies but fewer than
2 pass validation, a hardcoded fallback set is returned transparently. You cannot
distinguish these from real strategies by the response shape.

---

## Pattern 2: Staged with Intent Routing

Use when you want to branch the conversation before committing to strategy generation.

```
Step 1: route_intent(query)

Step 2: Branch on action
  ├─ "analyze_risk"     → generate_strategies(query, wallet_id)
  ├─ "execute_strategy" → generate_strategies(query, wallet_id)
  │                        (FastAPI intercepts this and returns confirmation strategies,
  │                         not analysis-backed ones — this is expected)
  ├─ "check_position"   → extract pool from parameters, then
  │                        analyze_portfolio(wallet_id, pool=<value>)
  ├─ "get_price"        → out of scope for this MCP server; use XRPL price feed
  └─ unknown/low conf.  → ask user to rephrase

Step 3: Check confidence
  ≥ 0.90  → proceed
  0.65–0.89 → proceed with caveat (keyword fallback may have been used)
  < 0.65  → ask user to clarify before proceeding
```

---

## Pattern 3: Lending-Aware Advisory

Use when the user mentions borrowing, lending, APY, or "health factor".

```
Step 1: get_lending_context(asset, wallet_id)

Step 2: Inspect open_loans
  for each loan:
    if health_factor < 1.2:
      URGENT — prepend to query: "My loan health factor is {hf}. "
      The generate_strategies response MUST include partial repayment
      in the conservative option (this is a Claude prompt guardrail).

    if health_factor < 1.5:
      ELEVATED RISK — note in query context.

    if liquidation_price is within VaR range:
      flag liquidation risk explicitly.

Step 3: generate_strategies(modified_query, wallet_id)
```

Note: `supply_apy` and `borrow_apy` from `lending_vaults` are decimals (0.042 = 4.2%).
`utilization_rate` is also a decimal. `kink_utilization` marks the inflection point
beyond which borrow APY rises steeply.

---

## Pattern 4: Portfolio Snapshot (metrics only, no strategy)

Use when the user wants to know their current numbers without a recommendation.

```
Step 1: analyze_portfolio(wallet_id, pool=null)
  → Returns plain-text PortfolioRiskSummary

Step 2: Parse key metrics from the text:
  - Total Value
  - Impermanent Loss %
  - Fee APR
  - Sharpe Ratio
  - VaR 95%
  - Net Carry (positive = position earns more than it costs)
  - Break-even Range (price range where fees offset IL)

Step 3: Present to user or use for conditional logic
  if sharpe < 0.5:   poor risk-adjusted return
  if net_carry < 0:  position is net-negative (costs more than it earns)
  if var_95 > 0.1 * total_value:  high tail risk
```

For a single-pool deep-dive:
```
analyze_portfolio(wallet_id, pool="XRP/USD")
```
Both `wallet_id` and `pool` must be provided. `pool` format is always `"ASSET1/ASSET2"`.

---

## Pattern 5: Execute a Strategy

After presenting strategies to the user and receiving their selection:

```
Step 1: User selects strategy (e.g. option_a)

Step 2: POST to FastAPI /strategy/execute
  {
    "strategy_id": "option_a",
    "wallet_id":   "rXXX...",
    "strategy":    <full strategy object from generate response>
  }

Step 3: Parse execution_summary
  - summary_lines: list of human-readable trade descriptions
  - il_estimate:   IL at ±10% price move (for deposit actions)
  - fee_estimate:  projected APR or lending yield
  - net_cost:      XRPL network fee estimate

Step 4: Present to user and request wallet signature
  NOTE: /strategy/execute is currently SIMULATED. No XRPL transactions are submitted.
  The tx_hash returned is a SHA-256 digest, not a real ledger hash.
  Real on-chain execution requires wallet signing via Xaman or Crossmark.
```

---

## Edge Cases and Gotchas

**`check_position` requires a pool.** If `route_intent` returns `action: "check_position"`
but `parameters` contains no `pool` key, either ask the user which pool they mean or fall
back to `analyze_portfolio(wallet_id, pool=null)` for the full portfolio.

**`execute_strategy` is intercepted before Rust.** The FastAPI backend catches this action
and returns confirmation UI strategies without calling the Rust backend. This is correct
behaviour, not an error.

**Plain-text vs JSON from Rust.** The Rust `/analyze` endpoint returns a plain-text
prompt string, not JSON. The `analyze_portfolio` MCP tool wraps it as `{"raw": "..."}`.
Parse the text directly with regex or string matching if you need specific metric values.

**Ollama availability.** The Intent Router's LLM runs locally. If Ollama is not running,
the service falls back to a keyword classifier. You cannot tell which path was taken from
the gRPC response — but confidence will be lower (0.60–0.75 vs 0.95). This does not
affect the validity of the response.

**Demo wallet on lend-devnet.** Address `rprsHgtUT6r1ZuWQXAVSaAzDtxtH7q8VBF` is
pre-funded on the lending devnet. The Rust backend must use
`XRPL_ENDPOINT=https://lend.devnet.rippletest.net:51234/` for lending vault data to
appear in portfolio summaries.

**Slippage is percent, not basis points.** `estimated_slippage: 0.2` means 0.2%, not
20%. The hard cap is 1.0 (= 1%).

**APY/APR values from Rust are fractions.** `fee_apr: 0.084` = 8.4% APR. Multiply by
100 before displaying.

---

## Strategy Taxonomy (for interpreting output)

| `id` | Label | `risk_score` | `trade_actions` |
|---|---|---|---|
| `option_a` | Conservative | 1–3 | Capital preservation — hedging, withdrawal, or lending |
| `option_b` | Yield-Focused | 4–8 | Fee maximisation — AMM deposits, lending, delta-neutral LP |
| `option_c` | Do Nothing | 1–4 | Always empty — hold current position |

`option_b` with a delta-neutral strategy (borrow + deposit) will have `risk_score` 5–7.
If `net_carry < 0`, the strategy description will note that the hedge costs more than it
earns — this is a guardrail, not an error.

---

## Startup Order

Services must start in this order (each depends on the previous being ready):

```
1. ollama serve                          (Ollama, for Intent Router LLM)
2. python llm-orchestration/src/intent_router_service.py
3. cargo run --release  (fin-analysis-backend/)
4. python backend/api.py
5. python mcp-server/server.py           (MCP server last — depends on all above)
```

The MCP server itself has no startup check — it will return `{"error": "..."}` from any
tool whose upstream service is down.
