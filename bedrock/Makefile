POOL_WASM   := target/wasm32-unknown-unknown/release/uniswap_v3_xrpl_contract.wasm
MGR_WASM    := target/wasm32-unknown-unknown/release/uniswap_v3_xrpl_manager.wasm
WASM_OPT    := wasm-opt
OPT_FLAGS   := -Oz --strip-debug --strip-producers --zero-filled-memory --remove-unused-module-elements --dce

.PHONY: build build-pool build-manager deploy-pool deploy-manager test all

all: build

build: build-pool build-manager

build-pool:
	cargo build --target wasm32-unknown-unknown --release --package uniswap-v3-xrpl-contract
	$(WASM_OPT) $(OPT_FLAGS) $(POOL_WASM) -o $(POOL_WASM)
	@echo "Pool WASM size: $$(wc -c < $(POOL_WASM)) bytes"

build-manager:
	cargo build --target wasm32-unknown-unknown --release --package uniswap-v3-xrpl-manager
	$(WASM_OPT) $(OPT_FLAGS) $(MGR_WASM) -o $(MGR_WASM)
	@echo "Manager WASM size: $$(wc -c < $(MGR_WASM)) bytes"

test:
	cargo test --package uniswap-v3-xrpl-contract
	cargo test --package uniswap-v3-xrpl-manager
	cargo test --package uniswap-v3-xrpl-adapter

# Deploy pool contract (run first).
# After deploying, note the Pool address P, then run:
#   bedrock call P set_manager --params '{"manager":"<M>"}' --wallet <seed>
deploy-pool: build-pool
	bedrock deploy --skip-build

# Deploy manager contract (run after pool).
# After deploying, note the Manager address M, then run:
#   bedrock call P set_manager --params '{"manager":"<M>"}' --wallet <seed>
#   bedrock call M setup --params '{"pool":"<P>"}' --wallet <seed>
deploy-manager: build-manager
	@echo "TODO: update bedrock.toml to point to manager WASM, then run bedrock deploy --skip-build"
	@echo "Pool WASM: $(POOL_WASM)"
	@echo "Manager WASM: $(MGR_WASM)"
