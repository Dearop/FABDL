/**
 * Vault Registry Service
 *
 * Lending-devnet execution is limited to a small, deterministic set of vaults.
 * Vault discovery happens from explicit vault IDs, with optional broker lookup
 * per vault account when needed.
 */

import { Client } from 'xrpl'
import { getXrplWsUrl } from '@/lib/wallet-providers'
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

export type VaultKey = string

const CONNECT_TIMEOUT_MS = 10_000
const REQUEST_TIMEOUT_MS = 15_000

const CONFIGURED_VAULTS: Record<VaultKey, VaultEntry> = {
  XRP: {
    vaultId: '0003696200AC68669B62D187FCBC2911CBA663DA5EBF8BE328177F3BE3834E76',
    asset: { currency: 'XRP' },
    vaultAccount: 'r4RhiW8F5HYmK93SwjuFApBXv7Z8AZWyFJ',
    loanBrokerId: '93A79D15EEF29D43EA36F75F770D400A0B565FFD81370E70D1D7D7C8260C4266',
    loanBrokerAccount: 'rGYY67QvsdU4YJNACALcxoW6y6GR5DUdoB',
  },
  USD: {
    vaultId: '0004F9850AE032287576264B2DD2B8C151C3323C0BCDAFE0B1D1BD19B1B85ECE',
    asset: { currency: 'USD', issuer: 'rh6KkQSYrG9arNP5b5KjK4ggZhVvD1nZ2C' },
    vaultAccount: 'rG5qS52cGeMmDQpfXS1TnL1qGaScbW12x7',
    loanBrokerId: '5937D9F102AA863BD2EAA709DFCBAC51F8D22C4C0F26F3BEFAA8309E5C1A952A',
    loanBrokerAccount: 'rEdpSH6qEtb2BFFF8Ph87zdLLcLz2QxaeG',
  },
}

const validatedVaults = new Map<VaultKey, VaultEntry>()

async function withTimeout<T>(
  promise: Promise<T>,
  timeoutMs: number,
  label: string,
): Promise<T> {
  let timeoutHandle: ReturnType<typeof setTimeout> | undefined

  try {
    return await Promise.race([
      promise,
      new Promise<T>((_, reject) => {
        timeoutHandle = setTimeout(() => {
          reject(new Error(`${label} timed out after ${timeoutMs}ms`))
        }, timeoutMs)
      }),
    ])
  } finally {
    if (timeoutHandle) clearTimeout(timeoutHandle)
  }
}

function normalizeVaultKey(assetCode: string): VaultKey {
  return assetCode.trim().toUpperCase()
}

function cloneVault(entry: VaultEntry): VaultEntry {
  return {
    ...entry,
    asset: entry.asset.currency === 'XRP' ? { currency: 'XRP' } : { ...entry.asset },
  }
}

function assetFromVaultNode(node: Record<string, unknown>): PoolAsset | null {
  const raw = node.Asset as Record<string, string> | undefined
  if (!raw?.currency) return null
  if (raw.currency === 'XRP') return { currency: 'XRP' }
  if (raw.issuer) return { currency: raw.currency.toUpperCase(), issuer: raw.issuer }
  return null
}

function assetMatchesKey(asset: PoolAsset, key: VaultKey): boolean {
  return asset.currency.toUpperCase() === key
}

async function fetchVaultNode(
  client: Client,
  vaultId: string,
): Promise<Record<string, unknown>> {
  const response = await withTimeout(
    client.request({
      command: 'ledger_entry',
      ledger_index: 'validated',
      index: vaultId,
    } as Parameters<typeof client.request>[0]),
    REQUEST_TIMEOUT_MS,
    `Vault lookup ${vaultId}`,
  )

  return response.result.node as Record<string, unknown>
}

async function resolveLoanBrokerForVault(
  client: Client,
  vault: VaultEntry,
): Promise<Pick<VaultEntry, 'loanBrokerId' | 'loanBrokerAccount'>> {
  const response = await withTimeout(
    client.request({
      command: 'account_objects',
      account: vault.vaultAccount,
      type: 'loan_broker',
      ledger_index: 'validated',
      limit: 20,
    } as Parameters<typeof client.request>[0]),
    REQUEST_TIMEOUT_MS,
    `Loan broker lookup ${vault.vaultAccount}`,
  )

  const objects = (response.result.account_objects ?? []) as Array<Record<string, unknown>>
  for (const entry of objects) {
    if (entry.LedgerEntryType !== 'LoanBroker') continue
    if ((entry.VaultID as string | undefined) !== vault.vaultId) continue

    return {
      loanBrokerId: (entry.index as string | undefined) ?? vault.loanBrokerId,
      loanBrokerAccount: (entry.Account as string | undefined) ?? vault.loanBrokerAccount,
    }
  }

  return {
    loanBrokerId: vault.loanBrokerId,
    loanBrokerAccount: vault.loanBrokerAccount,
  }
}

async function validateConfiguredVault(
  client: Client,
  entry: VaultEntry,
): Promise<VaultEntry> {
  const node = await fetchVaultNode(client, entry.vaultId)
  if (node.LedgerEntryType !== 'Vault') {
    throw new Error(`Configured vault ${entry.vaultId} is not a live Vault ledger entry`)
  }

  const asset = assetFromVaultNode(node)
  if (!asset || !assetMatchesKey(asset, entry.asset.currency.toUpperCase())) {
    throw new Error(
      `Configured vault ${entry.vaultId} does not match expected asset ${entry.asset.currency}`,
    )
  }

  const vaultAccount = (node.Account as string | undefined) ?? entry.vaultAccount
  const broker = await resolveLoanBrokerForVault(client, { ...entry, asset, vaultAccount })

  return {
    ...entry,
    asset,
    vaultAccount,
    ...broker,
  }
}

export function listConfiguredVaults(): VaultKey[] {
  return Object.keys(CONFIGURED_VAULTS)
}

export function isVaultSupportedForExecution(assetCode: string | null | undefined): boolean {
  if (!assetCode) return false
  return Object.prototype.hasOwnProperty.call(CONFIGURED_VAULTS, normalizeVaultKey(assetCode))
}

export async function resolveVaultByAsset(
  assetCode: string,
  wsUrl: string = getXrplWsUrl(),
): Promise<VaultEntry> {
  const key = normalizeVaultKey(assetCode)
  const cached = validatedVaults.get(key)
  if (cached) return cached

  const configured = CONFIGURED_VAULTS[key]
  if (!configured) {
    const available = listConfiguredVaults().join(', ') || 'none'
    throw new Error(
      `Vault "${key}" is not configured for live execution on lending devnet. Available vaults: ${available}.`,
    )
  }

  const client = new Client(wsUrl)
  await withTimeout(client.connect(), CONNECT_TIMEOUT_MS, 'Vault lookup websocket connect')

  try {
    const resolved = await validateConfiguredVault(client, configured)
    validatedVaults.set(key, resolved)
    return resolved
  } finally {
    if (client.isConnected()) await client.disconnect()
  }
}

export async function fetchAllVaults(): Promise<Map<VaultKey, VaultEntry>> {
  const entries: Array<[VaultKey, VaultEntry]> = Object.entries(CONFIGURED_VAULTS)
    .map(([key, value]): [VaultKey, VaultEntry] => [key, cloneVault(value)])
  return new Map<VaultKey, VaultEntry>(entries)
}
