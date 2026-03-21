\# Bedrock Reference



\## Overview



\*\*Bedrock\*\* is a framework for deploying smart contracts to XRPL and other blockchains that don't natively support smart contracts.



\*\*Key Innovation:\*\* Uses \*\*WebAssembly (WASM)\*\* for cross-chain contract execution.



\*\*Status:\*\* Research this further — Bedrock is a placeholder name. The actual project may be different (e.g., Hooks, Codius, or a new framework).



\---



\## Architecture (Hypothetical)



\### 1. Contract Development (Rust)



Write contracts in Rust (compiles to WASM):



```rust

\#\[bedrock::contract]

pub struct AMMStrategy {

&#x20;   owner: Address,

&#x20;   executions: Vec<Execution>,

}



\#\[bedrock::call]

pub fn execute\_delta\_hedge(

&#x20;   \&mut self,

&#x20;   asset\_in: Asset,

&#x20;   asset\_out: Asset,

&#x20;   amount: u64,

&#x20;   max\_slippage: u16,

) -> Result<ExecutionSummary, Error> {

&#x20;   // Contract logic here

&#x20;   Ok(ExecutionSummary { ... })

}

```



\---



\### 2. Compilation



Compile to WASM:

```bash

cargo build-bedrock --target wasm32-unknown-unknown --release

```



Output: `amm\_strategy.wasm` (binary blob)



\---



\### 3. Deployment



Deploy to XRPL (via Bedrock CLI):

```bash

bedrock deploy \\

&#x20; --contract amm\_strategy.wasm \\

&#x20; --network xrpl-mainnet \\

&#x20; --signer wallet.json

```



\*\*Result:\*\* Contract address (e.g., `rContract123...`)



\---



\### 4. Invocation (Frontend)



Call contract from JavaScript:

```javascript

import { BedrockClient } from '@bedrock/sdk';



const bedrock = new BedrockClient({

&#x20; network: 'xrpl-mainnet',

&#x20; contractAddress: 'rContract123...'

});



const result = await bedrock.call({

&#x20; method: 'execute\_delta\_hedge',

&#x20; args: {

&#x20;   asset\_in: { XRP: {} },

&#x20;   asset\_out: { IssuedCurrency: { code: 'USD', issuer: 'rN7n...' } },

&#x20;   amount: 1000,

&#x20;   max\_slippage: 50,

&#x20; },

&#x20; signer: userWallet

});



console.log(result);

```



\---



\## Key Questions (Research Needed)



1\. \*\*Does Bedrock actually exist for XRPL?\*\*

&#x20;  - If not, alternatives: XRPL Hooks, Codius, or custom XRPL transaction batching



2\. \*\*How does WASM interact with XRPL ledger?\*\*

&#x20;  - Can it call `AMMDeposit`, `AMMWithdraw` directly?

&#x20;  - Or does it emit events that a relayer picks up?



3\. \*\*What's the gas/fee model?\*\*

&#x20;  - Per-call fee? Subscription? Free (subsidized)?



4\. \*\*Is there a testnet?\*\*

&#x20;  - Critical for safe development



5\. \*\*What's the upgrade mechanism?\*\*

&#x20;  - Immutable contracts? Multi-sig upgrades?



\---



\## Alternative: XRPL Hooks



\*\*XRPL Hooks\*\* (XLS-38d) are native smart contracts on XRPL.



\*\*Key Differences:\*\*

\- Written in C (compiles to WASM)

\- Triggered by XRPL transactions (e.g., Payment, AMMDeposit)

\- No external invocation (must be triggered by on-ledger event)



\*\*Example Hook:\*\*

```c

\#include "hookapi.h"



int64\_t hook(uint32\_t reserved) {

&#x20;   // Triggered when user sends Payment

&#x20;   // Can automatically rebalance AMM position

&#x20;   return 0;

}

```



\*\*Use Case for Our System:\*\*

\- Hook listens for `Payment` to contract account

\- Payment memo contains strategy params (JSON)

\- Hook executes AMM operations (swap, deposit, withdraw)

\- Emits result as transaction metadata



\*\*Pros:\*\*

\- Native to XRPL (no external relayer)

\- Low latency (on-ledger execution)



\*\*Cons:\*\*

\- Limited to C/WASM (no Rust yet)

\- Harder to debug than off-chain backend



\---



\## Recommended Approach (Until Research Complete)



\### Hybrid Architecture



\*\*Phase 1 (MVP):\*\*

1\. Backend API handles all AMM logic

2\. User signs XRPL transactions directly (via Xaman wallet)

3\. No smart contract (just transaction batching)



\*\*Phase 2 (If Bedrock/Hooks are viable):\*\*

1\. Deploy smart contract for complex strategies

2\. Use contract for multi-step atomic operations

3\. Keep simple swaps as direct XRPL transactions



\*\*Why Hybrid:\*\*

\- Reduces dependency on unproven tech

\- Gives us time to research Bedrock/Hooks

\- Users can start trading immediately (no contract deployment risk)



\---



\## Next Steps



1\. \*\*Research actual XRPL smart contract options:\*\*

&#x20;  - XRPL Hooks (XLS-38d)

&#x20;  - Codius (defunct?)

&#x20;  - Flare Network (EVM-compatible, XRPL bridge)

&#x20;  - Custom transaction batching (no contract needed)



2\. \*\*Prototype "Hello World" contract:\*\*

&#x20;  - Deploy to XRPL testnet

&#x20;  - Call from JavaScript

&#x20;  - Verify it can interact with AMM



3\. \*\*Benchmark latency:\*\*

&#x20;  - Direct XRPL transaction: <5s

&#x20;  - Contract invocation: ?

&#x20;  - If contract adds >2s, stick with direct transactions



4\. \*\*Evaluate cost:\*\*

&#x20;  - XRPL transaction fee: \~$0.000005

&#x20;  - Contract call fee: ?

&#x20;  - If contract is 100x more expensive, may not be worth it



\---



\## References (To Verify)



\- \*\*XRPL Hooks:\*\* https://xrpl-hooks.readme.io/

\- \*\*XLS-38d Spec:\*\* https://github.com/XRPLF/XRPL-Standards/discussions/93

\- \*\*Codius (archived):\*\* https://github.com/codius/codius

\- \*\*Flare Network:\*\* https://flare.network/ (XRPL bridge, EVM contracts)



