'use client'

import { useEffect, useState } from 'react'
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog'
import { Button } from '@/components/ui/button'
import { Badge } from '@/components/ui/badge'
import { cn } from '@/lib/utils'
import {
  CheckCircle2,
  ExternalLink,
  Copy,
  ArrowRight,
  Layers,
} from 'lucide-react'
import type { Strategy } from '@/lib/types'
import type { StrategyExecutionResult } from '@/services/xrplTransactions'

interface TransactionSuccessModalProps {
  open: boolean
  onOpenChange: (open: boolean) => void
  strategy: Strategy | null
  result: StrategyExecutionResult | null
}

function explorerUrl(hash: string, network: string): string {
  if (network === 'testnet') {
    return `https://testnet.xrpl.org/transactions/${hash}`
  }
  // lending devnet doesn't have a public explorer, fall back to testnet explorer
  return `https://testnet.xrpl.org/transactions/${hash}`
}

export function TransactionSuccessModal({
  open,
  onOpenChange,
  strategy,
  result,
}: TransactionSuccessModalProps) {
  const [copied, setCopied] = useState(false)
  const [showCheck, setShowCheck] = useState(false)

  // Animate the checkmark in
  useEffect(() => {
    if (open) {
      setShowCheck(false)
      const timer = setTimeout(() => setShowCheck(true), 100)
      return () => clearTimeout(timer)
    }
  }, [open])

  if (!strategy || !result) return null

  const handleCopy = async () => {
    await navigator.clipboard.writeText(result.txHash)
    setCopied(true)
    setTimeout(() => setCopied(false), 2000)
  }

  const networkLabel = result.network === 'testnet' ? 'XRPL Testnet' : 'XRPL Lending Devnet'

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-w-lg border-green-500/20 bg-gradient-to-b from-green-500/5 to-background">
        <DialogHeader className="items-center text-center pb-2">
          {/* Animated checkmark */}
          <div
            className={cn(
              'flex items-center justify-center w-16 h-16 rounded-full bg-green-500/10 border-2 border-green-500/30 mb-3 transition-all duration-500',
              showCheck
                ? 'scale-100 opacity-100'
                : 'scale-50 opacity-0',
            )}
          >
            <CheckCircle2
              className={cn(
                'h-8 w-8 text-green-400 transition-all duration-500 delay-200',
                showCheck ? 'scale-100 opacity-100' : 'scale-0 opacity-0',
              )}
            />
          </div>
          <DialogTitle className="text-xl font-semibold text-foreground">
            Transaction Successful
          </DialogTitle>
          <p className="text-sm text-muted-foreground mt-1">
            Your strategy has been executed on-chain
          </p>
        </DialogHeader>

        <div className="space-y-4 mt-2">
          {/* Strategy summary */}
          <div className="rounded-lg border border-border bg-card p-4">
            <div className="flex items-center justify-between mb-2">
              <h4 className="font-medium text-foreground text-sm">{strategy.title}</h4>
              <Badge variant="outline" className="text-[10px] border-green-500/30 text-green-400">
                Confirmed
              </Badge>
            </div>
            <p className="text-xs text-muted-foreground line-clamp-2">{strategy.description}</p>
          </div>

          {/* Transaction actions breakdown */}
          {result.results.length > 0 && (
            <div className="space-y-2">
              <div className="flex items-center gap-1.5 text-xs font-medium text-muted-foreground">
                <Layers className="h-3.5 w-3.5" />
                <span>Executed Actions ({result.results.length})</span>
              </div>
              <div className="space-y-1.5">
                {strategy.trade_actions.map((action, i) => {
                  const txResult = result.results[i]
                  return (
                    <div
                      key={i}
                      className="flex items-center justify-between rounded-lg border border-border bg-muted/30 px-3 py-2"
                    >
                      <div className="flex items-center gap-2 text-sm">
                        <CheckCircle2 className="h-3.5 w-3.5 text-green-400 shrink-0" />
                        <span className="capitalize font-medium text-foreground">
                          {action.action}
                        </span>
                        <span className="text-muted-foreground">
                          {action.amount} {action.asset_in}
                        </span>
                        {action.action !== 'lend' && (
                          <>
                            <ArrowRight className="h-3 w-3 text-muted-foreground shrink-0" />
                            <span className="text-muted-foreground">{action.asset_out}</span>
                          </>
                        )}
                        {action.pool && (
                          <Badge variant="outline" className="text-[9px] ml-1">
                            {action.pool}
                          </Badge>
                        )}
                      </div>
                      {txResult && (
                        <a
                          href={explorerUrl(txResult.hash, result.network)}
                          target="_blank"
                          rel="noopener noreferrer"
                          className="text-primary hover:text-primary/80 transition-colors"
                          title="View on explorer"
                        >
                          <ExternalLink className="h-3.5 w-3.5" />
                        </a>
                      )}
                    </div>
                  )
                })}
              </div>
            </div>
          )}

          {/* Transaction hash */}
          <div className="rounded-lg border border-border bg-muted/30 p-3">
            <div className="flex items-center justify-between mb-1.5">
              <span className="text-xs font-medium text-muted-foreground">Transaction Hash</span>
              <Badge variant="outline" className="text-[9px]">
                {networkLabel}
              </Badge>
            </div>
            <div className="flex items-center gap-2">
              <code className="flex-1 text-xs font-mono text-foreground bg-background/50 rounded px-2 py-1.5 truncate border border-border">
                {result.txHash}
              </code>
              <Button
                variant="outline"
                size="icon"
                className="h-7 w-7 shrink-0"
                onClick={handleCopy}
                title="Copy hash"
              >
                {copied ? (
                  <CheckCircle2 className="h-3.5 w-3.5 text-green-400" />
                ) : (
                  <Copy className="h-3.5 w-3.5" />
                )}
              </Button>
            </div>
          </div>

          {/* Actions */}
          <div className="flex gap-2 pt-1">
            <Button
              variant="outline"
              className="flex-1"
              onClick={() => onOpenChange(false)}
            >
              Close
            </Button>
            <Button
              className="flex-1"
              onClick={() =>
                window.open(explorerUrl(result.txHash, result.network), '_blank')
              }
            >
              <ExternalLink className="h-4 w-4 mr-1.5" />
              View on Explorer
            </Button>
          </div>
        </div>
      </DialogContent>
    </Dialog>
  )
}
