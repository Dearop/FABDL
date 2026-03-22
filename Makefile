.PHONY: help check-deps install install-frontend install-backend install-intent install-rust pull-model \
        setup dev start-ollama start-intent start-backend start-frontend start-rust start-all stop clean logs

# Ollama binary — override if not on PATH: make start-ollama OLLAMA=path/to/ollama
OLLAMA ?= ollama

# Colors
BLUE   := \033[0;34m
GREEN  := \033[0;32m
YELLOW := \033[0;33m
RED    := \033[0;31m
NC     := \033[0m

# ==================== Help ====================

help:
	@echo "$(BLUE)━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━$(NC)"
	@echo "$(BLUE) VEGA — XRPL AI Trading Assistant$(NC)"
	@echo "$(BLUE)━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━$(NC)"
	@echo ""
	@echo "$(GREEN)First time?$(NC)"
	@echo "  $(YELLOW)make dev$(NC)               Check prerequisites, install everything, show next steps"
	@echo ""
	@echo "$(GREEN)Setup:$(NC)"
	@echo "  $(YELLOW)make check-deps$(NC)        Verify all required tools are installed"
	@echo "  $(YELLOW)make install$(NC)           Install all package dependencies"
	@echo "  $(YELLOW)make pull-model$(NC)        Pull the Llama 3.2 3B model into Ollama"
	@echo ""
	@echo "$(GREEN)Start services (each in its own terminal):$(NC)"
	@echo "  $(YELLOW)make start-ollama$(NC)      Ollama LLM server          :11434"
	@echo "  $(YELLOW)make start-intent$(NC)      Intent Router (gRPC)       :50051"
	@echo "  $(YELLOW)make start-rust$(NC)        Rust analysis backend      :3001"
	@echo "  $(YELLOW)make start-backend$(NC)     Python API                 :8000"
	@echo "  $(YELLOW)make start-frontend$(NC)    Next.js frontend           :3000"
	@echo ""
	@echo "$(GREEN)Utilities:$(NC)"
	@echo "  $(YELLOW)make setup$(NC)             Print full startup instructions"
	@echo "  $(YELLOW)make clean$(NC)             Remove build artefacts and caches"
	@echo "  $(YELLOW)make logs$(NC)              Show expected console output per service"
	@echo ""

# ==================== First-time quickstart ====================

dev: check-deps install pull-model
	@echo ""
	@echo "$(GREEN)━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━$(NC)"
	@echo "$(GREEN) All dependencies installed. Follow the steps below to start.$(NC)"
	@echo "$(GREEN)━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━$(NC)"
	@echo ""
	@echo "$(YELLOW)1. Set your Anthropic API key (required):$(NC)"
	@echo "   export ANTHROPIC_API_KEY=sk-ant-..."
	@echo ""
	@echo "$(YELLOW)2. Open 5 terminal tabs and run one command in each:$(NC)"
	@echo ""
	@echo "   $(BLUE)[Tab 1]$(NC) make start-ollama"
	@echo "   $(BLUE)[Tab 2]$(NC) make start-intent"
	@echo "   $(BLUE)[Tab 3]$(NC) make start-rust"
	@echo "   $(BLUE)[Tab 4]$(NC) make start-backend"
	@echo "   $(BLUE)[Tab 5]$(NC) make start-frontend"
	@echo ""
	@echo "$(GREEN)Terminal 2 - Intent Router (gRPC)$(NC)"
	@echo "  $$ make start-intent"
	@echo "  Expected: 'Intent Router listening on [::]:50051'"
	@echo ""
	@echo "$(GREEN)Terminal 3 - Rust Analysis Backend$(NC)"
	@echo "  $$ make start-rust"
	@echo "  Expected: 'listening on 0.0.0.0:3001'"
	@echo "  Expected: 'starting fin-analysis-backend endpoint=https://lend.devnet.rippletest.net:51234 network=lending-devnet port=3001'"
	@echo "            'listening on 0.0.0.0:3001'"

	@echo ""
	@echo "$(GREEN)Terminal 4 - Backend API$(NC)"
	@echo "  $$ make start-backend"
	@echo "  Expected: 'Uvicorn running on http://0.0.0.0:8000'"
	@echo ""
	@echo "$(GREEN)Terminal 5 - Frontend$(NC)"
	@echo "  $$ make start-frontend"
	@echo "  Expected: 'Ready in XXXms'"
	@echo ""
	@echo "$(GREEN)Step 4: Open Browser$(NC)"
	@echo "  Visit: http://localhost:3000"
	@echo ""

# ==================== Dependency check ====================

check-deps:
	@echo "$(BLUE)Checking prerequisites...$(NC)"
	@command -v node >/dev/null 2>&1 \
		&& echo "  $(GREEN)✔$(NC) Node.js  $$(node --version)" \
		|| (echo "  $(RED)✘ Node.js not found.$(NC) Install from https://nodejs.org (v18+)" && exit 1)
	@command -v python3 >/dev/null 2>&1 \
		&& echo "  $(GREEN)✔$(NC) Python   $$(python3 --version)" \
		|| (echo "  $(RED)✘ Python 3 not found.$(NC) Install from https://python.org (3.10+)" && exit 1)
	@command -v cargo >/dev/null 2>&1 \
		&& echo "  $(GREEN)✔$(NC) Rust     $$(cargo --version)" \
		|| (echo "  $(RED)✘ Rust not found.$(NC) Install from https://rustup.rs" && exit 1)
	@command -v $(OLLAMA) >/dev/null 2>&1 \
		&& echo "  $(GREEN)✔$(NC) Ollama   $$($(OLLAMA) --version 2>/dev/null || echo '(version unknown)')" \
		|| (echo "  $(RED)✘ Ollama not found.$(NC) Install from https://ollama.ai" && exit 1)
	@echo "  $(GREEN)✔$(NC) All prerequisites satisfied."

# ==================== Installation ====================

install: install-frontend install-backend install-intent install-rust
	@echo "$(GREEN)✅ All dependencies installed.$(NC)"

install-frontend:
	@echo "$(YELLOW)Installing frontend dependencies (npm)...$(NC)"
	cd frontend && npm install

install-backend:
	@echo "$(YELLOW)Installing backend Python dependencies...$(NC)"
	cd backend && pip install -r requirements.txt

install-intent:
	@echo "$(YELLOW)Installing intent-router Python dependencies...$(NC)"
	cd llm-orchestration && pip install -r requirements.txt

install-rust:
	@echo "$(YELLOW)Building Rust analysis backend...$(NC)"
	cd fin-analysis-backend && cargo build --release

pull-model:
	@echo "$(YELLOW)Pulling Llama 3.2 3B model into Ollama...$(NC)"
	$(OLLAMA) pull llama3.2:3b
	@echo "$(GREEN)✅ Model ready.$(NC)"

# ==================== Start services ====================

start-ollama:
	@echo "$(BLUE)━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━$(NC)"
	@echo "$(BLUE)🧠 Ollama LLM server  →  :11434$(NC)"
	@echo "$(BLUE)━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━$(NC)"
	$(OLLAMA) serve

start-intent:
	@echo "$(BLUE)━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━$(NC)"
	@echo "$(BLUE)🔄 Intent Router (gRPC)  →  :50051$(NC)"
	@echo "$(BLUE)━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━$(NC)"
	cd llm-orchestration && python3 src/intent_router_service.py --model llama3.2:3b

start-backend:
	@echo "$(BLUE)━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━$(NC)"
	@echo "$(BLUE)⚡ Python API  →  :8000$(NC)"
	@echo "$(BLUE)━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━$(NC)"
	cd backend && python3 api.py

start-frontend:
	@echo "$(BLUE)━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━$(NC)"
	@echo "$(BLUE)🎨 Frontend  →  http://localhost:3000$(NC)"
	@echo "$(BLUE)━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━$(NC)"
	cd frontend && npm run dev

start-rust:
	@echo "$(BLUE)━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━$(NC)"
	@echo "$(BLUE)🦀 Rust analysis backend  →  :3001$(NC)"
	@echo "$(BLUE)━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━$(NC)"
	cd fin-analysis-backend && PORT=3001 cargo run --release

# ==================== Setup instructions ====================

setup:
	@echo "$(BLUE)━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━$(NC)"
	@echo "$(BLUE) VEGA — Full Setup Instructions$(NC)"
	@echo "$(BLUE)━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━$(NC)"
	@echo ""
	@echo "$(GREEN)Required tools:$(NC)"
	@echo "  • Node.js 18+     https://nodejs.org"
	@echo "  • Python 3.10+    https://python.org"
	@echo "  • Rust / Cargo    https://rustup.rs"
	@echo "  • Ollama          https://ollama.ai"
	@echo ""
	@echo "$(GREEN)Required env var:$(NC)"
	@echo "  export ANTHROPIC_API_KEY=sk-ant-...   (Claude strategy generation)"
	@echo "  $$ export RUST_BACKEND_URL=http://localhost:3001"
	@echo ""
	@echo "$(GREEN)Optional env var:$(NC)"
	@echo ""
	@echo "$(GREEN)One-time setup:$(NC)"
	@echo "  make dev          # checks deps, installs packages, pulls Llama model"
	@echo ""
	@echo "$(GREEN)Start all 5 services (one per terminal tab):$(NC)"
	@echo ""
	@echo "  $(YELLOW)[Tab 1]$(NC) make start-ollama    # LLM engine          :11434"
	@echo "  $(YELLOW)[Tab 2]$(NC) make start-intent    # gRPC intent router  :50051"
	@echo "  $(YELLOW)[Tab 3]$(NC) make start-rust      # Rust quant backend  :3001"
	@echo "  $(YELLOW)[Tab 4]$(NC) make start-backend   # Python API          :8000"
	@echo "  $(YELLOW)[Tab 5]$(NC) make start-frontend  # Next.js frontend    :3000"
	@echo ""
	@echo "  Then open http://localhost:3000"
	@echo ""

start-all: setup

# ==================== Utilities ====================

clean:
	@echo "$(YELLOW)Cleaning build artefacts...$(NC)"
	rm -rf frontend/node_modules frontend/.next
	find backend llm-orchestration -type d -name __pycache__ -exec rm -rf {} + 2>/dev/null || true
	find . -type f -name "*.pyc" -delete
	@echo "$(GREEN)✅ Clean complete.$(NC)"

logs:
	@echo "$(BLUE)━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━$(NC)"
	@echo "$(BLUE) Expected output when all 5 services are healthy$(NC)"
	@echo "$(BLUE)━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━$(NC)"
	@echo ""
	@echo "$(GREEN)[Tab 1] Ollama:$(NC)"
	@echo "  Listening on 127.0.0.1:11434"
	@echo ""
	@echo "$(GREEN)[Tab 2] Intent Router:$(NC)"
	@echo "  INFO:__main__:Intent Router listening on [::]:50051"
	@echo "  INFO:__main__:Using model: llama3.2:3b"
	@echo "  INFO:__main__:LLM available: True"
	@echo ""
	@echo "$(GREEN)Terminal 3 - Rust Analysis Backend:$(NC)"
	@echo "  INFO fin_analysis_backend: starting fin-analysis-backend endpoint=https://lend.devnet.rippletest.net:51234 network=lending-devnet port=3001"
	@echo "  INFO fin_analysis_backend: listening on 0.0.0.0:3001"
	@echo ""
	@echo "$(GREEN)[Tab 4] Python API:$(NC)"
	@echo "  INFO:     Uvicorn running on http://0.0.0.0:8000"
	@echo "  INFO:     Application startup complete."
	@echo ""
	@echo "$(GREEN)[Tab 5] Frontend:$(NC)"
	@echo "  ▲ Next.js 16.2.0 (Turbopack)"
	@echo "  ✓ Ready in XXXms  →  http://localhost:3000"
	@echo ""
