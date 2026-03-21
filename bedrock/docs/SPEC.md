# XRPL-in-Time: Chain Specification

**Version:** 0.1
**Status:** Active Design
**Owner:** Alex
**Last Updated:** 2026-03-21

---

## 1. What Is XRPL-in-Time?

XRPL-in-Time is an isolated, state-forked execution environment seeded from a live XRPL mainnet snapshot and evolved exclusively by VEGA-informed user decisions. It is not a fork of XRPL's ledger software. It is a Bedrock WASM smart contract environment whose initial state is derived from XRPL mainnet via the XRPL Commons Firehose data stream, and whose subsequent state transitions are restricted to a defined set of user-authorised financial operations.

The environment serves two purposes:

1. **Decision validation** — simulate a proposed LP position or borrow/lend action against real-world state before committing to mainnet
2. **Risk quantification** — run price-path scenarios through the V3 pool to compute IL, fee yield, and break-even without touching real funds

After a user authorises execution, the resulting transaction is submitted to XRPL mainnet. The XRPL-in-Time instance for that session is discarded. Each new session forks fresh from the latest mainnet snapshot.

---

## 2. Formal Invariants

The following invariants must hold at all times. They are ordered from highest to lowest architectural priority.

### INV-1: State Origin

The initial state of XRPL-in-Time MUST be derived exclusively from a verified XRPL mainnet ledger snapshot obtained via the XRPL Commons Firehose stream. No state is hand-authored or mocked for production sessions. The specific ledger index used for seeding MUST be recorded and accessible for auditability.

> **Scope note:** "XRPL Commons" refers to the organisation and tooling that provides the Firehose stream. The source of truth is XRPL mainnet ledger state, not any XRPL Commons-specific chain or environment.

### INV-2: State Isolation After Fork

After initialisation from the snapshot, XRPL-in-Time MUST NOT accept any inbound state updates from XRPL mainnet. Price does not re-anchor to mainnet mid-session. New liquidity from external actors does not enter. The environment diverges from mainnet immediately and intentionally upon fork. This divergence is the feature.

### INV-3: Token Pair

The AMM pool MUST be instantiated with exactly two tokens:

| Role | Asset | Type |
|---|---|---|
| Token 0 | **XRP** | Native (`AssetKind::Xrp`) |
| Token 1 | **RLUSD** | Issued (`AssetKind::Issued`, issuer: Ripple) |

All LP, borrow, and lend operations within XRPL-in-Time are denominated in this pair. No other pairs are in scope for the initial version.

> **Rationale:** XRP/RLUSD is the highest-liquidity institutionally credible pair on XRPL mainnet as of Q1 2026. RLUSD is Ripple's regulated USD stablecoin, providing a stable reference asset for IL and VaR calculations.

### INV-4: AMM Mathematics

The AMM pool MUST implement Uniswap V3 concentrated liquidity mathematics. Specifically:

- Price space represented as `sqrt_price_q64_64: u128` in Q64.64 fixed-point format
- Tick-based price discretisation with base `1.0001` (i.e. `price(i) = 1.0001^i`)
- Per-position concentrated liquidity ranges defined by `(lower_tick, upper_tick)`
- Fee growth accumulators computed per-position, not pool-wide
- Multi-tick swap loop with bitmap-accelerated tick traversal
- No floating-point arithmetic anywhere in state transition logic

This is V3-inspired, not EVM-identical. Settlement routes through Bedrock, not Ethereum. See [uniswap_v3_xrpl_protocol_spec.md](uniswap_v3_xrpl_protocol_spec.md) for the full mathematical specification.

### INV-5: Permitted State Mutations

State transitions in XRPL-in-Time fall into three categories. Only Category A and Category B mutations are permitted in a live session. Category C mutations are gated behind the removable admin interface defined in Section 5.

**Category A — User LP Operations (priority scope)**
- `mint` — add concentrated liquidity to a tick range
- `burn` — remove liquidity from a position
- `collect` — claim accrued fees from a position

**Category B — User Borrow/Lend Operations (extended scope)**
- `deposit_collateral` — lock XRP or RLUSD as collateral
- `borrow` — draw a debt position against deposited collateral
- `repay` — reduce or close a debt position
- `withdraw_collateral` — recover collateral after repayment (subject to health factor)
- `liquidate` — third-party liquidation of undercollateralised position (permissionless)

**Category C — Admin Operations (removable, see Section 5)**
- `set_protocol_fee` — adjust protocol fee share
- `set_pause` — freeze/unfreeze state-mutating operations

> **Note on swaps:** `swap_exact_in` currently mutates pool state (price, tick, fee growth). Within XRPL-in-Time, swaps are used as a simulation primitive by VEGA to model price-path scenarios. Whether external users can initiate swaps directly is an open design decision and MUST be resolved before mainnet deployment.

### INV-6: Programmability

The environment MUST be programmable. Programmability is provided by the Bedrock WASM runtime. The contract compiles to `wasm32-unknown-unknown` and is executed by a Bedrock node (local or alphanet). This satisfies the programmability requirement that XRPL mainnet cannot meet natively.

---

## 3. Chain Initialisation

### 3.1 State Extraction from Firehose

The following XRPL ledger objects MUST be extracted at the snapshot ledger index:

| XRPL Object | Field | Maps To |
|---|---|---|
| `AMM` (XRP/RLUSD pool) | `Amount` | `reserve_xrp` |
| `AMM` (XRP/RLUSD pool) | `Amount2` | `reserve_rlusd` |
| `AMM` (XRP/RLUSD pool) | `LPTokenBalance` | reference only |
| `AccountRoot` (user) | XRP balance | user Token 0 balance |
| `RippleState` (user/RLUSD) | `Balance` | user Token 1 balance |

All other XRPL state (offers, escrows, NFTs, other AMM pools, other trust lines) is out of scope and MUST NOT be loaded.

> **Interim:** Until the Firehose hosted service is available (target Q3 2026), state extraction uses XRPL JSON-RPC (`amm_info`, `account_info`, `account_lines`). The extraction interface MUST be abstracted so Firehose drops in without changing downstream code.

### 3.2 Pool Seeding

From the extracted reserves, derive the initial `sqrt_price_q64_64`:

```
sqrt_price_q64_64 = floor( sqrt(reserve_rlusd / reserve_xrp) * 2^64 )
```

Call `initialize_pool` with:
- `sqrt_price`: derived above
- `fee_bps`: match the live XRPL AMM fee at snapshot time (default 30 = 0.30%)
- `protocol_fee_bps`: 0 for simulation sessions; configurable for production

Seed initial liquidity as a full-range position (`tickLower = -887272`, `tickUpper = 887272`) using the snapshot reserves. This is mathematically equivalent to the constant-product V2 pool on mainnet and preserves the real price impact profile. Users and VEGA then add concentrated positions on top.

---

## 4. AMM Pool: LP Scope (Priority)

### 4.1 Position Lifecycle

Each LP position is identified by the key `(owner: AccountId, lower_tick: i32, upper_tick: i32)`. Positions are non-fungible. The complete lifecycle is:

```
mint(lower_tick, upper_tick, liquidity_delta)
    → returns (amount0_required, amount1_required)

[price moves, fees accrue]

collect(lower_tick, upper_tick, max_amount_0, max_amount_1)
    → returns (fees_collected_0, fees_collected_1)

burn(lower_tick, upper_tick, liquidity_delta)
    → returns (amount0_returned, amount1_returned)
```

### 4.2 What VEGA Computes from Position State

After a `mint` is simulated:

| Metric | Source |
|---|---|
| Required capital (amount0, amount1) | Return values of `mint` |
| IL at price P | `burn` at simulated price P, compare to HODL value |
| Fee yield | `fee_growth_global` delta across simulated swap volume |
| Break-even price | Price at which `fee_yield = IL` |
| Active range probability | Requires volatility estimate (VEGA quant layer, not contract) |

### 4.3 Single-Asset vs Multi-Asset Deposit

Single-asset deposit on XRPL AMM is mechanically a swap of half the input into the other asset followed by a deposit. In XRPL-in-Time, this is modelled as `swap_exact_in` followed by `mint`. The swap spread is a real cost VEGA MUST include in the IL/fee break-even calculation.

Multi-asset deposit maps directly to `mint` with both tokens provided at the current pool ratio.

---

## 5. Admin Interface: Access Controls (Removable)

### 5.1 Motivation

The current `ContractConfig` bundles `owner`, `paused`, and `max_slippage_bps` into a single struct with no separation between safety controls and configuration. Owner-gated operations (`set_protocol_fee`, `set_pause`) mutate state through the same interface as user operations. This is acceptable for development but creates a governance risk in production: if the admin interface is undesirable (e.g. the team decides the contract should be fully immutable), there is currently no clean path to remove it.

### 5.2 Requirement

Admin operations MUST be separated into a discrete `AdminInterface` that:

1. Is invoked through a separate, clearly named entry point (not mixed with user-facing functions)
2. Can be **disabled entirely** by setting `admin_enabled: bool = false` — after which all admin entry points return `NotAuthorized` unconditionally
3. Can be **transferred** to a new owner address (for multi-sig or DAO upgrade path)
4. Can be **renounced** by setting owner to the zero address, making the contract permanently immutable

### 5.3 Admin-Controlled Parameters

| Parameter | Current Location | Risk If Abused |
|---|---|---|
| `protocol_fee_share_bps` | `set_protocol_fee` | Fee drain from LPs |
| `paused` | `set_pause` | Denial of service |
| `max_slippage_bps` | `ContractConfig` (init only) | Forced high slippage |

### 5.4 Proposed Separation

```
// User-facing interface (always available if pool initialised and not paused)
mint(...)
burn(...)
collect(...)
swap_exact_in(...)
deposit_collateral(...)   // borrow/lend scope
borrow(...)               // borrow/lend scope
repay(...)                // borrow/lend scope
withdraw_collateral(...)  // borrow/lend scope
liquidate(...)            // permissionless, always available

// Admin interface (gated by admin_enabled flag)
admin_set_protocol_fee(sender, bps)
admin_set_pause(sender, paused)
admin_transfer_ownership(sender, new_owner)
admin_renounce_ownership(sender)
admin_disable_interface(sender)   // one-way: sets admin_enabled = false permanently
```

The `admin_disable_interface` function is a one-way latch. Once called, no admin operation can ever be invoked again. This gives developers a clear path to ship with admin controls during development and remove them for production trustlessness.

---

## 6. Borrow/Lend Scope (Extended)

LP is the priority. Borrow/lend is added to scope with the following requirements. Implementation follows LP completion.

### 6.1 Model

A simple overcollateralised lending model:

- Users deposit XRP or RLUSD as collateral
- Users borrow the opposite asset against that collateral
- Positions accrue interest at a rate determined by utilisation
- Positions below the liquidation threshold (`health_factor < 1.0`) can be liquidated by any caller

### 6.2 Required State

```
CollateralPosition {
    owner: AccountId,
    asset: Asset,
    amount: u128,
}

DebtPosition {
    owner: AccountId,
    asset: Asset,
    principal: u128,
    interest_accumulated: u128,
    last_updated_ledger: u64,
}
```

### 6.3 Risk Parameters (to be calibrated)

| Parameter | Description | Initial Value (TBD) |
|---|---|---|
| `max_ltv` | Max loan-to-value ratio at origination | ~70% |
| `liquidation_threshold` | Health factor below which liquidation is permitted | 1.0 |
| `liquidation_bonus` | Bonus paid to liquidator | ~5% |
| `interest_rate_model` | Utilisation-based (linear or kinked) | TBD |

### 6.4 What VEGA Computes for Borrow/Lend

| Metric | Source |
|---|---|
| Liquidation price | Derived from collateral value and debt at threshold |
| Probability of liquidation | Requires XRP/RLUSD volatility model (VEGA quant layer) |
| Effective borrowing cost | Interest rate × duration estimate |
| Health factor over time | VaR-style simulation of collateral price paths |

---

## 7. Session Lifecycle

```
1. User opens VEGA session
        ↓
2. Extract XRPL mainnet state via JSON-RPC / Firehose
        ↓
3. initialize_pool(sqrt_price_from_reserves, fee_bps)
   seed_full_range_liquidity(reserves)
        ↓
4. VEGA simulates recommended position:
   - mint(tickLower, tickUpper, liquidity)
   - simulate price paths via swap_exact_in
   - compute IL, fee yield, break-even
        ↓
5. User reviews risk report and chooses strategy
        ↓
6. User authorises → transaction submitted to XRPL mainnet
        ↓
7. XRPL-in-Time session discarded
   Next session: fresh fork from latest snapshot
```

---

## 8. Out of Scope (Current Version)

| Feature | Reason |
|---|---|
| Multiple token pairs | Scope: XRP/RLUSD only |
| Cross-pool routing / multi-hop swaps | Single pool per contract instance |
| Flash loans | Callback model not suitable for XRPL |
| Oracle (TWAP) accumulation | Infrastructure present in tick state; not activated |
| NFT position tokens | XRPL NFT support exists; deferred post-MVP |
| DAO / governance token | Out of scope for hackathon |
| Protocol fee claiming | Admin interface handles this; distribution deferred |

---

## 9. Open Questions

1. **Swaps by external users:** Should `swap_exact_in` be callable by non-VEGA actors during a live session, or is it restricted to VEGA's simulation use only?
2. **Multi-user sessions:** If two users fork from the same snapshot, do they share a single pool instance (shared fee accrual) or get isolated instances? Currently: isolated instances recommended.
3. **Interest rate model for borrow/lend:** Linear (simple) or kinked (Aave-style)? Depends on target utilisation profile.
4. **Liquidation in simulation context:** Should `liquidate` be callable in XRPL-in-Time sessions, or is it only meaningful on mainnet? Useful for stress-testing collateral positions.

---

## 10. References

| Document | Location |
|---|---|
| V3 protocol math spec | [uniswap_v3_xrpl_protocol_spec.md](uniswap_v3_xrpl_protocol_spec.md) |
| Bedrock runtime overview | [bedrock.md](bedrock.md) |
| Execution adapter design | [uniswap_v3_xrpl_execution_adapter.md](uniswap_v3_xrpl_execution_adapter.md) |
| Core module implementation | [uniswap_v3_xrpl_core_modules.md](uniswap_v3_xrpl_core_modules.md) |
| Rollout and safety plan | [uniswap_v3_xrpl_rollout_safety.md](uniswap_v3_xrpl_rollout_safety.md) |
| XRPL AMM reference | [../../references/XRPL-AMM.md](../../references/XRPL-AMM.md) |
| Project overview | [../../PROJECT.md](../../PROJECT.md) |
