# AMM Smart Contract — Implementation Reference

This document describes what was actually built. It supersedes the design-phase
`SMART-CONTRACT.md`, which described intended architecture before code existed.

---

## What Was Built

A Uniswap v3-inspired concentrated-liquidity AMM compiled to a single WASM
binary for deployment on Bedrock (XRPL's smart-contract layer). The contract
is fully self-contained: it implements the complete swap math, fee accounting,
TWAP oracle, position management, and a lifecycle hook system, all without
external dependencies at runtime.

The codebase is split into two Rust crates:

| Crate | Path | Purpose |
|---|---|---|
| `uniswap-v3-xrpl-contract` | `bedrock/contract/` | The on-chain AMM logic |
| `uniswap-v3-xrpl-adapter` | `bedrock/adapter/` | Off-chain routing shim |

All 54 tests pass. The contract builds to a 375-byte WASM binary.

---

## Module Map

```
contract/src/
├── lib.rs          — exported ABI functions, state structs, storage persistence
├── math.rs         — sqrt price arithmetic, tick ↔ price conversion, swap step math
├── swap.rs         — multi-tick swap loop engine
├── tick.rs         — tick state, fee growth accumulators, tick crossing
├── tick_bitmap.rs  — 256-bit word bitmap for O(1) next-tick lookup
├── position.rs     — LP position accounting, fee collection
├── oracle.rs       — TWAP circular buffer (Uniswap v3 §5)
├── hooks.rs        — lifecycle callback system
├── codec.rs        — binary serialisation for host storage persistence
└── types.rs        — shared types (AccountId, ContractError, Asset)
```

---

## State Model

The entire pool state lives in one `ContractState` blob serialised to host
storage on every function entry/exit (WASM builds only; native test builds
use a thread-local).

### PoolState

| Field | Type | Description |
|---|---|---|
| `sqrt_price_q64_64` | `u128` | Current sqrt price in Q64.64 fixed-point |
| `current_tick` | `i32` | Floor tick of current price |
| `liquidity_active` | `u128` | Active liquidity in the current tick range |
| `fee_bps` | `u16` | LP fee in basis points (e.g. 30 = 0.3%) |
| `protocol_fee_share_bps` | `u16` | Protocol's share of LP fee (max 2500 = 25%) |
| `fee_growth_global_0_q128` | `u128` | Cumulative fee growth for token0, Q128 |
| `fee_growth_global_1_q128` | `u128` | Cumulative fee growth for token1, Q128 |
| `protocol_fees_0` | `u128` | Uncollected protocol fees, token0 |
| `protocol_fees_1` | `u128` | Uncollected protocol fees, token1 |
| `seconds_per_liquidity_q128` | `u128` | Global oracle accumulator, Q128 |
| `last_block_timestamp` | `u32` | Timestamp of last oracle write |
| `initialized` | `bool` | Whether `initialize_pool` has been called |

### ContractConfig

| Field | Type | Description |
|---|---|---|
| `owner` | `[u8; 20]` | Address allowed to call admin functions |
| `paused` | `bool` | Global circuit breaker |
| `max_slippage_bps` | `u16` | Hard slippage cap (default 100 = 1%) |
| `tick_spacing` | `i32` | Minimum tick granularity (default 10) |
| `hook_id` | `HookId` | Lifecycle hook attached to this pool |

---

## Exported ABI Functions

Every function returns 0 on success or an error code on failure, except where
noted. Timestamps are UNIX seconds as `u32`.

### `initialize_pool`
```
initialize_pool(sender, sqrt_price_q64_64, fee_bps, protocol_fee_share_bps,
                timestamp, hook_id) -> u32
```
Owner-only. Sets the initial price, fee tier, and hook. Also writes the first
oracle observation. Can only be called once.

### `mint`
```
mint(sender, lower_tick, upper_tick, liquidity_delta) -> u32
```
Add `liquidity_delta` to the position `(sender, lower_tick, upper_tick)`.
Ticks must be multiples of `tick_spacing`. Updates `liquidity_active` if the
current tick falls in range. Fires `before_mint` / `after_mint` hooks.

### `burn`
```
burn(sender, lower_tick, upper_tick, liquidity_delta) -> u32
```
Remove `liquidity_delta` from the sender's position. Accrued fees are credited
to `tokens_owed` in the position for later collection via `collect`. Fires
`before_burn` / `after_burn` hooks.

### `collect`
```
collect(sender, lower_tick, upper_tick, max_amount_0, max_amount_1) -> u32
```
Withdraw up to `max_amount_0` / `max_amount_1` of accrued fees from the
sender's position.

### `swap_exact_in`
```
swap_exact_in(sender, amount_in, min_amount_out, zero_for_one,
              sqrt_price_limit_q64_64, timestamp) -> u64
```
Execute a swap for an exact input amount. Returns `amount_out` (0 on failure).
- `zero_for_one = 1`: token0 → token1, price moves down
- `zero_for_one = 0`: token1 → token0, price moves up
- `sqrt_price_limit`: hard price boundary (slippage protection)
- `min_amount_out`: output floor; returns 0 if not met
- Also enforces the global `max_slippage_bps` cap
- Fires `before_swap` / `after_swap` hooks

### `observe`
```
observe(sender, seconds_agos_packed, timestamp) -> u64
```
Read TWAP oracle. `seconds_agos_packed` packs two `u32` windows into a `u64`
(low 32 bits = window 0, high 32 bits = window 1). Returns packed
`tick_cumulative` values. Divide cumulative difference by the time window to
get the TWAP tick.

### `increase_observation_cardinality`
```
increase_observation_cardinality(sender, next) -> u32
```
Grow the oracle ring buffer to hold `next` observations (max 65,535). This is
the only way to extend TWAP history depth.

### `collect_protocol`
```
collect_protocol(sender, max_amount_0, max_amount_1) -> u64
```
Owner-only. Withdraw accumulated protocol fees. Returns packed
`(collected_0 as u32) | (collected_1 as u32 << 32)`.

### `set_protocol_fee`
```
set_protocol_fee(sender, protocol_fee_share_bps) -> u32
```
Owner-only. Update the protocol's share of LP fees (max 2500 bps = 25%).

### `set_pause`
```
set_pause(sender, paused) -> u32
```
Owner-only. `paused = 1` blocks all swaps, mints, and burns. `paused = 0`
resumes. Emergency use only.

---

## Core Algorithms

### Swap Loop (`swap.rs`)

Implements the Uniswap v3 multi-tick traversal:

1. Find the next initialised tick in the swap direction using the bitmap.
2. Compute how far the price moves with the remaining input (`compute_swap_step`).
3. If price reaches the tick boundary: cross the tick (flip fee accumulators,
   apply `liquidity_net` to `liquidity_active`), continue.
4. If price does not reach the boundary: finish. The loop ends when input is
   exhausted, the price limit is hit, or 64 tick crossings have occurred.

The output is a `SwapResult` containing final price, tick, liquidity, fee
growth delta, protocol fee, and ticks crossed. `liquidity_active` is updated
from `SwapResult.liquidity_after` so multi-tick swaps are always correct.

### Fixed-Point Math (`math.rs`)

- **Price format**: `sqrt(price) * 2^64`, stored as `u128` (Q64.64).
- **Tick**: `floor(log_{1.0001}(price))`. The range is ±887,272 ticks,
  corresponding to a price range of roughly 0.000001× to 1,000,000×.
- **`compute_swap_step`**: Given current price, target price, liquidity, and
  remaining input, returns the new price, input consumed, output produced, and
  fee. Uses Q64.64 arithmetic throughout.
- **Fee**: deducted from input before computing output.
  `fee_amount = amount_in * fee_bps / 10_000`.

### TWAP Oracle (`oracle.rs`)

Implements Uniswap v3 §5: a circular ring buffer of `Observation` records.

Each observation stores `(timestamp, tick_cumulative, seconds_per_liquidity_q128)`.

**Write semantics**: At most one observation per block. If `advance_oracle` is
called twice in the same block, the second call is a no-op — the live tick
extends the last observation in `observe()`.

**Query semantics**: `observe()` accepts a list of `seconds_ago` windows and
returns interpolated cumulative values. Binary search locates the surrounding
checkpoints; linear interpolation fills in between.

**Precision note**: `seconds_per_liquidity` uses Q64 precision
(`(delta << 64) / liquidity`) rather than Q128, because `delta << 128` would
overflow `u128`. TWAP range queries remain correct because the imprecision is
consistent across all observations.

**Cardinality**: starts at 1. Call `increase_observation_cardinality` to
extend history. The buffer grows one slot at a time as new blocks arrive.

### Tick Crossing (`tick.rs`)

When the swap loop reaches a tick boundary, `TickMap::cross()` is called:

- Flips `fee_growth_outside_0` and `fee_growth_outside_1` relative to global.
- Flips `tick_cumulative_outside` and `seconds_per_liquidity_outside_q128`
  relative to current oracle values.
- Updates `seconds_outside`.
- Returns `liquidity_net` (signed), which is added to or subtracted from
  `liquidity_active` depending on swap direction.

### Fee Accounting

LP fees are tracked as global per-unit-liquidity accumulators (`fee_growth_global_*_q128`).

When a swap step produces a fee:
- Protocol cut = `fee_amount * protocol_fee_share_bps / 10_000`
- LP portion = `fee_amount - protocol_cut`
- LP fee growth increment = `(lp_fee << 64) / liquidity` (Q128 using Q64 precision)

Position fees are collected via the fee-growth-inside calculation:
`fee_inside = fee_global - fee_outside_lower - fee_outside_upper`

---

## Hook System (`hooks.rs`)

A lifecycle callback mechanism attached to the pool at initialisation time.
All hook implementations are compiled into the WASM binary — there are no
cross-contract calls.

### The `Hook` Trait

```rust
pub trait Hook: Sync {
    fn before_swap(&self, ctx: &HookContext, zero_for_one: bool, amount_in: u64)
        -> Result<(), ContractError>;
    fn after_swap(&self, ctx: &HookContext, outcome: &SwapOutcome)
        -> Result<(), ContractError>;
    fn before_mint(&self, ctx: &HookContext, lower: i32, upper: i32, delta: u128)
        -> Result<(), ContractError>;
    fn after_mint(&self, ctx: &HookContext, lower: i32, upper: i32, delta: u128)
        -> Result<(), ContractError>;
    fn before_burn(&self, ctx: &HookContext, lower: i32, upper: i32, delta: u128)
        -> Result<(), ContractError>;
    fn after_burn(&self, ctx: &HookContext, lower: i32, upper: i32, delta: u128)
        -> Result<(), ContractError>;
}
```

All methods default to `Ok(())`. A hook only overrides what it needs.
Returning `Err` from any `before_*` method aborts the operation — no state is
mutated.

### `HookContext`

A read-only snapshot passed to every hook call:

```rust
pub struct HookContext {
    pub current_tick: i32,
    pub sqrt_price: u128,
    pub liquidity: u128,
    pub fee_bps: u16,
}
```

### Built-in Hooks

| `HookId` | Byte | Behaviour |
|---|---|---|
| `None` | `0` | No-op on all lifecycle points |
| `ConservativeHedge` | `1` | See below |
| `YieldRebalance` | `2` | See below |

**`ConservativeHedge`**
- `before_swap`: rejects any swap where `amount_in > 5%` of `liquidity_active`.
  Prevents outsized single trades from causing disproportionate IL.
- `before_mint`: rejects positions narrower than 200 ticks. Ensures positions
  remain in-range across typical daily volatility and continue earning fees.

**`YieldRebalance`**
- `before_mint`: requires `lower < current_tick < upper`. Forces providers to
  keep their liquidity centred on the current price so it is always active and
  earning fees. Positions above or below the current price are rejected.

### Extending the Hook System

To add a new hook:
1. Implement `Hook` for a new zero-size struct.
2. Add a variant to `HookId` with the next byte value.
3. Add a `static` instance and one match arm in `get()`.
4. Add the `hook_id` byte encoding to `codec.rs` decode/encode for `ContractConfig`
   (it already delegates to `HookId::from_u8` / `to_u8`).

No other code changes are needed. The trait interface is stable.

### What Hooks Can and Cannot Do

**Can do:**
- Gate any operation (return `Err` to abort before state changes)
- Inspect full pool state via `HookContext`
- Enforce custom position geometry (tick width, centring requirements)
- Enforce custom swap size limits
- Implement any rule that can be computed from the pool state snapshot

**Cannot do:**
- Initiate new transactions (hooks fire inside an existing transaction)
- Call other contracts (no cross-contract calls in this deployment model)
- Accumulate their own persistent state (no per-hook storage allocation)
- Access external price data (only the pool's own TWAP is available)
- Execute autonomous rebalancing (the user must sign every new transaction)

---

## Execution Adapter (`adapter/src/lib.rs`)

An off-chain Rust crate that routes swap requests to one of two backends:

```
SwapRequest → DualPathAdapter → BedrockDirect  (primary)
                              → DirectXrpl     (fallback)
```

**`DualPathAdapter`** selects a path based on availability flags and
`prefer_bedrock`. If the primary path fails, it automatically retries on the
secondary path.

**Pre-submission validation** (both paths):
- Rejects `amount_in = 0`
- Enforces `min_amount_out >= 99% of amount_in` (1% max slippage)

In this MVP implementation both backends delegate to the same in-process
contract via `uniswap_v3_xrpl_contract::swap_exact_in`. In production,
`submit_bedrock` would serialise the call into a Bedrock transaction and
`submit_xrpl` would construct `AMMSwap` XRPL transactions.

---

## Persistence (`codec.rs`)

On WASM builds the entire `ContractState` is serialised to a single blob under
the storage key `b"amm_state_v1"` on every exported function exit, and
deserialised on entry. On native builds this is skipped; state lives in a
`thread_local`.

The format is little-endian, no padding. Fixed-size sections come first so the
variable-length maps (ticks, positions, oracle observations) can be appended
without a global length prefix.

| Section | Fixed bytes | Variable bytes |
|---|---|---|
| PoolState | 125 | — |
| ContractConfig | 28 | — |
| OracleBuffer header | 10 | 29 × cardinality |
| TickMap | 4 | 100 × tick count |
| TickBitmap | 4 | 34 × word count |
| PositionMap | 4 | 108 × position count |

---

## Error Codes

| Code | Variant | Meaning |
|---|---|---|
| 1 | `InvalidTickRange` | `lower >= upper`, or hook rejected range |
| 2 | `TickSpacingViolation` | Tick not a multiple of `tick_spacing` |
| 3 | `SlippageLimitExceeded` | Output below floor, price limit wrong direction, or hook blocked swap |
| 4 | `NotAuthorized` | Caller is not the owner |
| 5 | `Paused` | Contract is paused |
| 6 | `MathOverflow` | Arithmetic overflow in price/amount calculation |
| 7 | `InvalidLiquidityDelta` | Zero-amount swap or liquidity underflow |
| 8 | `PoolNotInitialized` | Operation called before `initialize_pool` |

---

## Explicit Limitations

The following are deliberate limitations of the current implementation, not
bugs:

- **Single pool**: one contract instance = one pool. Factory/multi-pool is out
  of scope.
- **No token transfers**: the contract tracks amounts but does not move tokens.
  The calling layer (Bedrock or XRPL) is responsible for actual asset custody.
- **No multi-hop routing**: swaps cross one pool only.
- **No autonomous rebalancing**: hooks can detect conditions but cannot
  initiate new transactions. Every action requires a user signature.
- **Hooks are internal**: third parties cannot deploy custom hooks without
  recompiling the binary. All hook logic must be in the crate.
- **Bedrock is a placeholder**: "Bedrock" is the project's working name for
  XRPL's smart-contract layer. The actual deployment target is unresolved.
  See `docs/bedrock.md`.

---

## Test Coverage

54 tests across all modules:

| Module | Tests | What is covered |
|---|---|---|
| `hooks` | 11 | All three hook variants; boundary conditions; HookId codec roundtrip |
| `oracle` | 8 | Init, write, observe (exact match, interpolation, live state), cardinality, idempotency |
| `codec` | 3 | State roundtrip; pool fields; tick map |
| `swap` | 6 | Price direction; fee accumulation; price limit; invalid limit |
| `math` | 6 | Tick ↔ price roundtrip; swap step; delta amounts; monotonicity |
| `tick` | 3 | Update, crossing, fee growth inside |
| `tick_bitmap` | 4 | Flip, find, search direction |
| `position` | 4 | Mint, burn, collect, underflow guard |
| `lib` (integration) | 9 | Full lifecycle; pause; slippage cap; oracle; protocol fees; auth |
| `adapter` | 5 | Path selection; fallback; slippage guard; zero input |

Run all tests:
```bash
cargo test
```

Build WASM:
```bash
cargo build --target wasm32-unknown-unknown --release -p uniswap-v3-xrpl-contract
```
