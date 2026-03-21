# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Status

This is a **research & design phase** project — no code exists yet. All current content is documentation and architecture planning.

## Architecture Overview

A conversational AI system that simplifies XRPL AMM DeFi trading from a manual 4-step process into a 3-step AI-assisted workflow:
1. User describes goal in natural language
2. AI generates 3 risk-ranked strategies with PnL visualizations
3. User reviews and clicks execute (signs transaction via wallet)

### Three-Phase Data Flow

**Phase 1 — Ingestion & Analysis (off-chain)**
- On-device Llama 3.2 3B (quantized, <100ms) routes intent from natural language → structured JSON
- Backend queries XRPL node for LP balances, AMM pool states, historical prices
- Quant models compute IL, delta exposure, VaR, Sharpe ratio

**Phase 2 — Strategy Generation (off-chain)**
- Risk JSON sent to Claude Sonnet 4.5 (via API)
- LLM generates 3 strategies: Conservative (delta hedge), Yield-Focused (rebalance), Do Nothing
- Validation pipeline enforces hard constraints before strategies reach user

**Phase 3 — Execution (on-chain)**
- Frontend displays 3 strategy cards with PnL graphs
- User signs via Xaman or Crossmark wallet
- Transaction targets Bedrock smart contract (or direct XRPL if Bedrock is unavailable)
- XRPL AMM executes: AMMDeposit, AMMWithdraw, swaps

### Key Design Decisions

- **Local intent router** (Llama 3.2 on-device): privacy (queries never leave device), <100ms latency, zero cost
- **Isolated Quant LLM** (Claude Sonnet 4.5): protected from prompt injection, all strategy I/O pairs logged for auditability
- **User-signed transactions only**: backend never holds keys; Bedrock contract logic is public/auditable
- **Hybrid MVP**: if Bedrock is unavailable, fall back to backend-constructed multi-step XRPL transactions

## Key Context Files

Load based on the area you're working in:

| Area | File |
|---|---|
| Overall architecture, data flow, tech stack, API specs | `architecture/SYSTEM-DESIGN.md` |
| IL, delta, VaR, Sharpe formulas; strategy taxonomy | `quant/RISK-MODELS.md` |
| Rust smart contract design, security features, testing | `bedrock/SMART-CONTRACT.md` |
| Intent router & strategy generator prompts, safety guardrails | `llm-orchestration/PROMPTS.md` |
| XRPL AMM transaction types, query APIs, swap mechanics | `references/XRPL-AMM.md` |
| Bedrock viability research, alternatives (Hooks, direct XRPL) | `references/BEDROCK.md` |

## Critical Open Questions

Before writing implementation code, verify:

1. **Bedrock viability** — does it actually exist for XRPL? Alternatives: XRPL Hooks (XLS-38d), Flare Network (EVM+bridge), or direct XRPL transaction batching
2. **Smart contract interaction model** — Option A: contract submits XRPL txs directly; Option B: Bedrock ↔ XRPL Hook bridge

## Quantitative Metrics

Core formulas in play:
- **IL**: `2 * sqrt(price_ratio) / (1 + price_ratio) - 1`
- **VaR**: Historical simulation over 1,000 scenarios
- **Sharpe**: `(Return - RiskFreeRate) / Volatility` (>2.0 excellent, <0.5 poor)
- **Net return**: `Fee income - IL - Gas costs`

## Smart Contract Interface (Rust/WASM)

```rust
pub fn execute_strategy(
    asset_in: Asset,
    asset_out: Asset,
    amount: u64,
    max_slippage: u16,  // basis points; hard cap 100 = 1%
    strategy_type: StrategyType
) -> Result<ExecutionSummary, ContractError>
```

Security invariants: slippage hard-capped at 1%, explicit user signature per execution, strategy allowlist (prevents hallucinated strategies from executing), owner pause mechanism.

## LLM Safety Guardrails

The Quant LLM prompt must enforce:
- Never recommend >1% slippage
- Always include "Do Nothing" as an option
- Cap risk scores at 8/10
- Post-generation validation in Python before strategies reach UI
- Fallback to hardcoded conservative strategy if validation fails

## XRPL Notes

- Gas: ~$0.000005 per transaction (micro-rebalancing is economically viable)
- No concentrated liquidity (capital-inefficient vs. Uniswap V3)
- No multi-hop routing yet
- ~50 active AMM pools as of March 2026
- Libraries: `xrpl.js` (JS), `xrpl` crate 0.9+ (Rust), `xrpl-py` (Python)

## Success Criteria

| Metric | Target |
|---|---|
| Risk projection accuracy | ±5% vs. actual PnL |
| Strategy success rate | >75% beating HODL |
| End-to-end latency | <8s query → on-chain confirmation |
| Unauthorized trades | 0 (LLM hallucinations must never execute) |
