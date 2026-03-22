# VEGA — XRPL AI Trading Assistant

A conversational AI system that analyzes XRPL AMM portfolio risk, generates quantitative trading strategies with visual risk profiles, and executes them on-chain.

**Core flow:** Describe your goal in natural language → AI generates 3 risk-ranked strategies with PnL visualizations → Review and click execute (sign via wallet).

---

## Architecture

```
User Chat (Next.js)
    ↓
Intent Router  ← local Llama 3.2 3B via Ollama (<100ms, on-device)
    ↓ gRPC :50051
Backend API (Python/FastAPI :8000)
    ↓
Fin Analysis Backend (Rust/Axum :3001)  ←  IL, VaR, Sharpe, delta
    ↓
LLM Orchestration (Claude Sonnet 4.6)  ←  strategy generation
    ↓
Frontend  ←  strategy cards + PnL graphs
    ↓  Xaman / Crossmark wallet signature
Bedrock Smart Contract (Rust/WASM) → XRPL Native AMM
```

XRPL ledger data is streamed and indexed by `firehose-xrpl` (Go).
AI tooling is exposed via `mcp-server` (MCP stdio transport).

---

## Modules

| Directory | Language | Role |
|---|---|---|
| `frontend/` | Next.js + TypeScript | Chat UI, strategy cards, wallet integration |
| `llm-orchestration/` | Python + gRPC | Intent router (Llama) + strategy generator (Claude) |
| `fin-analysis-backend/` | Rust | Quant risk models: IL, VaR, Sharpe, delta |
| `backend/` | Python (FastAPI) | XRPL data API, orchestration glue |
| `firehose-xrpl/` | Go | XRPL ledger streaming & Protobuf decoding |
| `bedrock/` | Rust/WASM | On-chain execution smart contract |
| `mcp-server/` | Python | MCP server exposing VEGA tools to AI clients |

---

## Usage Guide

### Prerequisites

- [Node.js](https://nodejs.org/) 18+
- Python 3.10+
- [Rust](https://rustup.rs/) + Cargo
- [Ollama](https://ollama.ai/) (for local LLM)
- An `ANTHROPIC_API_KEY` (for strategy generation)

### 1. Install dependencies

```bash
make install
```

This installs frontend npm packages, backend Python deps, and builds the Rust analysis backend.

### 2. Set your API key

```bash
export ANTHROPIC_API_KEY=sk-ant-...
```

### 3. Start all services (5 terminals)

| Terminal | Command | Port | Expected output |
|---|---|---|---|
| 1 — Ollama | `make start-ollama` | 11434 | `Listening on 127.0.0.1:11434` |
| 2 — Intent Router | `make start-intent` | 50051 | `Intent Router listening on [::]:50051` |
| 3 — Rust Analysis | `make start-rust` | 3001 | `listening on 0.0.0.0:3001` |
| 4 — Backend API | `make start-backend` | 8000 | `Uvicorn running on http://0.0.0.0:8000` |
| 5 — Frontend | `make start-frontend` | 3000 | `Ready in XXXms` |

Or just run `make setup` to print these instructions.

### 4. Open the app

Visit **http://localhost:3000** and connect your Xaman or Crossmark wallet.

### 5. Example queries

```
"Analyze my portfolio risk"
"How much IL do I have on XRP/USDC?"
"Show me strategies to reduce my delta exposure"
"What's the fee APY on my current pool?"
```

The system classifies your intent locally (no data leaves your device), fetches your XRPL positions, computes risk metrics, and returns 3 strategy options to review before any transaction is signed.

---

### MCP Server (optional)

Expose VEGA tools to Claude Desktop or any MCP-compatible client:

```bash
cd mcp-server && pip install -r requirements.txt && python server.py
```

Available tools: `route_intent`, `analyze_portfolio`, `generate_strategies`, `get_lending_context`.

### XRPL Firehose (optional)

Stream and index raw XRPL ledger data:

```bash
cd firehose-xrpl
go build -o firexrpl ./cmd/firexrpl
./firexrpl fetch rpc 80000000 --endpoints https://s1.ripple.com:51234/ --state-dir /data/poller
```

---

## Key Design Decisions

- **On-device intent router** — privacy-first, zero cloud cost, <100ms latency
- **Isolated strategy LLM** — protected from prompt injection; all I/O logged for auditability
- **User-signed transactions only** — backend never holds private keys
- **Slippage hard-capped at 1%** in the smart contract; LLM hallucinations cannot execute

## Success Criteria

| Metric | Target |
|---|---|
| Risk projection accuracy | ±5% vs. actual PnL |
| Strategy success rate | >75% beating HODL |
| End-to-end latency | <8s query → on-chain confirmation |
| Unauthorized trades | 0 |
