# Phase 0 Feasibility Gate: Uniswap v3-Inspired AMM on Bedrock/XRPL

## Scope

This document closes the feasibility gate for the execution substrate and records the go/no-go decision required by the implementation plan.

## Inputs Reviewed

- `custom_amm/docs/bedrock.md`
- `custom_amm/docs/uniswap_v3.md`
- `references/BEDROCK.md`
- `references/XRPL-AMM.md`
- `bedrock/SMART-CONTRACT.md`
- `custom_amm/deps/bedrock/docs/guide/getting-started.md`
- `custom_amm/deps/bedrock/docs/guide/deployment-and-calling.md`

## Capability Matrix


| Capability                                          | Bedrock Direct   | Bedrock + Hook Bridge           | Direct XRPL    |
| --------------------------------------------------- | ---------------- | ------------------------------- | -------------- |
| Build/deploy Rust WASM                              | Yes              | Yes                             | N/A            |
| Expose contract ABI methods                         | Yes              | Yes                             | N/A            |
| Native concentrated liquidity                       | No               | No                              | No             |
| Directly model v3 ticks/positions in custom state   | Yes              | Yes                             | Off-chain only |
| Submit native XRPL AMM tx path from execution layer | Unproven in docs | Possible with additional bridge | Yes            |
| Lowest integration risk                             | Medium           | High                            | Low            |
| Time to first production execution                  | Medium           | High                            | Low            |


## Findings

1. **XRPL AMM does not provide native concentrated liquidity**, so v3 semantics must be simulated either in contract state or off-chain strategy logic.
2. **Bedrock toolchain is usable for Rust/WASM contracts** and ABI generation, but docs do not provide definitive proof of direct on-ledger AMM operation coverage for all required strategy paths.
3. **Direct XRPL execution is the safest production fallback** for AMM tx reliability while Bedrock adapter capability is validated.

## Decision Record

- **Primary target:** `BedrockDirect` for programmable position/tick logic and guardrails.
- **Fallback target:** `DirectXRPLOnly` for transaction execution whenever Bedrock AMM bridging is unsupported for a path.
- **Deferred option:** `BedrockHookBridge` only if a direct Bedrock route is blocked and bridging latency/reliability is acceptable.

## Go/No-Go Outcome

- **Go** for phased implementation.
- **Condition:** Treat Bedrock execution path as capability-gated at runtime. Keep direct XRPL execution path production-ready from day one.

## Exit Criteria for Phase 0

- Decision recorded (this file).
- Runtime architecture must support dual execution paths.
- Security invariants (slippage cap, signed execution, pause) enforced regardless of chosen path.

