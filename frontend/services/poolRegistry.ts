/**
 * Pool Registry Service
 *
 * Fetches all AMM pools from an XRPL node by walking ledger_data pages.
 * Pure async logic — no React dependencies.
 */

import { Client } from 'xrpl'

// --------------- Types ---------------

export type XrpAsset = { currency: 'XRP' }
export type TokenAsset = { currency: string; issuer: string }
export type PoolAsset = XrpAsset | TokenAsset

export interface PoolEntry {
  /** The AMM's on-ledger account address */
  ammAccount: string
  asset1: PoolAsset
  asset2: PoolAsset
  /** Trading fee in basis points (e.g. 500 = 0.5%) */
  tradingFee: number
}

/** Normalised pool key, e.g. "XRP/USD" — XRP always first, then alphabetical */
export type PoolKey = string

// --------------- Helpers ---------------

/** Normalise an asset to a short label for use in pool keys */
function assetLabel(asset: PoolAsset): string {
  return asset.currency === 'XRP' ? 'XRP' : asset.currency
}

/**
 * Build a canonical pool key from two assets.
 * XRP always comes first; for two non-XRP assets, sort alphabetically.
 */
export function poolKey(a: PoolAsset, b: PoolAsset): PoolKey {
  const la = assetLabel(a)
  const lb = assetLabel(b)
  if (la === 'XRP') return `XRP/${lb}`
  if (lb === 'XRP') return `XRP/${la}`
  return la < lb ? `${la}/${lb}` : `${lb}/${la}`
}

/** Parse an XRPL ledger Asset object into a PoolAsset */
function parseAsset(raw: Record<string, string>): PoolAsset {
  if (raw.currency === 'XRP') return { currency: 'XRP' }
  return { currency: raw.currency, issuer: raw.issuer }
}

// --------------- Core Fetch ---------------

const LENDING_DEVNET_WS = 'wss://lend.devnet.rippletest.net:51233/'

/**
 * Fetch all AMM pools from the given XRPL websocket endpoint.
 * Follows pagination markers until all pages are consumed.
 * Returns a Map keyed by normalised pool key (e.g. "XRP/USD").
 */
export async function fetchAllPools(
  wsUrl: string = LENDING_DEVNET_WS,
): Promise<Map<PoolKey, PoolEntry>> {
  const client = new Client(wsUrl)
  await client.connect()

  const pools = new Map<PoolKey, PoolEntry>()

  try {
    let marker: unknown = undefined

    do {
      const response = await client.request({
        command: 'ledger_data',
        type: 'amm',
        limit: 400,
        ...(marker !== undefined ? { marker } : {}),
      } as Parameters<typeof client.request>[0])

      const { state, marker: nextMarker } = response.result as {
        state: Array<Record<string, unknown>>
        marker?: unknown
      }

      for (const entry of state) {
        if (entry.LedgerEntryType !== 'AMM') continue

        const asset1 = parseAsset(entry.Asset as Record<string, string>)
        const asset2 = parseAsset(entry.Asset2 as Record<string, string>)
        const key = poolKey(asset1, asset2)

        pools.set(key, {
          ammAccount: entry.Account as string,
          asset1,
          asset2,
          tradingFee: (entry.TradingFee as number) ?? 0,
        })
      }

      marker = nextMarker
    } while (marker !== undefined)
  } finally {
    if (client.isConnected()) await client.disconnect()
  }

  return pools
}
