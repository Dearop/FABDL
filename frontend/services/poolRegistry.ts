/**
 * Pool Registry Service
 *
 * Live AMM execution on lending devnet must use an allowlist of issuer-qualified
 * pool assets. XRPL JSON-RPC does not provide a bounded pool-discovery path that
 * is safe to run in the browser during execution.
 */

// --------------- Types ---------------

export type XrpAsset = { currency: 'XRP' }
export type TokenAsset = { currency: string; issuer: string }
export type PoolAsset = XrpAsset | TokenAsset

export interface PoolEntry {
  /** Optional AMM account address, if known */
  ammAccount: string
  asset1: PoolAsset
  asset2: PoolAsset
  /** Trading fee in basis points (e.g. 500 = 0.5%) */
  tradingFee: number
}

export type PoolKey = string

interface ConfiguredPoolInput {
  ammAccount?: string
  asset1: PoolAsset
  asset2: PoolAsset
  tradingFee?: number
}

// --------------- Helpers ---------------

function assetLabel(asset: PoolAsset): string {
  return asset.currency === 'XRP' ? 'XRP' : asset.currency.toUpperCase()
}

export function poolKey(a: PoolAsset, b: PoolAsset): PoolKey {
  const la = assetLabel(a)
  const lb = assetLabel(b)
  if (la === 'XRP') return `XRP/${lb}`
  if (lb === 'XRP') return `XRP/${la}`
  return la < lb ? `${la}/${lb}` : `${lb}/${la}`
}

export function normalizePoolLabel(pool: string): PoolKey {
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

  return poolKey(a, b)
}

function isTokenAsset(value: unknown): value is TokenAsset {
  return Boolean(
    value &&
      typeof value === 'object' &&
      'currency' in value &&
      'issuer' in value &&
      typeof (value as TokenAsset).currency === 'string' &&
      typeof (value as TokenAsset).issuer === 'string' &&
      (value as TokenAsset).issuer.length > 0,
  )
}

function isPoolAsset(value: unknown): value is PoolAsset {
  if (!value || typeof value !== 'object' || !('currency' in value)) return false
  const currency = (value as { currency?: unknown }).currency
  if (currency === 'XRP') return true
  return isTokenAsset(value)
}

function loadConfiguredPools(): Map<PoolKey, PoolEntry> {
  const raw = process.env.NEXT_PUBLIC_EXECUTION_POOL_REGISTRY_JSON
  if (!raw) return new Map()

  try {
    const parsed = JSON.parse(raw) as unknown
    if (!Array.isArray(parsed)) {
      console.warn('[poolRegistry] NEXT_PUBLIC_EXECUTION_POOL_REGISTRY_JSON must be a JSON array')
      return new Map()
    }

    const pools = new Map<PoolKey, PoolEntry>()
    for (const candidate of parsed) {
      if (!candidate || typeof candidate !== 'object') continue
      const { asset1, asset2, ammAccount, tradingFee } = candidate as ConfiguredPoolInput
      if (!isPoolAsset(asset1) || !isPoolAsset(asset2)) continue

      const key = poolKey(asset1, asset2)
      pools.set(key, {
        ammAccount: ammAccount ?? '',
        asset1,
        asset2,
        tradingFee: tradingFee ?? 0,
      })
    }

    return pools
  } catch (error) {
    console.warn('[poolRegistry] failed to parse NEXT_PUBLIC_EXECUTION_POOL_REGISTRY_JSON', error)
    return new Map()
  }
}

const CONFIGURED_POOLS = loadConfiguredPools()

// --------------- Public API ---------------

export function listConfiguredPools(): PoolKey[] {
  return [...CONFIGURED_POOLS.keys()]
}

export function isPoolConfigured(pool: string | null | undefined): boolean {
  if (!pool) return false

  try {
    return CONFIGURED_POOLS.has(normalizePoolLabel(pool))
  } catch {
    return false
  }
}

export function resolveConfiguredPool(pool: string): PoolEntry {
  const key = normalizePoolLabel(pool)
  const entry = CONFIGURED_POOLS.get(key)

  if (!entry) {
    const available = listConfiguredPools().join(', ') || 'none'
    throw new Error(
      `AMM pool "${key}" is not configured for live execution on lending devnet. Available configured pools: ${available}.`,
    )
  }

  return entry
}

export async function fetchAllPools(): Promise<Map<PoolKey, PoolEntry>> {
  return new Map(CONFIGURED_POOLS)
}
