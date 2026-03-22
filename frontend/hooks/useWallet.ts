'use client'

import { useState, useEffect, useCallback, useRef } from 'react'
import {
  KeyEntryProvider,
  OtsuProvider,
  CrossmarkProvider,
  type WalletProvider,
  type ProviderType,
  type SignAndSubmitFn,
} from '@/lib/wallet-providers'

// --------------- Constants ---------------

const STORAGE_KEY = 'xrpl_wallet'
const PROVIDER_KEY = 'xrpl_provider_type'

// --------------- Exported Types ---------------

export type WalletState = {
  address: string | null
  isConnecting: boolean
  providerType: ProviderType | null
  connect: () => Promise<void>
  connectWithKey: (secret: string) => Promise<void>
  generateNewWallet: () => { address: string; secret: string }
  disconnect: () => void
  signAndSubmit: SignAndSubmitFn
}

// --------------- Hook ---------------

export function useWallet(): WalletState {
  const [address, setAddress] = useState<string | null>(null)
  const [isConnecting, setIsConnecting] = useState(false)
  const [providerType, setProviderType] = useState<ProviderType | null>(null)

  const providerRef = useRef<WalletProvider | null>(null)
  const secretRef = useRef<string | null>(null)

  // Rehydrate address + provider type on mount
  useEffect(() => {
    try {
      const storedAddress = localStorage.getItem(STORAGE_KEY)
      const storedProvider = localStorage.getItem(PROVIDER_KEY) as ProviderType | null
      if (storedAddress) {
        setAddress(storedAddress)
        setProviderType(storedProvider)
        // For extension providers, try to recreate the provider instance
        if (storedProvider === 'otsu' && window.xrpl) {
          providerRef.current = new OtsuProvider()
        } else if (storedProvider === 'crossmark' && window.crossmark) {
          providerRef.current = new CrossmarkProvider()
        }
        // key-entry: address is shown but secret is gone; user must reconnect
      }
    } catch {
      // localStorage unavailable
    }
  }, [])

  // Auto-detect extension provider: Otsu > Crossmark
  const connect = useCallback(async () => {
    setIsConnecting(true)
    try {
      let provider: WalletProvider

      if (typeof window !== 'undefined' && window.xrpl) {
        provider = new OtsuProvider()
      } else if (typeof window !== 'undefined' && window.crossmark) {
        provider = new CrossmarkProvider()
      } else {
        throw new Error(
          'No wallet extension detected. Use Key Entry to connect.',
        )
      }

      const addr = await provider.connect()
      providerRef.current = provider
      setAddress(addr)
      setProviderType(provider.type)
      try {
        localStorage.setItem(PROVIDER_KEY, provider.type)
      } catch {
        // ignore
      }
    } finally {
      setIsConnecting(false)
    }
  }, [])

  // Connect with a manually-entered secret key
  const connectWithKey = useCallback(async (secret: string) => {
    setIsConnecting(true)
    try {
      secretRef.current = secret
      const provider = new KeyEntryProvider(secretRef)
      const addr = await provider.connect()
      providerRef.current = provider
      setAddress(addr)
      setProviderType('key-entry')
      try {
        localStorage.setItem(PROVIDER_KEY, 'key-entry')
      } catch {
        // ignore
      }
    } catch (err) {
      secretRef.current = null
      throw err
    } finally {
      setIsConnecting(false)
    }
  }, [])

  // Generate a new XRPL keypair (does not auto-connect)
  const generateNewWallet = useCallback(() => {
    return KeyEntryProvider.generateNewWallet()
  }, [])

  // Disconnect and clear all state
  const disconnect = useCallback(() => {
    providerRef.current?.disconnect()
    providerRef.current = null
    secretRef.current = null
    setAddress(null)
    setProviderType(null)
    try {
      localStorage.removeItem(STORAGE_KEY)
      localStorage.removeItem(PROVIDER_KEY)
    } catch {
      // ignore
    }
  }, [])

  // Delegate signing to the active provider
  const signAndSubmit: SignAndSubmitFn = useCallback(
    async (tx: Record<string, unknown>) => {
      if (!providerRef.current) {
        throw new Error('No wallet connected')
      }
      if (providerRef.current.type === 'key-entry' && !secretRef.current) {
        throw new Error(
          'Secret key expired. Please reconnect with your key.',
        )
      }
      return providerRef.current.signAndSubmit(tx)
    },
    [],
  )

  return {
    address,
    isConnecting,
    providerType,
    connect,
    connectWithKey,
    generateNewWallet,
    disconnect,
    signAndSubmit,
  }
}
