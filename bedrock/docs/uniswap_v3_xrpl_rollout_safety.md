# Phase 5 Rollout Controls, Observability, and Security Checklist

## Staged Rollout Plan

### Stage 0: Internal Testnet
- Enable allowlisted wallets only.
- Per-tx notional cap: low.
- Per-pool allowlist enabled.
- Runtime path metrics collection enabled.

### Stage 1: Limited Alpha
- Expand wallet allowlist to pilot users.
- Daily volume cap enforced.
- Automatic fallback from Bedrock path to direct XRPL path enabled.
- Manual incident response runbook validated.

### Stage 2: Public Beta
- Increase per-tx and daily caps after passing reliability thresholds.
- Keep pause switch and emergency kill route active.
- Keep risky operations behind feature flags.

### Stage 3: General Availability
- Require completed audit actions and verified recovery drills.
- Keep policy-based caps adjustable without redeploy.

## Operational Controls
- `pause` switch controlled by owner/multisig.
- Strategy operation allowlist (no arbitrary method dispatch).
- Slippage hard cap globally enforced.
- Circuit breaker:
  - trigger on repeated failures
  - trigger on abnormal slippage outliers
  - trigger on adapter fallback rate spike

## Observability Requirements
- Emit events for:
  - strategy execution attempt
  - selected execution path
  - swap/mint/burn result
  - slippage and fee outcomes
  - fallback or failure reason
- Track dashboards:
  - success rate by path
  - p50/p95 confirmation latency
  - slippage distribution
  - failure classification
  - fallback rate over time

## Security Readiness Checklist
- Threat model documented and reviewed.
- All critical paths covered by tests and invariant checks.
- Independent security review for:
  - math correctness
  - auth and pause logic
  - adapter failover behavior
- Replay and signature misuse checks validated.
- Key management:
  - owner is multisig
  - rotation and emergency process documented
- Incident response:
  - who can pause
  - expected notification process
  - rollback/recovery steps

## Go-Live Gates
- Integration success >= 99% across both paths.
- No unresolved critical findings from audit.
- Runbook dry-run completed successfully.
- Rollout cap values approved by governance/owners.
