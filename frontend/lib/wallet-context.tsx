'use client'

import { createContext, useContext, useState, useCallback, type ReactNode } from 'react'
import type { WalletInfo, AppStatus, Strategy } from './types'
import { OtsuWallet } from './otsu-wallet'

interface WalletContextType {
  wallet: WalletInfo | null
  status: AppStatus
  strategies: Strategy[]
  selectedStrategy: Strategy | null
  txHash: string | null
  error: string | null
  lastQuery: string
  connectWallet: () => Promise<void>
  disconnectWallet: () => void
  setStatus: (status: AppStatus) => void
  setStrategies: (strategies: Strategy[]) => void
  setSelectedStrategy: (strategy: Strategy | null) => void
  setTxHash: (hash: string | null) => void
  setError: (error: string | null) => void
  setLastQuery: (query: string) => void
  resetToReady: () => void
}

const WalletContext = createContext<WalletContextType | null>(null)

export function WalletProvider({ children }: { children: ReactNode }) {
  const [wallet, setWallet] = useState<WalletInfo | null>(null)
  const [status, setStatus] = useState<AppStatus>('disconnected')
  const [strategies, setStrategies] = useState<Strategy[]>([])
  const [selectedStrategy, setSelectedStrategy] = useState<Strategy | null>(null)
  const [txHash, setTxHash] = useState<string | null>(null)
  const [error, setError] = useState<string | null>(null)
  const [lastQuery, setLastQuery] = useState('')

  const connectWallet = useCallback(async () => {
    setStatus('connecting')
    setError(null)

    try {
      // TEST BRANCH: skip wallet extension entirely
      setWallet({
        address: 'rHb9CJAWyB4rj91VRWn96DkukG4bwdtyTh',
        balance: '1000 XRP',
        network: 'mainnet',
      })
      setStatus('ready')
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to connect wallet')
      setStatus('disconnected')
    }
  }, [])

  const disconnectWallet = useCallback(() => {

    setWallet(null)
    setStatus('disconnected')
    setStrategies([])
    setSelectedStrategy(null)
    setTxHash(null)
    setError(null)
    setLastQuery('')
  }, [])

  const resetToReady = useCallback(() => {
    setStatus('ready')
    setStrategies([])
    setSelectedStrategy(null)
    setTxHash(null)
    setError(null)
    setLastQuery('')
  }, [])

  return (
    <WalletContext.Provider
      value={{
        wallet,
        status,
        strategies,
        selectedStrategy,
        txHash,
        error,
        lastQuery,
        connectWallet,
        disconnectWallet,
        setStatus,
        setStrategies,
        setSelectedStrategy,
        setTxHash,
        setError,
        setLastQuery,
        resetToReady
      }}
    >
      {children}
    </WalletContext.Provider>
  )
}

export function useWallet() {
  const context = useContext(WalletContext)
  if (!context) {
    throw new Error('useWallet must be used within a WalletProvider')
  }
  return context
}
