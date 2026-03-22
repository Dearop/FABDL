/**
 * API Service for XRPL AI Trading Backend
 * Handles all HTTP calls to the backend API
 */

import type { Strategy } from '@/lib/types'

const API_BASE = process.env.NEXT_PUBLIC_API_URL || 'http://localhost:8000'

// Types
export interface IntentResponse {
  action: string
  scope: string
  confidence: number
  is_valid: boolean
  parameters: Array<{ key: string; value: string }>
}

export interface GenerateStrategiesResponse {
  intent?: IntentResponse
  strategies: Strategy[]
  wallet_id: string
  mode?: string
}

// ==================== Wallet Endpoints ====================

/**
 * Connect a user's wallet to the system
 */
export async function connectWallet(address: string, network: string = 'testnet') {
  const response = await fetch(`${API_BASE}/wallet/connect`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ address, network })
  })

  if (!response.ok) {
    throw new Error(`Failed to connect wallet: ${response.statusText}`)
  }

  return response.json()
}

// ==================== Query Endpoints ====================

/**
 * Send a query to the Intent Router for classification
 */
export async function classifyQuery(query: string, walletId: string) {
  const response = await fetch(`${API_BASE}/query/classify`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ user_query: query, wallet_id: walletId })
  })

  if (!response.ok) {
    throw new Error(`Failed to classify query: ${response.statusText}`)
  }

  return response.json()
}

/**
 * Generate 3 trading strategies based on a user query
 * This is the MCP-orchestrated workflow endpoint
 */
export async function generateStrategies(
  query: string,
  walletId: string
): Promise<GenerateStrategiesResponse> {
  console.debug('[frontend/api] generateStrategies request', {
    endpoint: `${API_BASE}/strategies/generate-mcp`,
    walletId,
    query,
  })
  const response = await fetch(`${API_BASE}/strategies/generate-mcp`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ user_query: query, wallet_id: walletId })
  })

  if (!response.ok) {
    const error = await response.json().catch(() => ({}))
    console.error('[frontend/api] generateStrategies failed', {
      status: response.status,
      statusText: response.statusText,
      error,
    })
    throw new Error(
      error.detail || `Failed to generate strategies: ${response.statusText}`
    )
  }

  const data = await response.json()
  console.debug('[frontend/api] generateStrategies response', {
    mode: data.mode,
    strategyCount: data.strategies?.length ?? 0,
    strategyIds: data.strategies?.map((strategy: Strategy) => strategy.id) ?? [],
  })
  return data
}

// ==================== Strategy Execution ====================

/**
 * Execute a selected strategy
 */
export async function executeStrategy(strategyId: string, walletId: string) {
  const response = await fetch(`${API_BASE}/strategy/execute`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ strategy_id: strategyId, wallet_id: walletId })
  })

  if (!response.ok) {
    throw new Error(`Failed to execute strategy: ${response.statusText}`)
  }

  return response.json()
}

/**
 * Poll for the status of a strategy execution
 */
export async function getStrategyStatus(txHash: string) {
  const response = await fetch(`${API_BASE}/strategy/status/${txHash}`)

  if (!response.ok) {
    throw new Error(`Failed to get strategy status: ${response.statusText}`)
  }

  return response.json()
}

// ==================== Health Check ====================

/**
 * Check if backend is healthy
 */
export async function checkBackendHealth() {
  try {
    const response = await fetch(`${API_BASE}/health`)
    return response.ok
  } catch {
    return false
  }
}
