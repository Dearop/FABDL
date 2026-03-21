.PHONY: help install setup start-ollama start-intent start-backend start-frontend start-rust start-all stop install-frontend install-backend install-rust clean logs

# Ollama binary — override if not on PATH: make start-ollama OLLAMA=path/to/ollama
OLLAMA ?= ollama

# Colors for output
BLUE := \033[0;34m
GREEN := \033[0;32m
YELLOW := \033[0;33m
RED := \033[0;31m
NC := \033[0m # No Color

help:
	@echo "$(BLUE)━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━$(NC)"
	@echo "$(BLUE)XRPL AI Trading Assistant - Development Commands$(NC)"
	@echo "$(BLUE)━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━$(NC)"
	@echo ""
	@echo "$(GREEN)Setup Commands:$(NC)"
	@echo "  $(YELLOW)make install$(NC)              Install all dependencies (frontend + backend)"
	@echo "  $(YELLOW)make install-frontend$(NC)     Install frontend dependencies"
	@echo "  $(YELLOW)make install-backend$(NC)      Install backend dependencies"
	@echo ""
	@echo "$(GREEN)Start Individual Services (each in new terminal):$(NC)"
	@echo "  $(YELLOW)make start-ollama$(NC)         Start Ollama LLM server (localhost:11434)"
	@echo "  $(YELLOW)make start-intent$(NC)         Start Intent Router (localhost:50051)"
	@echo "  $(YELLOW)make start-backend$(NC)        Start Backend API (localhost:8000)"
	@echo "  $(YELLOW)make start-frontend$(NC)       Start Frontend (localhost:3000)"
	@echo ""
	@echo "$(GREEN)Quick Start (READ FIRST):$(NC)"
	@echo "  $(YELLOW)make setup$(NC)                Show setup instructions"
	@echo "  $(YELLOW)make start-all$(NC)            Display terminal instructions for all 4 services"
	@echo ""
	@echo "$(GREEN)Utilities:$(NC)"
	@echo "  $(YELLOW)make clean$(NC)                Clean all node_modules, __pycache__, .next"
	@echo "  $(YELLOW)make logs$(NC)                 Show example of expected console logs"
	@echo ""

setup:
	@echo "$(BLUE)━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━$(NC)"
	@echo "$(BLUE)SETUP INSTRUCTIONS$(NC)"
	@echo "$(BLUE)━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━$(NC)"
	@echo ""
	@echo "$(GREEN)Step 1: Install Dependencies$(NC)"
	@echo "  $$ make install"
	@echo ""
	@echo "$(GREEN)Step 2: Set ANTHROPIC_API_KEY$(NC)"
	@echo "  $$ export ANTHROPIC_API_KEY=sk-ant-..."
	@echo ""
	@echo "$(GREEN)Step 3: Open 5 Terminal Windows/Tabs$(NC)"
	@echo ""
	@echo "$(GREEN)Terminal 1 - Ollama (LLM Engine)$(NC)"
	@echo "  $$ make start-ollama"
	@echo "  Expected: 'Listening on 127.0.0.1:11434'"
	@echo ""
	@echo "$(GREEN)Terminal 2 - Intent Router (gRPC)$(NC)"
	@echo "  $$ make start-intent"
	@echo "  Expected: 'Intent Router listening on [::]:50051'"
	@echo ""
	@echo "$(GREEN)Terminal 3 - Rust Analysis Backend$(NC)"
	@echo "  $$ make start-rust"
	@echo "  Expected: 'listening on 0.0.0.0:3001'"
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

# ==================== Installation ====================

install: install-backend install-frontend install-rust
	@echo "$(GREEN)✅ All dependencies installed!$(NC)"

install-frontend:
	@echo "$(YELLOW)📦 Installing frontend dependencies...$(NC)"
	cd frontend && npm install

install-backend:
	@echo "$(YELLOW)📦 Installing backend dependencies...$(NC)"
	cd backend && pip install -r requirements.txt

install-rust:
	@echo "$(YELLOW)📦 Building Rust analysis backend...$(NC)"
	cd fin-analysis-backend && cargo build --release

# ==================== Start Individual Services ====================

start-ollama:
	@echo "$(BLUE)━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━$(NC)"
	@echo "$(BLUE)🧠 Starting Ollama (LLM Engine)$(NC)"
	@echo "$(BLUE)Port: 11434$(NC)"
	@echo "$(BLUE)━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━$(NC)"
	$(OLLAMA) serve

start-intent:
	@echo "$(BLUE)━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━$(NC)"
	@echo "$(BLUE)🔄 Starting Intent Router (gRPC)$(NC)"
	@echo "$(BLUE)Port: 50051$(NC)"
	@echo "$(BLUE)━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━$(NC)"
	cd llm-orchestration && python3 src/intent_router_service.py

start-backend:
	@echo "$(BLUE)━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━$(NC)"
	@echo "$(BLUE)⚡ Starting Backend API$(NC)"
	@echo "$(BLUE)Port: 8000$(NC)"
	@echo "$(BLUE)━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━$(NC)"
	cd backend && python3 api.py

start-frontend:
	@echo "$(BLUE)━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━$(NC)"
	@echo "$(BLUE)🎨 Starting Frontend (Next.js)$(NC)"
	@echo "$(BLUE)Port: 3000$(NC)"
	@echo "$(BLUE)Visit: http://localhost:3000$(NC)"
	@echo "$(BLUE)━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━$(NC)"
	cd frontend && npm run dev

start-rust:
	@echo "$(BLUE)━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━$(NC)"
	@echo "$(BLUE)🦀 Starting Rust Analysis Backend$(NC)"
	@echo "$(BLUE)Port: 3001$(NC)"
	@echo "$(BLUE)━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━$(NC)"
	cd fin-analysis-backend && PORT=3001 cargo run --release

# ==================== Start All (Instructions) ====================

start-all:
	@echo "$(BLUE)━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━$(NC)"
	@echo "$(GREEN)🚀 STARTING ALL 5 SERVICES$(NC)"
	@echo "$(BLUE)━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━$(NC)"
	@echo ""
	@echo "$(YELLOW)Set your API key before starting Terminal 4:$(NC)"
	@echo "  $$ export ANTHROPIC_API_KEY=sk-ant-..."
	@echo ""
	@echo "$(YELLOW)Open 5 separate terminal windows and run:$(NC)"
	@echo ""
	@echo "$(RED)[Terminal 1]$(NC) $$ make start-ollama"
	@echo "$(RED)[Terminal 2]$(NC) $$ make start-intent"
	@echo "$(RED)[Terminal 3]$(NC) $$ make start-rust"
	@echo "$(RED)[Terminal 4]$(NC) $$ make start-backend"
	@echo "$(RED)[Terminal 5]$(NC) $$ make start-frontend"
	@echo ""
	@echo "$(GREEN)Once all 5 are running:$(NC)"
	@echo "  Open browser → http://localhost:3000"
	@echo ""
	@echo "$(YELLOW)Tip: Use tmux for parallel execution:$(NC)"
	@echo "  $$ tmux new-session -d -s ollama 'make start-ollama'"
	@echo "  $$ tmux new-window -t ollama -n intent 'make start-intent'"
	@echo "  $$ tmux new-window -t ollama -n rust 'make start-rust'"
	@echo "  $$ tmux new-window -t ollama -n backend 'make start-backend'"
	@echo "  $$ tmux new-window -t ollama -n frontend 'make start-frontend'"
	@echo ""

# ==================== Utilities ====================

clean:
	@echo "$(YELLOW)🧹 Cleaning up...$(NC)"
	rm -rf frontend/node_modules frontend/.next
	find backend -type d -name __pycache__ -exec rm -rf {} + 2>/dev/null || true
	find llm-orchestration -type d -name __pycache__ -exec rm -rf {} + 2>/dev/null || true
	find . -type f -name "*.pyc" -delete
	@echo "$(GREEN)✅ Clean complete!$(NC)"

logs:
	@echo "$(BLUE)━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━$(NC)"
	@echo "$(BLUE)EXPECTED CONSOLE OUTPUT (5 services)$(NC)"
	@echo "$(BLUE)━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━$(NC)"
	@echo ""
	@echo "$(GREEN)Terminal 1 - Ollama:$(NC)"
	@echo "  Listening on 127.0.0.1:11434 (version 0.18.2)"
	@echo ""
	@echo "$(GREEN)Terminal 2 - Intent Router:$(NC)"
	@echo "  INFO:__main__:Intent Router listening on [::]:50051"
	@echo "  INFO:__main__:Using model: llama2"
	@echo "  INFO:__main__:LLM available: True"
	@echo ""
	@echo "$(GREEN)Terminal 3 - Rust Analysis Backend:$(NC)"
	@echo "  INFO fin_analysis_backend: starting fin-analysis-backend endpoint=https://xrplcluster.com port=3001"
	@echo "  INFO fin_analysis_backend: listening on 0.0.0.0:3001"
	@echo ""
	@echo "$(GREEN)Terminal 4 - Backend:$(NC)"
	@echo "  INFO:     Uvicorn running on http://0.0.0.0:8000"
	@echo "  INFO:     Application startup complete."
	@echo ""
	@echo "$(GREEN)Terminal 5 - Frontend:$(NC)"
	@echo "  ▲ Next.js 16.2.0 (Turbopack)"
	@echo "  - Local:         http://localhost:3000"
	@echo "  ✓ Ready in XXXms"
	@echo ""
	@echo "$(YELLOW)When you submit a query, you'll see this flow:$(NC)"
	@echo "  [Frontend]      POST /strategies/generate"
	@echo "  [Backend]       Step 1: Classifying intent via gRPC..."
	@echo "  [Intent Router] Received query, calling Ollama..."
	@echo "  [Intent Router] Classified: action=analyze_risk scope=portfolio"
	@echo "  [Backend]       Step 2: Building IntentRouterOutput for Rust..."
	@echo "  [Backend]       Step 3: POST http://localhost:3001/analyze"
	@echo "  [Backend]       Step 4: Calling Claude claude-sonnet-4-6..."
	@echo "  [Backend]       Step 5: 3 strategies passed validation"
	@echo "  [Frontend]      Displaying strategies"
	@echo ""
