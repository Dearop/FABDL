// XRPL AI Trading System Types

export interface TradeAction {
  action: 'swap' | 'deposit' | 'withdraw' | 'lend' | 'borrow'
  asset_in: string
  asset_out: string
  amount: number
  estimated_slippage: number
  // XRPL v2 AMM extensions
  amount2?: number | null        // second asset amount for two-asset deposits
  pool?: string | null           // e.g. "XRP/USD"
  deposit_mode?: 'single_asset' | 'two_asset' | null
  // XLS-66d lending extensions
  interest_rate?: number | null  // annualized %, for lend/borrow actions
  term_days?: number | null      // loan term in days
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

export interface ExecutionSummary {
  simulated: boolean
  summary_lines: string[]   // human-readable lines: "Swapped 50 XRP → USD via XRP/USD pool"
  il_estimate?: string      // e.g. "Estimated IL: -2.3%"
  fee_estimate?: string     // e.g. "Estimated Fee APR: 15.2%"
  net_cost?: string         // e.g. "Est. network fee: 0.000012 XRP"
}

export interface AppState {
  wallet: WalletInfo | null
  status: AppStatus
  lastQuery: string
  strategies: Strategy[]
  selectedStrategy: Strategy | null
  txHash: string | null
  executionSummary: ExecutionSummary | null
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
