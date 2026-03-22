'use client'

import { useWallet } from '@/lib/wallet-context'
import { Button } from '@/components/ui/button'
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card'
import { Spinner } from '@/components/ui/spinner'
import { CheckCircle2, ExternalLink, RotateCcw, Wallet, FlaskConical } from 'lucide-react'

export function ExecutionStatus() {
  const { status, selectedStrategy, txHash, executionSummary, resetToReady } = useWallet()

  const isExecuting = status === 'executing'
  const isExecuted = status === 'executed'

  if (!selectedStrategy) return null

  return (
    <Card className="max-w-xl mx-auto bg-card border-border">
      <CardHeader className="text-center">
        <CardTitle className="flex items-center justify-center gap-2">
          {isExecuting ? (
            <>
              <Spinner className="h-5 w-5 text-primary" />
              Executing Strategy
            </>
          ) : (
            <>
              <CheckCircle2 className="h-5 w-5 text-risk-low" />
              Strategy Executed
            </>
          )}
        </CardTitle>
        <CardDescription>
          {isExecuting
            ? 'Signing transaction with Otsu Wallet...'
            : 'Your transaction has been confirmed on the XRPL'
          }
        </CardDescription>
      </CardHeader>

      <CardContent className="space-y-6">
        {/* Simulated Transaction Banner */}
        {executionSummary?.simulated && isExecuted && (
          <div className="flex items-start gap-3 p-4 rounded-lg bg-risk-medium/10 border border-risk-medium/30">
            <FlaskConical className="h-5 w-5 text-risk-medium shrink-0 mt-0.5" />
            <div>
              <p className="text-sm font-medium text-foreground">Simulated Transaction</p>
              <p className="text-xs text-muted-foreground">
                This is a simulation of the trades that would be executed on the XRPL lending devnet.
              </p>
            </div>
          </div>
        )}

        {/* Strategy Info */}
        <div className="p-4 rounded-lg bg-muted/50 border border-border">
          <h4 className="font-medium text-foreground">{selectedStrategy.title}</h4>
          <p className="text-sm text-muted-foreground mt-1">
            Expected return: {selectedStrategy.projected_return_7d.expected}
          </p>
        </div>

        {/* Execution Status */}
        {isExecuting && (
          <div className="space-y-4">
            <div className="flex items-center gap-3 p-3 rounded-lg bg-primary/10 border border-primary/20">
              <Wallet className="h-5 w-5 text-primary animate-pulse" />
              <div>
                <p className="text-sm font-medium text-foreground">Waiting for wallet signature</p>
                <p className="text-xs text-muted-foreground">Please approve the transaction in your wallet</p>
              </div>
            </div>
          </div>
        )}

        {/* Success State */}
        {isExecuted && txHash && (
          <div className="space-y-4">
            <div className="flex items-center gap-2 p-3 rounded-lg bg-risk-low/10 border border-risk-low/20">
              <CheckCircle2 className="h-5 w-5 text-risk-low shrink-0" />
              <p className="text-sm text-risk-low">Transaction confirmed successfully!</p>
            </div>

            {/* Execution Summary */}
            {executionSummary && (
              <div className="space-y-3 p-4 rounded-lg bg-card border border-border">
                <h4 className="text-sm font-medium text-foreground">Execution Summary</h4>
                <div className="space-y-1.5">
                  {executionSummary.summary_lines.map((line, i) => (
                    <p key={i} className="text-sm text-muted-foreground flex items-start gap-2">
                      <span className="text-primary mt-0.5">•</span>
                      {line}
                    </p>
                  ))}
                </div>
                {(executionSummary.il_estimate || executionSummary.fee_estimate || executionSummary.net_cost) && (
                  <div className="border-t border-border pt-3 mt-3 space-y-1">
                    {executionSummary.il_estimate && (
                      <p className="text-xs text-risk-medium">{executionSummary.il_estimate}</p>
                    )}
                    {executionSummary.fee_estimate && (
                      <p className="text-xs text-risk-low">{executionSummary.fee_estimate}</p>
                    )}
                    {executionSummary.net_cost && (
                      <p className="text-xs text-muted-foreground">{executionSummary.net_cost}</p>
                    )}
                  </div>
                )}
              </div>
            )}

            {/* Transaction Hash */}
            <div className="space-y-2">
              <p className="text-sm text-muted-foreground">Transaction Hash</p>
              <div className="flex items-center gap-2">
                <code className="flex-1 p-3 rounded-lg bg-muted font-mono text-xs text-foreground break-all">
                  {txHash}
                </code>
                <Button
                  variant="outline"
                  size="icon"
                  asChild
                >
                  <a
                    href={`https://testnet.xrpl.org/transactions/${txHash}`}
                    target="_blank"
                    rel="noopener noreferrer"
                  >
                    <ExternalLink className="h-4 w-4" />
                  </a>
                </Button>
              </div>
            </div>

            <Button onClick={resetToReady} className="w-full gap-2">
              <RotateCcw className="h-4 w-4" />
              New Query
            </Button>
          </div>
        )}
      </CardContent>
    </Card>
  )
}
