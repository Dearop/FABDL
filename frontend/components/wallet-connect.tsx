'use client'

import { useEffect, useState, useCallback } from 'react'
import { useRouter } from 'next/navigation'
import { useWallet } from '@/hooks/useWallet'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card'
import { Spinner } from '@/components/ui/spinner'
import { Alert, AlertDescription } from '@/components/ui/alert'
import { Wallet, Shield, Zap, CheckCircle2, AlertTriangle, Eye, EyeOff } from 'lucide-react'

export function WalletConnect() {
  const router = useRouter()
  const wallet = useWallet()

  const [mode, setMode] = useState<'main' | 'key'>('main')
  const [secret, setSecret] = useState('')
  const [showSecret, setShowSecret] = useState(false)
  const [error, setError] = useState<string | null>(null)

  // Redirect to trading once connected
  useEffect(() => {
    if (wallet.address) {
      const timer = setTimeout(() => router.push('/trading'), 1200)
      return () => clearTimeout(timer)
    }
  }, [wallet.address, router])

  const handleConnect = useCallback(async () => {
    setError(null)
    try {
      await wallet.connect()
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Could not connect wallet')
    }
  }, [wallet])

  const handleKeyConnect = useCallback(async () => {
    if (!secret.trim()) return
    setError(null)
    try {
      await wallet.connectWithKey(secret.trim())
      setSecret('')
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Invalid secret key')
    }
  }, [wallet, secret])

  return (
    <div className="min-h-screen flex flex-col items-center justify-center p-4 bg-background">
      {/* Background gradient */}
      <div className="absolute inset-0 overflow-hidden pointer-events-none">
        <div className="absolute top-1/4 left-1/2 -translate-x-1/2 w-[600px] h-[600px] bg-primary/20 rounded-full blur-[120px]" />
      </div>

      <div className="relative z-10 w-full max-w-md space-y-8">
        {/* Header */}
        <div className="text-center space-y-4">
          <div className="inline-flex h-16 w-16 items-center justify-center rounded-2xl bg-primary">
            <Zap className="h-8 w-8 text-primary-foreground" />
          </div>
          <h1 className="text-3xl font-bold text-foreground text-balance">
            Connect Your XRPL Wallet
          </h1>
          <p className="text-muted-foreground text-balance">
            Connect your wallet to access AI-powered trading strategies on the XRP Ledger
          </p>
        </div>

        {/* Connection card */}
        <Card className="border-border bg-card/50 backdrop-blur">
          {wallet.address ? (
            /* ── Connected state ── */
            <>
              <CardHeader className="text-center">
                <CardTitle className="text-lg">Wallet Connected</CardTitle>
              </CardHeader>
              <CardContent className="space-y-4">
                <div className="flex items-center gap-2 p-3 rounded-lg bg-green-500/10 border border-green-500/20">
                  <CheckCircle2 className="h-5 w-5 text-green-400 shrink-0" />
                  <div className="flex-1">
                    <p className="text-sm text-green-400">Connected successfully</p>
                    <p className="text-xs text-muted-foreground mt-1">Redirecting to Trading Assistant...</p>
                  </div>
                  <Spinner className="h-4 w-4 text-green-400" />
                </div>
                <div className="space-y-2 p-4 rounded-lg bg-muted/50 text-sm">
                  <div className="flex justify-between">
                    <span className="text-muted-foreground">Address</span>
                    <span className="font-mono">{wallet.address.slice(0, 8)}...{wallet.address.slice(-6)}</span>
                  </div>
                  <div className="flex justify-between">
                    <span className="text-muted-foreground">Provider</span>
                    <span className="font-medium capitalize">{wallet.providerType ?? '—'}</span>
                  </div>
                </div>
              </CardContent>
            </>
          ) : mode === 'main' ? (
            /* ── Main options ── */
            <>
              <CardHeader className="text-center">
                <CardTitle className="text-lg">Connect Wallet</CardTitle>
                <CardDescription>Use a browser extension or enter a secret key</CardDescription>
              </CardHeader>
              <CardContent className="space-y-3">
                <Button
                  onClick={handleConnect}
                  disabled={wallet.isConnecting}
                  className="w-full h-12 text-base gap-2"
                  size="lg"
                >
                  {wallet.isConnecting ? (
                    <><Spinner className="h-5 w-5" /> Connecting...</>
                  ) : (
                    <><Wallet className="h-5 w-5" /> Connect Wallet</>
                  )}
                </Button>

                <div className="relative flex items-center gap-2 py-1">
                  <div className="flex-1 border-t border-border" />
                  <span className="text-xs text-muted-foreground">or</span>
                  <div className="flex-1 border-t border-border" />
                </div>

                <Button
                  variant="outline"
                  className="w-full gap-2"
                  onClick={() => { setError(null); setMode('key') }}
                  disabled={wallet.isConnecting}
                >
                  Enter Secret Key
                </Button>

                {error && (
                  <Alert className="border-destructive/30 bg-destructive/10">
                    <AlertTriangle className="h-4 w-4 text-destructive" />
                    <AlertDescription className="text-destructive text-sm">{error}</AlertDescription>
                  </Alert>
                )}
              </CardContent>
            </>
          ) : (
            /* ── Key entry ── */
            <>
              <CardHeader className="text-center">
                <CardTitle className="text-lg">Enter Secret Key</CardTitle>
                <CardDescription>Testnet / devnet only</CardDescription>
              </CardHeader>
              <CardContent className="space-y-4">
                <Alert className="border-amber-500/30 bg-amber-500/10">
                  <AlertTriangle className="h-4 w-4 text-amber-400" />
                  <AlertDescription className="text-amber-400 text-xs">
                    Never enter a mainnet secret key here.
                  </AlertDescription>
                </Alert>

                <div className="space-y-2">
                  <Label htmlFor="secret">Secret Key</Label>
                  <div className="relative">
                    <Input
                      id="secret"
                      type={showSecret ? 'text' : 'password'}
                      value={secret}
                      onChange={e => { setSecret(e.target.value); setError(null) }}
                      onKeyDown={e => e.key === 'Enter' && handleKeyConnect()}
                      placeholder="sEdV... or seed..."
                      className="pr-10 font-mono text-sm"
                      disabled={wallet.isConnecting}
                    />
                    <button
                      type="button"
                      onClick={() => setShowSecret(v => !v)}
                      className="absolute right-2 top-1/2 -translate-y-1/2 text-muted-foreground hover:text-foreground"
                    >
                      {showSecret ? <EyeOff className="h-4 w-4" /> : <Eye className="h-4 w-4" />}
                    </button>
                  </div>
                </div>

                {error && (
                  <p className="text-xs text-destructive">{error}</p>
                )}

                <div className="flex gap-2">
                  <Button variant="outline" className="flex-1" onClick={() => { setMode('main'); setError(null) }}>
                    Back
                  </Button>
                  <Button
                    className="flex-1"
                    onClick={handleKeyConnect}
                    disabled={!secret.trim() || wallet.isConnecting}
                  >
                    {wallet.isConnecting ? 'Connecting...' : 'Connect'}
                  </Button>
                </div>
              </CardContent>
            </>
          )}
        </Card>

        {/* Feature hints */}
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
              <p className="text-xs text-muted-foreground">Intelligent trading strategies tailored to your portfolio</p>
            </div>
          </div>
        </div>

        <p className="text-center text-xs text-muted-foreground">
          Powered by Local LLM + Claude Sonnet
        </p>
      </div>
    </div>
  )
}
