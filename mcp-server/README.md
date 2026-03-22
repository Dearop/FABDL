# VEGA MCP Server

MCP tool server that exposes VEGA's trading services to any MCP-compatible client.

## Setup

```bash
pip install -r requirements.txt
python server.py
```

The server communicates over stdio (MCP stdio transport).

## Tools

| Tool | Description |
|---|---|
| `route_intent` | Classify a natural language trading query into a structured intent |
| `analyze_portfolio` | Fetch XRPL AMM positions and compute quant risk metrics (IL, VaR, Sharpe, delta, fee APR) for a wallet |
| `generate_strategies` | Given a natural language query and wallet address, return 3 risk-ranked trading strategies |
| `get_lending_context` | Fetch current XLS-66d vault APYs and utilization rates for a given asset |
