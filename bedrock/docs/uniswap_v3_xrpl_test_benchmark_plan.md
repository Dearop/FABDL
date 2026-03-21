# Phase 4 Test and Benchmark Harness

## Test Suites

### Unit and Invariant Tests
- Tick validity and spacing.
- Pause and auth guards.
- Slippage guard behavior.
- Liquidity accounting monotonicity.

Implemented scaffold:
- `bedrock/uniswap_v3_xrpl_tests.rs`

### Property/Fuzz Targets
- Randomized tick crossing sequences.
- Randomized liquidity deltas with boundary cases.
- Randomized swap direction and limit prices.

Suggested fuzz properties:
1. Active liquidity never negative.
2. Fee growth is monotonic non-decreasing.
3. Crossing initialized ticks updates `liquidity_active` by exactly `liquidity_net`.

## Integration Test Matrix
- Path A: Bedrock direct execution.
- Path B: direct XRPL execution fallback.
- Cases:
  - mint -> swap -> collect
  - swap exact in with low slippage threshold (expect revert)
  - pause and resume controls
  - adapter path failover with one path intentionally disabled

## Benchmark Harness
### Metrics
- Submit latency (ms)
- Confirmation latency (ms)
- Effective fee/cost per operation
- Fallback rate (%)

### Workload Profiles
- Small swaps (retail profile)
- Medium swaps (active LP profile)
- Burst traffic (stress profile)

### Output Format
Use a per-run JSON artifact:
```json
{
  "run_id": "2026-03-21T00:00:00Z",
  "path": "BedrockDirect",
  "workload": "medium_swaps",
  "p50_ms": 0,
  "p95_ms": 0,
  "avg_cost_drops": 0,
  "success_rate": 0.0
}
```

## Pass/Fail Thresholds
- No invariant failures.
- Integration success >= 99% for healthy path.
- Fallback success >= 99% when primary path disabled.
- End-to-end target under 8s for production profile.
