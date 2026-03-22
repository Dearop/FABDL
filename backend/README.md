# Backend API

REST API that bridges frontend requests to the Intent Router gRPC service and handles strategy generation.

## Architecture

```
Frontend (Next.js)
    ↓ (HTTP)
Backend API (FastAPI)
    ↓ (gRPC)
Intent Router (Llama 3.2 via Ollama)
```

## Setup

### 1. Install Dependencies

```bash
pip install -r requirements.txt
```

### 2. Ensure Intent Router is Running

The backend expects the Intent Router gRPC service on `localhost:50051`:

```bash
# Terminal 1: Start Ollama
ollama serve

# Terminal 2: Start Intent Router
cd ../llm-orchestration
python src/intent_router_service.py
```

### 3. Start Backend API

```bash
python api.py
```

Expected output:
```
INFO:     Uvicorn running on http://0.0.0.0:8000
INFO:     Application startup complete
```

### 4. Start the Rust analysis backend on lending devnet

The API expects the Rust backend on `localhost:3001`. The backend now defaults to
XRPL lending devnet (`https://lend.devnet.rippletest.net:51234`) so it matches the
manual demo flow.

```bash
# Terminal 3: Start Rust backend with the lending-devnet default
cd ../fin-analysis-backend
cargo run
```

If you need to point Rust at a different network, override it explicitly:

```bash
XRPL_ENDPOINT=https://s.altnet.rippletest.net:51234 cargo run
```

## API Endpoints

### Wallet

- `POST /wallet/connect` — Connect a user's wallet
  ```json
  {
    "address": "rN7n7otQDd6FczFgLdlqtyMVrn3NnrcVcN",
    "network": "testnet"
  }
  ```

### Query

- `POST /query/classify` — Classify a query with Intent Router
- `POST /strategies/generate` — Generate 3 trading strategies
  ```json
  {
    "user_query": "Analyze my portfolio risk",
    "wallet_id": "rN7n7otQDd6FczFgLdlqtyMVrn3NnrcVcN"
  }
  ```

### Strategy Execution

- `POST /strategy/execute` — Execute a strategy
  ```json
  {
    "strategy_id": "option_a",
    "wallet_id": "rN7n7otQDd6FczFgLdlqtyMVrn3NnrcVcN"
  }
  ```

- `GET /strategy/status/{tx_hash}` — Poll for execution status

### Health

- `GET /health` — Check API health

## Usage in Frontend

Use the provided hooks and services:

```typescript
import { useTradingAssistant } from '@/hooks/useTradingAssistant'

export function TradingApp() {
  const {
    state,
    wallet,
    strategies,
    handleConnectWallet,
    handleSubmitQuery,
    handleExecuteStrategy
  } = useTradingAssistant()

  return (
    <>
      {state === 'disconnected' && (
        <button onClick={() => handleConnectWallet('rN7n...')}>
          Connect Wallet
        </button>
      )}

      {state === 'ready' && (
        <input
          placeholder="Ask something..."
          onKeyPress={(e) => {
            if (e.key === 'Enter') {
              handleSubmitQuery(e.currentTarget.value)
            }
          }}
        />
      )}

      {state === 'strategies_loaded' && (
        strategies.map((strategy) => (
          <button
            key={strategy.id}
            onClick={() => handleExecuteStrategy(strategy.id)}
          >
            {strategy.title}
          </button>
        ))
      )}
    </>
  )
}
```

## Development

### Auto-reload

The API runs with `reload=True`, so changes to `api.py` will automatically reload the server.

### Testing with curl

```bash
# Health check
curl http://localhost:8000/health

# Connect wallet
curl -X POST http://localhost:8000/wallet/connect \
  -H "Content-Type: application/json" \
  -d '{"address":"rN7n7otQDd6FczFgLdlqtyMVrn3NnrcVcN"}'

# Generate strategies
curl -X POST http://localhost:8000/strategies/generate \
  -H "Content-Type: application/json" \
  -d '{"user_query":"Analyze my risk","wallet_id":"rN7n7otQDd6FczFgLdlqtyMVrn3NnrcVcN"}'
```

## Next Steps

- [ ] Integrate with real Claude API for strategy generation
- [ ] Implement XRPL wallet queries
- [ ] Add transaction signing with Otsu Wallet
- [ ] Implement transaction broadcasting to XRPL
- [ ] Add database for storing strategies/transactions
- [ ] Add authentication/authorization
