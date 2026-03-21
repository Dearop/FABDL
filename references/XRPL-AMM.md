\# XRPL AMM Reference



\## Overview



XRPL introduced native AMM support in the XLS-30d amendment (activated Jan 2024).



\*\*Key Features:\*\*

\- Constant product formula (xy=k)

\- Automated market making (no order books)

\- LP tokens (fungible, transferable)

\- Auction slots (24h exclusive fee boost)

\- Voting mechanism (change trading fee)



\*\*Documentation:\*\* https://xrpl.org/docs/concepts/tokens/decentralized-exchange/automated-market-makers/



\---



\## Core Ledger Objects



\### AMMObject

Represents an AMM instance on the ledger.



\*\*Fields:\*\*

\- `Account`: AMM account address

\- `Asset`: First asset in pair (e.g., XRP)

\- `Asset2`: Second asset in pair (e.g., USD)

\- `TradingFee`: Current fee in basis points (0-1000, default 500 = 0.5%)

\- `LPTokenBalance`: Total LP tokens issued

\- `VoteSlots`: Array of accounts that voted on fee



\*\*Example:\*\*

```json

{

&#x20; "Account": "rE54zDvgnghAoPopCgvtiqWNq3dU5y836S",

&#x20; "Asset": {"currency": "XRP"},

&#x20; "Asset2": {"currency": "USD", "issuer": "rN7n..."},

&#x20; "TradingFee": 500,

&#x20; "LPTokenBalance": {

&#x20;   "currency": "03930D02208264E2E40EC1B0C09E4DB96EE197B1",

&#x20;   "issuer": "rE54zDvgnghAoPopCgvtiqWNq3dU5y836S",

&#x20;   "value": "10000"

&#x20; }

}

```



\---



\## Transaction Types



\### 1. AMMCreate

Create a new AMM instance.



\*\*Parameters:\*\*

\- `Asset`: First asset

\- `Asset2`: Second asset

\- `TradingFee`: Initial fee (0-1000 bps)

\- `Amount`: Initial deposit of Asset

\- `Amount2`: Initial deposit of Asset2



\*\*Constraints:\*\*

\- Must deposit both assets

\- Ratio determines initial price

\- Creator receives LP tokens



\*\*Example:\*\*

```json

{

&#x20; "TransactionType": "AMMCreate",

&#x20; "Account": "rN7n7otQDd6FczFgLdlqtyMVrn3NnrcVcN",

&#x20; "Asset": {"currency": "XRP"},

&#x20; "Asset2": {"currency": "USD", "issuer": "rN7n..."},

&#x20; "TradingFee": 500,

&#x20; "Amount": "1000000000",  // 1000 XRP (in drops)

&#x20; "Amount2": {"currency": "USD", "issuer": "rN7n...", "value": "500"}

}

```



\---



\### 2. AMMDeposit

Add liquidity to an existing AMM.



\*\*Modes:\*\*

1\. \*\*Balanced deposit\*\* (both assets, maintain ratio)

2\. \*\*Single-sided deposit\*\* (one asset, changes price)

3\. \*\*LP token target\*\* (deposit to reach specific LP token amount)



\*\*Parameters:\*\*

\- `Asset`: First asset

\- `Asset2`: Second asset

\- `Amount`: Optional deposit of Asset

\- `Amount2`: Optional deposit of Asset2

\- `LPTokenOut`: Target LP tokens to receive



\*\*Example (Balanced):\*\*

```json

{

&#x20; "TransactionType": "AMMDeposit",

&#x20; "Account": "rN7n7otQDd6FczFgLdlqtyMVrn3NnrcVcN",

&#x20; "Asset": {"currency": "XRP"},

&#x20; "Asset2": {"currency": "USD", "issuer": "rN7n..."},

&#x20; "Amount": "500000000",  // 500 XRP

&#x20; "Amount2": {"currency": "USD", "issuer": "rN7n...", "value": "250"}

}

```



\---



\### 3. AMMWithdraw

Remove liquidity from AMM.



\*\*Modes:\*\*

1\. \*\*Burn LP tokens\*\* (withdraw both assets proportionally)

2\. \*\*Single-sided withdrawal\*\* (one asset, changes price)

3\. \*\*Exact amount withdrawal\*\* (specify asset amounts)



\*\*Parameters:\*\*

\- `Asset`: First asset

\- `Asset2`: Second asset

\- `LPTokenIn`: LP tokens to burn

\- `Amount`: Optional withdrawal amount of Asset

\- `Amount2`: Optional withdrawal amount of Asset2



\*\*Example (Burn LP):\*\*

```json

{

&#x20; "TransactionType": "AMMWithdraw",

&#x20; "Account": "rN7n7otQDd6FczFgLdlqtyMVrn3NnrcVcN",

&#x20; "Asset": {"currency": "XRP"},

&#x20; "Asset2": {"currency": "USD", "issuer": "rN7n..."},

&#x20; "LPTokenIn": {

&#x20;   "currency": "03930D02208264E2E40EC1B0C09E4DB96EE197B1",

&#x20;   "issuer": "rE54zDvgnghAoPopCgvtiqWNq3dU5y836S",

&#x20;   "value": "100"

&#x20; }

}

```



\---



\### 4. AMMVote

Vote to change the AMM trading fee.



\*\*Parameters:\*\*

\- `Asset`: First asset

\- `Asset2`: Second asset

\- `TradingFee`: Proposed fee (0-1000 bps)



\*\*Mechanism:\*\*

\- Each LP can vote once (weight = LP token balance)

\- Fee changes gradually (weighted average of votes)

\- Min 8 votes required for change



\*\*Example:\*\*

```json

{

&#x20; "TransactionType": "AMMVote",

&#x20; "Account": "rN7n7otQDd6FczFgLdlqtyMVrn3NnrcVcN",

&#x20; "Asset": {"currency": "XRP"},

&#x20; "Asset2": {"currency": "USD", "issuer": "rN7n..."},

&#x20; "TradingFee": 300  // Vote for 0.3% fee

}

```



\---



\### 5. AMMBid

Bid for 24-hour auction slot (exclusive fee boost).



\*\*Parameters:\*\*

\- `Asset`: First asset

\- `Asset2`: Second asset

\- `BidMin`: Minimum bid amount (in LP tokens)

\- `BidMax`: Maximum bid amount



\*\*Mechanism:\*\*

\- Highest bidder wins 24h slot

\- Gets discounted trading fees (or rebate)

\- Bid paid in LP tokens (burned)

\- Slot expires after 24h



\*\*Example:\*\*

```json

{

&#x20; "TransactionType": "AMMBid",

&#x20; "Account": "rN7n7otQDd6FczFgLdlqtyMVrn3NnrcVcN",

&#x20; "Asset": {"currency": "XRP"},

&#x20; "Asset2": {"currency": "USD", "issuer": "rN7n..."},

&#x20; "BidMin": {

&#x20;   "currency": "03930D02208264E2E40EC1B0C09E4DB96EE197B1",

&#x20;   "issuer": "rE54zDvgnghAoPopCgvtiqWNq3dU5y836S",

&#x20;   "value": "50"

&#x20; }

}

```



\---



\## Query APIs



\### amm\_info

Get current state of an AMM.



\*\*Request:\*\*

```json

{

&#x20; "command": "amm\_info",

&#x20; "asset": {"currency": "XRP"},

&#x20; "asset2": {"currency": "USD", "issuer": "rN7n..."}

}

```



\*\*Response:\*\*

```json

{

&#x20; "amm": {

&#x20;   "account": "rE54zDvgnghAoPopCgvtiqWNq3dU5y836S",

&#x20;   "amount": "1000000000",  // 1000 XRP

&#x20;   "amount2": {"currency": "USD", "value": "500"},

&#x20;   "lp\_token": {

&#x20;     "currency": "03930D02208264E2E40EC1B0C09E4DB96EE197B1",

&#x20;     "value": "10000"

&#x20;   },

&#x20;   "trading\_fee": 500,

&#x20;   "auction\_slot": {

&#x20;     "account": "rAuction...",

&#x20;     "price": {"value": "50"},

&#x20;     "expiration": 732456000

&#x20;   }

&#x20; }

}

```



\---



\### account\_lines (for LP tokens)

Query a user's LP token holdings.



\*\*Request:\*\*

```json

{

&#x20; "command": "account\_lines",

&#x20; "account": "rN7n7otQDd6FczFgLdlqtyMVrn3NnrcVcN"

}

```



\*\*Response (filtered):\*\*

```json

{

&#x20; "lines": \[

&#x20;   {

&#x20;     "currency": "03930D02208264E2E40EC1B0C09E4DB96EE197B1",

&#x20;     "issuer": "rE54zDvgnghAoPopCgvtiqWNq3dU5y836S",

&#x20;     "balance": "250",

&#x20;     "limit": "0"

&#x20;   }

&#x20; ]

}

```



\---



\## Swap Mechanics



\### Constant Product Formula

```

x \* y = k

```



Where:

\- `x` = reserves of Asset

\- `y` = reserves of Asset2

\- `k` = constant product



\*\*After swap:\*\*

```

(x + Δx) \* (y - Δy) = k

```



Solving for `Δy`:

```

Δy = y - (k / (x + Δx))

```



\*\*With fees:\*\*

```

Δy\_after\_fee = Δy \* (1 - trading\_fee)

```



\---



\### Example Calculation



\*\*Initial state:\*\*

\- XRP reserves: 1000

\- USD reserves: 500

\- k = 500,000

\- Trading fee: 0.5%



\*\*User swaps 100 XRP:\*\*

```

New XRP reserves: 1000 + 100 = 1100

New USD reserves: 500,000 / 1100 ≈ 454.55

USD out (before fee): 500 - 454.55 = 45.45

USD out (after fee): 45.45 \* 0.995 = 45.23

```



\*\*Slippage:\*\*

```

Expected (at mid price): 100 XRP \* 0.5 = 50 USD

Actual: 45.23 USD

Slippage: (50 - 45.23) / 50 = 9.54%

```



\---



\## Impermanent Loss Formula



\*\*For price ratio `r`:\*\*

```

IL = 2 \* sqrt(r) / (1 + r) - 1

```



\*\*Example:\*\*

\- Entry price: 1 XRP = 0.5 USD

\- Current price: 1 XRP = 1.0 USD

\- Price ratio: 2

\- IL: 2 \* sqrt(2) / 3 - 1 ≈ -5.7%



\*\*Interpretation:\*\* If you had held assets separately, you'd be 5.7% richer.



\---



\## Fee Yield Calculation



\*\*Daily fee income:\*\*

```

Fee\_income = Trading\_volume \* Trading\_fee \* Your\_LP\_share

```



\*\*Example:\*\*

\- Pool: 10,000 LP tokens total

\- Your LP tokens: 250 (2.5% of pool)

\- Daily volume: $80,000

\- Trading fee: 0.5%

\- Your daily income: $80,000 \* 0.005 \* 0.025 = $10



\*\*APY (annualized):\*\*

```

APY = (Daily\_income / Your\_liquidity) \* 365

APY = ($10 / $1,250) \* 365 ≈ 292%

```



(This assumes constant volume, which is unrealistic.)



\---



\## Slippage Calculation



\*\*Price impact:\*\*

```

Price\_impact = |Actual\_price - Mid\_price| / Mid\_price

```



\*\*Mid price (before trade):\*\*

```

Mid\_price = Reserve\_out / Reserve\_in

```



\*\*Example:\*\*

\- XRP reserves: 1000, USD reserves: 500

\- Mid price: 0.5 USD/XRP

\- After 100 XRP swap: 45.23 USD out

\- Actual price: 45.23 / 100 = 0.4523 USD/XRP

\- Price impact: (0.5 - 0.4523) / 0.5 = 9.54%



\---



\## Auction Slot Economics



\*\*Use case:\*\* High-frequency traders or arbitrageurs



\*\*Benefits:\*\*

\- Discounted fees (e.g., 0.1% instead of 0.5%)

\- Or fee rebates (earn from other traders' fees)



\*\*Bid strategy:\*\*

```

Max\_bid = Expected\_trading\_volume \* Fee\_discount \* 24h

```



\*\*Example:\*\*

\- Expected volume: $1M/day

\- Fee discount: 0.4% (0.5% → 0.1%)

\- Savings: $1M \* 0.004 = $4,000

\- Max bid: LP tokens worth $4,000 (break-even)



\---



\## Gas Costs



\*\*XRPL transaction fees:\*\*

\- Standard: 0.00001 XRP (\~$0.000005 USD)

\- High load: Up to 0.001 XRP (\~$0.0005 USD)



\*\*Compared to Ethereum:\*\*

\- Uniswap swap: $5-$50 (depending on gas price)

\- XRPL swap: $0.000005 (1 million times cheaper)



\*\*Implication:\*\* Micro-rebalancing is economically viable on XRPL.



\---



\## Limitations



1\. \*\*No concentrated liquidity\*\* (unlike Uniswap V3)

&#x20;  - Capital inefficiency for stable pairs

&#x20;  - Full price range always active



2\. \*\*No multi-hop routing\*\* (yet)

&#x20;  - Must manually route XRP → BTC → USD

&#x20;  - Less efficient than aggregators (1inch, Matcha)



3\. \*\*Limited pairs\*\*

&#x20;  - Only \~50 active AMM pools (as of March 2026)

&#x20;  - Most liquidity in XRP/USD, XRP/BTC



4\. \*\*Auction slot complexity\*\*

&#x20;  - Hard for retail users to understand

&#x20;  - Favors sophisticated traders



\---



\## Tools \& Libraries



\### JavaScript

```bash

npm install xrpl

```



\*\*Example:\*\*

```javascript

const xrpl = require('xrpl');



const client = new xrpl.Client('wss://xrplcluster.com');

await client.connect();



const amm = await client.request({

&#x20; command: 'amm\_info',

&#x20; asset: { currency: 'XRP' },

&#x20; asset2: { currency: 'USD', issuer: 'rN7n...' }

});



console.log(amm.result.amm);

```



\---



\### Rust

```toml

\[dependencies]

xrpl = "0.9"

```



\*\*Example:\*\*

```rust

use xrpl::client::Client;



let client = Client::new("wss://xrplcluster.com").await?;

let amm = client.amm\_info(

&#x20;   Currency::XRP,

&#x20;   Currency::issued("USD", "rN7n...")

).await?;



println!("{:?}", amm);

```



\---



\### Python

```bash

pip install xrpl-py

```



\*\*Example:\*\*

```python

from xrpl.clients import JsonRpcClient

from xrpl.models.requests import AMMInfo



client = JsonRpcClient("https://xrplcluster.com")

amm = client.request(AMMInfo(

&#x20;   asset={"currency": "XRP"},

&#x20;   asset2={"currency": "USD", "issuer": "rN7n..."}

))



print(amm.result)

```



\---



\## Testnet



\*\*Faucet:\*\* https://xrpl.org/xrp-testnet-faucet.html

\*\*Explorer:\*\* https://testnet.xrpl.org/



\*\*Create test AMM:\*\*

1\. Get test XRP from faucet

2\. Issue test USD (via `TrustSet`)

3\. Call `AMMCreate` with test assets

4\. Verify on explorer



\---



\## References



\- \*\*XLS-30d Spec:\*\* https://github.com/XRPLF/XRPL-Standards/discussions/78

\- \*\*XRPL Docs:\*\* https://xrpl.org/docs/concepts/tokens/decentralized-exchange/automated-market-makers/

\- \*\*AMM FAQ:\*\* https://xrpl.org/docs/tutorials/how-tos/use-tokens/use-an-automated-market-maker/

\- \*\*xrpl.js Docs:\*\* https://js.xrpl.org/



