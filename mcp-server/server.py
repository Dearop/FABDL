"""
MCP server that wraps VEGA's existing services (Intent Router, Rust quant engine,
FastAPI strategy generator) as four callable tools for any MCP-compatible client.
"""

import asyncio
import json
import sys
import os

import grpc
import httpx
from mcp.server import Server
from mcp.server.stdio import run_stdio
from mcp.types import Tool, TextContent

# ── Proto stubs (same path the FastAPI backend uses) ──
sys.path.insert(0, os.path.join(os.path.dirname(__file__), "../llm-orchestration/src"))
import intent_router_pb2
import intent_router_pb2_grpc

# ── Service addresses ──
INTENT_ROUTER_ADDR = os.environ.get("INTENT_ROUTER_ADDR", "localhost:50051")
RUST_BACKEND_URL = os.environ.get("RUST_BACKEND_URL", "http://localhost:3001")
FASTAPI_URL = os.environ.get("FASTAPI_URL", "http://localhost:8000")

HTTP_TIMEOUT = 30.0

app = Server("vega-mcp")


# ════════════════════════════════════════════════════════
#  Tool definitions
# ════════════════════════════════════════════════════════

TOOLS = [
    Tool(
        name="route_intent",
        description="Classify a natural language trading query into a structured intent",
        inputSchema={
            "type": "object",
            "properties": {
                "query": {"type": "string", "description": "Natural language trading query"},
            },
            "required": ["query"],
        },
    ),
    Tool(
        name="analyze_portfolio",
        description=(
            "Fetch XRPL AMM positions and compute quant risk metrics "
            "(IL, VaR, Sharpe, delta, fee APR) for a wallet"
        ),
        inputSchema={
            "type": "object",
            "properties": {
                "wallet_id": {"type": "string", "description": "XRPL wallet address (r...)"},
                "pool": {
                    "type": ["string", "null"],
                    "description": "AMM pool pair, e.g. 'XRP/USD'. null for whole portfolio.",
                },
            },
            "required": ["wallet_id"],
        },
    ),
    Tool(
        name="generate_strategies",
        description=(
            "Given a natural language query and wallet address, "
            "return 3 risk-ranked trading strategies"
        ),
        inputSchema={
            "type": "object",
            "properties": {
                "user_query": {"type": "string", "description": "Natural language trading query"},
                "wallet_id": {"type": "string", "description": "XRPL wallet address"},
            },
            "required": ["user_query", "wallet_id"],
        },
    ),
    Tool(
        name="get_lending_context",
        description=(
            "Fetch the lending-related risk summary for a wallet and asset"
        ),
        inputSchema={
            "type": "object",
            "properties": {
                "asset": {"type": "string", "description": "Asset ticker, e.g. 'XRP'"},
                "wallet_id": {"type": "string", "description": "XRPL wallet address"},
            },
            "required": ["asset", "wallet_id"],
        },
    ),
]


# ════════════════════════════════════════════════════════
#  Tool implementations
# ════════════════════════════════════════════════════════

async def _route_intent(query: str) -> dict:
    """Call the Intent Router gRPC service."""
    try:
        channel = grpc.aio.insecure_channel(INTENT_ROUTER_ADDR)
        stub = intent_router_pb2_grpc.IntentRouterStub(channel)
        request = intent_router_pb2.IntentRequest(
            user_query=query,
            timestamp=int(asyncio.get_event_loop().time()),
        )
        response = await stub.ClassifyIntent(request)
        await channel.close()
        return {
            "action": response.action,
            "scope": response.scope,
            "confidence": round(response.confidence, 2),
            "parameters": [{"key": p.key, "value": p.value} for p in response.parameters],
        }
    except Exception as exc:
        return {"error": f"Intent Router gRPC call failed: {exc}"}


async def _analyze_portfolio(wallet_id: str, pool: str | None) -> dict:
    """POST to Rust /analyze with action=analyze_risk."""
    payload = {
        "action": "analyze_risk",
        "scope": "portfolio",
        "parameters": {"wallet_address": wallet_id},
        "confidence": 1.0,
    }
    if pool:
        payload["parameters"]["pool"] = pool
    try:
        async with httpx.AsyncClient(timeout=HTTP_TIMEOUT) as client:
            resp = await client.post(f"{RUST_BACKEND_URL}/analyze", json=payload)
        if resp.status_code != 200:
            return {"error": f"Rust backend returned {resp.status_code}: {resp.text}"}
        # Rust may return plain-text prompt or JSON; try JSON first
        try:
            return resp.json()
        except json.JSONDecodeError:
            return {"raw": resp.text}
    except Exception as exc:
        return {"error": f"Rust backend request failed: {exc}"}


async def _generate_strategies(user_query: str, wallet_id: str) -> dict:
    """POST to FastAPI /strategies/generate."""
    payload = {"user_query": user_query, "wallet_id": wallet_id}
    try:
        async with httpx.AsyncClient(timeout=HTTP_TIMEOUT) as client:
            resp = await client.post(f"{FASTAPI_URL}/strategies/generate", json=payload)
        if resp.status_code != 200:
            return {"error": f"Strategy service returned {resp.status_code}: {resp.text}"}
        data = resp.json()
        return {"strategies": data.get("strategies", [])}
    except Exception as exc:
        return {"error": f"Strategy generation request failed: {exc}"}


async def _get_lending_context(asset: str, wallet_id: str) -> dict:
    """POST to Rust /analyze with action=analyze_risk and return the text summary."""
    payload = {
        "action": "analyze_risk",
        "scope": "portfolio",
        "parameters": {"wallet_address": wallet_id, "focus": asset},
        "confidence": 1.0,
    }
    try:
        async with httpx.AsyncClient(timeout=HTTP_TIMEOUT) as client:
            resp = await client.post(f"{RUST_BACKEND_URL}/analyze", json=payload)
        if resp.status_code != 200:
            return {"error": f"Rust backend returned {resp.status_code}: {resp.text}"}
        return {
            "asset": asset,
            "risk_summary": resp.text,
        }
    except Exception as exc:
        return {"error": f"Lending context request failed: {exc}"}


# ════════════════════════════════════════════════════════
#  MCP handlers
# ════════════════════════════════════════════════════════

@app.list_tools()
async def list_tools() -> list[Tool]:
    return TOOLS


@app.call_tool()
async def call_tool(name: str, arguments: dict) -> list[TextContent]:
    if name == "route_intent":
        result = await _route_intent(arguments["query"])
    elif name == "analyze_portfolio":
        result = await _analyze_portfolio(
            arguments["wallet_id"],
            arguments.get("pool"),
        )
    elif name == "generate_strategies":
        result = await _generate_strategies(
            arguments["user_query"],
            arguments["wallet_id"],
        )
    elif name == "get_lending_context":
        result = await _get_lending_context(
            arguments["asset"],
            arguments["wallet_id"],
        )
    else:
        result = {"error": f"Unknown tool: {name}"}

    return [TextContent(type="text", text=json.dumps(result, indent=2))]


# ════════════════════════════════════════════════════════
#  Entry point
# ════════════════════════════════════════════════════════

async def main():
    await run_stdio(app)


if __name__ == "__main__":
    asyncio.run(main())
