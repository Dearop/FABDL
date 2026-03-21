# Phase 3 Execution Adapter: Dual-Path Policy

## Objective
Provide one execution interface that prefers Bedrock contract settlement and falls back to direct XRPL tx submission when capability, latency, or reliability constraints are hit.

## Routing Policy
1. If Bedrock direct path is healthy and supported for this operation, use it.
2. If Bedrock path fails capability check or runtime submit, fail over to direct XRPL path.
3. If direct XRPL path is unavailable, return explicit error and do not silently retry unsafe routes.

## Failure Domains
- Capability failure (unsupported operation)
- Submission failure (transport or signing failure)
- Confirmation timeout

## Atomicity Policy
- Single operation (`swap_exact_in`) is atomic per path.
- Multi-step plans use saga-style compensating actions where platform atomicity is unavailable.
- Compensating actions must be idempotent and auditable.

## Security Requirements Across Both Paths
- User signature required.
- Slippage cap validated before submit and re-validated post-quote.
- Emergency pause blocks adapter dispatch.
- Full execution event logging for path chosen, hash, and outcome.

## Implemented Artifact
- `bedrock/uniswap_v3_xrpl_execution_adapter.rs`
