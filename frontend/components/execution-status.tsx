'use client'

import { useWallet } from '@/lib/wallet-context'
import { Button } from '@/components/ui/button'
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card'
import { Spinner } from '@/components/ui/spinner'
import { CheckCircle2, ExternalLink, RotateCcw, Wallet } from 'lucide-react'

export function ExecutionStatus() {
  const { status, selectedStrategy, txHash, resetToReady } = useWallet()

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

            <div className="space-y-2">
              <p className="text-sm text-muted-foreground">Transaction Hash</p>
              <div className="flex items-center gap-2">
                <code className="flex-1 p-3 rounded-lg bg-muted font-mono text-sm text-foreground break-all">
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

            <div className="p-4 rounded-lg bg-muted/30 border border-border">
              <p className="text-sm text-muted-foreground">
                Monitoring on-chain... Your strategy is now active and will be tracked automatically.
              </p>
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
