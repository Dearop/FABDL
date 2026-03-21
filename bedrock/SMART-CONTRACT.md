> **DEPRECATED** — This file is the original pre-implementation design sketch.
> It describes intended architecture before any code existed and is no longer accurate.
> The authoritative reference is [`docs/IMPLEMENTATION.md`](docs/IMPLEMENTATION.md).

\# Bedrock Smart Contract Design



\## Overview



A Rust-based smart contract deployed via Bedrock that executes quantitative trading strategies on XRPL native AMM pools.



\*\*Core Function:\*\*

```rust

pub fn execute\_strategy(

&#x20;   asset\_in: Asset,

&#x20;   asset\_out: Asset,

&#x20;   amount: u64,

&#x20;   max\_slippage: u16,  // basis points (e.g., 50 = 0.5%)

&#x20;   strategy\_type: StrategyType

) -> Result<ExecutionSummary, ContractError>

```



\---



\## Contract Architecture



\### State

```rust

pub struct AMMStrategyContract {

&#x20;   owner: AccountId,

&#x20;   approved\_strategies: Vec<StrategyType>,

&#x20;   execution\_log: Vec<Execution>,

&#x20;   paused: bool,

&#x20;   slippage\_limit: u16,  // Global max (e.g., 100 = 1%)

}

```



\### Strategy Types

```rust

pub enum StrategyType {

&#x20;   DeltaHedge,        // Reduce directional exposure

&#x20;   Rebalance,         // Shift to different pool

&#x20;   Exit,              // Full withdrawal

&#x20;   DoNothing,         // No-op (for logging intent)

}

```



\### Execution Record

```rust

pub struct Execution {

&#x20;   timestamp: u64,

&#x20;   user: AccountId,

&#x20;   strategy: StrategyType,

&#x20;   asset\_in: Asset,

&#x20;   asset\_out: Asset,

&#x20;   amount\_in: u64,

&#x20;   amount\_out: u64,

&#x20;   actual\_slippage: u16,

&#x20;   tx\_hash: Hash,

}

```



\---



\## Core Logic



\### 1. Delta Hedge Strategy



\*\*Goal:\*\* Reduce exposure to price movements



\*\*Implementation:\*\*

```rust

fn execute\_delta\_hedge(

&#x20;   ctx: \&mut Context,

&#x20;   asset\_in: Asset,  // e.g., XRP

&#x20;   asset\_out: Asset, // e.g., USD

&#x20;   amount: u64,

&#x20;   max\_slippage: u16

) -> Result<ExecutionSummary, ContractError> {

&#x20;   // Validate inputs

&#x20;   require!(amount > 0, "Amount must be positive");

&#x20;   require!(max\_slippage <= ctx.state.slippage\_limit, "Slippage too high");



&#x20;   // Fetch current AMM state

&#x20;   let amm\_info = xrpl::get\_amm\_info(asset\_in, asset\_out)?;



&#x20;   // Calculate expected output

&#x20;   let expected\_out = amm\_info.calc\_swap\_output(amount);

&#x20;   let min\_out = expected\_out \* (10000 - max\_slippage) / 10000;



&#x20;   // Execute swap via XRPL AMM

&#x20;   let result = xrpl::amm\_swap(

&#x20;       asset\_in,

&#x20;       asset\_out,

&#x20;       amount,

&#x20;       min\_out,

&#x20;       ctx.sender

&#x20;   )?;



&#x20;   // Log execution

&#x20;   ctx.state.execution\_log.push(Execution {

&#x20;       timestamp: ctx.block\_time,

&#x20;       user: ctx.sender,

&#x20;       strategy: StrategyType::DeltaHedge,

&#x20;       asset\_in,

&#x20;       asset\_out,

&#x20;       amount\_in: amount,

&#x20;       amount\_out: result.amount\_out,

&#x20;       actual\_slippage: calc\_slippage(expected\_out, result.amount\_out),

&#x20;       tx\_hash: result.tx\_hash,

&#x20;   });



&#x20;   Ok(ExecutionSummary {

&#x20;       success: true,

&#x20;       amount\_out: result.amount\_out,

&#x20;       fee\_paid: result.fee,

&#x20;   })

}

```



\---



\### 2. Rebalance Strategy



\*\*Goal:\*\* Move liquidity to a different AMM pool



\*\*Steps:\*\*

1\. Withdraw from current pool (AMM → assets)

2\. Deposit into target pool (assets → AMM)



\*\*Implementation:\*\*

```rust

fn execute\_rebalance(

&#x20;   ctx: \&mut Context,

&#x20;   from\_pool: AMMPool,

&#x20;   to\_pool: AMMPool,

&#x20;   lp\_tokens: u64,

&#x20;   max\_slippage: u16

) -> Result<ExecutionSummary, ContractError> {

&#x20;   // Step 1: Withdraw from current pool

&#x20;   let (asset1\_out, asset2\_out) = xrpl::amm\_withdraw(

&#x20;       from\_pool,

&#x20;       lp\_tokens,

&#x20;       ctx.sender

&#x20;   )?;



&#x20;   // Step 2: Calculate optimal deposit amounts for target pool

&#x20;   let to\_amm\_info = xrpl::get\_amm\_info(to\_pool.asset1, to\_pool.asset2)?;

&#x20;   let (deposit1, deposit2) = to\_amm\_info.calc\_balanced\_deposit(

&#x20;       asset1\_out,

&#x20;       asset2\_out

&#x20;   );



&#x20;   // Step 3: Deposit into target pool

&#x20;   let new\_lp\_tokens = xrpl::amm\_deposit(

&#x20;       to\_pool,

&#x20;       deposit1,

&#x20;       deposit2,

&#x20;       ctx.sender

&#x20;   )?;



&#x20;   // Log execution

&#x20;   ctx.state.execution\_log.push(Execution {

&#x20;       timestamp: ctx.block\_time,

&#x20;       user: ctx.sender,

&#x20;       strategy: StrategyType::Rebalance,

&#x20;       // ... other fields

&#x20;   });



&#x20;   Ok(ExecutionSummary {

&#x20;       success: true,

&#x20;       new\_lp\_tokens,

&#x20;       // ... other fields

&#x20;   })

}

```



\---



\### 3. Exit Strategy



\*\*Goal:\*\* Full withdrawal from AMM



\*\*Implementation:\*\*

```rust

fn execute\_exit(

&#x20;   ctx: \&mut Context,

&#x20;   pool: AMMPool,

&#x20;   lp\_tokens: u64

) -> Result<ExecutionSummary, ContractError> {

&#x20;   // Withdraw all LP tokens

&#x20;   let (asset1\_out, asset2\_out) = xrpl::amm\_withdraw(

&#x20;       pool,

&#x20;       lp\_tokens,

&#x20;       ctx.sender

&#x20;   )?;



&#x20;   // Transfer assets back to user

&#x20;   xrpl::transfer(asset1\_out, ctx.sender)?;

&#x20;   xrpl::transfer(asset2\_out, ctx.sender)?;



&#x20;   Ok(ExecutionSummary {

&#x20;       success: true,

&#x20;       assets\_returned: vec!\[asset1\_out, asset2\_out],

&#x20;   })

}

```



\---



\## Security Features



\### 1. Slippage Protection



\*\*Hard Limits:\*\*

\- Per-transaction max: 1% (100 basis points)

\- User-specified max: Must be ≤ global limit

\- Revert if actual slippage exceeds limit



\*\*Implementation:\*\*

```rust

fn calc\_slippage(expected: u64, actual: u64) -> u16 {

&#x20;   let diff = if expected > actual {

&#x20;       expected - actual

&#x20;   } else {

&#x20;       0

&#x20;   };

&#x20;   ((diff \* 10000) / expected) as u16

}

```



\---



\### 2. Signature Verification



\*\*Requirement:\*\* Every strategy execution must be explicitly signed by the user



\*\*Flow:\*\*

1\. Frontend constructs transaction payload

2\. User signs with wallet (Xaman, Crossmark)

3\. Contract verifies signature matches `ctx.sender`

4\. Execute only if valid



\---



\### 3. Pause Mechanism



\*\*Use Case:\*\* Emergency stop (e.g., XRPL AMM bug discovered)



\*\*Implementation:\*\*

```rust

pub fn pause(\&mut self, ctx: \&Context) -> Result<(), ContractError> {

&#x20;   require!(ctx.sender == self.owner, "Only owner can pause");

&#x20;   self.paused = true;

&#x20;   Ok(())

}



pub fn unpause(\&mut self, ctx: \&Context) -> Result<(), ContractError> {

&#x20;   require!(ctx.sender == self.owner, "Only owner can unpause");

&#x20;   self.paused = false;

&#x20;   Ok(())

}

```



\*\*Check in all execution paths:\*\*

```rust

require!(!ctx.state.paused, "Contract is paused");

```



\---



\### 4. Strategy Allowlist



\*\*Goal:\*\* Prevent LLM from executing arbitrary/untested strategies



\*\*Implementation:\*\*

```rust

pub fn add\_strategy(\&mut self, ctx: \&Context, strategy: StrategyType) -> Result<(), ContractError> {

&#x20;   require!(ctx.sender == self.owner, "Only owner can add strategies");

&#x20;   self.approved\_strategies.push(strategy);

&#x20;   Ok(())

}



fn require\_approved(\&self, strategy: StrategyType) -> Result<(), ContractError> {

&#x20;   require!(

&#x20;       self.approved\_strategies.contains(\&strategy),

&#x20;       "Strategy not approved"

&#x20;   );

&#x20;   Ok(())

}

```



\---



\## XRPL Integration



\### Native AMM Operations



\*\*XRPL supports AMM via ledger objects (not EVM contracts)\*\*



Bedrock must bridge to XRPL via:



\#### Option A: Direct XRPL Transaction Submission

```rust

// Submit XRPL transaction from Bedrock contract

xrpl::submit\_transaction(TxType::AMMDeposit {

&#x20;   account: ctx.sender,

&#x20;   asset: Currency::XRP,

&#x20;   asset2: Currency::USD("rIssuer..."),

&#x20;   amount: 1000,

&#x20;   amount2: 500,

});

```



\#### Option B: Bedrock ↔ XRPL Hook

\- Deploy XRPL Hook that listens for Bedrock events

\- Bedrock emits event: `AMMStrategyRequested`

\- XRPL Hook executes corresponding AMM operation



\*\*Preferred:\*\* Option A (if Bedrock supports direct XRPL calls)



\---



\### Asset Representation



\*\*XRPL has native XRP + issued currencies\*\*



```rust

pub enum Asset {

&#x20;   XRP,

&#x20;   IssuedCurrency {

&#x20;       code: String,      // e.g., "USD"

&#x20;       issuer: AccountId, // e.g., "rN7n7otQDd6FczFgLdlqtyMVrn3NnrcVcN"

&#x20;   },

}

```



\*\*Example:\*\*

```rust

let xrp = Asset::XRP;

let usd = Asset::IssuedCurrency {

&#x20;   code: "USD".to\_string(),

&#x20;   issuer: "rN7n7otQDd6FczFgLdlqtyMVrn3NnrcVcN".parse()?,

};

```



\---



\## Gas Optimization



\*\*XRPL transactions are cheap (\~0.00001 XRP = $0.00001)\*\*



But Bedrock execution might have its own costs.



\*\*Optimization Strategies:\*\*

1\. Batch multiple swaps into single transaction (if possible)

2\. Cache AMM pool state (avoid redundant ledger queries)

3\. Use efficient Rust data structures (`Vec` > `HashMap` for small datasets)



\---



\## Testing Strategy



\### Unit Tests

```rust

\#\[test]

fn test\_delta\_hedge\_slippage\_protection() {

&#x20;   let mut contract = setup\_contract();

&#x20;   let result = contract.execute\_delta\_hedge(

&#x20;       Asset::XRP,

&#x20;       Asset::USD,

&#x20;       1000,

&#x20;       50  // 0.5% slippage

&#x20;   );



&#x20;   // Should revert if actual slippage > 0.5%

&#x20;   assert!(result.is\_err());

}

```



\### Integration Tests

1\. Deploy contract to Bedrock testnet

2\. Submit strategy execution via `call.js`

3\. Query XRPL testnet to verify AMM state change

4\. Confirm execution log updated correctly



\### Fuzz Testing

\- Random asset pairs

\- Random amounts (0 to 1M XRP)

\- Random slippage (0 to 500 bps)

\- Ensure no panics, always either success or expected error



\---



\## Deployment



\### Prerequisites

1\. Bedrock CLI installed

2\. XRPL testnet wallet with XRP

3\. Rust toolchain (1.75+)



\### Steps

```bash

\# Build contract

cargo build-bedrock --release



\# Deploy to testnet

bedrock deploy \\

&#x20; --contract target/wasm32-unknown-unknown/release/amm\_strategy.wasm \\

&#x20; --network xrpl-testnet \\

&#x20; --signer wallet.json



\# Verify deployment

bedrock query contract <CONTRACT\_ADDRESS> get\_state



\### Mainnet Checklist

\- \[ ] Audit by 2+ security firms

\- \[ ] Formal verification of slippage logic

\- \[ ] Multi-sig owner (3-of-5)

\- \[ ] Gradual rollout (caps on TVL)

\- \[ ] Bug bounty program ($50k+)



\---



\## Frontend Integration



\### Bedrock `call.js` Example



```javascript

import { BedrockClient } from '@bedrock/sdk';



const bedrock = new BedrockClient({

&#x20; network: 'xrpl-mainnet',

&#x20; contractAddress: '0x...'

});



async function executeDeltaHedge(user, amount, maxSlippage) {

&#x20; const tx = await bedrock.call({

&#x20;   method: 'execute\_strategy',

&#x20;   args: {

&#x20;     asset\_in: { XRP: {} },

&#x20;     asset\_out: { IssuedCurrency: { code: 'USD', issuer: 'rN7n...' } },

&#x20;     amount: amount,

&#x20;     max\_slippage: maxSlippage,

&#x20;     strategy\_type: { DeltaHedge: {} }

&#x20;   },

&#x20;   signer: user.wallet

&#x20; });



&#x20; return tx.wait();  // Wait for XRPL confirmation

}

```



\---



\## Monitoring \& Analytics



\### On-Chain Events



\*\*Emit on every execution:\*\*

```rust

emit!(ExecutionEvent {

&#x20;   user: ctx.sender,

&#x20;   strategy: StrategyType::DeltaHedge,

&#x20;   amount\_in: 1000,

&#x20;   amount\_out: 498,

&#x20;   timestamp: ctx.block\_time

});

```



\*\*Subscribe in backend:\*\*

```javascript

bedrock.subscribe('ExecutionEvent', (event) => {

&#x20; console.log(`User ${event.user} executed ${event.strategy}`);

&#x20; updateAnalytics(event);

});

```



\---



\### Dashboard Metrics

\- Total volume traded (last 24h / 7d / 30d)

\- Most popular strategy (by count / by volume)

\- Average slippage (per strategy type)

\- Success rate (% of non-reverted txs)



\---



\## Open Questions



1\. \*\*Does Bedrock support direct XRPL transaction submission?\*\* (Or must we use Hooks?)

2\. \*\*What's the gas model?\*\* (Per-call fee? Subscription?)

3\. \*\*Can we batch multiple strategy steps into one transaction?\*\* (Atomicity)

4\. \*\*How do we handle XRPL account reserves?\*\* (20 XRP minimum balance)

5\. \*\*Is there a Bedrock testnet for XRPL?\*\* (For safe experimentation)



\---



\## Next Steps



1\. Research Bedrock docs: XRPL integration specifics

2\. Write minimal "Hello World" contract (deploy + call)

3\. Implement `execute\_delta\_hedge` (simplest strategy)

4\. Test on XRPL testnet with real AMM pool

5\. Build `call.js` wrapper for frontend integration



