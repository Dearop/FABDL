# fabdl

EPFL FABDL final project — Uniswap V3 USDC/WETH 0.05% pool analytics.

Pool: `0x88e6A0c2dDD26FEEb64F039a2c41296FcB3f5640` (Ethereum mainnet).

## Setup

```bash
python -m venv .venv && source .venv/bin/activate
pip install -e ".[dev]"
cp .env.example .env     # fill in RPC_URL (any JSON-RPC endpoint)
```

The RPC layer is **endpoint-agnostic**: point `RPC_URL` at Alchemy, Infura, Ankr,
a free public endpoint, a self-hosted Geth/Erigon/Reth, or a local Anvil fork —
the rest of the code doesn't care.

## Layout

- `src/fabdl/core/` — correctness-critical V3 math ports (pure-int, no floats)
- `src/fabdl/io/` — generic RPC client, log fetcher, parquet writers
- `src/fabdl/extract/` — Module 1 (4 parquet deliverables)
- `src/fabdl/liquidity/` — Module 2
- `src/fabdl/simulate/` — Module 3 (swap simulator)
- `src/fabdl/lp/` — Module 4 (LP analytics, IL, fees)
- `src/fabdl/hedge/` — Module 5 (greeks, perp data, backtest)
- `notebooks/` — one per module for the report
