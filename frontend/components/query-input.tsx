'use client'

import { useState } from 'react'
import { useWallet } from '@/lib/wallet-context'
import { Button } from '@/components/ui/button'
import { Textarea } from '@/components/ui/textarea'
import { Spinner } from '@/components/ui/spinner'
import { Send, Sparkles } from 'lucide-react'
import type { Strategy } from '@/lib/types'

const EXAMPLE_QUERIES = [
  'Analyze my portfolio risk',
  'Find arbitrage opportunities',
  'Suggest a hedging strategy',
  'Optimize my XRP holdings'
]

// Mock strategies for demo (fallback when backend is unavailable)
function generateMockStrategies(query: string): Strategy[] {
  return [
    {
      id: 'option_a',
      title: 'Conservative: Swap to Stablecoin',
      description: 'Reduce directional exposure by swapping half your XRP to USD. Preserves capital in volatile markets.',
      risk_score: 2,
      projected_return_7d: { expected: '$5', best_case: '$15', worst_case: '-$3' },
      trade_actions: [
        { action: 'swap', asset_in: 'XRP', asset_out: 'USD', amount: 50, estimated_slippage: 0.3, pool: 'XRP/USD' }
      ],
      pros: ['Low risk exposure', 'Capital preservation'],
      cons: ['Limited upside', 'Transaction fees apply']
    },
    {
      id: 'option_b',
      title: 'Yield: Two-Asset AMM Deposit',
      description: 'Deposit XRP and USD proportionally into the XRP/USD AMM pool to earn trading fees.',
      risk_score: 5,
      projected_return_7d: { expected: '$12', best_case: '$30', worst_case: '-$8' },
      trade_actions: [
        { action: 'deposit', asset_in: 'XRP', asset_out: 'USD', amount: 40, estimated_slippage: 0.0, pool: 'XRP/USD', deposit_mode: 'two_asset', amount2: 100 }
      ],
      pros: ['Earn trading fees', 'No deposit fee (proportional)'],
      cons: ['Impermanent loss risk', 'Capital locked in pool']
    },
    {
      id: 'option_c',
      title: 'Do Nothing: Hold Position',
      description: 'Keep current holdings unchanged. Monitor market conditions before committing capital.',
      risk_score: 2,
      projected_return_7d: { expected: '$0', best_case: '$0', worst_case: '$0' },
      trade_actions: [],
      pros: ['Zero transaction cost', 'No action needed'],
      cons: ['No yield earned', 'Exposed to XRP price movement']
    }
  ]
}

export function QueryInput() {
  const [query, setQuery] = useState('')
  const { status, setStatus, setStrategies, setLastQuery, lastQuery, wallet } = useWallet()

  const isQuerying = status === 'querying'
  const hasStrategies = status === 'strategies_loaded' || status === 'executing' || status === 'executed'

  const handleSubmit = async () => {
    if (!query.trim() || isQuerying || !wallet) return

    console.log(`\n${'='.repeat(60)}`)
    console.log(`🎯 [Frontend] Sending query to backend...`)
    console.log(`   Query: "${query}"`)
    console.log(`   Wallet: ${wallet.address}`)
    console.log(`${'='.repeat(60)}\n`)

    setStatus('querying')
    setLastQuery(query)

    try {
      // Call real backend API
      const apiUrl = process.env.NEXT_PUBLIC_API_URL || 'http://localhost:8000'
      console.log(`📡 [Frontend] POST ${apiUrl}/strategies/generate`)
      
      const response = await fetch(`${apiUrl}/strategies/generate`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ user_query: query, wallet_id: wallet.address })
      })

      console.log(`📊 [Frontend] Response status: ${response.status}`)

      if (!response.ok) {
        const error = await response.json().catch(() => ({}))
        throw new Error(error.detail || `Failed to generate strategies: ${response.statusText}`)
      }

      const data = await response.json()
      console.log(`✅ [Frontend] Received ${data.strategies.length} strategies`)
      console.log(data)
      
      setStrategies(data.strategies)
      setStatus('strategies_loaded')
      console.log(`\n✅ [Frontend] Displaying strategies\n`)
    } catch (err) {
      console.error('❌ [Frontend] Error generating strategies:', err)
      // Fallback to mock strategies if API fails
      const strategies = generateMockStrategies(query)
      setStrategies(strategies)
      setStatus('strategies_loaded')
    }
  }

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === 'Enter' && !e.shiftKey) {
      e.preventDefault()
      handleSubmit()
    }
  }

  return (
    <div className="space-y-4">
      <div className="relative">
        <Textarea
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          onKeyDown={handleKeyDown}
          placeholder="Ask anything... (e.g., 'Analyze my portfolio risk')"
          className="min-h-[120px] resize-none pr-14 text-base bg-input border-border"
          disabled={isQuerying}
        />
        <Button
          onClick={handleSubmit}
          disabled={!query.trim() || isQuerying}
          size="icon"
          className="absolute bottom-3 right-3"
        >
          {isQuerying ? (
            <Spinner className="h-4 w-4" />
          ) : (
            <Send className="h-4 w-4" />
          )}
        </Button>
      </div>

      {!hasStrategies && (
        <div className="space-y-2">
          <p className="text-xs text-muted-foreground flex items-center gap-1">
            <Sparkles className="h-3 w-3" />
            Try these examples:
          </p>
          <div className="flex flex-wrap gap-2">
            {EXAMPLE_QUERIES.map((example) => (
              <button
                key={example}
                onClick={() => setQuery(example)}
                className="text-xs px-3 py-1.5 rounded-full bg-secondary text-secondary-foreground hover:bg-accent transition-colors"
              >
                {example}
              </button>
            ))}
          </div>
        </div>
      )}

      {hasStrategies && lastQuery && (
        <div className="flex items-start gap-2 p-3 rounded-lg bg-muted/50">
          <Sparkles className="h-4 w-4 text-primary mt-0.5 shrink-0" />
          <div>
            <p className="text-xs text-muted-foreground">Your query:</p>
            <p className="text-sm text-foreground">{lastQuery}</p>
          </div>
        </div>
      )}
    </div>
  )
}
