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

// Mock strategies for demo
function generateMockStrategies(query: string): Strategy[] {
  return [
    {
      id: '1',
      title: 'Conservative: Full Delta Hedge',
      description: 'Minimize exposure to market volatility by hedging your XRP position with stablecoin reserves. This strategy prioritizes capital preservation over growth.',
      risk_score: 2,
      projected_return_7d: {
        expected: '$80',
        best_case: '$150',
        worst_case: '-$20'
      },
      trade_actions: [
        { action: 'swap', asset_in: 'XRP', asset_out: 'USD', amount: 1500, estimated_slippage: 0.3 }
      ],
      pros: ['Low risk exposure', 'Stable returns', 'Capital preservation'],
      cons: ['Limited upside potential', 'Transaction fees apply']
    },
    {
      id: '2',
      title: 'Moderate: Balanced Reallocation',
      description: 'Diversify your portfolio by spreading assets across multiple trading pairs. Balances risk and reward for steady growth.',
      risk_score: 5,
      projected_return_7d: {
        expected: '$250',
        best_case: '$500',
        worst_case: '-$100'
      },
      trade_actions: [
        { action: 'swap', asset_in: 'XRP', asset_out: 'USD', amount: 800, estimated_slippage: 0.25 },
        { action: 'swap', asset_in: 'XRP', asset_out: 'ETH', amount: 500, estimated_slippage: 0.4 }
      ],
      pros: ['Diversified exposure', 'Moderate growth potential', 'Reduced single-asset risk'],
      cons: ['Moderate volatility', 'Multiple transaction fees']
    },
    {
      id: '3',
      title: 'Aggressive: Leveraged Growth',
      description: 'Maximize returns by concentrating positions in high-momentum assets. Suitable for experienced traders with higher risk tolerance.',
      risk_score: 8,
      projected_return_7d: {
        expected: '$600',
        best_case: '$1500',
        worst_case: '-$400'
      },
      trade_actions: [
        { action: 'swap', asset_in: 'USD', asset_out: 'XRP', amount: 2000, estimated_slippage: 0.5 },
        { action: 'deposit', asset_in: 'XRP', asset_out: 'XRP-LP', amount: 1000, estimated_slippage: 0.2 }
      ],
      pros: ['High growth potential', 'Market momentum capture', 'Compound returns'],
      cons: ['High volatility risk', 'Potential significant losses', 'Requires active monitoring']
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
