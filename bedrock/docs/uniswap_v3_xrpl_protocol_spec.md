# Uniswap v3-Inspired Protocol Spec for Bedrock/XRPL

## 1. Purpose
Define a deterministic AMM protocol that preserves core Uniswap v3 mechanics (ticks, concentrated ranges, per-position fee accounting) while remaining executable on XRPL infrastructure.

## 2. Compatibility Model
- **Native XRPL AMM:** still constant-product full-range liquidity.
- **This protocol:** implements v3-like market state in contract storage and routes settlement through either Bedrock execution or direct XRPL operations.
- **Result:** behavior is v3-inspired, not protocol-identical to Ethereum Uniswap v3.

## 3. Price and Tick Model
- Tick base: `1.0001`.
- Tick index: signed integer `i`.
- Price mapping: `price(i) = 1.0001^i`.
- Internal execution uses `sqrt_price_q64_64` for numerical stability.
- Pool defines `tick_spacing`; only `i % tick_spacing == 0` ticks may initialize.
- `current_tick` tracks floor tick under current price.

## 4. Core State
### 4.1 Pool State
- `sqrt_price_q64_64: u128`
- `current_tick: i32`
- `liquidity_active: u128`
- `fee_bps: u16`
- `protocol_fee_share_bps: u16`
- `fee_growth_global_0_q128: u128`
- `fee_growth_global_1_q128: u128`
- `protocol_fees_0: u128`
- `protocol_fees_1: u128`

### 4.2 Tick State
- `liquidity_gross: u128`
- `liquidity_net: i128`
- `fee_growth_outside_0_q128: u128`
- `fee_growth_outside_1_q128: u128`
- `seconds_outside: u64`
- `tick_cumulative_outside: i128`
- `seconds_per_liquidity_outside_q128: u128`

### 4.3 Position State
- `owner: AccountId`
- `lower_tick: i32`
- `upper_tick: i32`
- `liquidity: u128`
- `fee_growth_inside_0_last_q128: u128`
- `fee_growth_inside_1_last_q128: u128`
- `tokens_owed_0: u128`
- `tokens_owed_1: u128`

## 5. Tick Initialization and Bitmap
- Maintain sparse tick map and 256-bit word bitmap index.
- On first position reference to a tick, initialize tick state and set bitmap bit.
- On full dereference (`liquidity_gross == 0`), clear bitmap bit.
- During swap traversal, lookup next initialized tick by bitmap scan in swap direction.

## 6. Mint / Burn / Collect Semantics
### Mint (`add_liquidity`)
1. Validate tick bounds and spacing.
2. Update position fees owed before changing liquidity.
3. Apply `liquidity_delta > 0` to:
   - lower tick `liquidity_net += delta`
   - upper tick `liquidity_net -= delta`
   - both ticks `liquidity_gross += delta`
4. If in range, update `liquidity_active`.
5. Compute required token deltas using v3 piecewise formulas.

### Burn (`remove_liquidity`)
1. Update owed fees.
2. Apply negative `liquidity_delta`.
3. Update tick and pool liquidity.
4. Move principal outputs to claimable balances.

### Collect (`collect_fees`)
- Transfers `tokens_owed_0/1` to user.
- Resets claimable balance counters.

## 7. Fee Accounting
- Swap fees accrue globally to `fee_growth_global_{0,1}`.
- Protocol share split by `protocol_fee_share_bps`.
- Position fee growth inside range:
  - `fee_growth_inside = fee_global - fee_below(lower) - fee_above(upper)`
- Owed delta:
  - `tokens_owed += liquidity * (fee_growth_inside_now - fee_growth_inside_last)`

## 8. Swap Execution
Swaps run in a loop of single-tick steps:
1. Find next initialized tick in direction.
2. Compute candidate next price from amount remaining and active liquidity.
3. If candidate stays before boundary, complete within current tick.
4. Else consume to boundary, cross tick, apply `liquidity_net`, continue.
5. Enforce user slippage constraints and max price impact guardrails.

### Crossing Rules
- On crossing tick:
  - flip outside accumulators (`fee_growth_outside`, `seconds_outside`, cumulative trackers).
  - update `liquidity_active +=/- liquidity_net` depending on direction.
  - set `current_tick` to crossed boundary.

## 9. Rounding and Determinism
- Use floor rounding for outputs, ceil for required inputs.
- Standardize fixed-point helpers and overflow checks.
- All math is deterministic and no floating-point operations are allowed in state transition logic.

## 10. Non-Portable Features and Approximations
1. **Native v3 NFT positions:** represented as storage entries keyed by `(owner, lower, upper)`; optional NFT wrapper is deferred.
2. **Oracle parity:** TWAP/liquidity oracle compatibility is provided at contract level, not native XRPL AMM level.
3. **Settlement:** depends on adapter path; behavior is preserved even if settlement backend differs.

## 11. Safety Invariants
- User-signed operation required.
- `max_slippage_bps <= global_slippage_cap_bps` always.
- Pool math must preserve non-negative reserves and non-negative liquidity.
- Tick crossing cannot skip initialized tick updates.
- Pause mode blocks state-mutating operations except governance emergency functions.
