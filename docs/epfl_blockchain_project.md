# EPFL — Financial Applications of Blockchains and Distributed Ledgers  
## Final Project — March 2026

**College of Management of Technology | Financial Engineering**  

- **Team size:** 4–5 students  
- **Deadline:** 22 May 2026, 23:59 CET  
- **Submission:** Moodle + email (dimitrios.karyampas@epfl.ch)  

---

# Project Overview

## Motivation

Decentralised exchanges built on Automated Market Maker (AMM) protocols have fundamentally altered how financial liquidity is provided and consumed.  

Unlike centralised limit order books, AMMs such as **Uniswap V3** rely on passive liquidity provision across discrete price ranges. Execution quality is shaped by liquidity distribution rather than active quote competition.

---

## The Pool Under Study

| Parameter | Value |
|----------|------|
| Protocol | Uniswap V3 |
| Pair | USDC / WETH |
| Fee tier | 0.05% (tick spacing = 10) |
| Contract address | `0x88e6A0c2dDD26FEEb64F039a2c41296FcB3f5640` |
| Network | Ethereum Mainnet |
| Token0 | USDC (6 decimals) |
| Token1 | WETH (18 decimals) |

---

## Study Window

At least a **six-month historical observation window** (e.g., 1 Oct 2025 – 31 Mar 2026).

---

## Project Structure

| Module | Title | Weight |
|--------|------|--------|
| 1 | On-Chain Data Extraction | 20% |
| 2 | Liquidity Distribution Analysis | 20% |
| 3 | Slippage Simulation and Execution Cost | 20% |
| 4 | Liquidity Provision Analytics | 25% |
| 5 | Dynamic Hedging of Impermanent Loss | 15% |

---

## Overall Deliverables

| # | Deliverable | Description |
|--|------------|-------------|
| 1 | Codebase | Python repo covering all modules |
| 2 | Report | Research report with labeled figures |

**Note:** All on-chain data must be extracted via **direct RPC calls**.

---

# Module 1 — On-Chain Data Extraction

## Goal
Reconstruct contract state from raw Ethereum data.

## Required Outputs

### 1. `swap_events.parquet`
Columns include:
- block_number, timestamp
- tx hash
- signed USDC/WETH amounts (raw + adjusted)
- price, liquidity, tick
- trade direction
- USD notional

---

### 2. `mint_burn_events.parquet`
Full history since deployment.

Columns:
- block info
- event type (mint/burn)
- LP address
- tick range
- liquidity + token amounts

---

### 3. `liquidity_snapshots.parquet`
Daily reconstruction of liquidity map L(tick)

Columns:
- snapshot block/time
- tick index
- liquidityNet / liquidityGross
- active liquidity
- price bounds

---

### 4. `slot0_snapshots.parquet`
Daily pool state via `slot0()`.

Columns:
- price (sqrtPriceX96 + human-readable)
- tick
- observation index
- unlocked flag

---

# Module 2 — Liquidity Distribution Analysis

## Tasks
- Liquidity profiles
- TVL decomposition
- Concentration metrics (ILR, HHI)

---

# Module 3 — Slippage Simulation

## Tasks
- Implement Uniswap V3 swap simulator
- Run simulation grid
- Analyze price impact
- Compute effective spread

---

# Module 4 — Liquidity Provision Analytics

## Tasks
- Define LP positions
- Compute fees
- Compute impermanent loss

---

# Module 5 — Dynamic Hedging

## Tasks
- LP payoff + delta + gamma
- Collect perp + funding data
- Backtest hedging strategies
- Analyze limitations

---

# Final Note

This is a full-stack quant + crypto project combining:
- On-chain data engineering  
- Microstructure modeling  
- Simulation  
- Derivatives hedging  
