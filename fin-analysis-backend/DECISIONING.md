## What "Informed" Actually Means in This Context
For each of your three use cases, "informed" means something specific:

### AMM Liquidity Provision

- You know your IL function at every price ratio (it's deterministic from the AMM formula)
- You know your fee accrual rate (from pool volume history)
- The question is: at what price movement does IL eat your fees? That break-even is calculable. VEGA should show it.

### Trading

- You need expected value, not just direction. A trade that's right 60% of the time and loses 3x when wrong has negative EV.
- Slippage, spread, and gas are knowable in advance. A trade you can't model the cost of is a trade you shouldn't take.

### Lend/Borrow

- Risk here is liquidation probability given collateral volatility. That's a VaR problem with a hard boundary condition.
- The informed question is: what's the probability my collateral falls below the liquidation threshold before I repay? If you can't answer that, don't borrow.

## Analysis Framework
The hard part isn't the analysis framework — you clearly have that. The hard part is: 
**Data quality** — your calculations are only as good as your pool state inputs (hence why Firehose matters)
**Follow Professional Standard** 
1. The position — what you own and why
2. The risk — quantified, with a distribution and worst-case
3. The hedge — what partially offsets it, and at what cost
4. The exit — under what conditions you close, predefined