import { Client, Wallet } from 'xrpl'
import type { MutableRefObject } from 'react'
import sdk from '@crossmarkio/sdk'

// --------------- Constants ---------------

/** All providers use the same XRPL network — configurable via env var. */
const XRPL_WS =
  process.env.NEXT_PUBLIC_XRPL_WS || 'wss://s.devnet.rippletest.net:51233/'
const STORAGE_KEY = 'xrpl_wallet'

// --------------- Network Helpers ---------------

export type XrplNetwork = 'devnet' | 'testnet' | 'lend-devnet'

/** Current network label derived from the configured WS URL. */
export function currentNetwork(): XrplNetwork {
  const ws = XRPL_WS
  if (ws.includes('lend.devnet')) return 'lend-devnet'
  if (ws.includes('altnet')) return 'testnet'
  return 'devnet'
}

export function networkForProvider(providerType: ProviderType | null): XrplNetwork | null {
  if (!providerType || providerType === 'crossmark') return null
  return currentNetwork()
}

export function getXrplWsUrl(): string {
  return XRPL_WS
}

// --------------- Shared Types ---------------

export type SignAndSubmitFn = (
  tx: Record<string, unknown>,
) => Promise<{ hash: string }>

export type ProviderType = 'key-entry' | 'otsu' | 'crossmark'

export interface WalletProvider {
  type: ProviderType
  connect(): Promise<string> // returns address
  signAndSubmit: SignAndSubmitFn
  disconnect(): void
}

// --------------- KeyEntryProvider ---------------

export class KeyEntryProvider implements WalletProvider {
  type = 'key-entry' as const
  private secretRef: MutableRefObject<string | null>

  constructor(secretRef: MutableRefObject<string | null>) {
    this.secretRef = secretRef
  }

  async connect(): Promise<string> {
    const secret = this.secretRef.current
    if (!secret) throw new Error('No secret key provided')

    const wallet = Wallet.fromSecret(secret)
    const address = wallet.classicAddress
    try {
      localStorage.setItem(STORAGE_KEY, address)
    } catch {
      // ignore storage errors
    }
    return address
  }

  async signAndSubmit(
    tx: Record<string, unknown>,
  ): Promise<{ hash: string }> {
    const secret = this.secretRef.current
    if (!secret) {
      throw new Error(
        'Secret key expired. Please reconnect with your key.',
      )
    }

    const wallet = Wallet.fromSecret(secret)
    const client = new Client(XRPL_WS)

    try {
      await client.connect()

      const prepared = await client.autofill({
        Account: wallet.classicAddress,
        ...tx,
      })

      const signed = wallet.sign(prepared)
      const result = await client.submitAndWait(signed.tx_blob)

      const meta = result.result.meta
      const engineResult =
        typeof meta === 'object' && meta !== null && 'TransactionResult' in meta
          ? (meta as { TransactionResult: string }).TransactionResult
          : undefined

      if (engineResult && engineResult !== 'tesSUCCESS') {
        throw new Error(`Transaction failed: ${engineResult}`)
      }

      return { hash: result.result.hash }
    } finally {
      if (client.isConnected()) {
        await client.disconnect()
      }
    }
  }

  disconnect(): void {
    this.secretRef.current = null
    try {
      localStorage.removeItem(STORAGE_KEY)
    } catch {
      // ignore
    }
  }

  static generateNewWallet(): { address: string; secret: string } {
    const wallet = Wallet.generate()
    return { address: wallet.classicAddress, secret: wallet.seed! }
  }
}

// --------------- OtsuProvider ---------------

export class OtsuProvider implements WalletProvider {
  type = 'otsu' as const

  async connect(): Promise<string> {
    if (!window.xrpl) {
      throw new Error('Otsu wallet extension not detected')
    }
    const result = await window.xrpl.connect()
    const address = result.address
    try {
      localStorage.setItem(STORAGE_KEY, address)
    } catch {
      // ignore
    }
    return address
  }

  async signAndSubmit(
    tx: Record<string, unknown>,
  ): Promise<{ hash: string }> {
    if (!window.xrpl) {
      throw new Error('Otsu wallet extension not detected')
    }
    const result = await window.xrpl.signAndSubmit(tx)
    return { hash: result.hash }
  }

  disconnect(): void {
    try {
      localStorage.removeItem(STORAGE_KEY)
    } catch {
      // ignore
    }
  }
}

// --------------- CrossmarkProvider ---------------

export class CrossmarkProvider implements WalletProvider {
  type = 'crossmark' as const

  async connect(): Promise<string> {
    if (typeof window === 'undefined' || !window.crossmark) {
      throw new Error('Crossmark extension not installed')
    }
    const response = await sdk.methods.signInAndWait()
    const address: string = response.data.address
    try {
      localStorage.setItem(STORAGE_KEY, address)
    } catch {
      // ignore
    }
    return address
  }

  async signAndSubmit(): Promise<{ hash: string }> {
    throw new Error(
      'Crossmark is identity-only and cannot sign devnet transactions. Reconnect with Key Entry or Otsu wallet.',
    )
  }

  disconnect(): void {
    try {
      localStorage.removeItem(STORAGE_KEY)
    } catch {
      // ignore
    }
  }
}
