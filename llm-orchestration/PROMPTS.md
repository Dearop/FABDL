PROMPTS.md                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                          

\# LLM Orchestration \& Prompt Engineering



\## Architecture



\### Two-LLM System



\*\*Local LLM (Intent Router)\*\*

\- \*\*Model:\*\* Llama 3.2 3B (quantized, runs on device)

\- \*\*Role:\*\* Parse user queries → extract intent + parameters

\- \*\*Latency:\*\* <100ms

\- \*\*Privacy:\*\* Zero data leaves device



\*\*Quant LLM (Strategy Generator)\*\*

\- \*\*Model:\*\* Claude Sonnet 4.5 (via API)

\- \*\*Role:\*\* Transform risk metrics → human-readable strategies

\- \*\*Latency:\*\* \~1s

\- \*\*Context:\*\* Isolated from user chat (no prompt injection)



\---



\## Phase 1: Intent Routing (Local LLM)



\### Prompt Template

> **Note:** This is the live template from `llm-orchestration/src/intent_router_service.py`. The user query is injected at the top; the model is instructed to return JSON only with no surrounding text.

```

TASK: Classify the user query into a JSON format. RESPOND ONLY WITH JSON, NOTHING ELSE.

USER QUERY: {user_query}

RETURN ONLY THIS JSON FORMAT (no explanation, no extra text):
{"action": "analyze_risk|execute_strategy|check_position|get_price", "scope": "portfolio|specific_asset|specific_pool", "confidence": 0.0-1.0, "parameters": {}}

Examples:
- "analyze my portfolio" → {"action":"analyze_risk","scope":"portfolio","confidence":0.95,"parameters":{}}
- "XRP price" → {"action":"get_price","scope":"specific_asset","confidence":0.9,"parameters":{"asset":"XRP"}}
- "hedge strategy" → {"action":"execute_strategy","scope":"portfolio","confidence":0.85,"parameters":{"strategy":"conservative"}}

REMEMBER: RESPOND ONLY WITH JSON. NO WORDS BEFORE OR AFTER.

```



\### Expected Outputs

All outputs map to the `IntentRouterOutput` Rust struct: `action`, `scope`, `confidence`, and a nested `parameters` object with optional keys `wallet_address`, `pool`, `focus`, `strategy`.

\*\*Case 1: Risk Analysis\*\*

```json

{

&#x20; "action": "analyze\_risk",

&#x20; "scope": "portfolio",

&#x20; "confidence": 0.95,

&#x20; "parameters": {

&#x20;   "wallet\_address": "rN7n7otQDd6FczFgLdlqtyMVrn3NnrcVcN"

&#x20; }

}

```



\*\*Case 2: Strategy Execution\*\*

```json

{

&#x20; "action": "execute\_strategy",

&#x20; "scope": "portfolio",

&#x20; "confidence": 0.85,

&#x20; "parameters": {

&#x20;   "strategy": "conservative"

&#x20; }

}

```



\*\*Case 3: Price Check\*\*

```json

{

&#x20; "action": "get\_price",

&#x20; "scope": "specific\_asset",

&#x20; "confidence": 0.9,

&#x20; "parameters": {

&#x20;   "asset": "XRP"

&#x20; }

}

```



\---



\## Phase 2: Strategy Generation (Quant LLM)



\### System Prompt



```

You are a quantitative trading strategist specializing in automated market makers (AMMs) on the XRPL blockchain.



Your role:

1\. Analyze portfolio risk metrics

2\. Generate 2-3 concrete trading strategies

3\. Explain trade-offs in simple terms

4\. Provide numerical projections



Constraints:

\- Be concise (max 50 words per strategy)

\- Avoid jargon (explain "delta" as "directional exposure")

\- Always include a "Do Nothing" option

\- Never recommend strategies with >1% slippage



Output format: Structured JSON (see examples below)

```



\---



\### Strategy Generation Prompt

The prompt is rendered dynamically from `PortfolioRiskSummary` by the Rust backend.
Single-position example (one pool):

```

Portfolio Risk Summary:

\- Total Value: $50,000 USD

\- Impermanent Loss: 2.3% (-$1,150)

\- Delta Exposure: 1,500 XRP (\~$750 if XRP drops 10%)

\- 7-Day Fee Income: $120

\- Current XRP Price: $0.50 USD

\- Fee APR: 15.0%

\- Sharpe Ratio: 1.2

\- VaR (95%, 1-day): $1,200

\- Break-even Range: $0.4200 - $0.6100



AMM Pool Details:

\- Pool: XRP/USD, Value: $50000, LP Share: 2.0%, IL: 2.3% (-$1150), Fee APR: 15.0%, 7d Fees: $120, Delta: 1500 XRP



Task: Generate 3 strategies to manage this position.

```

Multi-pool portfolio example (two or more pools):

```

Portfolio Risk Summary:

\- Total Value: $80,000 USD

\- Impermanent Loss: 1.8% (-$1,440)

\- Delta Exposure: 2,200 XRP (\~$1,100 if XRP drops 10%)

\- 7-Day Fee Income: $210

\- Current XRP Price: $0.50 USD

\- Fee APR: 17.5%

\- Sharpe Ratio: 1.4

\- VaR (95%, 1-day): $1,900

\- Break-even Range: $0.3900 - $0.6400



AMM Pool Details (all positions):

\- Pool: XRP/USD, Value: $50000, LP Share: 2.0%, IL: 2.3% (-$1150), Fee APR: 15.0%, 7d Fees: $120, Delta: 1500 XRP

\- Pool: XRP/BTC, Value: $30000, LP Share: 1.2%, IL: 1.1% (-$330), Fee APR: 21.0%, 7d Fees: $90, Delta: 700 XRP



Task: Generate 3 strategies to manage this portfolio.



Output as JSON:

{

&#x20; "strategies": \[

&#x20;   {

&#x20;     "id": "option\_a",

&#x20;     "title": "Conservative: Full Delta Hedge",

&#x20;     "description": "Sell 1,500 XRP to lock in current value. Removes price risk, keeps earning fees.",

&#x20;     "risk\_score": 2,  // 1-10 scale

&#x20;     "projected\_return\_7d": {

&#x20;       "best\_case": "$150",

&#x20;       "expected": "$80",

&#x20;       "worst\_case": "-$20"

&#x20;     },

&#x20;     "trade\_actions": \[

&#x20;       {

&#x20;         "action": "swap",

&#x20;         "asset\_in": "XRP",

&#x20;         "asset\_out": "USD",

&#x20;         "amount": 1500,

&#x20;         "estimated\_slippage": 0.3

&#x20;       }

&#x20;     ],

&#x20;     "pros": \["No price risk", "Still earn fees"],

&#x20;     "cons": \["Miss upside if XRP pumps", "Small swap fee"]

&#x20;   },

&#x20;   {

&#x20;     "id": "option\_b",

&#x20;     "title": "Yield-Focused: Rebalance to High-Fee Pool",

&#x20;     "description": "Move 25% of liquidity to XRP/BTC pool. Higher fees (20% APY) but more volatility.",

&#x20;     "risk\_score": 6,

&#x20;     "projected\_return\_7d": {

&#x20;       "best\_case": "$200",

&#x20;       "expected": "$110",

&#x20;       "worst\_case": "-$80"

&#x20;     },

&#x20;     "trade\_actions": \[

&#x20;       {

&#x20;         "action": "withdraw",

&#x20;         "pool": "XRP/USD",

&#x20;         "lp\_tokens": 500

&#x20;       },

&#x20;       {

&#x20;         "action": "deposit",

&#x20;         "pool": "XRP/BTC",

&#x20;         "asset1\_amount": 12500,

&#x20;         "asset2\_amount": 0.25

&#x20;       }

&#x20;     ],

&#x20;     "pros": \["Higher fee income", "BTC correlation hedge"],

&#x20;     "cons": \["XRP/BTC can diverge", "Two transaction fees"]

&#x20;   },

&#x20;   {

&#x20;     "id": "option\_c",

&#x20;     "title": "Do Nothing: Ride It Out",

&#x20;     "description": "Keep current position. IL is temporary if XRP stabilizes. Fees are strong.",

&#x20;     "risk\_score": 5,

&#x20;     "projected\_return\_7d": {

&#x20;       "best\_case": "$180",

&#x20;       "expected": "$100",

&#x20;       "worst\_case": "-$150"

&#x20;     },

&#x20;     "trade\_actions": \[],

&#x20;     "pros": \["Zero fees", "Simple", "Fees offset IL over time"],

&#x20;     "cons": \["Exposed to further IL if XRP moves", "No active risk management"]

&#x20;   }

&#x20; ],

&#x20; "recommendation": "option\_a",

&#x20; "reasoning": "Given the 2.3% IL and high delta exposure, protecting capital is the priority. Option A locks in value while keeping fee income active."

}

```



\---



\### Few-Shot Examples



\*\*Example 1: Low Volatility, High Fees\*\*



Input:

```

\- IL: 0.5%

\- Delta: 800 XRP

\- Fee APY: 22%

\- XRP volatility: 15% (low)

```



Output:

```json

{

&#x20; "strategies": \[

&#x20;   {

&#x20;     "id": "option\_a",

&#x20;     "title": "Increase Liquidity",

&#x20;     "description": "Add $10k to capture more of the 22% APY. Low volatility = low IL risk.",

&#x20;     "risk\_score": 3

&#x20;   },

&#x20;   {

&#x20;     "id": "option\_b",

&#x20;     "title": "Partial Hedge",

&#x20;     "description": "Sell 400 XRP (50% delta reduction). Keep upside, reduce downside.",

&#x20;     "risk\_score": 4

&#x20;   },

&#x20;   {

&#x20;     "id": "option\_c",

&#x20;     "title": "Do Nothing",

&#x20;     "description": "Conditions are favorable. Let fees compound.",

&#x20;     "risk\_score": 4

&#x20;   }

&#x20; ]

}

```



\---



\*\*Example 2: High IL, Dropping Fees\*\*



Input:

```

\- IL: 7.2%

\- Delta: 2000 XRP

\- Fee APY: 8% (down from 18% last week)

\- XRP volatility: 45% (high)

```



Output:

```json

{

&#x20; "strategies": \[

&#x20;   {

&#x20;     "id": "option\_a",

&#x20;     "title": "Exit Position",

&#x20;     "description": "Withdraw all LP tokens. IL won't recover if volume stays low.",

&#x20;     "risk\_score": 2

&#x20;   },

&#x20;   {

&#x20;     "id": "option\_b",

&#x20;     "title": "Partial Exit + Hedge",

&#x20;     "description": "Withdraw 50%, sell 1000 XRP. Reduces exposure by 75%.",

&#x20;     "risk\_score": 3

&#x20;   },

&#x20;   {

&#x20;     "id": "option\_c",

&#x20;     "title": "Do Nothing",

&#x20;     "description": "Wait for volume to pick up. Risky but might recover.",

&#x20;     "risk\_score": 8

&#x20;   }

&#x20; ]

}

```



\---



\## Safety Guardrails



\### Hard Constraints (Enforced in Prompt)



1\. \*\*Never recommend >1% slippage\*\*

&#x20;  - Reject any strategy requiring high slippage

&#x20;  - Suggest breaking large trades into chunks



2\. \*\*Always include "Do Nothing"\*\*

&#x20;  - Prevents pressure to over-trade

&#x20;  - Acknowledges uncertainty



3\. \*\*Cap risk scores at 8/10\*\*

&#x20;  - No "extremely risky" strategies

&#x20;  - If risk >8, suggest smaller position size



4\. \*\*Require min 2 options\*\*

&#x20;  - Prevents "one-size-fits-all" bias

&#x20;  - Forces LLM to consider trade-offs



\---



\### Validation (Post-Generation)



\*\*Backend checks before showing to user:\*\*



```python

def validate\_strategy(strategy: dict) -> bool:

&#x20;   # Check slippage

&#x20;   for action in strategy\['trade\_actions']:

&#x20;       if action.get('estimated\_slippage', 0) > 1.0:

&#x20;           return False



&#x20;   # Check risk score

&#x20;   if strategy\['risk\_score'] > 8:

&#x20;       return False



&#x20;   # Check for required fields

&#x20;   required = \['id', 'title', 'description', 'risk\_score']

&#x20;   if not all(field in strategy for field in required):

&#x20;       return False



&#x20;   return True

```



\*\*If validation fails:\*\*

\- Log the raw LLM output (for debugging)

\- Retry with adjusted prompt (add more constraints)

\- Fall back to hardcoded conservative strategy



\---



\## Context Window Management



\### Problem

Claude Sonnet has 200k token limit, but we want to keep costs low.



\### Solution: Structured Input



\*\*Minimal Context (per request):\*\*

\- Current portfolio state (500 tokens)

\- Recent market data (300 tokens)

\- User preferences (100 tokens)

\- \*\*Total: \~900 tokens input\*\*



\*\*Avoid:\*\*

\- Full transaction history (use summary stats instead)

\- Raw XRPL ledger data (pre-process into metrics)

\- Redundant explanations (use code/abbreviations)



\---



\## Multimodal: Risk Graph Generation



\### Approach 1: LLM → Plotting Library



\*\*Flow:\*\*

1\. Quant LLM outputs structured data (JSON with PnL projections)

2\. Backend parses JSON

3\. Matplotlib/D3.js renders graph

4\. Return graph as PNG/SVG to frontend



\*\*Pros:\*\* Reliable, customizable, fast

\*\*Cons:\*\* Requires separate plotting service



\---



\### Approach 2: Vision-LLM for Graph Validation



\*\*Use Case:\*\* Verify generated graphs don't have errors



\*\*Flow:\*\*

1\. Generate graph (Approach 1)

2\. Feed image to GPT-4 Vision: "Does this graph correctly show delta-hedge vs. HODL?"

3\. If LLM says "No" → regenerate with corrected params



\*\*Cost:\*\* \~$0.01 per graph validation



\---



\## Prompt Versioning



\### Why It Matters

\- Prompts evolve (add constraints, improve clarity)

\- Need to A/B test effectiveness

\- Audit trail for debugging bad strategies



\### System

```

prompts/

&#x20; strategy\_generation\_v1.txt

&#x20; strategy\_generation\_v2.txt  ← current

&#x20; strategy\_generation\_v3.txt  ← testing

```



\*\*Metadata:\*\*

```json

{

&#x20; "version": "v2",

&#x20; "created": "2026-03-15",

&#x20; "changes": "Added slippage constraint, removed jargon",

&#x20; "performance": {

&#x20;   "avg\_user\_rating": 4.2,

&#x20;   "strategy\_success\_rate": 78%

&#x20; }

}

```



\---



\## Cost Optimization



\### Current Cost (Claude Sonnet 4.5)

\- Input: $3 / 1M tokens

\- Output: $15 / 1M tokens



\### Per-Request Estimate

\- Input: \~1k tokens = $0.003

\- Output: \~500 tokens = $0.0075

\- \*\*Total: \~$0.01 per strategy generation\*\*



\### Cost Reduction Tactics

1\. \*\*Cache common prompts\*\* (Claude supports prompt caching)

2\. \*\*Use cheaper model for simple queries\*\* (Haiku for "check price")

3\. \*\*Rate limit users\*\* (max 10 strategy generations per day)

4\. \*\*Fine-tune smaller model\*\* (Llama 3.2 8B for strategy generation)



\---



\## A/B Testing Framework



\*\*Experiment:\*\* Compare "Conservative + Yield + Do Nothing" vs. "Low/Med/High Risk"



\*\*Metrics:\*\*

\- User satisfaction (thumbs up/down)

\- Strategy execution rate (% users who click "Execute")

\- Realized PnL (track outcomes over 30 days)



\*\*Implementation:\*\*

```python

if user.id % 2 == 0:

&#x20;   prompt = load\_prompt("strategy\_generation\_v2.txt")

else:

&#x20;   prompt = load\_prompt("strategy\_generation\_v3.txt")

```



\*\*Report:\*\*

\- V2 (Conservative/Yield/DoNothing): 72% execute, 4.1★ rating

\- V3 (Low/Med/High Risk): 68% execute, 3.9★ rating

\- \*\*Winner: V2\*\* (better clarity, less intimidating)



\---



\## Error Handling



\### Scenario 1: LLM Returns Invalid JSON



\*\*Fallback:\*\*

```python

try:

&#x20;   strategies = json.loads(llm\_response)

except JSONDecodeError:

&#x20;   log\_error("LLM output was not valid JSON", llm\_response)

&#x20;   strategies = get\_default\_conservative\_strategy()

```



\---



\### Scenario 2: LLM Hallucinates Assets



\*\*Validation:\*\*

```python

ALLOWED\_ASSETS = {'XRP', 'USD', 'BTC', 'ETH'}



for action in strategy\['trade\_actions']:

&#x20;   if action\['asset\_in'] not in ALLOWED\_ASSETS:

&#x20;       return None  # Reject strategy

```



\---



\### Scenario 3: LLM Times Out



\*\*Retry Logic:\*\*

```python

for attempt in range(3):

&#x20;   try:

&#x20;       response = call\_llm(prompt, timeout=10)

&#x20;       break

&#x20;   except TimeoutError:

&#x20;       if attempt == 2:

&#x20;           return get\_default\_conservative\_strategy()

```



\---



\## Open Questions



1\. \*\*Should we let users customize risk tolerance?\*\* (e.g., "Show me only low-risk options")

2\. \*\*Can we use a smaller, fine-tuned model for strategy generation?\*\* (Cost + latency win)

3\. \*\*How do we handle conflicting strategies?\*\* (e.g., "Hedge" vs. "Increase exposure")

4\. \*\*Should we show LLM reasoning?\*\* (Transparency vs. clutter)

5\. \*\*What's the right balance of automation vs. user control?\*\* (One-click execute vs. review-every-param)



\---



\## Next Steps



1\. Prototype local LLM intent router (test on 50 sample queries)

2\. Build Quant LLM prompt template (validate with 10 real portfolio snapshots)

3\. Implement strategy validation logic (catch hallucinations/errors)

4\. A/B test prompt variations (measure execution rate + satisfaction)

5\. Estimate costs at scale (1k users, 5 queries/day/user)







