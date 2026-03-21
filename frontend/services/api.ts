/**
 * API Service for XRPL AI Trading Backend
 * Handles all HTTP calls to the backend API
 */

const API_BASE = process.env.NEXT_PUBLIC_API_URL || 'http://localhost:8000'

// Types
export interface IntentResponse {
  action: string
  scope: string
  confidence: number
  is_valid: boolean
  parameters: Array<{ key: string; value: string }>
}

export interface TradeAction {
  action: 'swap' | 'deposit' | 'withdraw'
  asset_in?: string
  asset_out?: string
  amount?: number
  pool?: string
  estimated_slippage?: number
  [key: string]: any
}

export interface Strategy {
  id: string
  title: string
  description: string
  risk_score: number
  projected_return_7d: {
    best_case: string
    expected: string
    worst_case: string
  }
  trade_actions: TradeAction[]
  pros: string[]
  cons: string[]
}

export interface GenerateStrategiesResponse {
  intent: IntentResponse
  strategies: Strategy[]
  wallet_id: string
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
 * This is the main workflow endpoint
 */
export async function generateStrategies(
  query: string,
  walletId: string
): Promise<GenerateStrategiesResponse> {
  const response = await fetch(`${API_BASE}/strategies/generate`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ user_query: query, wallet_id: walletId })
  })

  if (!response.ok) {
    const error = await response.json().catch(() => ({}))
    throw new Error(
      error.detail || `Failed to generate strategies: ${response.statusText}`
    )
  }

  return response.json()
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
