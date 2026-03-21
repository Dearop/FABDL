// XRPL AI Trading System Types

export interface TradeAction {
  action: 'swap' | 'deposit' | 'withdraw'
  asset_in: string
  asset_out: string
  amount: number
  estimated_slippage: number
}

export interface Strategy {
  id: string
  title: string
  description: string
  risk_score: number // 1-10
  projected_return_7d: {
    best_case: string
    expected: string
    worst_case: string
  }
  trade_actions: TradeAction[]
  pros: string[]
  cons: string[]
}

export interface WalletInfo {
  address: string
  balance: string
  network: string
}

export type AppStatus = 
  | 'disconnected' 
  | 'connecting'
  | 'ready' 
  | 'querying' 
  | 'strategies_loaded' 
  | 'executing'
  | 'executed'

export interface AppState {
  wallet: WalletInfo | null
  status: AppStatus
  lastQuery: string
  strategies: Strategy[]
  selectedStrategy: Strategy | null
  txHash: string | null
  error: string | null
}

// Otsu Wallet types (based on common XRPL wallet patterns)
export interface OtsuWalletResponse {
  address: string
  publicKey: string
  network: string
}

export interface TransactionResult {
  hash: string
  status: 'success' | 'failed'
  message?: string
}
