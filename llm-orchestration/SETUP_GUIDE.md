# Quick Setup Guide for Intent Router

## TL;DR - Get Running in 5 Minutes

### 1. Install Ollama (One-time)
```bash
# macOS
brew install ollama

# Or download from https://ollama.ai/download
```

### 2. Install Dependencies
```bash
# From llm-orchestration directory
pip install -r requirements.txt
```

### 3. Generate gRPC Code
```bash
# Using Makefile (easiest)
make generate

# Or manually:
python -m grpc_tools.protoc \
  -I./proto \
  --python_out=./src \
  --grpc_python_out=./src \
  proto/intent_router.proto
```

### 4. Start Ollama Service
```bash
ollama serve
# This runs in foreground. Open new terminal for next step.
```

### 5. Pull Model
```bash
# New terminal
ollama pull llama2
```

### 6. Start Intent Router Service
```bash
python src/intent_router_service.py
# Should see: "Intent Router listening on [::]:50051"
```

### 7. Run Tests
```bash
# New terminal
python tests/test_intent_router.py
# Should see: "8/8 tests passed (100.0%)"
```

## Troubleshooting

### Error: "Cannot find grpc_tools.protoc"
```bash
pip install grpcio-tools
```

### Error: "Ollama not found"
```bash
# Ensure Ollama is installed and in PATH
which ollama

# If not in PATH, add to your shell profile:
# export PATH="/Applications/Ollama.app/Contents/MacOS:$PATH"
```

### Error: "Connection refused" when running tests
```bash
# Make sure Intent Router service is running
# In step 6, did you see "Intent Router listening on [::]:50051"?

# If yes, the tests should connect
# If no, check that Ollama service is running (step 4)
```

### LLM response is invalid JSON
```bash
# Try a different model:
ollama pull neural-chat

# Then restart service with:
python src/intent_router_service.py --model neural-chat
```

## Using Makefile (Recommended)

```bash
make help              # Show all commands
make install           # Install dependencies
make generate          # Generate gRPC code
make serve             # Start service
make test              # Run tests
make setup-ollama      # Download Ollama
make pull-model        # Download Llama 3.2 3B
make pull-model-fast   # Download Neural Chat (2x faster)
make clean             # Remove cache
```

## What Gets Created

After `generate`, you'll have:
- `src/intent_router_pb2.py` - Protocol Buffer message definitions
- `src/intent_router_pb2_grpc.py` - gRPC service stubs

These are auto-generated from `proto/intent_router.proto`.

## Architecture

```
User Query
   ↓
gRPC Request (IntentRequest)
   ↓
intent_router_service.py (Main service)
   ↓
Ollama (Local LLM)
   ↓
Llama 3.2 3B (Inference)
   ↓
Parse JSON Output
   ↓
gRPC Response (IntentResponse)
```

## Next: Integrate with Backend

Once tests pass, your Intent Router is ready. Send responses to your backend:

```python
# From test_intent_router.py or your own client
response = stub.ClassifyIntent(request)

# Send to backend
backend_client.send_intent(response)
```

## Performance Expectations

- **First run**: ~2 seconds (model loading)
- **Subsequent runs**: ~100-200ms per query
- **Throughput**: ~5-10 queries/second

## Still Stuck?

1. Check `intent_router_service.py` logs (verbose)
2. Check `raw_llm_output` in test responses
3. Verify Ollama is responding:
   ```bash
   ollama run llama2 "Hello"
   ```

---

**Ready to start?** → Run `make setup-ollama && make install && make generate`
