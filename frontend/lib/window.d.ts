/** Global type declarations for wallet extension providers injected into window */

interface OtsuXrplProvider {
  connect(params?: { scopes?: string[] }): Promise<{ address: string }>
  disconnect(): Promise<void>
  getAddress(): Promise<{ address: string }>
  getNetwork(): Promise<{ network: string }>
  signTransaction(tx: Record<string, unknown>): Promise<{ tx_blob: string; hash: string }>
  signAndSubmit(tx: Record<string, unknown>): Promise<{ tx_blob: string; hash: string }>
  switchNetwork(networkId: string): Promise<{ network: string }>
  on(event: string, callback: (...args: unknown[]) => void): void
  off(event: string, callback: (...args: unknown[]) => void): void
}

declare global {
  interface Window {
    xrpl?: OtsuXrplProvider
    crossmark?: unknown // presence-check only; actual API via @crossmarkio/sdk
  }
}

export {}
