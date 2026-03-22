/**
 * XRPL Transaction Builder
 *
 * Converts strategy trade_actions into XRPL transactions and submits
 * them via the connected wallet provider's signAndSubmit function.
 */

import type { Strategy, TradeAction } from '@/lib/types'
import type { SignAndSubmitFn } from '@/lib/wallet-providers'
import {
  poolKey,
  type PoolAsset,
  type PoolEntry,
  type PoolKey,
} from '@/services/poolRegistry'
import type { VaultEntry, VaultKey } from '@/services/vaultRegistry'

export type PoolRegistry = Map<PoolKey, PoolEntry>
export type VaultRegistry = Map<VaultKey, VaultEntry>

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

function resolvePool(
  pool: string,
  registry: PoolRegistry,
): { asset1: PoolAsset; asset2: PoolAsset; entry: PoolEntry } {
  const parts = pool.split('/')
  if (parts.length !== 2) {
    throw new Error(`Invalid pool format "${pool}" - expected "ASSET1/ASSET2"`)
  }

  const a: PoolAsset = parts[0].trim().toUpperCase() === 'XRP'
    ? { currency: 'XRP' }
    : { currency: parts[0].trim().toUpperCase(), issuer: '' }
  const b: PoolAsset = parts[1].trim().toUpperCase() === 'XRP'
    ? { currency: 'XRP' }
    : { currency: parts[1].trim().toUpperCase(), issuer: '' }

  const key = poolKey(a, b)
  const entry = registry.get(key)
  if (!entry) {
    const available = [...registry.keys()].join(', ') || 'none'
    throw new Error(`Pool "${key}" not found in registry. Available pools: ${available}`)
  }

  return { asset1: entry.asset1, asset2: entry.asset2, entry }
}

function resolveVault(
  assetCode: string,
  registry: VaultRegistry,
): VaultEntry {
  const key = assetCode.trim().toUpperCase()
  const entry = registry.get(key)

  if (!entry) {
    const available = [...registry.keys()].join(', ') || 'none'
    throw new Error(`Vault "${key}" not found in registry. Available vaults: ${available}`)
  }

  return entry
}

function toLoanRateUnits(ratePercent: number | null | undefined): number {
  if (ratePercent == null) return 0
  return Math.max(0, Math.round(ratePercent * 100))
}

function buildSwapTx(
  action: TradeAction,
  walletAddress: string,
  registry: PoolRegistry,
): Record<string, unknown> {
  const pool = action.pool
  if (!pool) throw new Error('Swap action missing pool field')

  const { asset1, asset2 } = resolvePool(pool, registry)

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
  registry: PoolRegistry,
): Record<string, unknown> {
  const pool = action.pool
  if (!pool) throw new Error('Deposit action missing pool field')

  const { asset1, asset2 } = resolvePool(pool, registry)
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
  registry: PoolRegistry,
): Record<string, unknown> {
  const pool = action.pool
  if (!pool) throw new Error('Withdraw action missing pool field')

  const { asset1, asset2 } = resolvePool(pool, registry)

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

function buildLendTx(
  action: TradeAction,
  walletAddress: string,
  registry: VaultRegistry,
): Record<string, unknown> {
  const vault = resolveVault(action.asset_in, registry)

  return {
    TransactionType: 'VaultDeposit',
    Account: walletAddress,
    VaultID: vault.vaultId,
    Amount: toAmount(vault.asset, action.amount),
  }
}

function buildBorrowTx(
  action: TradeAction,
  walletAddress: string,
  registry: VaultRegistry,
): Record<string, unknown> {
  const vault = resolveVault(action.asset_out, registry)

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

/**
 * Build XRPL transactions from strategy trade_actions and submit them
 * sequentially via the provided signAndSubmit function.
 */
export async function buildAndSubmitStrategy(
  strategy: Strategy,
  walletAddress: string,
  signAndSubmit: SignAndSubmitFn,
  poolRegistry: PoolRegistry,
  vaultRegistry: VaultRegistry,
): Promise<{ txHash: string; status: string }> {
  if (!walletAddress) {
    throw new Error('No wallet address - connect a wallet first')
  }

  if (strategy.trade_actions.length === 0) {
    throw new Error('Strategy has no trade actions to execute')
  }

  const prepared: Array<{ action: TradeAction['action']; tx: Record<string, unknown> }> = []

  for (const action of strategy.trade_actions) {
    let tx: Record<string, unknown>

    switch (action.action) {
      case 'swap':
        tx = buildSwapTx(action, walletAddress, poolRegistry)
        break
      case 'deposit':
        tx = buildDepositTx(action, walletAddress, poolRegistry)
        break
      case 'withdraw':
        tx = buildWithdrawTx(action, walletAddress, poolRegistry)
        break
      case 'lend':
        tx = buildLendTx(action, walletAddress, vaultRegistry)
        break
      case 'borrow':
        tx = buildBorrowTx(action, walletAddress, vaultRegistry)
        throw new Error(
          `Borrow action prepared a LoanSet for broker ${(tx.LoanBrokerID as string) ?? 'unknown'}, but submitting it still requires the loan broker's first signature before your wallet can counter-sign.`,
        )
      default:
        throw new Error(`Unknown action type: ${(action as TradeAction).action}`)
    }

    prepared.push({ action: action.action, tx })
  }

  const results: { hash: string; action: string }[] = []

  for (const item of prepared) {
    try {
      const result = await signAndSubmit(item.tx)
      results.push({ hash: result.hash, action: item.action })
    } catch (err) {
      throw new Error(
        `Transaction failed on "${item.action}" action: ${err instanceof Error ? err.message : 'Unknown error'}`,
      )
    }
  }

  return {
    txHash: results[results.length - 1].hash,
    status: 'success',
  }
}
