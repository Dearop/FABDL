.PHONY: install generate serve test clean help

help:
	@echo "Intent Router - Make Commands"
	@echo ""
	@echo "install      - Install Python dependencies"
	@echo "generate     - Generate gRPC code from .proto"
	@echo "serve        - Start the Intent Router service"
	@echo "test         - Run test suite"
	@echo "test-verbose - Run tests with verbose output"
	@echo "setup-ollama  - Download and setup Ollama"
	@echo "pull-model   - Pull Llama 3.2 model"
	@echo "clean        - Remove generated files and cache"
	@echo ""

install:
	pip install -r requirements.txt
	@echo "✓ Dependencies installed"

generate:
	python -m grpc_tools.protoc \
		-I./proto \
		--python_out=./src \
		--grpc_python_out=./src \
		proto/intent_router.proto
	@echo "✓ gRPC code generated"

serve:
	cd src && python intent_router_service.py --model llama2 --host "[::]:50051"

serve-neural-chat:
	cd src && python intent_router_service.py --model neural-chat --host "[::]:50051"

test:
	cd tests && python test_intent_router.py

test-verbose:
	cd tests && python -u test_intent_router.py

setup-ollama:
	@echo "Installing Ollama..."
	@command -v ollama >/dev/null 2>&1 || { \
		echo "Downloading Ollama..."; \
		curl -fsSL https://ollama.ai/install.sh | sh; \
	}
	@echo "✓ Ollama ready"
	@echo "Start with: ollama serve"

pull-model:
	ollama pull llama2
	@echo "✓ Llama 3.2 3B model downloaded"

pull-model-fast:
	ollama pull neural-chat
	@echo "✓ Neural Chat model downloaded (faster inference)"

clean:
	find . -type d -name __pycache__ -exec rm -rf {} + 2>/dev/null || true
	find . -name "*.pyc" -delete
	find . -name "*.pyo" -delete
	rm -f src/intent_router_pb2*.py
	@echo "✓ Cache cleaned"

docker-build:
	docker build -t intent-router:latest .
	@echo "✓ Docker image built"

docker-run:
	docker run -p 50051:50051 intent-router:latest
	@echo "✓ Docker container started"
