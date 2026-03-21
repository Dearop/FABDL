# Firehose XRPL

Firehose integration for XRP Ledger (XRPL), enabling efficient streaming and indexing of XRPL ledger data using the Firehose/Substreams framework.

## Overview

This project fetches validated ledgers from rippled JSON-RPC endpoints and converts them to Firehose blocks.

### Key Features

- Polls validated XRPL ledgers via JSON-RPC
- Single API call per ledger (transactions included inline)
- Binary format support for efficient data transfer
- Full transaction type support (Payments, NFTs, AMM, DEX, etc.)
- Compatible with Firehose/Substreams ecosystem

## Installation

### Prerequisites

- Go 1.22 or later
- Access to an XRPL RPC endpoint

### Build from source

```bash
go build -o firexrpl ./cmd/firexrpl
```

### Using Docker

```bash
docker build -t firehose-xrpl .
```

## Usage

### Check connectivity

```bash
# Test against mainnet
firexrpl tool-check-ledger --endpoint https://s1.ripple.com:51234/

# Test against testnet
firexrpl tool-check-ledger --endpoint https://s.altnet.rippletest.net:51234/

# Fetch specific ledger with transaction decoding
firexrpl tool-check-ledger --endpoint https://s1.ripple.com:51234/ --ledger 80000000 --decode-transactions
```

### Fetch blocks

```bash
# Start fetching from ledger 80000000
firexrpl fetch rpc 80000000 \
  --endpoints https://s1.ripple.com:51234/ \
  --state-dir /data/poller
```

### Running with Firecore

```bash
firecore start reader-node merger \
  --reader-node-path=firexrpl \
  --reader-node-arguments='fetch rpc 80000000 \
    --state-dir /data/poller \
    --endpoints https://s1.ripple.com:51234/'
```

## XRPL RPC Endpoints

| Network | Endpoint                                 | Notes                |
| ------- | ---------------------------------------- | -------------------- |
| Mainnet | `https://s1.ripple.com:51234/`           | Public, rate-limited |
| Mainnet | `https://xrplcluster.com/`               | Public cluster       |
| Testnet | `https://s.altnet.rippletest.net:51234/` | Test network         |
| Devnet  | `https://s.devnet.rippletest.net:51234/` | Dev network          |

## Configuration Flags

| Flag                            | Default        | Description                            |
| ------------------------------- | -------------- | -------------------------------------- |
| `--endpoints`                   | required       | XRPL RPC endpoints (comma-separated)   |
| `--state-dir`                   | `/data/poller` | State persistence directory            |
| `--interval-between-fetch`      | `0`            | Delay between fetches                  |
| `--latest-block-retry-interval` | `1s`           | Retry interval when waiting for ledger |
| `--max-block-fetch-duration`    | `10s`          | Timeout per ledger fetch               |

## Protobuf Schema

The XRPL block schema is defined in `proto/sf/xrpl/type/v1/block.proto`:

```protobuf
message Block {
  uint64 number = 1;              // Ledger sequence number
  bytes hash = 2;                 // Ledger hash
  Header header = 3;              // Ledger header
  repeated Transaction transactions = 5;
  google.protobuf.Timestamp close_time = 6;
}

message Transaction {
  bytes hash = 1;                 // Transaction hash
  TransactionResult result = 2;   // Result code
  bytes tx_blob = 4;              // Raw transaction (binary)
  bytes meta_blob = 5;            // Transaction metadata (binary)
  TransactionType tx_type = 6;    // Transaction type enum
}
```

### Generating protobuf code

```bash
cd proto
buf generate
```

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│ rippled Node (JSON-RPC)                                     │
└───────────────────────────┬─────────────────────────────────┘
                            │ HTTP JSON-RPC
┌───────────────────────────▼─────────────────────────────────┐
│ RPC Client (rpc/client.go)                                  │
│ ├─ GetLatestLedger()   → ledger_closed                     │
│ └─ GetLedger()         → ledger (with transactions=true)   │
└───────────────────────────┬─────────────────────────────────┘
                            │
┌───────────────────────────▼─────────────────────────────────┐
│ Fetcher (rpc/fetcher.go)                                    │
│ 1. Poll until ledger validated                              │
│ 2. Fetch ledger + transactions (single call!)               │
│ 3. Decode binary blobs using xrpl-go                        │
│ 4. Build protobuf Block → bstream.Block                    │
└───────────────────────────┬─────────────────────────────────┘
                            │
┌───────────────────────────▼─────────────────────────────────┐
│ Firehose Stack (firecore)                                   │
└─────────────────────────────────────────────────────────────┘
```

## Dependencies

- [xrpl-go](https://github.com/Peersyst/xrpl-go) - XRPL SDK for Go (binary codec, RPC client)
- [firehose-core](https://github.com/streamingfast/firehose-core) - Firehose infrastructure
- [bstream](https://github.com/streamingfast/bstream) - Block streaming library

## License

Apache 2.0

## Contributing

Contributions are welcome! Please read the contributing guidelines before submitting PRs.
