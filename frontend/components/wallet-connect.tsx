'use client'

import { useEffect } from 'react'
import { useRouter } from 'next/navigation'
import { useWallet } from '@/lib/wallet-context'
import { Button } from '@/components/ui/button'
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card'
import { Spinner } from '@/components/ui/spinner'
import { Wallet, Shield, Zap, ArrowRight, CheckCircle2, AlertCircle } from 'lucide-react'

export function WalletConnect() {
  const router = useRouter()
  const { wallet, status, error, connectWallet } = useWallet()

  const isConnecting = status === 'connecting'
  const isConnected = wallet !== null

  // Auto-redirect to trading dashboard after successful connection
  useEffect(() => {
    if (isConnected) {
      console.log('[v0] Wallet connected, redirecting to /trading...')
      const timer = setTimeout(() => {
        router.push('/trading')
      }, 1500) // Small delay to show success state
      return () => clearTimeout(timer)
    }
  }, [isConnected, router])

  return (
    <div className="min-h-screen flex flex-col items-center justify-center p-4 bg-background">
      {/* Background gradient effect */}
      <div className="absolute inset-0 overflow-hidden pointer-events-none">
        <div className="absolute top-1/4 left-1/2 -translate-x-1/2 w-[600px] h-[600px] bg-primary/20 rounded-full blur-[120px]" />
      </div>

      <div className="relative z-10 w-full max-w-md space-y-8">
        {/* Logo and title */}
        <div className="text-center space-y-4">
          <div className="inline-flex h-16 w-16 items-center justify-center rounded-2xl bg-primary">
            <Zap className="h-8 w-8 text-primary-foreground" />
          </div>
          <h1 className="text-3xl font-bold text-foreground text-balance">
            Connect Your XRPL Wallet
          </h1>
          <p className="text-muted-foreground text-balance">
            Connect your Otsu Wallet to access AI-powered trading strategies on the XRP Ledger
          </p>
        </div>

        {/* Connection card */}
        <Card className="border-border bg-card/50 backdrop-blur">
          <CardHeader className="text-center">
            <CardTitle className="text-lg">Otsu Wallet</CardTitle>
            <CardDescription>
              Secure connection to local XRPL testnet
            </CardDescription>
          </CardHeader>
          <CardContent className="space-y-6">
            {!isConnected ? (
              <>
                <Button 
                  onClick={connectWallet} 
                  disabled={isConnecting}
                  className="w-full h-12 text-base gap-2"
                  size="lg"
                >
                  {isConnecting ? (
                    <>
                      <Spinner className="h-5 w-5" />
                      Connecting...
                    </>
                  ) : (
                    <>
                      <Wallet className="h-5 w-5" />
                      Connect Otsu Wallet
                    </>
                  )}
                </Button>

                {error && (
                  <div className="flex items-center gap-2 p-3 rounded-lg bg-destructive/10 border border-destructive/20">
                    <AlertCircle className="h-5 w-5 text-destructive shrink-0" />
                    <p className="text-sm text-destructive">{error}</p>
                  </div>
                )}
              </>
            ) : (
              <div className="space-y-4">
                <div className="flex items-center gap-2 p-3 rounded-lg bg-success/10 border border-success/20">
                  <CheckCircle2 className="h-5 w-5 text-success shrink-0" />
                  <div className="flex-1">
                    <p className="text-sm text-success">Wallet connected successfully</p>
                    <p className="text-xs text-muted-foreground mt-1">Redirecting to Trading Assistant...</p>
                  </div>
                  <Spinner className="h-4 w-4 text-success" />
                </div>

                <div className="space-y-3 p-4 rounded-lg bg-muted/50">
                  <div className="flex items-center justify-between">
                    <span className="text-sm text-muted-foreground">Address</span>
                    <span className="text-sm font-mono text-foreground">
                      {wallet.address.slice(0, 8)}...{wallet.address.slice(-6)}
                    </span>
                  </div>
                  <div className="flex items-center justify-between">
                    <span className="text-sm text-muted-foreground">Network</span>
                    <span className="text-sm font-medium text-foreground">{wallet.network}</span>
                  </div>
                  <div className="flex items-center justify-between">
                    <span className="text-sm text-muted-foreground">Balance</span>
                    <span className="text-sm font-medium text-foreground">{wallet.balance}</span>
                  </div>
                </div>
              </div>
            )}
          </CardContent>
        </Card>

        {/* Features */}
        <div className="grid gap-4">
          <div className="flex items-start gap-3 p-4 rounded-lg bg-card/30 border border-border">
            <Shield className="h-5 w-5 text-primary mt-0.5 shrink-0" />
            <div>
              <p className="text-sm font-medium text-foreground">Secure & Private</p>
              <p className="text-xs text-muted-foreground">Your keys never leave your wallet</p>
            </div>
          </div>
          <div className="flex items-start gap-3 p-4 rounded-lg bg-card/30 border border-border">
            <Zap className="h-5 w-5 text-primary mt-0.5 shrink-0" />
            <div>
              <p className="text-sm font-medium text-foreground">AI-Powered Analysis</p>
              <p className="text-xs text-muted-foreground">Get intelligent trading strategies tailored to your portfolio</p>
            </div>
          </div>
        </div>

        

        {/* Footer */}
        <p className="text-center text-xs text-muted-foreground">
          Powered by Local LLM + Claude Sonnet
        </p>
      </div>
    </div>
  )
}
