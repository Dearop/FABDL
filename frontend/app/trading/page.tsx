'use client'

import { useEffect } from 'react'
import { useRouter } from 'next/navigation'
import { useWallet } from '@/lib/wallet-context'
import { Header } from '@/components/header'
import { QueryInput } from '@/components/query-input'
import { StrategyGrid } from '@/components/strategy-grid'
import { ExecutionStatus } from '@/components/execution-status'
import { Empty } from '@/components/ui/empty'
import { TrendingUp, Brain } from 'lucide-react'

export default function TradingPage() {
  const router = useRouter()
  const { wallet, status, strategies } = useWallet()

  // Redirect to home if not connected
  useEffect(() => {
    if (!wallet) {
      router.push('/')
    }
  }, [wallet, router])

  if (!wallet) {
    return null
  }

  const showStrategies = status === 'strategies_loaded' && strategies.length > 0
  const showExecution = status === 'executing' || status === 'executed'
  const showEmpty = status === 'ready' && strategies.length === 0

  return (
    <div className="min-h-screen bg-background">
      <Header />
      
      <main className="container mx-auto px-4 py-8">
        <div className="max-w-6xl mx-auto space-y-8">
          {/* Page header */}
          <div className="space-y-2">
            <h1 className="text-2xl font-bold text-foreground flex items-center gap-2">
              <Brain className="h-6 w-6 text-primary" />
              AI Trading Assistant
            </h1>
            <p className="text-muted-foreground">
              Describe your trading goals and get AI-powered strategy recommendations
            </p>
          </div>

          {/* Query input */}
          <div className="bg-card border border-border rounded-xl p-6">
            <QueryInput />
          </div>

          {/* Strategies or empty state */}
          {showExecution ? (
            <ExecutionStatus />
          ) : showStrategies ? (
            <StrategyGrid />
          ) : showEmpty ? (
            <div className="bg-card border border-border rounded-xl p-12">
              <Empty
                icon={<TrendingUp className="h-12 w-12 text-muted-foreground" />}
                title="No strategies yet"
                description="Enter a query above to generate AI-powered trading strategies"
              />
            </div>
          ) : null}

          {/* Footer */}
          <footer className="text-center pt-8 border-t border-border">
            <p className="text-xs text-muted-foreground">
              Powered by Local LLM + Claude Sonnet | 
              <a href="#" className="text-primary hover:underline ml-1">Documentation</a>
            </p>
          </footer>
        </div>
      </main>
    </div>
  )
}
