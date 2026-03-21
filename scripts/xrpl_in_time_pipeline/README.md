# XRPL-in-Time Pipeline

This folder contains a practical pipeline to:

1. Read partial XRPL mainnet AMM state (target pools + LP holders)
2. Extract a replay window of AMM/DEX activity
3. Build a deterministic seed spec for local chain reconstruction

## What this implements

- Multi-pool profile support (`target_pool_count`, default 25)
- LP holder selection (`top_lp_holders_per_pool`, default 10)
- Replay window controlled by one config key (`replay_window_secs`, default 3600)
- Seed spec output consumed by local seeding scripts/tools
- AMM discovery from XRPL `ledger_data` (`pool_selection_mode: "discover"`)

## Files

- `pipeline.py`: main CLI entrypoint
- `config.example.json`: copy and edit for your environment

## Usage

From repository root:

```bash
python3 scripts/xrpl_in_time_pipeline/pipeline.py snapshot \
  --config scripts/xrpl_in_time_pipeline/config.example.json \
  --out artifacts/xrpl_in_time/snapshot.json

python3 scripts/xrpl_in_time_pipeline/pipeline.py replay \
  --config scripts/xrpl_in_time_pipeline/config.example.json \
  --snapshot artifacts/xrpl_in_time/snapshot.json \
  --out artifacts/xrpl_in_time/replay.json

python3 scripts/xrpl_in_time_pipeline/pipeline.py seed-spec \
  --config scripts/xrpl_in_time_pipeline/config.example.json \
  --snapshot artifacts/xrpl_in_time/snapshot.json \
  --replay artifacts/xrpl_in_time/replay.json \
  --out artifacts/xrpl_in_time/seed-spec.json
```

## Notes

- This pipeline uses XRPL JSON-RPC now and is designed to be Firehose-compatible later.
- `pool_selection_mode`:
  - `"discover"`: scan `ledger_data` AMM objects and auto-select top pools
  - `"configured"`: use explicit `pools` from config
- If `strict_pool_count` is true, it enforces exactly `target_pool_count` selected pools.
- Holder extraction uses `account_lines` on the AMM account and ranks by LP trustline balance.
- AMM lookups use `amm_account` when available to avoid asset/issuer formatting errors.
