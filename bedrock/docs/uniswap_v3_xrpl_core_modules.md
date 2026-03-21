# Phase 2 Core Modules and ABI Surface

## Module Boundaries
- `pool_state`: global pool state, fee globals, protocol fee counters.
- `tick_state`: per-tick liquidity deltas and outside accumulators.
- `position_state`: per-owner range positions and owed fees.
- `swap_engine`: within-tick execution and cross-tick traversal loop.
- `fee_engine`: fee split, growth accounting, and collection updates.
- `guards`: pause, auth, slippage caps, tick validity, bounds checks.
- `adapter`: settlement dispatch into Bedrock direct path or direct XRPL path.

## ABI Methods
### Lifecycle
- `initialize_pool(sqrt_price_q64_64, protocol_fee_share_bps)`

### LP Operations
- `mint(lower_tick, upper_tick, liquidity_delta)`
- `burn(lower_tick, upper_tick, liquidity_delta)`
- `collect(lower_tick, upper_tick, max_amount_0, max_amount_1)`

### Trading
- `swap_exact_in(amount_in, min_amount_out, zero_for_one, sqrt_price_limit_q64_64)`

### Governance and Safety
- `set_protocol_fee(protocol_fee_share_bps)`
- `set_pause(paused)`

## File Implemented
The contract skeleton implementing this structure is at:
- `bedrock/uniswap_v3_xrpl_contract_skeleton.rs`

## Notes
- State mappings are intentionally scaffolded and marked TODO where persistent host bindings are required.
- All methods are designed to keep deterministic transitions and enforce safety checks before state mutation.
