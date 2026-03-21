# Bedrock

Bedrock is a developer tool for building, deploying, and interacting with XRPL smart contracts written in Rust. It provides a complete CLI workflow for XRPL smart contract development, compiling Rust contracts to WebAssembly and handling deployment to XRPL networks. Think Foundry, but for XRPL.

The tool includes a build system for compiling Rust to optimized WebAssembly, smart deployment with auto-build and ABI generation, contract interaction capabilities, local XRPL node management via Docker, automatic ABI extraction from Rust code annotations, and secure wallet management with AES-256-GCM encryption through the Jade wallet manager.

## Project Initialization

The `bedrock init` command creates a new XRPL smart contract project with the necessary directory structure including bedrock.toml configuration, Cargo.toml for Rust dependencies, and boilerplate contract code.

```bash
# Create a new Bedrock smart contract project
bedrock init my-contract
cd my-contract

# Project structure created:
# my-contract/
# ├── bedrock.toml          # Project configuration
# ├── contract/
# │   ├── Cargo.toml        # Rust package manifest
# │   └── src/
# │       └── lib.rs        # Smart contract boilerplate

# Example bedrock.toml configuration:
# [project]
# name = "my-contract"
# version = "0.1.0"
#
# [build]
# source = "contract/src/lib.rs"
# target = "wasm32-unknown-unknown"
#
# [networks.local]
# url = "ws://localhost:6006"
# network_id = 63456
# faucet_url = "http://localhost:8080/faucet"
#
# [networks.alphanet]
# url = "wss://alphanet.nerdnest.xyz"
# network_id = 21465
# faucet_url = "https://alphanet.faucet.nerdnest.xyz/accounts"
```

## Building Contracts

The `bedrock build` command compiles Rust smart contracts to WebAssembly (WASM) for deployment on XRPL. It wraps cargo with sensible defaults, validates the Rust toolchain, ensures the wasm32 target is installed, and reports build results including file size and duration.

```bash
# Build in release mode (default, optimized for size ~156 KB)
bedrock build

# Build in debug mode (faster compilation, larger output ~1.2 MB)
bedrock build --release=false

# Expected output for release build:
# Building smart contract...
#    Mode: Release (optimized)
#    Source: contract/src/lib.rs
#
# ✓ Build completed successfully!
#
# Output:   contract/target/wasm32-unknown-unknown/release/my_contract.wasm
# Size:     156.4 KB
# Duration: 5.1s

# Recommended Cargo.toml settings for optimized WASM:
# [lib]
# crate-type = ["cdylib"]
#
# [profile.release]
# opt-level = "z"     # Optimize for size
# lto = true          # Link-time optimization
# strip = true        # Remove debug symbols
# panic = "abort"     # Smaller panic handler
```

## Deploying Contracts

The `bedrock deploy` command performs smart deployment which automatically builds the contract, generates the ABI from source annotations, and deploys to the specified network. The deployment fee is 100 XRP (100,000,000 drops).

```bash
# Deploy to alphanet (default network)
bedrock deploy

# Deploy to local node
bedrock deploy --network local

# Deploy with a specific wallet seed
bedrock deploy --wallet sEd7rXqNMpQgMyXXXXXXXXXXXXX

# Deploy with ed25519 algorithm
bedrock deploy --algorithm ed25519

# Skip automatic rebuild (use existing WASM)
bedrock deploy --skip-build

# Skip ABI generation (use existing abi.json)
bedrock deploy --skip-abi

# Full deployment with all options
bedrock deploy \
  --network alphanet \
  --wallet sEd7rXqNMpQgMyXXXXXXXXXXXXX \
  --algorithm secp256k1

# Expected output:
# ✓ Contract deployed successfully!
#   Wallet Address: rHb9CJAWyB4rj91VRWn96DkukG4bwdtyTh
#   Wallet Seed:    sEd7rXqNMpQgMyXXXXXXXXXXXXX (save this!)
#   Contract:       rContract123XXXXXXXXXXXXXXXXXXXXXXX
#   Tx Hash:        ABC123...
```

## Calling Contract Functions

The `bedrock call` command invokes functions on deployed smart contracts. It requires a wallet seed for signing transactions, supports JSON parameters inline or from a file, and has a default transaction fee of 1 XRP (1,000,000 drops).

```bash
# Simple call without parameters
bedrock call rContract123... hello --wallet sEd7...

# Call with inline JSON parameters
bedrock call rContract123... transfer \
  --wallet sEd7... \
  --params '{"to":"rRecipient...","amount":1000}'

# Call with parameters from a JSON file
bedrock call rContract123... register \
  --wallet sEd7... \
  --params-file params.json

# Call with custom gas and fee
bedrock call rContract123... expensive_operation \
  --wallet sEd7... \
  --gas 5000000 \
  --fee 2000000

# Call on local network
bedrock call rContract123... test_function \
  --wallet sEd7... \
  --network local

# Call with custom ABI file
bedrock call rContract123... myFunction \
  --wallet sEd7... \
  --abi ./custom-abi.json

# Example params.json file:
# {
#   "to": "rRecipientAddress123...",
#   "amount": 1000,
#   "memo": "Payment for services"
# }
```

## Local Node Management

The `bedrock node` commands manage a local XRPL development node running in Docker, providing a fast isolated environment with pre-configured genesis ledger and pre-funded test accounts.

```bash
# Start local XRPL node (requires Docker)
bedrock node start

# Check node status
bedrock node status
# Output:
# Local XRPL Node Status
# ===================================
# Status:      Running
# Container:   a1b2c3d4e5f6
# Image:       transia/alphanet:latest
# Ports:
#   - 6006->6006/tcp (WebSocket)
#   - 5005->5005/tcp (JSON-RPC)
#   - 51235->51235/tcp (Peer)
#
# Endpoints:
#   WebSocket: ws://localhost:6006
#   RPC:       http://localhost:5005

# View container logs
bedrock node logs

# Stop the local node
bedrock node stop

# Local node endpoints available after start:
# - WebSocket: ws://localhost:6006
# - Faucet: http://localhost:8080/faucet

# Example: Connect from JavaScript
# const { Client } = require('@transia/xrpl');
# const client = new Client('ws://localhost:6006');
# await client.connect();
```

## Wallet Management (Jade)

Jade is Bedrock's built-in wallet management tool providing secure encrypted storage using AES-256-GCM encryption with PBKDF2 key derivation. Wallets are stored in `~/.config/bedrock/wallets/`.

```bash
# Create a new XRPL wallet (secp256k1 by default)
bedrock jade new my-dev-wallet

# Create a new wallet with ed25519 algorithm
bedrock jade new my-ed-wallet --algorithm ed25519

# Import an existing wallet from seed
bedrock jade import my-existing-wallet
# (prompts for seed and password)

# Import with ed25519 algorithm
bedrock jade import my-ed-wallet --algorithm ed25519

# List all stored wallets
bedrock jade list

# Export wallet credentials (requires password)
bedrock jade export my-wallet
# Output:
#   Name:    my-wallet
#   Address: rHb9CJAWyB4rj91VRWn96DkukG4bwdtyTh
#   Seed:    sEd7rXqNMpQgMyXXXXXXXXXXXXX

# Remove a wallet permanently
bedrock jade remove my-old-wallet

# Using exported seed with other commands:
bedrock deploy --wallet sEd7rXqNMpQgMyXXXXXXXXXXXXX --network alphanet
bedrock call rContract... hello --wallet sEd7rXqNMpQgMyXXXXXXXXXXXXX
```

## Requesting Testnet Funds

The `bedrock faucet` command requests testnet funds from the XRPL faucet to fund wallets for development and testing.

```bash
# Generate a new wallet and fund it automatically
bedrock faucet

# Fund a specific address
bedrock faucet --address rMyAddress123...

# Fund using an existing wallet seed
bedrock faucet --wallet sEd7...

# Fund on local network
bedrock faucet --network local

# Fund with ed25519 wallet
bedrock faucet --algorithm ed25519

# Faucet endpoints:
# - Local:    http://localhost:8080/faucet
# - Alphanet: https://alphanet.faucet.nerdnest.xyz/accounts
```

## ABI Generation and Annotations

Bedrock automatically generates Application Binary Interface (ABI) definitions from JSDoc-style annotations in Rust smart contract source code during deployment. The ABI describes functions, parameters, types, and return values.

```rust
// Example annotated smart contract (contract/src/lib.rs)
#![cfg_attr(target_arch = "wasm32", no_std)]

#[cfg(not(target_arch = "wasm32"))]
extern crate std;

use xrpl_wasm_macros::wasm_export;
use xrpl_wasm_std::host::trace::trace;

/// @xrpl-function hello
#[wasm_export]
fn hello() -> i32 {
    let _ = trace("Hello from XRPL Smart Contract!");
    0
}

/// @xrpl-function register
/// @flag 0
/// @param name VL - Domain name to register (required)
/// @param resolver ACCOUNT - Resolver address (required)
/// @flag 1
/// @param duration UINT64 - Registration duration in seconds (optional)
/// @return UINT32 - Status code
#[wasm_export]
fn register(name: Blob, resolver: AccountId, duration: u64) -> u32 {
    let _ = trace("Registering domain...");
    0
}

/// @xrpl-function transfer
/// @param name VL - Domain name to transfer
/// @param new_owner ACCOUNT - New owner address
#[wasm_export]
fn transfer(name: Blob, new_owner: AccountId) -> i32 {
    let _ = trace("Transferring domain...");
    0
}

// Supported parameter types:
// UINT8, UINT16, UINT32, UINT64, UINT128, UINT256 - Unsigned integers
// VL       - Variable length bytes/string
// ACCOUNT  - XRPL account address (20 bytes)
// AMOUNT   - XRP or IOU amount
// CURRENCY - Currency code
// ISSUE    - Currency and issuer pair
// NUMBER   - Floating-point number

// Flag values:
// @flag 0 - Required parameter (default)
// @flag 1 - Optional parameter
```

## Cache and Cleanup

The `bedrock clean` command removes build artifacts and cached JavaScript modules, which will be automatically reinstalled on the next command that requires them.

```bash
# Remove cached modules and dependencies
bedrock clean

# What it removes:
# - Extracted JavaScript modules (deploy.js, call.js, faucet.js)
# - Installed npm dependencies (node_modules)
# - Version tracking file

# Cache locations:
# - Linux/macOS: ~/.cache/bedrock/modules/
# - Wallets: ~/.config/bedrock/wallets/

# Manual cache removal (alternative)
rm -rf ~/.cache/bedrock
```

## Complete Development Workflow

A comprehensive example showing the typical local development loop and testnet deployment workflow.

```bash
# Terminal 1: Start local development environment
bedrock init my-contract
cd my-contract
bedrock node start

# Terminal 2: Development cycle
# 1. Build the contract
bedrock build

# 2. Deploy to local node
bedrock deploy --network local
# Save output: wallet seed = sXXX..., contract = rXXX...

# 3. Test the contract
bedrock call rContract... hello --wallet sXXX... --network local

# 4. Make changes to contract/src/lib.rs, then redeploy (auto-rebuilds)
bedrock deploy --network local

# 5. When ready for testnet
bedrock deploy --network alphanet
# Save the wallet seed and contract address

# 6. Call on testnet
bedrock call rContract... myFunction \
  --wallet sXXX... \
  --network alphanet \
  --params '{"key":"value"}'

# Using saved wallets workflow
bedrock jade new dev-wallet
bedrock faucet --wallet $(bedrock jade export dev-wallet | grep Seed | awk '{print $2}')
bedrock deploy --wallet sXXX... --network alphanet
```

## Summary

Bedrock is designed for developers building XRPL smart contracts who need a streamlined workflow from contract development to deployment. The primary use cases include rapid local development with the Docker-based XRPL node, automated building and ABI generation, secure wallet management, and seamless deployment to both local and testnet environments. The tool is particularly suited for Rust developers familiar with Foundry-style tooling who want to build on the XRPL blockchain.

Integration patterns typically involve initializing a project with `bedrock init`, writing annotated Rust contracts, using `bedrock node start` for local testing, iterating with `bedrock deploy --network local`, and finally deploying to alphanet with `bedrock deploy --network alphanet`. Wallet credentials can be securely managed with the Jade wallet system, and the faucet command provides easy access to testnet funds. The embedded JavaScript modules handle XRPL transaction complexity, allowing developers to focus on contract logic rather than blockchain infrastructure.
