/**
 * Vault Registry Service
 *
 * Fetches all XLS-65 Single Asset Vaults from the lending devnet.
 * Mirrors poolRegistry.ts - pure async logic, no React dependencies.
 *
 * The live lending devnet exposes discoverable vaults and loan brokers
 * through AccountRoot helpers plus ledger_entry lookups for the actual
 * Vault / LoanBroker objects.
 */

import { Client } from 'xrpl'
import type { PoolAsset } from '@/services/poolRegistry'

export interface VaultEntry {
  /** 64-hex VaultID used in VaultDeposit / LoanSet transactions */
  vaultId: string
  /** The asset held in this vault */
  asset: PoolAsset
  /** Pseudo-account associated with the vault */
  vaultAccount: string
  /** Optional live broker associated with this vault asset */
  loanBrokerId?: string
  /** Pseudo-account associated with the broker */
  loanBrokerAccount?: string
}

/** Vault key: just the currency ticker, e.g. "XRP" or "USD" */
export type VaultKey = string

const LENDING_DEVNET_WS = 'wss://lend.devnet.rippletest.net:51233/'

function assetFromVaultNode(node: Record<string, unknown>): PoolAsset | null {
  const raw = node.Asset as Record<string, string> | undefined
  if (!raw) return null
  if (raw.currency === 'XRP') return { currency: 'XRP' }
  if (raw.currency && raw.issuer) return { currency: raw.currency, issuer: raw.issuer }
  return null
}

async function fetchLedgerEntry(
  client: Client,
  index: string,
): Promise<Record<string, unknown> | null> {
  try {
    const response = await client.request({
      command: 'ledger_entry',
      ledger_index: 'validated',
      index,
    } as Parameters<typeof client.request>[0])

    return response.result.node as Record<string, unknown>
  } catch {
    return null
  }
}

async function fetchEntriesInChunks(
  client: Client,
  indexes: string[],
): Promise<Map<string, Record<string, unknown>>> {
  const results = new Map<string, Record<string, unknown>>()

  for (let i = 0; i < indexes.length; i += 20) {
    const chunk = indexes.slice(i, i + 20)
    const entries = await Promise.all(
      chunk.map(async (index) => [index, await fetchLedgerEntry(client, index)] as const),
    )

    for (const [index, entry] of entries) {
      if (entry) results.set(index, entry)
    }
  }

  return results
}

/**
 * Fetch all Single Asset Vaults from the given XRPL websocket endpoint.
 * Returns a Map keyed by asset currency ticker (e.g. "XRP", "USD").
 * If multiple vaults exist for the same asset, a broker-enabled vault wins.
 */
export async function fetchAllVaults(
  wsUrl: string = LENDING_DEVNET_WS,
): Promise<Map<VaultKey, VaultEntry>> {
  const client = new Client(wsUrl)
  await client.connect()

  const vaultRoots: Array<{ vaultId: string; vaultAccount: string }> = []
  const brokerRoots: Array<{ loanBrokerId: string; loanBrokerAccount: string }> = []

  try {
    let marker: unknown = undefined

    do {
      const response = await client.request({
        command: 'ledger_data',
        ledger_index: 'validated',
        type: 'account',
        limit: 400,
        ...(marker !== undefined ? { marker } : {}),
      } as Parameters<typeof client.request>[0])

      const { state, marker: nextMarker } = response.result as {
        state: Array<Record<string, unknown>>
        marker?: unknown
      }

      for (const entry of state) {
        if (entry.LedgerEntryType !== 'AccountRoot') continue

        const vaultId = entry.VaultID as string | undefined
        if (vaultId) {
          vaultRoots.push({
            vaultId,
            vaultAccount: (entry.Account as string) ?? '',
          })
        }

        const loanBrokerId = entry.LoanBrokerID as string | undefined
        if (loanBrokerId) {
          brokerRoots.push({
            loanBrokerId,
            loanBrokerAccount: (entry.Account as string) ?? '',
          })
        }
      }

      marker = nextMarker
    } while (marker !== undefined)

    const vaultNodes = await fetchEntriesInChunks(
      client,
      vaultRoots.map((entry) => entry.vaultId),
    )
    const brokerNodes = await fetchEntriesInChunks(
      client,
      brokerRoots.map((entry) => entry.loanBrokerId),
    )

    const brokerByVaultId = new Map<string, { loanBrokerId: string; loanBrokerAccount: string }>()
    for (const brokerRoot of brokerRoots) {
      const node = brokerNodes.get(brokerRoot.loanBrokerId)
      if (!node || node.LedgerEntryType !== 'LoanBroker') continue

      const vaultId = node.VaultID as string | undefined
      if (!vaultId || brokerByVaultId.has(vaultId)) continue

      brokerByVaultId.set(vaultId, {
        loanBrokerId: brokerRoot.loanBrokerId,
        loanBrokerAccount: brokerRoot.loanBrokerAccount,
      })
    }

    const vaults = new Map<VaultKey, VaultEntry>()
    for (const vaultRoot of vaultRoots) {
      const node = vaultNodes.get(vaultRoot.vaultId)
      if (!node || node.LedgerEntryType !== 'Vault') continue

      const asset = assetFromVaultNode(node)
      if (!asset) continue

      const key = asset.currency
      const broker = brokerByVaultId.get(vaultRoot.vaultId)
      const candidate: VaultEntry = {
        vaultId: vaultRoot.vaultId,
        asset,
        vaultAccount: vaultRoot.vaultAccount,
        ...(broker ?? {}),
      }

      const existing = vaults.get(key)
      if (!existing || (!existing.loanBrokerId && candidate.loanBrokerId)) {
        vaults.set(key, candidate)
      }
    }

    return vaults
  } finally {
    if (client.isConnected()) await client.disconnect()
  }
}
