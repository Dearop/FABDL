# Wallet Integration Architecture

## Overview

The frontend wallet layer is intentionally split into three concerns that compose cleanly:

1. **Provider abstraction** — who holds the key and how signing happens
2. **Pool registry** — what XRPL AMM pools exist and their asset issuers
3. **Transaction builder** — how strategy intent maps to XRPL wire format

These three are kept strictly separate so each can evolve independently (e.g., swap out the signing provider without touching transaction construction, or update pool discovery without touching the hook API).

---

## Provider Priority and Capabilities

Three wallet providers are supported, checked in this order at runtime:

```
KeyEntryProvider  →  OtsuProvider  →  CrossmarkProvider
(primary)            (extension)       (identity-only fallback)
```

| Provider | Identity | Signs Devnet Txs | How |
|---|---|---|---|
| KeyEntryProvider | ✓ | ✓ | xrpl.js `Wallet.fromSecret()` + `Client.submitAndWait()` |
| OtsuProvider | ✓ | ✓ | `window.xrpl.signAndSubmit()` — delegates to extension |
| CrossmarkProvider | ✓ | ✗ | `sdk.methods.signInAndWait()` — devnet signing throws |

**Why Crossmark cannot sign devnet transactions:** Crossmark connects to whichever network the extension is configured for (mainnet, testnet, standard devnet). The XRPL lending devnet (`wss://lend.devnet.rippletest.net:51233/`) is a separate endpoint with XLS-66d enabled that Crossmark does not list as a built-in network. Transactions submitted through Crossmark would silently go to the wrong network. Crossmark is kept as a fallback to at least provide address/identity for the chat flow.

---

## Secret Key Lifecycle

This is the most security-sensitive aspect of the integration. The invariant is:

> **The secret key is never written to any persistent storage. It lives only in a React ref and is lost on page refresh by design.**

```
User types secret
      ↓
secretRef.current = secret          ← React ref, not state (never serialised)
      ↓
Wallet.fromSecret(secret)           ← validates, derives address
      ↓
address → localStorage('xrpl_wallet')    ← only address persists
providerType → localStorage('xrpl_provider_type')
      ↓
Page refresh
      ↓
Address rehydrated from localStorage, providerType restored
secretRef.current = null            ← secret is gone
      ↓
UI shows address but Execute throws 'Secret key expired — reconnect'
```

On disconnect: `secretRef.current = null`, both localStorage keys removed.

This is acceptable for a devnet hackathon demo. For mainnet production, the secret would never be entered at all — the extension (Otsu) handles it.

---

## Signing Flow per Provider

### KeyEntryProvider (`signAndSubmit`)

```
xrpl.Client(LENDING_DEVNET_WS).connect()
      ↓
client.autofill(tx)        ← fills Sequence, Fee, LastLedgerSequence
      ↓
Wallet.fromSecret(secret).sign(autofilled)   ← produces tx_blob + hash
      ↓
client.submitAndWait(tx_blob)   ← submits to lending devnet, awaits validation
      ↓
check meta.TransactionResult === 'tesSUCCESS'
      ↓
client.disconnect()
      ↓
return { hash }
```

A new `Client` connection is created per `signAndSubmit` call and torn down immediately after. This avoids persistent WebSocket state in the browser and ensures clean reconnection even if the devnet node bounced.

### OtsuProvider (`signAndSubmit`)

```
window.xrpl.signAndSubmit(tx)
      ↓
Extension popup appears (user approves)
      ↓
Extension autofills, signs, submits on the currently-selected network
      ↓
return { hash }
```

**Important:** Otsu's network constants do not include the lending devnet by default. The user must manually add `wss://lend.devnet.rippletest.net:51233/` as a custom network in the extension and switch to it before executing strategies. Future work: call `window.xrpl.switchNetwork('lending-devnet')` automatically on connect once a custom network ID is established.

---

## Pool Registry

### Why it exists

XRPL AMM transactions (`AMMDeposit`, `AMMWithdraw`) require `Asset` and `Asset2` fields to identify the pool — not an AMM account address. These fields include the token issuer account (e.g. `{ currency: "USD", issuer: "rSomeIssuerOnDevnet..." }`). Issuers differ per network and cannot be hardcoded.

### Fetch strategy

On wallet connect, the registry fetches all AMM pools from the lending devnet via `ledger_data` with `type: "amm"`. This RPC walks pagination markers until all pages are consumed:

```
ledger_data { type: "amm", limit: 400 }
      ↓
Parse each AMM ledger entry:
  Asset  → PoolAsset (XRP or { currency, issuer })
  Asset2 → PoolAsset
  Account → ammAccount (the AMM's special account)
  TradingFee → basis points
      ↓
Build Map<PoolKey, PoolEntry>
  key = normalised pair string, e.g. "XRP/USD"
  (XRP always first; non-XRP pairs sorted alphabetically)
      ↓
Context provides pools to all consumers
```

### Lifecycle

```
walletAddress: null → non-null    →  fetch fires
walletAddress: non-null → null    →  registry cleared, in-flight fetch cancelled
wallet.refetch()                  →  explicit re-fetch (e.g. after network switch)
```

A `fetchIdRef` counter prevents stale results from a prior fetch landing after a rapid disconnect/reconnect.

---

## Transaction Construction

`buildAndSubmitStrategy(strategy, walletAddress, signAndSubmit, registry)` iterates `strategy.trade_actions` sequentially (not in parallel — later actions may depend on earlier ones, e.g. swap before deposit).

### Action → XRPL tx mapping

| Action | XRPL TransactionType | Key fields |
|---|---|---|
| `swap` | `Payment` | Self-payment (`Destination = Account`); `Amount` = desired output; `SendMax` = input × (1 + slippage) |
| `deposit` | `AMMDeposit` | `Asset`/`Asset2` from registry; `Flags`: `0x100000` two-asset, `0x80000` single-asset |
| `withdraw` | `AMMWithdraw` | `Asset`/`Asset2` from registry; `Flags`: `0x20000` proportional |
| `lend` | *(TODO)* | XLS-66d vault-based — deposit into Single Asset Vault (XLS-65); LoanBrokerSet lifecycle |
| `borrow` | *(TODO)* | XLS-66d `LoanSet` referencing a `VaultID` from the lending devnet |

### Asset resolution

`resolvePool(pool, registry)` takes a pool string like `"XRP/USD"`, normalises it to a canonical key, and returns the live `PoolAsset` objects (with real issuer addresses) from the registry. If the pool is missing, it throws with the full list of available pools to aid debugging.

---

## React Data Flow

```
TradingPage (outer shell)
│
├─ useWallet()                         ← address, providerType, signAndSubmit
│
└─ PoolRegistryProvider(walletAddress) ← triggers fetch on connect
   │
   └─ TradingPageInner
      │
      ├─ usePoolRegistry()             ← { pools, isLoading, error }
      ├─ useWallet() (via prop)
      │
      ├─ handleExecute()
      │   └─ buildAndSubmitStrategy(strategy, address, signAndSubmit, pools)
      │
      └─ KeyEntryModal
          ├─ onConnect(secret) → wallet.connectWithKey(secret)
          └─ onGenerate()      → wallet.generateNewWallet()
```

---

## Files Reference

| File | Role |
|---|---|
| `lib/wallet-providers.ts` | `KeyEntryProvider`, `OtsuProvider`, `CrossmarkProvider` classes + `SignAndSubmitFn` type |
| `lib/window.d.ts` | Global type declarations for `window.xrpl` and `window.crossmark` |
| `hooks/useWallet.ts` | Unified hook: `address`, `providerType`, `connect`, `connectWithKey`, `generateNewWallet`, `disconnect`, `signAndSubmit` |
| `services/poolRegistry.ts` | Pure `fetchAllPools(wsUrl)` — no React, follows pagination |
| `contexts/PoolRegistryContext.tsx` | `PoolRegistryProvider` + `usePoolRegistry()` hook |
| `services/xrplTransactions.ts` | `buildAndSubmitStrategy()` — transaction construction per action type |
| `components/KeyEntryModal.tsx` | "Enter Key" / "Generate New" modal with devnet warning |
| `app/trading/page.tsx` | Page shell + inner page; wires all of the above together |

---

## Open TODOs

- **XLS-66d lending/borrowing**: The `lend` and `borrow` action types currently throw. The XLS-66d spec uses Single Asset Vaults (XLS-65) for deposits and `LoanSet`/`LoanPay` transactions for borrowing. The `VaultID` for each asset on the lending devnet needs to be discovered (via `ledger_data` with `type: "vault"` or a dedicated RPC once the spec stabilises).
- **Otsu network switching**: When `OtsuProvider` connects, automatically call `window.xrpl.switchNetwork()` with the lending devnet endpoint so the user does not have to configure it manually in the extension.
- **XLS-66d vault registry**: Extend `poolRegistry.ts` (or create a parallel `vaultRegistry.ts`) to also fetch Single Asset Vaults from the devnet when XLS-66d is ready to be implemented.
