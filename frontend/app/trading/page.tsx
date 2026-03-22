'use client'

import { useState, useRef, useEffect, useMemo, useCallback } from 'react'
import { generateStrategies } from '@/services/api'
import { buildAndSubmitStrategy } from '@/services/xrplTransactions'
import type { Strategy } from '@/lib/types'
import { PoolRegistryProvider, usePoolRegistry } from '@/contexts/PoolRegistryContext'
import { Button } from '@/components/ui/button'
import { Card, CardHeader, CardContent, CardTitle, CardDescription } from '@/components/ui/card'
import { Badge } from '@/components/ui/badge'
import { Input } from '@/components/ui/input'
import { Spinner } from '@/components/ui/spinner'
import { ScrollArea } from '@/components/ui/scroll-area'
import { Toaster } from '@/components/ui/toaster'
import { useToast } from '@/hooks/use-toast'
import { useWallet } from '@/hooks/useWallet'
import { KeyEntryModal } from '@/components/KeyEntryModal'
import { cn } from '@/lib/utils'
import { Send, Wallet, ArrowRight, Check, X, Bot, KeyRound } from 'lucide-react'

// --------------- Types ---------------

interface Message {
  id: string
  role: 'user' | 'assistant' | 'error'
  content: string
  strategies?: Strategy[]
}

// --------------- Helpers ---------------

let msgCount = 0
function nextId() {
  msgCount = (msgCount + 1) % Number.MAX_SAFE_INTEGER
  return `msg-${msgCount}-${Date.now()}`
}

function riskBadge(score: number) {
  if (score <= 3) return { label: 'Low', cls: 'bg-green-500/15 text-green-400 border-green-500/30' }
  if (score <= 6) return { label: 'Medium', cls: 'bg-amber-500/15 text-amber-400 border-amber-500/30' }
  return { label: 'High', cls: 'bg-red-500/15 text-red-400 border-red-500/30' }
}

function truncateAddress(address: string) {
  return `r...${address.slice(-4)}`
}

const PROVIDER_LABELS: Record<string, string> = {
  'key-entry': 'devnet',
  otsu: 'otsu',
  crossmark: 'identity only',
}

// --------------- Page Shell (provides pool registry) ---------------

export default function TradingPage() {
  const wallet = useWallet()
  return (
    <PoolRegistryProvider walletAddress={wallet.address}>
      <TradingPageInner wallet={wallet} />
    </PoolRegistryProvider>
  )
}

// --------------- Inner Page (consumes pool registry) ---------------

function TradingPageInner({ wallet }: { wallet: ReturnType<typeof useWallet> }) {
  const walletId = wallet.address ?? ''
  const { pools, isLoading: poolsLoading, error: poolsError } = usePoolRegistry()

  const [messages, setMessages] = useState<Message[]>([])
  const [inputValue, setInputValue] = useState('')
  const [isLoading, setIsLoading] = useState(false)
  const [isExecuting, setIsExecuting] = useState<string | null>(null)
  const [keyEntryOpen, setKeyEntryOpen] = useState(false)

  const messagesEndRef = useRef<HTMLDivElement>(null)
  const inputRef = useRef<HTMLInputElement>(null)
  const { toast } = useToast()

  const canSign =
    wallet.address !== null &&
    wallet.providerType !== 'crossmark' &&
    !poolsLoading

  // Derived: latest strategies from most recent assistant message
  const latestStrategies = useMemo(() => {
    for (let i = messages.length - 1; i >= 0; i--) {
      if (messages[i].role === 'assistant' && messages[i].strategies?.length) {
        return messages[i].strategies!
      }
    }
    return []
  }, [messages])

  // Auto-scroll chat on new messages
  useEffect(() => {
    messagesEndRef.current?.scrollIntoView({ behavior: 'smooth' })
  }, [messages, isLoading])

  // Connect handler (auto-detect extension)
  const handleConnect = useCallback(async () => {
    try {
      await wallet.connect()
    } catch (err) {
      toast({
        title: 'Wallet connection failed',
        description: err instanceof Error ? err.message : 'Could not connect wallet',
        variant: 'destructive',
      })
    }
  }, [wallet, toast])

  // Send handler
  const handleSend = useCallback(async () => {
    const query = inputValue.trim()
    if (!query || isLoading || !wallet.address) return

    setInputValue('')
    setMessages(prev => [...prev, { id: nextId(), role: 'user', content: query }])
    setIsLoading(true)

    try {
      const response = await generateStrategies(query, walletId)
      const count = response.strategies?.length ?? 0
      setMessages(prev => [
        ...prev,
        {
          id: nextId(),
          role: 'assistant',
          content: count > 0
            ? `Here are ${count} strategies for your query.`
            : 'No strategies were generated for this query.',
          strategies: response.strategies,
        },
      ])
    } catch (err) {
      setMessages(prev => [
        ...prev,
        {
          id: nextId(),
          role: 'error',
          content: err instanceof Error ? err.message : 'Something went wrong. Please try again.',
        },
      ])
    } finally {
      setIsLoading(false)
      inputRef.current?.focus()
    }
  }, [inputValue, isLoading, walletId, wallet.address])

  // Execute handler — builds real XRPL transactions and submits
  const handleExecute = useCallback(async (strategyId: string) => {
    if (isExecuting) return

    if (wallet.providerType === 'crossmark') {
      toast({
        title: 'Cannot Execute',
        description: 'Crossmark is identity-only and cannot sign devnet transactions. Reconnect with Key Entry or Otsu.',
        variant: 'destructive',
      })
      return
    }

    const strategy = latestStrategies.find(s => s.id === strategyId)
    if (!strategy || !wallet.address) return

    setIsExecuting(strategyId)
    try {
      const result = await buildAndSubmitStrategy(
        strategy,
        wallet.address,
        wallet.signAndSubmit,
        pools,
      )
      toast({
        title: 'Strategy Executed',
        description: `Transaction hash: ${result.txHash.slice(0, 12)}...`,
      })
    } catch (err) {
      toast({
        title: 'Execution Failed',
        description: err instanceof Error ? err.message : 'Unknown error',
        variant: 'destructive',
      })
    } finally {
      setIsExecuting(null)
    }
  }, [isExecuting, wallet, latestStrategies, toast, pools])

  // Keyboard handler
  const handleKeyDown = useCallback(
    (e: React.KeyboardEvent) => {
      if (e.key === 'Enter' && !e.shiftKey) {
        e.preventDefault()
        handleSend()
      }
    },
    [handleSend],
  )

  const sendDisabled = !inputValue.trim() || isLoading || !wallet.address

  // ====================== JSX ======================

  return (
    <div className="flex flex-col h-screen bg-background">
      {/* ---- Top Bar ---- */}
      <header className="flex items-center justify-between border-b border-border px-4 py-3 shrink-0">
        <h1 className="text-lg font-semibold text-foreground">AI Trading Assistant</h1>
        <div className="flex items-center gap-2">
          {wallet.address ? (
            <>
              <span className="text-xs font-mono bg-muted px-3 py-1.5 rounded-md border border-border">
                {truncateAddress(wallet.address)}
              </span>
              {wallet.providerType && (
                <Badge
                  variant="outline"
                  className={cn(
                    'text-[10px]',
                    wallet.providerType === 'crossmark'
                      ? 'border-amber-500/30 text-amber-400'
                      : 'border-border text-muted-foreground',
                  )}
                >
                  {PROVIDER_LABELS[wallet.providerType]}
                </Badge>
              )}
              <Button variant="outline" size="sm" onClick={wallet.disconnect}>
                <Wallet className="h-4 w-4 mr-1" />
                Disconnect
              </Button>
            </>
          ) : (
            <>
              <Button
                variant="outline"
                size="sm"
                onClick={handleConnect}
                disabled={wallet.isConnecting}
              >
                {wallet.isConnecting ? (
                  <Spinner className="h-4 w-4 mr-1" />
                ) : (
                  <Wallet className="h-4 w-4 mr-1" />
                )}
                {wallet.isConnecting ? 'Connecting...' : 'Connect Wallet'}
              </Button>
              <Button
                variant="outline"
                size="sm"
                onClick={() => setKeyEntryOpen(true)}
              >
                <KeyRound className="h-4 w-4 mr-1" />
                Use Key
              </Button>
            </>
          )}
        </div>
      </header>

      {/* ---- Devnet Banner ---- */}
      {wallet.providerType === 'key-entry' && (
        <div className="bg-amber-500/10 border-b border-amber-500/30 px-4 py-2 text-xs text-amber-400 text-center">
          Connected to XRPL Lending Devnet — transactions use devnet funds only
        </div>
      )}

      {/* ---- Pool Registry Status ---- */}
      {poolsLoading && (
        <div className="bg-muted/50 border-b border-border px-4 py-1.5 text-xs text-muted-foreground text-center flex items-center justify-center gap-2">
          <Spinner className="h-3 w-3" />
          Loading pool registry…
        </div>
      )}
      {poolsError && (
        <div className="bg-destructive/10 border-b border-destructive/20 px-4 py-1.5 text-xs text-destructive text-center">
          Pool registry error: {poolsError}
        </div>
      )}

      {/* ---- Main Content ---- */}
      <div className="flex flex-1 overflow-hidden flex-col lg:flex-row">
        {/* ---- Chat Panel (left ~40%) ---- */}
        <div className="flex flex-col lg:w-2/5 border-b lg:border-b-0 lg:border-r border-border h-[50vh] lg:h-auto">
          <ScrollArea className="flex-1 p-4">
            <div className="space-y-4">
              {messages.length === 0 && (
                <div className="flex flex-col items-center justify-center text-muted-foreground text-sm py-20">
                  <Bot className="h-8 w-8 mb-2 opacity-50" />
                  <p>Ask me about trading strategies</p>
                </div>
              )}

              {messages.map(msg => (
                <div
                  key={msg.id}
                  className={cn(
                    'flex',
                    msg.role === 'user' ? 'justify-end' : 'justify-start',
                  )}
                >
                  <div
                    className={cn(
                      'rounded-lg px-3 py-2 max-w-[80%] text-sm whitespace-pre-wrap',
                      msg.role === 'user' && 'bg-primary text-primary-foreground',
                      msg.role === 'assistant' && 'bg-muted text-foreground',
                      msg.role === 'error' &&
                        'bg-destructive/10 text-destructive border border-destructive/20',
                    )}
                  >
                    {msg.content}
                  </div>
                </div>
              ))}

              {isLoading && (
                <div className="flex justify-start">
                  <div className="bg-muted rounded-lg px-4 py-2">
                    <Spinner className="h-4 w-4" />
                  </div>
                </div>
              )}

              <div ref={messagesEndRef} />
            </div>
          </ScrollArea>

          {/* Input bar */}
          <div className="border-t border-border p-4 shrink-0">
            <div className="flex items-center gap-2">
              <Input
                ref={inputRef}
                value={inputValue}
                onChange={e => setInputValue(e.target.value)}
                onKeyDown={handleKeyDown}
                placeholder={wallet.address ? 'Describe your trading goal...' : 'Connect wallet to continue'}
                disabled={isLoading || !wallet.address}
              />
              <Button
                size="icon"
                onClick={handleSend}
                disabled={sendDisabled}
                title={!wallet.address ? 'Connect wallet to continue' : undefined}
              >
                {isLoading ? <Spinner className="h-4 w-4" /> : <Send className="h-4 w-4" />}
              </Button>
            </div>
            {!wallet.address && (
              <p className="text-xs text-muted-foreground mt-2 text-center">
                Connect wallet to continue
              </p>
            )}
          </div>
        </div>

        {/* ---- Strategy Panel (right ~60%) ---- */}
        <div className="flex-1 lg:w-3/5 overflow-y-auto p-4">
          {latestStrategies.length === 0 ? (
            <div className="flex flex-col items-center justify-center h-full text-muted-foreground text-sm">
              <p>Strategies will appear here</p>
            </div>
          ) : (
            <div className="space-y-4">
              <h2 className="text-lg font-semibold text-foreground">Recommended Strategies</h2>
              <div className="grid gap-4 grid-cols-1 xl:grid-cols-3">
                {latestStrategies.map(strategy => (
                  <StrategyCard
                    key={strategy.id}
                    strategy={strategy}
                    isExecuting={isExecuting}
                    onExecute={handleExecute}
                    canSign={canSign}
                  />
                ))}
              </div>
            </div>
          )}
        </div>
      </div>

      {/* ---- Key Entry Modal ---- */}
      <KeyEntryModal
        open={keyEntryOpen}
        onOpenChange={setKeyEntryOpen}
        onConnect={async (secret) => {
          await wallet.connectWithKey(secret)
          setKeyEntryOpen(false)
        }}
        onGenerate={() => wallet.generateNewWallet()}
      />

      <Toaster />
    </div>
  )
}

// --------------- Strategy Card (inline sub-component) ---------------

function StrategyCard({
  strategy,
  isExecuting,
  onExecute,
  canSign,
}: {
  strategy: Strategy
  isExecuting: string | null
  onExecute: (id: string) => void
  canSign: boolean
}) {
  const { label, cls } = riskBadge(strategy.risk_score)

  return (
    <Card className="flex flex-col h-full">
      <CardHeader className="pb-3">
        <div className="flex items-center justify-between gap-2">
          <CardTitle className="text-base leading-tight">{strategy.title}</CardTitle>
          <Badge variant="outline" className={cn('shrink-0', cls)}>
            {label} ({strategy.risk_score})
          </Badge>
        </div>
        <CardDescription className="text-xs line-clamp-2">{strategy.description}</CardDescription>
      </CardHeader>

      <CardContent className="flex-1 flex flex-col gap-3 text-xs">
        {/* Pros / Cons */}
        <div className="grid grid-cols-2 gap-2">
          <div className="space-y-1">
            <p className="font-medium text-green-400">Pros</p>
            {strategy.pros.map((pro, i) => (
              <div key={i} className="flex items-start gap-1 text-muted-foreground">
                <Check className="h-3 w-3 text-green-400 mt-0.5 shrink-0" />
                <span className="line-clamp-2">{pro}</span>
              </div>
            ))}
          </div>
          <div className="space-y-1">
            <p className="font-medium text-red-400">Cons</p>
            {strategy.cons.map((con, i) => (
              <div key={i} className="flex items-start gap-1 text-muted-foreground">
                <X className="h-3 w-3 text-red-400 mt-0.5 shrink-0" />
                <span className="line-clamp-2">{con}</span>
              </div>
            ))}
          </div>
        </div>

        {/* Projected Returns */}
        <div>
          <p className="font-medium text-muted-foreground mb-1">7-Day Projected Return</p>
          <div className="grid grid-cols-3 gap-2 text-center">
            <div className="p-2 rounded bg-muted/50">
              <p className="text-muted-foreground">Worst</p>
              <p className="font-medium text-red-400">{strategy.projected_return_7d.worst_case}</p>
            </div>
            <div className="p-2 rounded bg-primary/10 border border-primary/20">
              <p className="text-muted-foreground">Expected</p>
              <p className="font-medium text-primary">{strategy.projected_return_7d.expected}</p>
            </div>
            <div className="p-2 rounded bg-muted/50">
              <p className="text-muted-foreground">Best</p>
              <p className="font-medium text-green-400">{strategy.projected_return_7d.best_case}</p>
            </div>
          </div>
        </div>

        {/* Trade Actions Table */}
        {strategy.trade_actions.length > 0 && (
          <div>
            <p className="font-medium text-muted-foreground mb-1">Trade Actions</p>
            <div className="space-y-1">
              {strategy.trade_actions.map((action, i) => (
                <div
                  key={i}
                  className="flex items-center gap-1.5 text-muted-foreground bg-muted/50 rounded px-2 py-1"
                >
                  <span className="capitalize font-medium text-foreground">{action.action}</span>
                  <span>{action.amount} {action.asset_in}</span>
                  <ArrowRight className="h-3 w-3 shrink-0" />
                  <span>{action.asset_out}</span>
                </div>
              ))}
            </div>
          </div>
        )}

        {/* Execute Button */}
        <Button
          className="mt-auto w-full"
          size="sm"
          onClick={() => onExecute(strategy.id)}
          disabled={isExecuting !== null || !canSign}
          title={!canSign ? 'Connect a signing wallet to execute' : undefined}
        >
          {isExecuting === strategy.id ? (
            <>
              <Spinner className="h-4 w-4 mr-1" />
              Executing...
            </>
          ) : (
            'Execute'
          )}
        </Button>
      </CardContent>
    </Card>
  )
}
