'use client'

import type { Strategy } from '@/lib/types'
import { 
  Dialog, 
  DialogContent, 
  DialogDescription, 
  DialogHeader, 
  DialogTitle 
} from '@/components/ui/dialog'
import { RiskIndicator } from '@/components/risk-indicator'
import { Check, X, ArrowRight, TrendingUp, TrendingDown, Target, AlertTriangle } from 'lucide-react'

interface StrategyDetailsModalProps {
  strategy: Strategy
  open: boolean
  onOpenChange: (open: boolean) => void
}

export function StrategyDetailsModal({ strategy, open, onOpenChange }: StrategyDetailsModalProps) {
  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-w-2xl max-h-[90vh] overflow-y-auto">
        <DialogHeader>
          <DialogTitle className="text-xl">{strategy.title}</DialogTitle>
          <DialogDescription>{strategy.description}</DialogDescription>
        </DialogHeader>

        <div className="space-y-6 mt-4">
          {/* Risk Analysis */}
          <div className="space-y-3">
            <h3 className="font-semibold text-foreground flex items-center gap-2">
              <AlertTriangle className="h-4 w-4 text-risk-medium" />
              Risk Analysis
            </h3>
            <RiskIndicator score={strategy.risk_score} size="lg" />
            <p className="text-sm text-muted-foreground">
              {strategy.risk_score <= 3 
                ? 'This strategy prioritizes capital preservation with minimal exposure to market volatility.'
                : strategy.risk_score <= 6
                ? 'This strategy balances risk and reward, suitable for moderate risk tolerance.'
                : 'This strategy involves significant market exposure and is suitable for experienced traders.'}
            </p>
          </div>

          {/* Projected Returns */}
          <div className="space-y-3">
            <h3 className="font-semibold text-foreground">Projected 7-day Returns</h3>
            <div className="grid grid-cols-3 gap-4">
              <div className="p-4 rounded-lg bg-muted/50 text-center">
                <TrendingDown className="h-6 w-6 text-risk-high mx-auto mb-2" />
                <p className="text-sm text-muted-foreground">Worst Case</p>
                <p className="text-xl font-bold text-risk-high">{strategy.projected_return_7d.worst_case}</p>
              </div>
              <div className="p-4 rounded-lg bg-primary/10 border border-primary/20 text-center">
                <Target className="h-6 w-6 text-primary mx-auto mb-2" />
                <p className="text-sm text-muted-foreground">Expected</p>
                <p className="text-xl font-bold text-primary">{strategy.projected_return_7d.expected}</p>
              </div>
              <div className="p-4 rounded-lg bg-muted/50 text-center">
                <TrendingUp className="h-6 w-6 text-risk-low mx-auto mb-2" />
                <p className="text-sm text-muted-foreground">Best Case</p>
                <p className="text-xl font-bold text-risk-low">{strategy.projected_return_7d.best_case}</p>
              </div>
            </div>
          </div>

          {/* Trade Actions */}
          <div className="space-y-3">
            <h3 className="font-semibold text-foreground">Trade Actions</h3>
            <div className="space-y-2">
              {strategy.trade_actions.map((action, index) => (
                <div 
                  key={index} 
                  className="flex items-center gap-3 p-3 rounded-lg bg-muted/50 border border-border"
                >
                  <span className="px-2 py-1 text-xs font-medium rounded bg-primary/20 text-primary capitalize">
                    {action.action}
                  </span>
                  <div className="flex items-center gap-2 flex-1">
                    <span className="font-medium text-foreground">
                      {action.amount} {action.asset_in}
                    </span>
                    <ArrowRight className="h-4 w-4 text-muted-foreground" />
                    <span className="font-medium text-foreground">{action.asset_out}</span>
                  </div>
                  <span className="text-sm text-muted-foreground">
                    Est. slippage: {action.estimated_slippage}%
                  </span>
                </div>
              ))}
            </div>
          </div>

          {/* Pros and Cons */}
          <div className="grid md:grid-cols-2 gap-6">
            <div className="space-y-3">
              <h3 className="font-semibold text-risk-low flex items-center gap-2">
                <Check className="h-4 w-4" />
                Advantages
              </h3>
              <ul className="space-y-2">
                {strategy.pros.map((pro, i) => (
                  <li key={i} className="flex items-start gap-2 text-sm text-muted-foreground">
                    <Check className="h-4 w-4 text-risk-low mt-0.5 shrink-0" />
                    {pro}
                  </li>
                ))}
              </ul>
            </div>
            <div className="space-y-3">
              <h3 className="font-semibold text-risk-high flex items-center gap-2">
                <X className="h-4 w-4" />
                Considerations
              </h3>
              <ul className="space-y-2">
                {strategy.cons.map((con, i) => (
                  <li key={i} className="flex items-start gap-2 text-sm text-muted-foreground">
                    <X className="h-4 w-4 text-risk-high mt-0.5 shrink-0" />
                    {con}
                  </li>
                ))}
              </ul>
            </div>
          </div>
        </div>
      </DialogContent>
    </Dialog>
  )
}
