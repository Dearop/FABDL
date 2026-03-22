'use client'

import { useState, useRef, useEffect, useMemo, useCallback } from 'react'
import { generateStrategies, executeStrategy } from '@/services/api'
import type { Strategy } from '@/services/api'
import { Button } from '@/components/ui/button'
import { Card, CardHeader, CardContent, CardTitle, CardDescription } from '@/components/ui/card'
import { Badge } from '@/components/ui/badge'
import { Input } from '@/components/ui/input'
import { Spinner } from '@/components/ui/spinner'
import { ScrollArea } from '@/components/ui/scroll-area'
import { Toaster } from '@/components/ui/toaster'
import { useToast } from '@/hooks/use-toast'
import { cn } from '@/lib/utils'
import { Send, Wallet, ArrowRight, Check, X, Bot } from 'lucide-react'

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

// --------------- Page Component ---------------

export default function TradingPage() {
  const walletId = 'rXXXdemo1234567890'

  const [messages, setMessages] = useState<Message[]>([])
  const [inputValue, setInputValue] = useState('')
  const [isLoading, setIsLoading] = useState(false)
  const [isExecuting, setIsExecuting] = useState<string | null>(null)

  const messagesEndRef = useRef<HTMLDivElement>(null)
  const inputRef = useRef<HTMLInputElement>(null)
  const { toast } = useToast()

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

  // Send handler
  const handleSend = useCallback(async () => {
    const query = inputValue.trim()
    if (!query || isLoading) return

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
  }, [inputValue, isLoading, walletId])

  // Execute handler
  const handleExecute = useCallback(async (strategyId: string) => {
    if (isExecuting) return
    setIsExecuting(strategyId)

    try {
      await executeStrategy(strategyId, walletId)
      toast({ title: 'Strategy Executed', description: 'Transaction submitted successfully.' })
    } catch (err) {
      toast({
        title: 'Execution Failed',
        description: err instanceof Error ? err.message : 'Unknown error',
        variant: 'destructive',
      })
    } finally {
      setIsExecuting(null)
    }
  }, [isExecuting, walletId, toast])

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

  // ====================== JSX ======================

  return (
    <div className="flex flex-col h-screen bg-background">
      {/* ---- Top Bar ---- */}
      <header className="flex items-center justify-between border-b border-border px-4 py-3 shrink-0">
        <h1 className="text-lg font-semibold text-foreground">AI Trading Assistant</h1>
        <div className="flex items-center gap-2">
          <Input
            readOnly
            value={walletId}
            className="w-48 text-xs font-mono bg-muted"
          />
          <Button variant="outline" size="sm" onClick={() => {}}>
            <Wallet className="h-4 w-4 mr-1" />
            Connect
          </Button>
        </div>
      </header>

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
                placeholder="Describe your trading goal..."
                disabled={isLoading}
              />
              <Button
                size="icon"
                onClick={handleSend}
                disabled={!inputValue.trim() || isLoading}
              >
                {isLoading ? <Spinner className="h-4 w-4" /> : <Send className="h-4 w-4" />}
              </Button>
            </div>
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
                  />
                ))}
              </div>
            </div>
          )}
        </div>
      </div>

      <Toaster />
    </div>
  )
}

// --------------- Strategy Card (inline sub-component) ---------------

function StrategyCard({
  strategy,
  isExecuting,
  onExecute,
}: {
  strategy: Strategy
  isExecuting: string | null
  onExecute: (id: string) => void
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
          disabled={isExecuting !== null}
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
