/**
 * Adapter for the Otsu Wallet browser extension.
 * The extension injects itself at window.xrpl.
 * API mirrors https://github.com/RomThpt/otsu-wallet (packages/api)
 */

export interface AddressInfo {
  address: string
}

export interface BalanceInfo {
  available: string
  total: string
  reserved: string
}

export interface NetworkInfo {
  networkId: string
}

export interface SignedTransaction {
  blob: string
  hash: string
}

type OtsuEventType = 'accountChanged' | 'networkChanged' | 'connected' | 'disconnected'
type OtsuEventCallback = (data: unknown) => void

interface XrplProvider {
  isOtsu?: boolean
  connect(params?: { scopes?: string[] }): Promise<AddressInfo>
  disconnect(): Promise<void>
  isConnected(): boolean
  getAddress(): Promise<AddressInfo>
  getNetwork(): Promise<NetworkInfo>
  getBalance(): Promise<BalanceInfo>
  signTransaction(tx: Record<string, unknown>): Promise<SignedTransaction>
  signAndSubmit(tx: Record<string, unknown>): Promise<SignedTransaction>
  on(event: OtsuEventType, callback: OtsuEventCallback): void
  off(event: OtsuEventType, callback: OtsuEventCallback): void
}

declare global {
  interface Window {
    xrpl?: XrplProvider
  }
}

export class OtsuWallet {
  static isInstalled(): boolean {
    return typeof window !== 'undefined' && !!window.xrpl
  }

  connect(params?: { scopes?: string[] }): Promise<AddressInfo> {
    if (!window.xrpl) throw new Error('Otsu Wallet extension not found')
    return window.xrpl.connect(params)
  }

  disconnect(): Promise<void> {
    if (!window.xrpl) throw new Error('Otsu Wallet extension not found')
    return window.xrpl.disconnect()
  }

  isConnected(): boolean {
    return !!window.xrpl?.isConnected()
  }

  getAddress(): Promise<AddressInfo> {
    if (!window.xrpl) throw new Error('Otsu Wallet extension not found')
    return window.xrpl.getAddress()
  }

  getNetwork(): Promise<NetworkInfo> {
    if (!window.xrpl) throw new Error('Otsu Wallet extension not found')
    return window.xrpl.getNetwork()
  }

  getBalance(): Promise<BalanceInfo> {
    if (!window.xrpl) throw new Error('Otsu Wallet extension not found')
    return window.xrpl.getBalance()
  }

  signTransaction(tx: Record<string, unknown>): Promise<SignedTransaction> {
    if (!window.xrpl) throw new Error('Otsu Wallet extension not found')
    return window.xrpl.signTransaction(tx)
  }

  signAndSubmit(tx: Record<string, unknown>): Promise<SignedTransaction> {
    if (!window.xrpl) throw new Error('Otsu Wallet extension not found')
    return window.xrpl.signAndSubmit(tx)
  }

  on(event: OtsuEventType, callback: OtsuEventCallback): void {
    window.xrpl?.on(event, callback)
  }

  off(event: OtsuEventType, callback: OtsuEventCallback): void {
    window.xrpl?.off(event, callback)
  }
}
