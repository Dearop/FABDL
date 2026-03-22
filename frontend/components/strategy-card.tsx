'use client'

import { useState } from 'react'
import type { Strategy } from '@/lib/types'
import { Button } from '@/components/ui/button'
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card'
import { 
  Accordion,
  AccordionContent,
  AccordionItem,
  AccordionTrigger
} from '@/components/ui/accordion'
import { RiskIndicator } from '@/components/risk-indicator'
import { StrategyDetailsModal } from '@/components/strategy-details-modal'
import { ConfirmTransactionModal } from '@/components/confirm-transaction-modal'
import { Check, X, ArrowRight, Info, TrendingUp, TrendingDown, Target, Landmark, HandCoins } from 'lucide-react'

interface StrategyCardProps {
  strategy: Strategy
}

export function StrategyCard({ strategy }: StrategyCardProps) {
  const [showDetails, setShowDetails] = useState(false)
  const [showConfirm, setShowConfirm] = useState(false)

  return (
    <>
      <Card className="flex flex-col h-full bg-card border-border hover:border-primary/50 transition-colors">
        <CardHeader className="pb-4">
          <CardTitle className="text-lg leading-tight">{strategy.title}</CardTitle>
          <CardDescription className="line-clamp-2">{strategy.description}</CardDescription>
        </CardHeader>
        
        <CardContent className="flex-1 flex flex-col gap-4">
          {/* Risk Score */}
          <RiskIndicator score={strategy.risk_score} />

          {/* Projected Returns */}
          <div className="space-y-2">
            <p className="text-sm text-muted-foreground">Projected 7-day Return</p>
            <div className="grid grid-cols-3 gap-2 text-center">
              <div className="p-2 rounded-lg bg-muted/50">
                <TrendingDown className="h-4 w-4 text-risk-high mx-auto mb-1" />
                <p className="text-xs text-muted-foreground">Worst</p>
                <p className="text-sm font-medium text-risk-high">{strategy.projected_return_7d.worst_case}</p>
              </div>
              <div className="p-2 rounded-lg bg-primary/10 border border-primary/20">
                <Target className="h-4 w-4 text-primary mx-auto mb-1" />
                <p className="text-xs text-muted-foreground">Expected</p>
                <p className="text-sm font-medium text-primary">{strategy.projected_return_7d.expected}</p>
              </div>
              <div className="p-2 rounded-lg bg-muted/50">
                <TrendingUp className="h-4 w-4 text-risk-low mx-auto mb-1" />
                <p className="text-xs text-muted-foreground">Best</p>
                <p className="text-sm font-medium text-risk-low">{strategy.projected_return_7d.best_case}</p>
              </div>
            </div>
          </div>

          {/* Trade Actions Accordion */}
          <Accordion type="single" collapsible className="w-full">
            <AccordionItem value="actions" className="border-border">
              <AccordionTrigger className="text-sm py-2 hover:no-underline">
                Trade Actions ({strategy.trade_actions.length})
              </AccordionTrigger>
              <AccordionContent>
                <div className="space-y-2">
                  {strategy.trade_actions.map((action, index) => (
                    <div key={index} className="flex flex-col gap-1 text-sm p-2 rounded bg-muted/50">
                      <div className="flex items-center gap-2">
                        {(action.action === 'lend' || action.action === 'borrow') ? (
                          action.action === 'lend' ? <Landmark className="h-3.5 w-3.5 text-primary" /> : <HandCoins className="h-3.5 w-3.5 text-risk-medium" />
                        ) : null}
                        <span className="capitalize font-medium text-foreground">{action.action}</span>
                        <span className="text-muted-foreground">
                          {action.amount} {action.asset_in}
                        </span>
                        {action.action !== 'lend' && (
                          <>
                            <ArrowRight className="h-3 w-3 text-muted-foreground" />
                            <span className="text-muted-foreground">
                              {action.amount2 ? `${action.amount2} ` : ''}{action.asset_out}
                            </span>
                          </>
                        )}
                        {action.estimated_slippage > 0 && (
                          <span className="ml-auto text-xs text-muted-foreground">
                            ~{action.estimated_slippage}% slip
                          </span>
                        )}
                      </div>
                      {/* Extra context for AMM and lending actions */}
                      <div className="flex gap-2 ml-5">
                        {action.pool && (
                          <span className="text-xs px-1.5 py-0.5 rounded bg-primary/10 text-primary">
                            {action.pool}
                          </span>
                        )}
                        {action.deposit_mode && (
                          <span className="text-xs px-1.5 py-0.5 rounded bg-muted text-muted-foreground">
                            {action.deposit_mode.replace('_', '-')}
                          </span>
                        )}
                        {action.interest_rate != null && (
                          <span className="text-xs px-1.5 py-0.5 rounded bg-risk-low/10 text-risk-low">
                            {action.interest_rate}% APR
                          </span>
                        )}
                        {action.term_days != null && (
                          <span className="text-xs px-1.5 py-0.5 rounded bg-muted text-muted-foreground">
                            {action.term_days}d term
                          </span>
                        )}
                      </div>
                    </div>
                  ))}
                </div>
              </AccordionContent>
            </AccordionItem>
          </Accordion>

          {/* Pros and Cons */}
          <div className="grid grid-cols-2 gap-4">
            <div className="space-y-1">
              <p className="text-xs font-medium text-risk-low">Pros</p>
              {strategy.pros.slice(0, 2).map((pro, i) => (
                <div key={i} className="flex items-start gap-1.5 text-xs text-muted-foreground">
                  <Check className="h-3 w-3 text-risk-low mt-0.5 shrink-0" />
                  <span className="line-clamp-1">{pro}</span>
                </div>
              ))}
            </div>
            <div className="space-y-1">
              <p className="text-xs font-medium text-risk-high">Cons</p>
              {strategy.cons.slice(0, 2).map((con, i) => (
                <div key={i} className="flex items-start gap-1.5 text-xs text-muted-foreground">
                  <X className="h-3 w-3 text-risk-high mt-0.5 shrink-0" />
                  <span className="line-clamp-1">{con}</span>
                </div>
              ))}
            </div>
          </div>

          {/* Actions */}
          <div className="flex gap-2 mt-auto pt-4">
            <Button 
              variant="outline" 
              className="flex-1"
              onClick={() => setShowDetails(true)}
            >
              <Info className="h-4 w-4 mr-1" />
              Details
            </Button>
            <Button 
              className="flex-1"
              onClick={() => setShowConfirm(true)}
            >
              Select
            </Button>
          </div>
        </CardContent>
      </Card>

      <StrategyDetailsModal 
        strategy={strategy}
        open={showDetails}
        onOpenChange={setShowDetails}
      />

      <ConfirmTransactionModal
        strategy={strategy}
        open={showConfirm}
        onOpenChange={setShowConfirm}
      />
    </>
  )
}
