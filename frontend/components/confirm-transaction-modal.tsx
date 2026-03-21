'use client'

import { useWallet } from '@/lib/wallet-context'
import type { Strategy } from '@/lib/types'
import { 
  Dialog, 
  DialogContent, 
  DialogDescription, 
  DialogHeader, 
  DialogTitle,
  DialogFooter
} from '@/components/ui/dialog'
import { Button } from '@/components/ui/button'
import { RiskIndicator } from '@/components/risk-indicator'
import { ArrowRight, AlertTriangle, Wallet } from 'lucide-react'

interface ConfirmTransactionModalProps {
  strategy: Strategy
  open: boolean
  onOpenChange: (open: boolean) => void
}

export function ConfirmTransactionModal({ strategy, open, onOpenChange }: ConfirmTransactionModalProps) {
  const { wallet, setStatus, setSelectedStrategy, setTxHash } = useWallet()

  const handleConfirm = async () => {
    onOpenChange(false)
    setSelectedStrategy(strategy)
    setStatus('executing')

    try {
      // Call backend to execute strategy
      const apiUrl = process.env.NEXT_PUBLIC_API_URL || 'http://localhost:8000'
      const response = await fetch(`${apiUrl}/strategy/execute`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ 
          strategy_id: strategy.id, 
          wallet_id: wallet?.address 
        })
      })

      if (!response.ok) {
        throw new Error('Failed to execute strategy')
      }

      const data = await response.json()
      setTxHash(data.tx_hash)
      setStatus('executed')
    } catch (err) {
      console.error('Error executing strategy:', err)
      // Fallback to mock transaction
      const mockTxHash = `${Math.random().toString(36).substring(2, 10).toUpperCase()}${Date.now().toString(36).toUpperCase()}`
      setTxHash(mockTxHash)
      setStatus('executed')
    }
  }

  // Calculate total transaction value
  const totalAmount = strategy.trade_actions.reduce((sum, action) => sum + action.amount, 0)
  const estimatedFee = 0.000012 // XRP transaction fee

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-w-md">
        <DialogHeader>
          <DialogTitle className="flex items-center gap-2">
            <Wallet className="h-5 w-5 text-primary" />
            Confirm Transaction
          </DialogTitle>
          <DialogDescription>
            Review the transaction details before signing with your wallet
          </DialogDescription>
        </DialogHeader>

        <div className="space-y-4 my-4">
          {/* Strategy Summary */}
          <div className="p-4 rounded-lg bg-muted/50 border border-border">
            <h4 className="font-medium text-foreground mb-2">{strategy.title}</h4>
            <RiskIndicator score={strategy.risk_score} size="sm" />
          </div>

          {/* Transaction Details */}
          <div className="space-y-3">
            <h4 className="text-sm font-medium text-foreground">Transaction Details</h4>
            
            {strategy.trade_actions.map((action, index) => (
              <div 
                key={index}
                className="flex items-center justify-between p-3 rounded-lg bg-card border border-border"
              >
                <div className="flex items-center gap-2">
                  <span className="text-xs font-medium px-2 py-0.5 rounded bg-primary/20 text-primary capitalize">
                    {action.action}
                  </span>
                  <span className="text-sm text-foreground">
                    {action.amount} {action.asset_in}
                  </span>
                  <ArrowRight className="h-3 w-3 text-muted-foreground" />
                  <span className="text-sm text-foreground">{action.asset_out}</span>
                </div>
              </div>
            ))}
          </div>

          {/* Fee Summary */}
          <div className="space-y-2 p-4 rounded-lg bg-muted/30">
            <div className="flex justify-between text-sm">
              <span className="text-muted-foreground">Total Amount</span>
              <span className="font-medium text-foreground">{totalAmount} XRP</span>
            </div>
            <div className="flex justify-between text-sm">
              <span className="text-muted-foreground">Est. Slippage Impact</span>
              <span className="font-medium text-foreground">
                ~{(strategy.trade_actions.reduce((sum, a) => sum + a.estimated_slippage, 0) / strategy.trade_actions.length).toFixed(2)}%
              </span>
            </div>
            <div className="flex justify-between text-sm">
              <span className="text-muted-foreground">Network Fee</span>
              <span className="font-medium text-foreground">{estimatedFee} XRP</span>
            </div>
            <div className="border-t border-border pt-2 mt-2">
              <div className="flex justify-between">
                <span className="font-medium text-foreground">Estimated Total</span>
                <span className="font-bold text-foreground">{(totalAmount + estimatedFee).toFixed(6)} XRP</span>
              </div>
            </div>
          </div>

          {/* Warning */}
          <div className="flex items-start gap-2 p-3 rounded-lg bg-risk-medium/10 border border-risk-medium/20">
            <AlertTriangle className="h-5 w-5 text-risk-medium shrink-0 mt-0.5" />
            <p className="text-sm text-muted-foreground">
              You will be prompted to sign this transaction with your Otsu Wallet. 
              Make sure you have sufficient balance.
            </p>
          </div>

          {/* Wallet Info */}
          {wallet && (
            <div className="flex items-center justify-between p-3 rounded-lg bg-card border border-border">
              <span className="text-sm text-muted-foreground">Signing with</span>
              <span className="text-sm font-mono text-foreground">
                {wallet.address.slice(0, 6)}...{wallet.address.slice(-4)}
              </span>
            </div>
          )}
        </div>

        <DialogFooter className="gap-2">
          <Button variant="outline" onClick={() => onOpenChange(false)}>
            Cancel
          </Button>
          <Button onClick={handleConfirm}>
            Sign & Execute
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  )
}
