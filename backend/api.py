"""
Backend API that bridges the frontend HTTP requests to the Intent Router gRPC service.
Handles wallet connection, query classification, strategy generation, and execution.
"""

import sys
import os
import asyncio
import json
import re
from typing import Optional

# Vendor directory (packages installed here when system pip Scripts dir is locked)
_VENDOR = os.path.join(os.path.dirname(__file__), 'vendor')
if os.path.isdir(_VENDOR) and _VENDOR not in sys.path:
    sys.path.insert(0, _VENDOR)

from fastapi import FastAPI, HTTPException
from fastapi.middleware.cors import CORSMiddleware
from pydantic import BaseModel
import grpc
import anthropic as _anthropic
import httpx

# Import the generated protobuf stubs
sys.path.insert(0, os.path.join(os.path.dirname(__file__), '../llm-orchestration/src'))
import intent_router_pb2
import intent_router_pb2_grpc

app = FastAPI(title="XRPL AI Trading Backend")

# Enable CORS for frontend requests
app.add_middleware(
    CORSMiddleware,
    allow_origins=["http://localhost:3000", "http://localhost:3001"],
    allow_credentials=True,
    allow_methods=["*"],
    allow_headers=["*"],
)

# ==================== Constants ====================
INTENT_ROUTER_HOST = "localhost"
INTENT_ROUTER_PORT = 50051

RUST_BACKEND_URL = os.environ.get("RUST_BACKEND_URL", "http://localhost:3001")
ANTHROPIC_API_KEY = os.environ.get("ANTHROPIC_API_KEY", "")

CLAUDE_SYSTEM_PROMPT = """You are VEGA, a quantitative trading strategist specializing in XRPL's native AMM (constant-product, Uniswap v2-style) and the XLS-66d lending protocol.

CONTEXT:
- XRPL AMM uses x*y=k formula. Each pool holds exactly 2 assets. At most one can be XRP.
- Deposit modes: "single_asset" (deposit one side, incurs fee) or "two_asset" (proportional, no fee).
- Withdraw modes mirror deposits. Swaps go through the AMM pool.
- XLS-66d lending: fixed-term loans from single-asset vaults. Lenders earn interest from utilization.
- Borrowing enables delta-neutral LP: borrow risky asset, deposit into AMM, hedge impermanent loss.
- Lending strategies should use VaR and Sharpe more heavily (direction risk is higher).

YOUR ROLE:
1. Analyze the portfolio risk metrics provided
2. Generate EXACTLY 3 strategies: one conservative, one yield-focused, one "Do Nothing"
3. Explain trade-offs in plain language (no jargon — say "directional exposure" not "delta")
4. Provide numerical projections based on the risk data

HARD CONSTRAINTS:
- Cap risk_score at 8 (integer, 1-8 scale)
- Never recommend estimated_slippage > 1.0 (percent)
- Only use these assets: XRP, USD, BTC, ETH, USDC, USDT
- Max 50 words per strategy description
- "Do Nothing" must always be option_c with risk_score 1-4 and empty trade_actions
- Generate exactly 3 strategies so validation always passes

OUTPUT: Return ONLY this JSON. No prose, no markdown fences, no explanation before or after.
{
  "strategies": [
    {
      "id": "option_a",
      "title": "string",
      "description": "string (max 50 words)",
      "risk_score": integer (1-8),
      "projected_return_7d": {
        "best_case": "$XXX",
        "expected": "$XXX",
        "worst_case": "-$XXX"
      },
      "trade_actions": [
        {
          "action": "swap" | "deposit" | "withdraw" | "lend" | "borrow",
          "asset_in": "XRP" | "USD" | "BTC" | "ETH" | "USDC" | "USDT",
          "asset_out": "XRP" | "USD" | "BTC" | "ETH" | "USDC" | "USDT",
          "amount": number,
          "amount2": number or null (for two-asset deposits only),
          "estimated_slippage": number (0 to 1.0, percent),
          "pool": "ASSET1/ASSET2" or null (e.g. "XRP/USD"),
          "deposit_mode": "single_asset" | "two_asset" | null,
          "interest_rate": number or null (annualized %, for lend/borrow only),
          "term_days": integer or null (for lend/borrow only)
        }
      ],
      "pros": ["string", "string"],
      "cons": ["string", "string"]
    }
  ]
}

STRATEGY TAXONOMY:
- option_a: Conservative — capital preservation, hedging, low risk (risk_score 1-3)
- option_b: Yield-Focused — fee maximization, AMM deposits, lending, moderate-high risk (risk_score 4-8)
- option_c: Do Nothing — hold current position, zero trade actions (risk_score 1-4)

LENDING STRATEGY GUIDANCE:
- "lend" action: supply asset to a vault, earn interest. asset_in = supplied asset, asset_out = same asset.
- "borrow" action: borrow from vault. asset_in = collateral, asset_out = borrowed asset.
- Delta-neutral LP: borrow risky asset + deposit into AMM = hedged IL. Higher risk_score (5-7).

DECISION METRICS — use these when lending/borrowing data is present in the portfolio summary:
- Net carry: the summary provides net_carry (fee_APR - borrow_APY - expected_IL). If net_carry < 0, the delta-neutral strategy costs more than it earns — flag this explicitly in cons and cap risk_score at 5.
- Health factor: any open loan with health_factor < 1.5 is at elevated liquidation risk — add +2 to risk_score for any strategy that increases borrowing. If health_factor < 1.2, the conservative strategy must recommend partial repayment.
- Liquidation price: state the liquidation price explicitly in the strategy description when recommending borrow actions — e.g. "liquidation triggers at $0.31 XRP".
- Liquidation penalty: factor liquidation_penalty_pct into worst-case projected_return_7d for any strategy involving open loans. Worst case = current loss + (collateral_usd * liquidation_penalty_pct) if liquidation_price is within the VaR range.
- Utilization and liquidity: if utilization_rate > kink_utilization, the borrow APY may spike further — flag in cons. If available_liquidity_usd < position value, warn that full withdrawal may not be possible immediately.
- CVaR vs VaR: the summary provides both var_95_usd and cvar_95_usd. For levered positions (any open loans), use cvar_95_usd in your worst_case projection — it better captures tail loss including liquidation gap risk.
- Gamma: the summary provides gamma_usd (negative for all LP positions). For delta-neutral strategies, note that delta ≈ 0 but gamma remains negative — large price moves in either direction still cause losses. Mention this in cons when recommending levered LP.
- Net hedging cost ratio = borrowing_cost / fee_APR. If > 1.0, the hedge costs more than it protects — flag in cons.
"""

ALLOWED_ASSETS = {"XRP", "USD", "BTC", "ETH", "USDC", "USDT"}
ALLOWED_ACTIONS = {"swap", "deposit", "withdraw", "lend", "borrow"}
ALLOWED_DEPOSIT_MODES = {"single_asset", "two_asset", None}

# ==================== Request/Response Models ====================

class WalletConnectRequest(BaseModel):
    address: str
    network: str = "testnet"

class QueryRequest(BaseModel):
    user_query: str
    wallet_id: str

class Parameter(BaseModel):
    key: str
    value: str

class Strategy(BaseModel):
    id: str
    title: str
    description: str
    risk_score: int
    projected_return_7d: dict
    trade_actions: list
    pros: list
    cons: list

# ==================== Intent Router gRPC Client ====================

async def classify_intent_with_grpc(user_query: str) -> dict:
    """
    Call the Intent Router gRPC service to classify a user query.
    Returns: {action, scope, confidence, is_valid, parameters}
    """
    try:
        print(f"\n🔍 [Backend] Classifying query: '{user_query}'")
        
        # Create gRPC channel
        channel = grpc.aio.secure_channel(
            f"{INTENT_ROUTER_HOST}:{INTENT_ROUTER_PORT}",
            grpc.ssl_channel_credentials()
        ) if INTENT_ROUTER_HOST != "localhost" else grpc.aio.insecure_channel(
            f"{INTENT_ROUTER_HOST}:{INTENT_ROUTER_PORT}"
        )
        
        stub = intent_router_pb2_grpc.IntentRouterStub(channel)
        
        print(f"📡 [Backend] Sending gRPC request to Intent Router at {INTENT_ROUTER_HOST}:{INTENT_ROUTER_PORT}")
        
        # Create request
        request = intent_router_pb2.IntentRequest(
            user_query=user_query,
            timestamp=int(asyncio.get_event_loop().time())
        )
        
        # Call gRPC service
        response = await stub.ClassifyIntent(request)
        
        # Close channel
        await channel.close()
        
        print(f"✅ [Backend] Intent classified successfully")
        print(f"   Action: {response.action}")
        print(f"   Scope: {response.scope}")
        print(f"   Confidence: {response.confidence}")
        
        # Convert protobuf response to dict
        return {
            "action": response.action,
            "scope": response.scope,
            "confidence": round(response.confidence, 2),
            "is_valid": response.is_valid,
            "parameters": [{"key": p.key, "value": p.value} for p in response.parameters]
        }
    
    except Exception as e:
        print(f"❌ [Backend] Error calling Intent Router: {e}")
        raise HTTPException(status_code=500, detail=f"Intent classification failed: {str(e)}")

# ==================== Pipeline Helpers ====================

def _build_intent_router_output(intent: dict, wallet_id: str) -> dict:
    """
    Convert gRPC IntentResponse dict + wallet_id into the JSON structure
    that the Rust POST /analyze endpoint expects as IntentRouterOutput.

    gRPC parameters arrive as:  [{"key": "pool", "value": "XRP/USD"}, ...]
    Rust expects:                {"wallet_address": "r...", "pool": "XRP/USD", ...}

    wallet_id is always injected as wallet_address (authoritative from frontend).
    """
    grpc_params = {p["key"]: p["value"] for p in intent.get("parameters", [])}
    parameters = {"wallet_address": wallet_id}
    if "pool" in grpc_params:
        parameters["pool"] = grpc_params["pool"]
    if "focus" in grpc_params:
        parameters["focus"] = grpc_params["focus"]
    return {
        "action": intent["action"],
        "scope": intent["scope"],
        "parameters": parameters,
        "confidence": intent.get("confidence", 0.0),
    }


async def _call_rust_backend(intent_payload: dict) -> str:
    """
    POST IntentRouterOutput JSON to the Rust /analyze endpoint.
    Returns the plain-text rendered PortfolioRiskSummary prompt string.
    """
    print(f"  [Backend] POST {RUST_BACKEND_URL}/analyze  payload={intent_payload}")
    async with httpx.AsyncClient(timeout=30.0) as client:
        response = await client.post(f"{RUST_BACKEND_URL}/analyze", json=intent_payload)
    if response.status_code != 200:
        raise HTTPException(
            status_code=response.status_code,
            detail=f"Rust analysis backend error: {response.text}",
        )
    print(f"  [Backend] Rust returned prompt ({len(response.text)} chars)")
    return response.text


async def _call_claude(prompt_text: str) -> dict:
    """
    Send the Rust-rendered prompt string to Claude Sonnet as the user message.
    Returns the parsed JSON response dict.

    asyncio.to_thread is used because the anthropic SDK is synchronous.
    """
    if not ANTHROPIC_API_KEY:
        raise HTTPException(status_code=500, detail="ANTHROPIC_API_KEY environment variable is not set")

    client = _anthropic.Anthropic(api_key=ANTHROPIC_API_KEY)

    print(f"  [Backend] Calling Claude claude-sonnet-4-6...")

    def _sync_call():
        return client.messages.create(
            model="claude-sonnet-4-6",
            max_tokens=1024,
            system=CLAUDE_SYSTEM_PROMPT,
            messages=[{"role": "user", "content": prompt_text}],
        )

    message = await asyncio.to_thread(_sync_call)
    raw = message.content[0].text.strip()
    print(f"  [Backend] Claude responded ({len(raw)} chars)")

    # Strip markdown code fences defensively
    fence = re.search(r"```(?:json)?\s*(\{.*?\})\s*```", raw, re.DOTALL)
    if fence:
        raw = fence.group(1)

    try:
        return json.loads(raw)
    except json.JSONDecodeError as exc:
        print(f"  [Backend] Claude JSON parse error: {exc}\n  Raw: {raw[:500]}")
        raise HTTPException(status_code=500, detail="Claude returned invalid JSON")


def _validate_strategy(s: dict) -> bool:
    """Enforce safety guardrails."""
    required = {"id", "title", "description", "risk_score"}
    if not required.issubset(s.keys()):
        print(f"  [Validation] Missing required fields: {required - set(s.keys())}")
        return False
    if not isinstance(s.get("risk_score"), (int, float)) or s["risk_score"] > 8:
        print(f"  [Validation] Invalid risk_score: {s.get('risk_score')}")
        return False
    if not isinstance(s.get("projected_return_7d"), dict):
        print(f"  [Validation] Missing or invalid projected_return_7d")
        return False
    ret = s["projected_return_7d"]
    if not all(k in ret for k in ("best_case", "expected", "worst_case")):
        print(f"  [Validation] projected_return_7d missing fields")
        return False
    if not isinstance(s.get("pros"), list) or not isinstance(s.get("cons"), list):
        print(f"  [Validation] Missing pros or cons arrays")
        return False
    for action in s.get("trade_actions", []):
        if action.get("estimated_slippage", 0) > 1.0:
            print(f"  [Validation] Slippage too high: {action.get('estimated_slippage')}")
            return False
        if action.get("action") and action["action"] not in ALLOWED_ACTIONS:
            print(f"  [Validation] Unknown action type: {action.get('action')}")
            return False
        for key in ("asset_in", "asset_out"):
            if action.get(key) and action[key] not in ALLOWED_ASSETS:
                print(f"  [Validation] Unknown asset: {action.get(key)}")
                return False
    return True


def _validate_and_filter_strategies(claude_response: dict) -> list:
    strategies = claude_response.get("strategies", [])
    valid = [s for s in strategies if _validate_strategy(s)]
    if len(valid) < 2:
        print(f"  [Backend] Only {len(valid)} strategies passed validation — using fallback")
        return _fallback_strategies()
    return valid


def _fallback_strategies() -> list:
    return [
        {
            "id": "option_a",
            "title": "Conservative: Exit to Stablecoin",
            "description": "Withdraw LP tokens and hold XRP and USD separately. Eliminates impermanent loss risk entirely.",
            "risk_score": 2,
            "projected_return_7d": {"best_case": "$0", "expected": "$0", "worst_case": "$0"},
            "trade_actions": [
                {"action": "withdraw", "asset_in": "XRP", "asset_out": "USD", "amount": 0, "estimated_slippage": 0.1, "pool": "XRP/USD", "deposit_mode": None, "amount2": None, "interest_rate": None, "term_days": None}
            ],
            "pros": ["Eliminates IL risk", "Capital preservation"],
            "cons": ["Stops earning fees", "Re-entry cost if markets stabilise"],
        },
        {
            "id": "option_b",
            "title": "Yield: Lend XRP in Vault",
            "description": "Supply XRP to a lending vault. Earn fixed interest without AMM directional exposure.",
            "risk_score": 3,
            "projected_return_7d": {"best_case": "$15", "expected": "$8", "worst_case": "$0"},
            "trade_actions": [
                {"action": "lend", "asset_in": "XRP", "asset_out": "XRP", "amount": 50, "estimated_slippage": 0.0, "pool": None, "deposit_mode": None, "amount2": None, "interest_rate": 5.0, "term_days": 30}
            ],
            "pros": ["Predictable yield", "No impermanent loss"],
            "cons": ["Lower returns than AMM fees", "Funds locked for term"],
        },
        {
            "id": "option_c",
            "title": "Do Nothing: Hold Current Position",
            "description": "Keep current position unchanged. Monitor fees vs. impermanent loss daily.",
            "risk_score": 3,
            "projected_return_7d": {"best_case": "$0", "expected": "$0", "worst_case": "$0"},
            "trade_actions": [],
            "pros": ["Zero transaction cost", "No action needed"],
            "cons": ["Exposed to IL if XRP moves", "No yield if not in AMM"],
        },
    ]


def _handle_execute_strategy_intent(intent: dict) -> list:
    """
    Rust's router.rs explicitly rejects execute_strategy with a 500 error.
    Return UI confirmation strategies from Python instead.
    """
    return [
        {
            "id": "option_a",
            "title": "Confirm Execution",
            "description": "Proceed to execute the selected strategy. Sign the transaction in your wallet.",
            "risk_score": 5,
            "projected_return_7d": {"best_case": "Varies", "expected": "Varies", "worst_case": "Varies"},
            "trade_actions": [],
            "pros": ["Implements your chosen strategy immediately"],
            "cons": ["Transaction fees apply", "Irreversible once signed"],
        },
        {
            "id": "option_b",
            "title": "Yield: Lend Idle Capital",
            "description": "Instead of executing, supply idle capital to a lending vault for fixed interest.",
            "risk_score": 3,
            "projected_return_7d": {"best_case": "$10", "expected": "$5", "worst_case": "$0"},
            "trade_actions": [
                {"action": "lend", "asset_in": "XRP", "asset_out": "XRP", "amount": 50, "estimated_slippage": 0.0, "pool": None, "deposit_mode": None, "amount2": None, "interest_rate": 5.0, "term_days": 30}
            ],
            "pros": ["Predictable yield", "No impermanent loss"],
            "cons": ["Lower returns", "Funds locked for term"],
        },
        {
            "id": "option_c",
            "title": "Do Nothing",
            "description": "Cancel and keep current position unchanged.",
            "risk_score": 2,
            "projected_return_7d": {"best_case": "$0", "expected": "$0", "worst_case": "$0"},
            "trade_actions": [],
            "pros": ["No cost", "No risk"],
            "cons": ["Missed opportunity"],
        },
    ]


# ==================== Health Check ====================

@app.get("/health")
async def health():
    """Health check endpoint"""
    return {"status": "ok", "service": "XRPL AI Trading Backend"}

# ==================== Wallet Endpoints ====================

@app.post("/wallet/connect")
async def connect_wallet(request: WalletConnectRequest):
    """
    Connect a user's wallet.
    """
    try:
        # In a real implementation, verify the wallet exists on XRPL
        # For now, just validate the address format
        if not request.address.startswith("r") or len(request.address) < 20:
            raise HTTPException(status_code=400, detail="Invalid XRPL address")
        
        return {
            "wallet_id": request.address,
            "verified": True,
            "network": request.network,
            "balance": "5000 XRP"  # Placeholder
        }
    except Exception as e:
        raise HTTPException(status_code=500, detail=str(e))

# ==================== Query Endpoints ====================

@app.post("/query/classify")
async def classify_query(request: QueryRequest):
    """
    Classify a user query using the Intent Router.
    Returns: intent classification (action, scope, parameters, etc.)
    """
    if not request.user_query:
        raise HTTPException(status_code=400, detail="user_query cannot be empty")
    
    # Call Intent Router gRPC service
    intent = await classify_intent_with_grpc(request.user_query)
    
    return {
        "intent": intent,
        "wallet_id": request.wallet_id
    }

@app.post("/strategies/generate")
async def generate_strategies(request: QueryRequest):
    """
    Generate trading strategies based on a natural language query.
    Flow:
    1. Classify intent via gRPC Intent Router
    2. Intercept execute_strategy before hitting Rust (Rust rejects it)
    3. Build IntentRouterOutput JSON, injecting wallet_id as wallet_address
    4. POST to Rust /analyze → get back rendered PortfolioRiskSummary prompt
    5. Send prompt to Claude Sonnet → get back structured strategy JSON
    6. Validate strategies against safety guardrails
    7. Return validated strategies to frontend
    """
    try:
        print(f"\n{'='*60}")
        print(f"[Backend] /strategies/generate called")
        print(f"   Wallet: {request.wallet_id}")
        print(f"   Query:  '{request.user_query}'")
        print(f"{'='*60}")

        # Step 1: Classify intent via gRPC Intent Router
        print(f"\n[Backend] Step 1: Classifying intent via gRPC...")
        intent = await classify_intent_with_grpc(request.user_query)
        print(f"  Action: {intent['action']}, Scope: {intent['scope']}, "
              f"Confidence: {intent['confidence']}, Valid: {intent['is_valid']}")

        if not intent["is_valid"]:
            raise HTTPException(
                status_code=400,
                detail=f"Intent classification confidence too low: {intent['confidence']}",
            )

        # Step 2: Intercept execute_strategy before Rust
        if intent["action"] == "execute_strategy":
            print(f"\n[Backend] Action is execute_strategy — returning UI confirmation strategies")
            return {
                "intent": intent,
                "strategies": _handle_execute_strategy_intent(intent),
                "wallet_id": request.wallet_id,
            }

        # Step 3: Build IntentRouterOutput JSON for Rust
        print(f"\n[Backend] Step 2: Building IntentRouterOutput for Rust backend...")
        intent_payload = _build_intent_router_output(intent, request.wallet_id)

        # Step 4: Call Rust analysis backend
        print(f"\n[Backend] Step 3: Calling Rust analysis backend...")
        prompt_text = await _call_rust_backend(intent_payload)

        # Step 5: Call Claude Sonnet
        print(f"\n[Backend] Step 4: Calling Claude Sonnet...")
        claude_response = await _call_claude(prompt_text)

        # Step 6: Validate and filter strategies
        print(f"\n[Backend] Step 5: Validating strategies...")
        strategies = _validate_and_filter_strategies(claude_response)
        print(f"  {len(strategies)} strategies passed validation")

        print(f"\n[Backend] /strategies/generate complete\n")

        return {
            "intent": intent,
            "strategies": strategies,
            "wallet_id": request.wallet_id,
        }

    except HTTPException:
        raise
    except Exception as e:
        print(f"[Backend] Unhandled error: {e}")
        raise HTTPException(status_code=500, detail=str(e))

# ==================== MCP Agentic Endpoint ====================

# Tool schemas match mcp-server/server.py exactly so the same backend functions
# serve both the stdio MCP server and the in-process agentic loop.
_MCP_TOOLS = [
    {
        "name": "route_intent",
        "description": "Classify a natural language trading query into a structured intent (action, scope, confidence, parameters)",
        "input_schema": {
            "type": "object",
            "properties": {"query": {"type": "string", "description": "Natural language trading query"}},
            "required": ["query"],
        },
    },
    {
        "name": "analyze_portfolio",
        "description": (
            "Fetch XRPL AMM positions and compute quant risk metrics "
            "(IL, CVaR, Sharpe, delta, gamma, net carry, fee APR, lending vaults, open loans) for a wallet. "
            "Call this before generating strategies."
        ),
        "input_schema": {
            "type": "object",
            "properties": {
                "wallet_id": {"type": "string", "description": "XRPL wallet address (r...)"},
                "pool": {"type": ["string", "null"], "description": "e.g. 'XRP/USD'. null = whole portfolio."},
            },
            "required": ["wallet_id"],
        },
    },
    {
        "name": "get_lending_context",
        "description": (
            "Fetch XLS-66d vault APYs, kink utilization, available liquidity, and the wallet's open loans. "
            "Call this when the user asks about borrowing, lending, or delta-neutral strategies."
        ),
        "input_schema": {
            "type": "object",
            "properties": {
                "asset": {"type": "string", "description": "Asset ticker, e.g. 'XRP'"},
                "wallet_id": {"type": "string", "description": "XRPL wallet address"},
            },
            "required": ["asset", "wallet_id"],
        },
    },
]

_MCP_SYSTEM_PROMPT = (
    CLAUDE_SYSTEM_PROMPT
    + """

ORCHESTRATION INSTRUCTIONS:
You are operating in tool-use mode. Before generating strategies you MUST:
1. Call analyze_portfolio with the wallet_id to get risk metrics.
2. If the portfolio has open loans OR the user mentions lending/borrowing, also call get_lending_context for each relevant asset.
3. Once you have the data, output the strategies JSON (same schema as above). Do not call tools after you have enough data.
Calling route_intent is optional — use it only if the user query is ambiguous.
"""
)


async def _execute_mcp_tool(tool_name: str, tool_input: dict, wallet_id: str) -> dict:
    """Execute a single MCP tool call in-process (mirrors mcp-server implementations)."""
    if tool_name == "route_intent":
        try:
            intent = await classify_intent_with_grpc(tool_input["query"])
            return intent
        except Exception as exc:
            return {"error": str(exc)}

    elif tool_name == "analyze_portfolio":
        try:
            payload = {
                "action": "analyze_risk",
                "scope": "portfolio",
                "parameters": {"wallet_address": tool_input["wallet_id"]},
                "confidence": 1.0,
            }
            if tool_input.get("pool"):
                payload["parameters"]["pool"] = tool_input["pool"]
            prompt_text = await _call_rust_backend(payload)
            return {"risk_summary": prompt_text}
        except Exception as exc:
            return {"error": str(exc)}

    elif tool_name == "get_lending_context":
        try:
            payload = {
                "action": "check_position",
                "scope": "specific_asset",
                "parameters": {
                    "wallet_address": tool_input["wallet_id"],
                    "focus": tool_input["asset"],
                },
                "confidence": 1.0,
            }
            async with httpx.AsyncClient(timeout=30.0) as client:
                resp = await client.post(f"{RUST_BACKEND_URL}/analyze", json=payload)
            if resp.status_code != 200:
                return {"error": f"Rust backend {resp.status_code}: {resp.text}"}
            try:
                data = resp.json()
                return {
                    "lending_vaults": data.get("lending_vaults", []),
                    "open_loans": data.get("open_loans", []),
                }
            except Exception:
                return {"raw": resp.text}
        except Exception as exc:
            return {"error": str(exc)}

    return {"error": f"Unknown tool: {tool_name}"}


@app.post("/strategies/generate-mcp")
async def generate_strategies_mcp(request: QueryRequest):
    """
    MCP-orchestrated strategy generation.
    Claude acts as orchestrator: it decides which tools to call, gathers
    portfolio and lending data, then generates strategies — rather than the
    hardcoded pipeline in /strategies/generate.
    """
    if not ANTHROPIC_API_KEY:
        raise HTTPException(status_code=500, detail="ANTHROPIC_API_KEY not set")

    client = _anthropic.Anthropic(api_key=ANTHROPIC_API_KEY)
    messages = [
        {
            "role": "user",
            "content": (
                f"User query: {request.user_query}\n"
                f"Wallet address: {request.wallet_id}\n\n"
                "Gather the necessary data using the available tools, then return the strategies JSON."
            ),
        }
    ]

    print(f"\n[MCP] /strategies/generate-mcp  wallet={request.wallet_id}  query='{request.user_query}'")

    def _sync_create(msgs):
        return client.messages.create(
            model="claude-sonnet-4-6",
            max_tokens=4096,
            system=_MCP_SYSTEM_PROMPT,
            tools=_MCP_TOOLS,
            messages=msgs,
        )

    MAX_TURNS = 8  # guard against infinite tool loops
    for turn in range(MAX_TURNS):
        response = await asyncio.to_thread(_sync_create, messages)
        print(f"  [MCP] turn {turn+1}: stop_reason={response.stop_reason}  blocks={len(response.content)}")

        if response.stop_reason == "end_turn":
            # Extract final text and parse strategies
            final_text = next(
                (b.text for b in response.content if hasattr(b, "text")), ""
            ).strip()
            fence = re.search(r"```(?:json)?\s*(\{.*?\})\s*```", final_text, re.DOTALL)
            if fence:
                final_text = fence.group(1)
            try:
                claude_response = json.loads(final_text)
            except json.JSONDecodeError:
                raise HTTPException(status_code=500, detail="Claude MCP response was not valid JSON")
            strategies = _validate_and_filter_strategies(claude_response)
            print(f"  [MCP] {len(strategies)} strategies validated\n")
            return {"strategies": strategies, "wallet_id": request.wallet_id, "mode": "mcp"}

        if response.stop_reason == "tool_use":
            # Add assistant turn, then execute all tool calls and add results
            messages.append({"role": "assistant", "content": response.content})
            tool_results = []
            for block in response.content:
                if block.type == "tool_use":
                    print(f"  [MCP] tool call: {block.name}  input={block.input}")
                    result = await _execute_mcp_tool(block.name, block.input, request.wallet_id)
                    print(f"  [MCP] tool result keys: {list(result.keys())}")
                    tool_results.append({
                        "type": "tool_result",
                        "tool_use_id": block.id,
                        "content": json.dumps(result),
                    })
            messages.append({"role": "user", "content": tool_results})
            continue

        # Unexpected stop reason
        raise HTTPException(status_code=500, detail=f"Unexpected stop_reason: {response.stop_reason}")

    raise HTTPException(status_code=500, detail="MCP agentic loop exceeded max turns without completing")


# ==================== Strategy Execution ====================

class StrategyExecuteRequest(BaseModel):
    strategy_id: str
    wallet_id: str
    strategy: Optional[dict] = None  # full strategy object for summary generation


def _build_execution_summary(strategy: dict) -> dict:
    """Build a human-readable simulated execution summary from a strategy."""
    lines = []
    trade_actions = strategy.get("trade_actions", [])

    if not trade_actions:
        lines.append("No trades executed — position held unchanged.")
    else:
        for action in trade_actions:
            act = action.get("action", "unknown")
            asset_in = action.get("asset_in", "?")
            asset_out = action.get("asset_out", "?")
            amount = action.get("amount", 0)
            amount2 = action.get("amount2")
            pool = action.get("pool")
            deposit_mode = action.get("deposit_mode")
            interest_rate = action.get("interest_rate")
            term_days = action.get("term_days")

            if act == "swap":
                pool_str = f" via {pool} pool" if pool else ""
                lines.append(f"Swapped {amount} {asset_in} → {asset_out}{pool_str}")
            elif act == "deposit":
                if deposit_mode == "two_asset" and amount2:
                    lines.append(f"Two-asset deposit: {amount} {asset_in} + {amount2} {asset_out} into {pool or 'AMM'} pool")
                else:
                    lines.append(f"Single-asset deposit: {amount} {asset_in} into {pool or 'AMM'} pool")
            elif act == "withdraw":
                lines.append(f"Withdrew {amount} {asset_in} from {pool or 'AMM'} pool")
            elif act == "lend":
                rate_str = f" at {interest_rate}% APR" if interest_rate else ""
                term_str = f" for {term_days} days" if term_days else ""
                lines.append(f"Supplied {amount} {asset_in} to lending vault{rate_str}{term_str}")
            elif act == "borrow":
                rate_str = f" at {interest_rate}% APR" if interest_rate else ""
                term_str = f" for {term_days} days" if term_days else ""
                lines.append(f"Borrowed {amount} {asset_out} (collateral: {amount} {asset_in}){rate_str}{term_str}")

    # Compute simple IL estimate for deposit actions
    deposit_actions = [a for a in trade_actions if a.get("action") == "deposit"]
    il_estimate = None
    if deposit_actions:
        il_estimate = "Estimated IL at ±10% price move: ~-0.5%"

    fee_estimate = None
    if deposit_actions:
        fee_estimate = "Estimated Fee APR: 5-15% (depends on pool volume)"

    lend_actions = [a for a in trade_actions if a.get("action") == "lend"]
    if lend_actions:
        rates = [a.get("interest_rate", 0) for a in lend_actions if a.get("interest_rate")]
        if rates:
            fee_estimate = f"Lending yield: {sum(rates)/len(rates):.1f}% APR"

    return {
        "simulated": True,
        "summary_lines": lines,
        "il_estimate": il_estimate,
        "fee_estimate": fee_estimate,
        "net_cost": "Est. network fee: 0.000012 XRP per transaction",
    }


@app.post("/strategy/execute")
async def execute_strategy(request: StrategyExecuteRequest):
    """
    Execute a selected strategy (simulated).
    Builds a human-readable summary of what trades would have been performed.
    In production, this would build XRPL transactions and request wallet signatures.
    """
    try:
        # Build execution summary from strategy if provided
        execution_summary = _build_execution_summary(request.strategy or {})

        # Simulated transaction hash
        import hashlib, time
        raw = f"{request.strategy_id}-{request.wallet_id}-{time.time()}"
        tx_hash = hashlib.sha256(raw.encode()).hexdigest()[:64].upper()

        return {
            "tx_hash": tx_hash,
            "status": "confirmed",
            "wallet_id": request.wallet_id,
            "strategy_id": request.strategy_id,
            "execution_summary": execution_summary,
        }
    except Exception as e:
        raise HTTPException(status_code=500, detail=str(e))

@app.get("/strategy/status/{tx_hash}")
async def get_strategy_status(tx_hash: str):
    """
    Poll for the status of a strategy execution.
    """
    try:
        # Placeholder: return mock status
        return {
            "tx_hash": tx_hash,
            "status": "confirmed",
            "block_number": 12345,
            "timestamp": 1700000000
        }
    except Exception as e:
        raise HTTPException(status_code=500, detail=str(e))

# ==================== Main ====================

if __name__ == "__main__":
    import uvicorn
    # Run without reload - for reload support, use CLI:
    # uvicorn api:app --reload --host 0.0.0.0 --port 8000
    uvicorn.run(app, host="0.0.0.0", port=8000)
