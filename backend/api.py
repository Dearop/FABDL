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
from typing import Optional

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
    Generate 3 trading strategies based on a query.
    Flow:
    1. Classify intent (Intent Router)
    2. Fetch wallet data from XRPL
    3. Generate strategies (Claude/Quant LLM)
    4. Return 3 options
    """
    try:
        print(f"\n" + "="*60)
        print(f"🚀 [Backend] /strategies/generate called")
        print(f"   Wallet: {request.wallet_id}")
        print(f"   Query: '{request.user_query}'")
        print(f"="*60)
        
        # Step 1: Classify intent
        print(f"\n⏳ [Backend] Step 1: Classifying intent...")
        intent = await classify_intent_with_grpc(request.user_query)
        print(f"✅ [Backend] Intent classification complete")
        
        # Step 2: Fetch wallet data from XRPL (placeholder)
        print(f"\n⏳ [Backend] Step 2: Fetching wallet data...")
        wallet_data = {
            "total_value_usd": 50000,
            "impermanent_loss_pct": 2.3,
            "delta_xrp": 1500,
            "sharpe_ratio": 1.2,
            "positions": [
                {
                    "pool": "XRP/USD",
                    "lp_tokens": 2000,
                    "share_of_pool": 0.04
                }
            ]
        }
        print(f"✅ [Backend] Wallet data fetched: ${wallet_data['total_value_usd']} USD")
        
        # Step 3: Generate strategies (placeholder - replace with real Claude call)
        print(f"\n⏳ [Backend] Step 3: Generating strategies...")
        strategies = _generate_placeholder_strategies(intent, wallet_data)
        print(f"✅ [Backend] Generated {len(strategies)} strategies")
        
        print(f"\n✅ [Backend] /strategies/generate complete - returning to frontend\n")
        
        return {
            "intent": intent,
            "strategies": strategies,
            "wallet_id": request.wallet_id
        }
    
    except Exception as e:
        print(f"❌ [Backend] Error: {e}")
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

# ==================== Helper Functions ====================

def _generate_placeholder_strategies(intent: dict, wallet_data: dict) -> list:
    """
    Generate placeholder strategies.
    Replace this with actual Claude/Quant LLM call.
    """
    return [
        {
            "id": "option_a",
            "title": "Conservative: Full Delta Hedge",
            "description": "Sell 1,500 XRP to lock in current value. Removes price risk, keeps earning fees.",
            "risk_score": 2,
            "projected_return_7d": {
                "best_case": "$150",
                "expected": "$80",
                "worst_case": "-$20"
            },
            "trade_actions": [
                {
                    "action": "swap",
                    "asset_in": "XRP",
                    "asset_out": "USD",
                    "amount": 1500,
                    "estimated_slippage": 0.3
                }
            ],
            "pros": ["No price risk", "Still earn fees"],
            "cons": ["Miss upside if XRP pumps", "Small swap fee"]
        },
        {
            "id": "option_b",
            "title": "Yield-Focused: Rebalance to High-Fee Pool",
            "description": "Move 25% of liquidity to XRP/BTC pool. Higher fees (20% APY) but more volatility.",
            "risk_score": 6,
            "projected_return_7d": {
                "best_case": "$200",
                "expected": "$110",
                "worst_case": "-$80"
            },
            "trade_actions": [
                {
                    "action": "withdraw",
                    "pool": "XRP/USD",
                    "lp_tokens": 500
                },
                {
                    "action": "deposit",
                    "pool": "XRP/BTC",
                    "asset1_amount": 12500,
                    "asset2_amount": 0.25
                }
            ],
            "pros": ["Higher fee income", "BTC correlation hedge"],
            "cons": ["XRP/BTC can diverge", "Two transaction fees"]
        },
        {
            "id": "option_c",
            "title": "Do Nothing: Ride It Out",
            "description": "Keep current position. IL is temporary if XRP stabilizes. Fees are strong.",
            "risk_score": 5,
            "projected_return_7d": {
                "best_case": "$180",
                "expected": "$100",
                "worst_case": "-$150"
            },
            "trade_actions": [],
            "pros": ["Zero fees", "Simple", "Fees offset IL over time"],
            "cons": ["Exposed to further IL if XRP moves", "No active risk management"]
        }
    ]

# ==================== Main ====================

if __name__ == "__main__":
    import uvicorn
    # Run without reload - for reload support, use CLI:
    # uvicorn api:app --reload --host 0.0.0.0 --port 8000
    uvicorn.run(app, host="0.0.0.0", port=8000)
