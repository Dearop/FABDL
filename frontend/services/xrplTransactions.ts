/**
 * XRPL Transaction Builder
 *
 * Converts strategy trade_actions into XRPL transactions and submits
 * them via the connected wallet provider's signAndSubmit function.
 */

import type { Strategy, TradeAction } from '@/lib/types'
import type { SignAndSubmitFn, XrplNetwork } from '@/lib/wallet-providers'
import {
  isPoolConfigured,
  normalizePoolLabel,
  resolveConfiguredPool,
  type PoolAsset,
} from '@/services/poolRegistry'
import {
  isDiscoveredPool,
  resolveDiscoveredPool,
} from '@/services/ammDiscovery'
import {
  isVaultSupportedForExecution,
  resolveVaultByAsset,
} from '@/services/vaultRegistry'

export interface StrategyExecutionSupport {
  executable: boolean
  reason?: string
}

export interface StrategyExecutionResult {
  txHash: string
  status: 'success'
  results: Array<{ action: string; hash: string }>
  network: XrplNetwork
}

function toDrops(xrpAmount: number): string {
  return String(Math.round(xrpAmount * 1_000_000))
}

function toAmount(
  asset: PoolAsset,
  amount: number,
): string | { currency: string; issuer: string; value: string } {
  if (asset.currency === 'XRP') return toDrops(amount)
  const { currency, issuer } = asset as { currency: string; issuer: string }
  return { currency, issuer, value: String(amount) }
}

function toLoanRateUnits(ratePercent: number | null | undefined): number {
  if (ratePercent == null) return 0
  return Math.max(0, Math.round(ratePercent * 100))
}

function unsupported(reason: string): StrategyExecutionSupport {
  return { executable: false, reason }
}

/** Resolve a pool: prefer live discovered pools, fall back to static registry. */
function resolvePool(pool: string): { asset1: PoolAsset; asset2: PoolAsset } {
  if (isDiscoveredPool(pool)) {
    const discovered = resolveDiscoveredPool(pool)
    return { asset1: discovered.asset1, asset2: discovered.asset2 }
  }
  const configured = resolveConfiguredPool(pool)
  return { asset1: configured.asset1, asset2: configured.asset2 }
}

function getActionExecutionSupport(action: TradeAction, network: XrplNetwork = 'lend-devnet'): StrategyExecutionSupport {
  switch (action.action) {
    case 'lend':
      if (network === 'testnet') {
        return unsupported('Lending is not available on XRPL Testnet.')
      }
      if (!isVaultSupportedForExecution(action.asset_in)) {
        return unsupported(
          `Live vault execution is currently enabled only for XRP and USD. Unsupported lending asset: ${action.asset_in.toUpperCase()}.`,
        )
      }
      return { executable: true }

    case 'borrow':
      if (network === 'testnet') {
        return unsupported('Borrowing is not available on XRPL Testnet.')
      }
      return unsupported(
        'Borrow execution is disabled in the demo until the loan broker signs first.',
      )

    case 'swap':
    case 'deposit':
    case 'withdraw':
      if (!action.pool) {
        return unsupported(`${action.action} action is missing a pool label.`)
      }
      try {
        normalizePoolLabel(action.pool) // validate format
      } catch (error) {
        return unsupported(error instanceof Error ? error.message : 'Invalid AMM pool label.')
      }
      // Check discovered pools first, fall back to static registry
      if (!isDiscoveredPool(action.pool) && !isPoolConfigured(action.pool)) {
        return unsupported(
          `AMM pool "${normalizePoolLabel(action.pool)}" was not found on-chain. It may not exist on the current network.`,
        )
      }
      return { executable: true }

    default:
      return unsupported(`Unknown action type: ${(action as TradeAction).action}`)
  }
}

export function getStrategyExecutionSupport(strategy: Strategy, network: XrplNetwork = 'lend-devnet'): StrategyExecutionSupport {
  if (strategy.trade_actions.length === 0) {
    return unsupported('This strategy has no on-chain transactions to sign.')
  }

  for (const action of strategy.trade_actions) {
    const support = getActionExecutionSupport(action, network)
    if (!support.executable) return support
  }

  return { executable: true }
}

function buildSwapTx(
  action: TradeAction,
  walletAddress: string,
): Record<string, unknown> {
  const pool = action.pool
  if (!pool) throw new Error('Swap action missing pool field')

  const { asset1, asset2 } = resolvePool(pool)

  const assetIn = action.asset_in.toUpperCase() === 'XRP' ||
    asset1.currency === action.asset_in.toUpperCase()
    ? asset1
    : asset2
  const assetOut = assetIn === asset1 ? asset2 : asset1

  const slippageMultiplier = 1 + (action.estimated_slippage ?? 0)

  return {
    TransactionType: 'Payment',
    Account: walletAddress,
    Destination: walletAddress,
    Amount: toAmount(assetOut, action.amount),
    SendMax: toAmount(assetIn, action.amount * slippageMultiplier),
    Flags: 0,
  }
}

function buildDepositTx(
  action: TradeAction,
  walletAddress: string,
): Record<string, unknown> {
  const pool = action.pool
  if (!pool) throw new Error('Deposit action missing pool field')

  const { asset1, asset2 } = resolvePool(pool)
  const flags = action.deposit_mode === 'two_asset' ? 1048576 : 524288

  const tx: Record<string, unknown> = {
    TransactionType: 'AMMDeposit',
    Account: walletAddress,
    Asset: asset1,
    Asset2: asset2,
    Amount: toAmount(
      asset1.currency === action.asset_in.toUpperCase() ? asset1 : asset2,
      action.amount,
    ),
    Flags: flags,
  }

  if (action.deposit_mode === 'two_asset' && action.amount2 != null) {
    tx.Amount2 = toAmount(
      asset1.currency === action.asset_in.toUpperCase() ? asset2 : asset1,
      action.amount2,
    )
  }

  return tx
}

function buildWithdrawTx(
  action: TradeAction,
  walletAddress: string,
): Record<string, unknown> {
  const pool = action.pool
  if (!pool) throw new Error('Withdraw action missing pool field')

  const { asset1, asset2 } = resolvePool(pool)

  return {
    TransactionType: 'AMMWithdraw',
    Account: walletAddress,
    Asset: asset1,
    Asset2: asset2,
    Amount: toAmount(
      asset1.currency === action.asset_in.toUpperCase() ? asset1 : asset2,
      action.amount,
    ),
    Flags: 131072,
  }
}

async function buildLendTx(
  action: TradeAction,
  walletAddress: string,
): Promise<Record<string, unknown>> {
  const vault = await resolveVaultByAsset(action.asset_in)

  return {
    TransactionType: 'VaultDeposit',
    Account: walletAddress,
    VaultID: vault.vaultId,
    Amount: toAmount(vault.asset, action.amount),
  }
}

async function buildBorrowTx(
  action: TradeAction,
  walletAddress: string,
): Promise<Record<string, unknown>> {
  const vault = await resolveVaultByAsset(action.asset_out)

  if (!vault.loanBrokerId || !vault.loanBrokerAccount) {
    throw new Error(
      `No live LoanBroker was found for ${action.asset_out}. Borrowing needs a loan broker linked to the target vault.`,
    )
  }

  const termDays = Math.max(1, Math.round(action.term_days ?? 30))
  const paymentInterval = termDays * 24 * 60 * 60
  const gracePeriod = Math.min(7 * 24 * 60 * 60, paymentInterval)

  return {
    TransactionType: 'LoanSet',
    Account: vault.loanBrokerAccount,
    Counterparty: walletAddress,
    LoanBrokerID: vault.loanBrokerId,
    PrincipalRequested: action.amount,
    InterestRate: toLoanRateUnits(action.interest_rate),
    PaymentTotal: 1,
    PaymentInterval: paymentInterval,
    GracePeriod: gracePeriod,
  }
}

export async function buildAndSubmitStrategy(
  strategy: Strategy,
  walletAddress: string,
  signAndSubmit: SignAndSubmitFn,
  network: XrplNetwork = 'lend-devnet',
): Promise<StrategyExecutionResult> {
  const support = getStrategyExecutionSupport(strategy, network)
  if (!support.executable) {
    throw new Error(support.reason ?? 'This strategy is not executable.')
  }

  console.debug('[xrplTransactions] buildAndSubmitStrategy start', {
    strategyId: strategy.id,
    title: strategy.title,
    walletAddress,
    network,
    tradeActions: strategy.trade_actions,
  })

  if (!walletAddress) {
    throw new Error('No wallet address - connect a wallet first')
  }

  const prepared: Array<{ action: TradeAction['action']; tx: Record<string, unknown> }> = []

  for (const action of strategy.trade_actions) {
    let tx: Record<string, unknown>
    console.debug('[xrplTransactions] preparing action', action)

    switch (action.action) {
      case 'swap':
        tx = buildSwapTx(action, walletAddress)
        break
      case 'deposit':
        tx = buildDepositTx(action, walletAddress)
        break
      case 'withdraw':
        tx = buildWithdrawTx(action, walletAddress)
        break
      case 'lend':
        tx = await buildLendTx(action, walletAddress)
        break
      case 'borrow':
        tx = await buildBorrowTx(action, walletAddress)
        throw new Error(
          `Borrow action prepared a LoanSet for broker ${(tx.LoanBrokerID as string) ?? 'unknown'}, but submitting it still requires the loan broker's first signature before your wallet can counter-sign.`,
        )
      default:
        throw new Error(`Unknown action type: ${(action as TradeAction).action}`)
    }

    console.debug('[xrplTransactions] prepared tx', {
      action: action.action,
      tx,
    })
    prepared.push({ action: action.action, tx })
  }

  const results: { hash: string; action: string }[] = []

  for (const item of prepared) {
    try {
      console.debug('[xrplTransactions] submitting tx', item)
      const result = await signAndSubmit(item.tx)
      console.debug('[xrplTransactions] submit result', {
        action: item.action,
        hash: result.hash,
      })
      results.push({ hash: result.hash, action: item.action })
    } catch (err) {
      console.error('[xrplTransactions] submit failed', {
        action: item.action,
        error: err instanceof Error ? err.message : err,
      })
      throw new Error(
        `Transaction failed on "${item.action}" action: ${err instanceof Error ? err.message : 'Unknown error'}`,
      )
    }
  }

  return {
    txHash: results[results.length - 1].hash,
    status: 'success',
    results,
    network,
  }
}
