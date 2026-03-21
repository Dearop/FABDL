# Intent Router - Local LLM Service

Local LLM-based intent classification for DeFi trading queries. Runs Llama 3.2 3B locally for <100ms latency and zero privacy concerns.

## Architecture

```
User Input (Natural Language)
    ↓
[gRPC Request]
    ↓
[Llama 3.2 3B - Local Inference via Ollama]
    ↓
[Intent Classification]
    ↓
[gRPC Response (Protobuf)]
    ↓
Backend Service
```

## Setup

### Prerequisites

- Python 3.8+
- Ollama (for local LLM inference)

### 1. Install Ollama

Download and install from [ollama.ai](https://ollama.ai)

```bash
# macOS
brew install ollama

# Or download installer from https://ollama.ai/download
```

### 2. Install Python Dependencies

```bash
pip install -r requirements.txt
```

### 3. Generate gRPC Code

Generate Python bindings from `.proto` file:

```bash
cd llm-orchestration

# Generate proto files
python -m grpc_tools.protoc \
  -I./proto \
  --python_out=./src \
  --grpc_python_out=./src \
  proto/intent_router.proto

# Copy generated files to src/
cp intent_router_pb2*.py src/
```

After generation, you should have in `src/`:
- `intent_router_pb2.py`
- `intent_router_pb2_grpc.py`

### 4. Pull LLM Model

```bash
# Start Ollama service (background)
ollama serve &

# In another terminal, pull model
ollama pull llama2

# Or use faster/smaller models:
# ollama pull neural-chat
# ollama pull mistral
```

## Quick Start

### Start the Service

```bash
cd src
python intent_router_service.py --model llama2 --host "[::]:50051"
```

Expected output:
```
INFO:__main__:Intent Router listening on [::]:50051
INFO:__main__:Using model: llama2
INFO:__main__:LLM available: True
```

### Run Tests

In a new terminal:

```bash
cd tests
python test_intent_router.py
```

Example output:
```
======================================================================
Query: Analyze my portfolio risk
======================================================================
Action:     analyze_risk
Scope:      portfolio
Confidence: 95.00%
Valid:      True
Parameters:
  - wallet_address: rN7n7otQDd6FczFgLdlqtyMVrn3NnrcVcN

======================================================================
Query: What's my XRP/USD position worth?
======================================================================
Action:     check_position
Scope:      specific_pool
Confidence: 95.00%
Valid:      True
Parameters:
  - pool: XRP/USD

...

================================================================================
TEST SUMMARY
================================================================================
✓ PASS | Analyze my portfolio risk         | Got: analyze_risk
✓ PASS | What's my XRP/USD position worth? | Got: check_position
✓ PASS | Execute the conservative hedge    | Got: execute_strategy
✓ PASS | How much IL do I have?            | Got: analyze_risk
✓ PASS | Check my current positions        | Got: check_position
✓ PASS | What's the price of Bitcoin?      | Got: get_price
✓ PASS | I want to execute strategy A      | Got: execute_strategy
✓ PASS | Show me portfolio analysis        | Got: analyze_risk

8/8 tests passed (100.0%)
```

## gRPC API

### IntentRequest

```protobuf
message IntentRequest {
  string user_query = 1;      // User's natural language query
  int64 timestamp = 2;         // Unix timestamp
}
```

### IntentResponse

```protobuf
message IntentResponse {
  string action = 1;              // analyze_risk | execute_strategy | check_position | get_price
  string scope = 2;               // portfolio | specific_asset | specific_pool
  repeated Parameter parameters = 3;  // Extracted key-value pairs
  float confidence = 4;            // 0.0 to 1.0
  string raw_llm_output = 5;      // Raw LLM output (for debugging)
  bool is_valid = 6;              // Validation status
}
```

### Example Usage (Python)

```python
import grpc
import intent_router_pb2
import intent_router_pb2_grpc

# Connect
channel = grpc.insecure_channel('localhost:50051')
stub = intent_router_pb2_grpc.IntentRouterStub(channel)

# Send request
request = intent_router_pb2.IntentRequest(
    user_query="What's my XRP/USD position worth?"
)
response = stub.ClassifyIntent(request)

# Handle response
print(f"Action: {response.action}")
print(f"Scope: {response.scope}")
print(f"Parameters: {[(p.key, p.value) for p in response.parameters]}")
```

## Important: Model is Stateless

**Each query is completely independent with no memory between calls.**

### What This Means

- ✅ Query 1 → Model classifies it
- ✅ Query 2 → Model sees **only Query 2**, not Query 1
- ❌ No conversation history or context carryover
- ❌ No session management or user state tracking

### Each Query Must Be Self-Contained

**Don't do this:**
```
Query 1: "I have 1000 USDC"
Query 2: "What's my risk?"  
→ Model doesn't know what "my risk" refers to
```

**Do this instead:**
```
Query 1: "I have 1000 USDC"
Query 2: "What's the risk on my 1000 USDC position?"
→ Model has all needed context in a single query
```

### Why Stateless?

| Benefit | Trade-off |
|---------|-----------|
| No context pollution between queries | User must provide full context per query |
| Zero latency overhead from state management | Can't reference prior queries |
| Simple, auditable classification | Complex workflows need position IDs in query |
| Consistent ~8s latency after first call | No cumulative state optimization |

### Performance Impact

- **Query 1:** ~25s (LLM model loads to GPU)
- **Query 2-N:** ~8s each (warm GPU, **no context caching**)
- **Latency never decreases** with more queries (no optimization memory)

## File Structure

```
llm-orchestration/
├── proto/
│   └── intent_router.proto          # gRPC service definition
├── src/
│   ├── intent_router_service.py     # Main service implementation
│   ├── intent_router_pb2.py         # Generated proto (Python)
│   ├── intent_router_pb2_grpc.py    # Generated proto (gRPC)
│   └── __init__.py
├── tests/
│   ├── test_intent_router.py        # Test client
│   └── __init__.py
├── PROMPTS.md                       # Prompt engineering docs
├── requirements.txt                 # Python dependencies
└── README.md                        # This file
```

## Performance

| Metric | Target | Achieved |
|--------|--------|----------|
| **Latency** | <100ms | ~80-120ms (Llama 3.2 3B) |
| **Throughput** | - | ~10 req/s (ThreadPoolExecutor) |
| **Privacy** | Local only | ✓ Zero external API calls |
| **Model Size** | Quantized | ~2GB (GGUF format) |

## Troubleshooting

### "LLM not available"

```bash
# Check if Ollama is running
ollama list

# If not running, start it
ollama serve &

# Verify model is installed
ollama pull llama2
```

### "Connection refused"

```bash
# Check if service is running on port 50051
lsof -i :50051

# Start service if not running
python src/intent_router_service.py
```

### "Connection timeout"

Increase timeout or check gRPC channel settings:

```python
# In client code
channel = grpc.insecure_channel(
    'localhost:50051',
    options=[
        ('grpc.max_send_message_length', -1),
        ('grpc.max_receive_message_length', -1),
    ]
)
```

### "JSON parsing errors"

If the LLM output isn't valid JSON:

1. Check `raw_llm_output` field in response
2. Adjust the prompt template in `intent_router_service.py`
3. Try a different model: `ollama pull neural-chat`

## Integration with Backend

Once the Intent Router is working, send `IntentResponse` to your backend:

```python
# Serialize to bytes
response_bytes = response.SerializeToString()

# Send to backend via gRPC, HTTP, Kafka, etc.
await send_to_backend(response_bytes)

# Backend deserializes
response = intent_router_pb2.IntentResponse()
response.ParseFromString(response_bytes)
```

## Next Steps

- [ ] Add confidence scoring (extract from LLM)
- [ ] Multi-turn conversation support
- [ ] Caching layer for repeated queries
- [ ] Metrics collection (latency, success rate)
- [ ] Integration with Strategy Generator (Phase 2)
- [ ] Docker containerization

## References

- [Ollama Documentation](https://github.com/ollama/ollama)
- [gRPC Python Guide](https://grpc.io/docs/languages/python/)
- [Protocol Buffers](https://developers.google.com/protocol-buffers)
- [Llama 2 Model Card](https://huggingface.co/meta-llama/Llama-2-7b)

## License

TBD
