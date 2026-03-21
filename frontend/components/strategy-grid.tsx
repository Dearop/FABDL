'use client'

import { useWallet } from '@/lib/wallet-context'
import { StrategyCard } from '@/components/strategy-card'
import { Spinner } from '@/components/ui/spinner'

export function StrategyGrid() {
  const { strategies, status } = useWallet()

  if (status === 'querying') {
    return (
      <div className="flex flex-col items-center justify-center py-16 gap-4">
        <Spinner className="h-8 w-8 text-primary" />
        <p className="text-muted-foreground">Analyzing your query and generating strategies...</p>
      </div>
    )
  }

  if (strategies.length === 0) {
    return null
  }

  return (
    <div className="space-y-4">
      <div className="flex items-center justify-between">
        <h2 className="text-xl font-semibold text-foreground">
          Recommended Strategies
        </h2>
        <p className="text-sm text-muted-foreground">
          {strategies.length} strategies found
        </p>
      </div>
      <div className="grid gap-6 md:grid-cols-2 lg:grid-cols-3">
        {strategies.map((strategy) => (
          <StrategyCard key={strategy.id} strategy={strategy} />
        ))}
      </div>
    </div>
  )
}
