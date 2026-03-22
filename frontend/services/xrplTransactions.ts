/**
 * XRPL Transaction Builder
 *
 * Converts strategy trade_actions into real XRPL transactions and submits
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

// --------------- Types ---------------

export type PoolRegistry = Map<PoolKey, PoolEntry>

// --------------- Helpers ---------------

/** Convert XRP amount to drops string. */
function toDrops(xrpAmount: number): string {
  return String(Math.round(xrpAmount * 1_000_000))
}

/**
 * Build an XRPL Amount value for a transaction field.
 * - XRP  → drops string
 * - Token → { currency, issuer, value }
 */
function toAmount(
  asset: PoolAsset,
  amount: number,
): string | { currency: string; issuer: string; value: string } {
  if (asset.currency === 'XRP') return toDrops(amount)
  const { currency, issuer } = asset as { currency: string; issuer: string }
  return { currency, issuer, value: String(amount) }
}

/**
 * Look up a pool by its key and return the ordered assets.
 * Throws a descriptive error if the pool is not in the registry.
 */
function resolvePool(
  pool: string,
  registry: PoolRegistry,
): { asset1: PoolAsset; asset2: PoolAsset; entry: PoolEntry } {
  // Normalise the incoming key to match registry format
  const parts = pool.split('/')
  if (parts.length !== 2) {
    throw new Error(`Invalid pool format "${pool}" — expected "ASSET1/ASSET2"`)
  }

  // Build synthetic PoolAsset objects just for key generation
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
    throw new Error(
      `Pool "${key}" not found in registry. Available pools: ${available}`,
    )
  }

  return { asset1: entry.asset1, asset2: entry.asset2, entry }
}

// --------------- Transaction Builders ---------------

function buildSwapTx(
  action: TradeAction,
  walletAddress: string,
  registry: PoolRegistry,
): Record<string, unknown> {
  const pool = action.pool
  if (!pool) throw new Error('Swap action missing pool field')

  const { asset1, asset2 } = resolvePool(pool, registry)

  // Determine which side is in (asset_in) and which is out (asset_out)
  const assetIn = action.asset_in.toUpperCase() === 'XRP' ||
    asset1.currency === action.asset_in.toUpperCase()
    ? asset1
    : asset2
  const assetOut = assetIn === asset1 ? asset2 : asset1

  const slippageMultiplier = 1 + (action.estimated_slippage ?? 0)

  // Cross-currency self-Payment: Amount = desired output, SendMax = max input
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

  // tfTwoAssetIfEmpty = 0x100000 (1048576)
  // tfSingleAsset     = 0x80000  (524288)
  const flags = action.deposit_mode === 'two_asset' ? 1048576 : 524288

  const tx: Record<string, unknown> = {
    TransactionType: 'AMMDeposit',
    Account: walletAddress,
    Asset: asset1,
    Asset2: asset2,
    Amount: toAmount(asset1.currency === action.asset_in.toUpperCase() ? asset1 : asset2, action.amount),
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

  // tfWithdrawAll (proportional) = 0x20000 (131072)
  return {
    TransactionType: 'AMMWithdraw',
    Account: walletAddress,
    Asset: asset1,
    Asset2: asset2,
    Amount: toAmount(asset1.currency === action.asset_in.toUpperCase() ? asset1 : asset2, action.amount),
    Flags: 131072,
  }
}

function buildLendTx(
  _action: TradeAction,
  _walletAddress: string,
): Record<string, unknown> {
  // TODO: XLS-66d lending on XRPL uses a vault-based model (XLS-65 Single Asset Vaults).
  // Lending flow:
  //   1. Depositor deposits into a Single Asset Vault (SAV) via XLS-65
  //   2. A LoanBroker manages the vault and issues loans via LoanBrokerSet
  //   3. Funds are lent out via LoanSet, which references a VaultID
  // The VaultID must be discovered from the lending devnet for the relevant asset.
  //
  // Relevant tx types: LoanBrokerSet, LoanBrokerCoverDeposit, LoanSet, LoanPay
  // Devnet: wss://lend.devnet.rippletest.net:51233/
  throw new Error(
    'Lending transactions are not yet implemented (XLS-66d vault-based model — see TODO)',
  )
}

function buildBorrowTx(
  _action: TradeAction,
  _walletAddress: string,
): Record<string, unknown> {
  // TODO: XLS-66d borrowing uses the LoanSet transaction referencing a VaultID.
  // Borrowing flow:
  //   1. Borrower calls LoanSet referencing a VaultID
  //   2. Principal is transferred from the SAV to the borrower
  //   3. Repayment via LoanPay; default handling via LoanManage
  //
  // Relevant tx types: LoanSet, LoanPay, LoanDelete, LoanManage
  // Devnet: wss://lend.devnet.rippletest.net:51233/
  throw new Error(
    'Borrowing transactions are not yet implemented (XLS-66d vault-based model — see TODO)',
  )
}

// --------------- Main Export ---------------

/**
 * Build XRPL transactions from strategy trade_actions and submit them
 * sequentially via the provided signAndSubmit function.
 *
 * @param registry - Pool registry fetched from the devnet. Must be non-empty
 *   for any action that references a pool (swap, deposit, withdraw).
 */
export async function buildAndSubmitStrategy(
  strategy: Strategy,
  walletAddress: string,
  signAndSubmit: SignAndSubmitFn,
  registry: PoolRegistry,
): Promise<{ txHash: string; status: string }> {
  if (!walletAddress) {
    throw new Error('No wallet address — connect a wallet first')
  }

  if (strategy.trade_actions.length === 0) {
    throw new Error('Strategy has no trade actions to execute')
  }

  const results: { hash: string; action: string }[] = []

  for (const action of strategy.trade_actions) {
    let tx: Record<string, unknown>

    switch (action.action) {
      case 'swap':
        tx = buildSwapTx(action, walletAddress, registry)
        break
      case 'deposit':
        tx = buildDepositTx(action, walletAddress, registry)
        break
      case 'withdraw':
        tx = buildWithdrawTx(action, walletAddress, registry)
        break
      case 'lend':
        tx = buildLendTx(action, walletAddress)
        break
      case 'borrow':
        tx = buildBorrowTx(action, walletAddress)
        break
      default:
        throw new Error(`Unknown action type: ${(action as TradeAction).action}`)
    }

    try {
      const result = await signAndSubmit(tx)
      results.push({ hash: result.hash, action: action.action })
    } catch (err) {
      throw new Error(
        `Transaction failed on "${action.action}" action: ${err instanceof Error ? err.message : 'Unknown error'}`,
      )
    }
  }

  return {
    txHash: results[results.length - 1].hash,
    status: 'success',
  }
}
