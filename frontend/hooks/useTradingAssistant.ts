/**
 * React Hook for managing trading assistant state and API calls
 */

'use client'

import { useState, useCallback } from 'react'
import {
  connectWallet,
  generateStrategies,
  executeStrategy,
  getStrategyStatus,
  checkBackendHealth,
  type GenerateStrategiesResponse,
  type Strategy
} from './api'

export type AppState = 'disconnected' | 'ready' | 'querying' | 'strategies_loaded' | 'executing' | 'error'

export interface Wallet {
  address: string
  balance: string
  network: string
}

export interface ExecutionResult {
  tx_hash: string
  status: string
  strategy_id: string
}

export function useTradingAssistant() {
  // State
  const [state, setState] = useState<AppState>('disconnected')
  const [wallet, setWallet] = useState<Wallet | null>(null)
  const [lastQuery, setLastQuery] = useState<string>('')
  const [strategies, setStrategies] = useState<Strategy[]>([])
  const [selectedStrategy, setSelectedStrategy] = useState<Strategy | null>(null)
  const [executionResult, setExecutionResult] = useState<ExecutionResult | null>(null)
  const [error, setError] = useState<string | null>(null)
  const [loading, setLoading] = useState(false)

  // Reset error when state changes
  const clearError = useCallback(() => setError(null), [])

  // ==================== Wallet Connection ====================

  const handleConnectWallet = useCallback(
    async (address: string, network: string = 'testnet') => {
      try {
        setLoading(true)
        clearError()

        // Check backend health first
        const isHealthy = await checkBackendHealth()
        if (!isHealthy) {
          throw new Error('Backend service is not available')
        }

        const result = await connectWallet(address, network)

        setWallet({
          address: result.wallet_id,
          balance: result.balance,
          network: result.network
        })
        setState('ready')
      } catch (err) {
        const errorMessage = err instanceof Error ? err.message : 'Failed to connect wallet'
        setError(errorMessage)
        setState('error')
      } finally {
        setLoading(false)
      }
    },
    [clearError]
  )

  // ==================== Query & Strategy Generation ====================

  const handleSubmitQuery = useCallback(
    async (query: string) => {
      if (!wallet) {
        setError('Wallet not connected')
        return
      }

      try {
        setLoading(true)
        clearError()
        setState('querying')
        setLastQuery(query)

        // Generate strategies based on query
        const response: GenerateStrategiesResponse = await generateStrategies(query, wallet.address)

        // Store strategies
        setStrategies(response.strategies)
        setState('strategies_loaded')
      } catch (err) {
        const errorMessage = err instanceof Error ? err.message : 'Failed to generate strategies'
        setError(errorMessage)
        setState('error')
      } finally {
        setLoading(false)
      }
    },
    [wallet, clearError]
  )

  // ==================== Strategy Execution ====================

  const handleSelectStrategy = useCallback((strategy: Strategy) => {
    setSelectedStrategy(strategy)
  }, [])

  const handleExecuteStrategy = useCallback(
    async (strategyId: string) => {
      if (!wallet) {
        setError('Wallet not connected')
        return
      }

      try {
        setLoading(true)
        clearError()
        setState('executing')

        // Execute the selected strategy
        const result = await executeStrategy(strategyId, wallet.address)

        setExecutionResult(result)

        // Optionally, start polling for status
        // You could implement auto-polling here
      } catch (err) {
        const errorMessage = err instanceof Error ? err.message : 'Failed to execute strategy'
        setError(errorMessage)
        setState('error')
      } finally {
        setLoading(false)
      }
    },
    [wallet, clearError]
  )

  // ==================== Reset ====================

  const handleReset = useCallback(() => {
    setState('ready')
    setStrategies([])
    setSelectedStrategy(null)
    setExecutionResult(null)
    setLastQuery('')
    clearError()
  }, [clearError])

  const handleDisconnect = useCallback(() => {
    setState('disconnected')
    setWallet(null)
    setStrategies([])
    setSelectedStrategy(null)
    setExecutionResult(null)
    setLastQuery('')
    clearError()
  }, [clearError])

  return {
    // State
    state,
    wallet,
    lastQuery,
    strategies,
    selectedStrategy,
    executionResult,
    error,
    loading,

    // Actions
    handleConnectWallet,
    handleSubmitQuery,
    handleSelectStrategy,
    handleExecuteStrategy,
    handleReset,
    handleDisconnect,
    clearError
  }
}
