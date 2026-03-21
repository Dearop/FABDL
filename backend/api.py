"""
Backend API that bridges the frontend HTTP requests to the Intent Router gRPC service.
Handles wallet connection, query classification, strategy generation, and execution.
"""

from fastapi import FastAPI, HTTPException
from fastapi.middleware.cors import CORSMiddleware
from pydantic import BaseModel
import grpc
import sys
import os
import asyncio
import json
import re
from typing import Optional
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

CLAUDE_SYSTEM_PROMPT = (
    "You are a quantitative trading strategist specializing in automated market makers "
    "(AMMs) on the XRPL blockchain.\n\n"
    "Your role:\n"
    "1. Analyze portfolio risk metrics\n"
    "2. Generate 2-3 concrete trading strategies\n"
    "3. Explain trade-offs in simple terms\n"
    "4. Provide numerical projections\n\n"
    "Constraints:\n"
    "- Be concise (max 50 words per strategy)\n"
    "- Avoid jargon (explain 'delta' as 'directional exposure')\n"
    "- Always include a 'Do Nothing' option\n"
    "- Never recommend strategies with >1% slippage\n"
    "- Cap risk_score at 8/10\n\n"
    "Output: Structured JSON only. No prose before or after the JSON block.\n"
    'Schema: {"strategies": [...], "recommendation": "option_id", "reasoning": "..."}'
)

ALLOWED_ASSETS = {"XRP", "USD", "BTC", "ETH", "USDC", "USDT"}

# ==================== Request/Response Models ====================

class WalletConnectRequest(BaseModel):
    address: str
    network: str = "testnet"

class QueryRequest(BaseModel):
    user_query: str
    wallet_id: str

class StrategySelectRequest(BaseModel):
    strategy_id: str
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
    """Enforce safety guardrails from PROMPTS.md."""
    if not {"id", "title", "description", "risk_score"}.issubset(s.keys()):
        return False
    if not isinstance(s.get("risk_score"), (int, float)) or s["risk_score"] > 8:
        return False
    for action in s.get("trade_actions", []):
        if action.get("estimated_slippage", 0) > 1.0:
            return False
        for key in ("asset_in", "asset_out"):
            if action.get(key) and action[key] not in ALLOWED_ASSETS:
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
            "title": "Do Nothing: Ride It Out",
            "description": "Keep current position. Monitor fees vs. IL daily.",
            "risk_score": 4,
            "projected_return_7d": {"best_case": "$0", "expected": "$0", "worst_case": "$0"},
            "trade_actions": [],
            "pros": ["Zero transaction cost", "Simple"],
            "cons": ["Exposed to IL if XRP moves"],
        },
        {
            "id": "option_b",
            "title": "Exit Position",
            "description": "Withdraw all LP tokens and hold XRP and USD separately.",
            "risk_score": 2,
            "projected_return_7d": {"best_case": "$0", "expected": "$0", "worst_case": "$0"},
            "trade_actions": [],
            "pros": ["Eliminates IL risk"],
            "cons": ["Stops earning fees"],
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
            "title": "Do Nothing",
            "description": "Cancel and keep current position unchanged.",
            "risk_score": 4,
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

# ==================== Strategy Execution ====================

@app.post("/strategy/execute")
async def execute_strategy(request: StrategySelectRequest):
    """
    Execute a selected strategy.
    In a real implementation, this would:
    1. Build XRPL transaction
    2. Request user signature via Otsu Wallet
    3. Broadcast to XRPL
    4. Return transaction hash
    """
    try:
        # Placeholder: return mock transaction hash
        return {
            "tx_hash": "0x1234567890abcdef",
            "status": "pending",
            "wallet_id": request.wallet_id,
            "strategy_id": request.strategy_id
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
