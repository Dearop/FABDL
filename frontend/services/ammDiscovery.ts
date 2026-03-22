/**
 * AMM Pool Discovery Service
 *
 * Fetches live AMM pools from the connected XRPL network at wallet
 * connection time. Discovered pools replace the static pool registry
 * for transaction building, ensuring we only reference pools that
 * actually exist on-chain.
 */

import { Client } from 'xrpl'
import { getXrplWsUrl } from '@/lib/wallet-providers'
import type { PoolAsset, PoolEntry, PoolKey } from '@/services/poolRegistry'

// --------------- Types ---------------

export interface DiscoveredPool extends PoolEntry {
  /** LP token balance (total supply) — useful for checking pool health */
  lpTokenBalance?: string
}

// --------------- Internal state ---------------

let discoveredPools = new Map<PoolKey, DiscoveredPool>()
let lastFetchMs = 0
const CACHE_TTL_MS = 5 * 60_000 // re-fetch every 5 min

// --------------- Helpers ---------------

function assetLabel(asset: PoolAsset): string {
  return asset.currency === 'XRP' ? 'XRP' : asset.currency.toUpperCase()
}

function poolKey(a: PoolAsset, b: PoolAsset): PoolKey {
  const la = assetLabel(a)
  const lb = assetLabel(b)
  if (la === 'XRP') return `XRP/${lb}`
  if (lb === 'XRP') return `XRP/${la}`
  return la < lb ? `${la}/${lb}` : `${lb}/${la}`
}

function parseAsset(raw: unknown): PoolAsset | null {
  if (!raw || typeof raw !== 'object') return null
  const obj = raw as Record<string, unknown>

  // XRP is represented as { currency: 'XRP' } or as a string "0" in some contexts
  if (obj.currency === 'XRP' || obj.currency === '0000000000000000000000000000000000000000') {
    return { currency: 'XRP' }
  }
  if (typeof obj.currency === 'string' && typeof obj.issuer === 'string') {
    return { currency: obj.currency.toUpperCase(), issuer: obj.issuer }
  }
  return null
}

// --------------- Public API ---------------

/**
 * Fetch all AMM pools from the XRPL network.
 * Results are cached for 5 minutes.
 */
export async function fetchLivePools(forceRefresh = false): Promise<Map<PoolKey, DiscoveredPool>> {
  if (!forceRefresh && discoveredPools.size > 0 && Date.now() - lastFetchMs < CACHE_TTL_MS) {
    return discoveredPools
  }

  const wsUrl = getXrplWsUrl()
  console.debug('[ammDiscovery] connecting to', wsUrl)
  const client = new Client(wsUrl)

  try {
    await client.connect()

    const pools = new Map<PoolKey, DiscoveredPool>()
    let marker: unknown = undefined

    // Paginate through ledger_data type=amm
    do {
      const request: Record<string, unknown> = {
        command: 'ledger_data',
        type: 'amm',
        ledger_index: 'validated',
        limit: 100,
      }
      if (marker) request.marker = marker

      const response = await client.request(request as Parameters<typeof client.request>[0])
      const state = (response.result as Record<string, unknown>).state as Array<Record<string, unknown>> | undefined

      if (!state || state.length === 0) break

      for (const entry of state) {
        if (entry.LedgerEntryType !== 'AMM') continue

        const asset1 = parseAsset(entry.Asset)
        const asset2 = parseAsset(entry.Asset2)
        if (!asset1 || !asset2) continue

        const key = poolKey(asset1, asset2)
        const ammAccount = (entry.Account as string) ?? ''
        const tradingFee = typeof entry.TradingFee === 'number' ? entry.TradingFee : 0

        // LP token balance
        const lpRaw = entry.LPTokenBalance as Record<string, unknown> | undefined
        const lpTokenBalance = lpRaw?.value as string | undefined

        pools.set(key, {
          ammAccount,
          asset1,
          asset2,
          tradingFee,
          lpTokenBalance,
        })
      }

      marker = (response.result as Record<string, unknown>).marker
    } while (marker)

    discoveredPools = pools
    lastFetchMs = Date.now()

    console.debug(`[ammDiscovery] discovered ${pools.size} AMM pools:`, [...pools.keys()])
    return pools
  } catch (err) {
    console.error('[ammDiscovery] failed to fetch pools', err)
    // Return whatever we had cached
    return discoveredPools
  } finally {
    if (client.isConnected()) {
      await client.disconnect()
    }
  }
}

/**
 * Get the cached discovered pools (does not fetch).
 * Call fetchLivePools() first at wallet connection time.
 */
export function getDiscoveredPools(): Map<PoolKey, DiscoveredPool> {
  return discoveredPools
}

/**
 * Check if a pool exists in the discovered set.
 */
export function isDiscoveredPool(poolLabel: string): boolean {
  try {
    const parts = poolLabel.split('/')
    if (parts.length !== 2) return false
    const a: PoolAsset = parts[0].trim().toUpperCase() === 'XRP'
      ? { currency: 'XRP' }
      : { currency: parts[0].trim().toUpperCase(), issuer: '' }
    const b: PoolAsset = parts[1].trim().toUpperCase() === 'XRP'
      ? { currency: 'XRP' }
      : { currency: parts[1].trim().toUpperCase(), issuer: '' }
    const key = poolKey(a, b)
    return discoveredPools.has(key)
  } catch {
    return false
  }
}

/**
 * Resolve a discovered pool by label. Throws if not found.
 */
export function resolveDiscoveredPool(poolLabel: string): DiscoveredPool {
  const parts = poolLabel.split('/')
  if (parts.length !== 2) {
    throw new Error(`Invalid pool format "${poolLabel}" — expected "ASSET1/ASSET2"`)
  }
  const a: PoolAsset = parts[0].trim().toUpperCase() === 'XRP'
    ? { currency: 'XRP' }
    : { currency: parts[0].trim().toUpperCase(), issuer: '' }
  const b: PoolAsset = parts[1].trim().toUpperCase() === 'XRP'
    ? { currency: 'XRP' }
    : { currency: parts[1].trim().toUpperCase(), issuer: '' }
  const key = poolKey(a, b)

  const pool = discoveredPools.get(key)
  if (!pool) {
    const available = [...discoveredPools.keys()].join(', ') || 'none'
    throw new Error(
      `AMM pool "${key}" was not found on-chain. Available pools: ${available}.`,
    )
  }
  return pool
}

/**
 * Returns a summary of available pools suitable for passing to the backend/Claude
 * so strategies only reference real pools.
 */
export function getAvailablePoolSummary(): string[] {
  return [...discoveredPools.keys()]
}
