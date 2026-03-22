'use client'

import { useState, useCallback } from 'react'
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogDescription,
} from '@/components/ui/dialog'
import { Tabs, TabsList, TabsTrigger, TabsContent } from '@/components/ui/tabs'
import { Input } from '@/components/ui/input'
import { Button } from '@/components/ui/button'
import { Label } from '@/components/ui/label'
import { Alert, AlertDescription } from '@/components/ui/alert'
import { Eye, EyeOff, Copy, AlertTriangle } from 'lucide-react'

interface KeyEntryModalProps {
  open: boolean
  onOpenChange: (open: boolean) => void
  onConnect: (secret: string) => Promise<void>
  onGenerate: () => { address: string; secret: string }
}

export function KeyEntryModal({
  open,
  onOpenChange,
  onConnect,
  onGenerate,
}: KeyEntryModalProps) {
  // Enter Key tab state
  const [secretInput, setSecretInput] = useState('')
  const [showSecret, setShowSecret] = useState(false)
  const [enterError, setEnterError] = useState<string | null>(null)
  const [isConnecting, setIsConnecting] = useState(false)

  // Generate tab state
  const [generated, setGenerated] = useState<{
    address: string
    secret: string
  } | null>(null)
  const [copied, setCopied] = useState<'address' | 'secret' | null>(null)

  const handleEnterSubmit = useCallback(async () => {
    if (!secretInput.trim()) return
    setEnterError(null)
    setIsConnecting(true)
    try {
      await onConnect(secretInput.trim())
      setSecretInput('')
      setShowSecret(false)
    } catch (err) {
      setEnterError(
        err instanceof Error ? err.message : 'Invalid secret key',
      )
    } finally {
      setIsConnecting(false)
    }
  }, [secretInput, onConnect])

  const handleGenerate = useCallback(() => {
    const result = onGenerate()
    setGenerated(result)
    setCopied(null)
  }, [onGenerate])

  const handleCopy = useCallback(
    async (value: string, field: 'address' | 'secret') => {
      await navigator.clipboard.writeText(value)
      setCopied(field)
      setTimeout(() => setCopied(null), 2000)
    },
    [],
  )

  const handleConnectGenerated = useCallback(async () => {
    if (!generated) return
    setIsConnecting(true)
    try {
      await onConnect(generated.secret)
      setGenerated(null)
    } catch (err) {
      setEnterError(
        err instanceof Error ? err.message : 'Failed to connect',
      )
    } finally {
      setIsConnecting(false)
    }
  }, [generated, onConnect])

  // Reset state when modal closes
  const handleOpenChange = useCallback(
    (nextOpen: boolean) => {
      if (!nextOpen) {
        setSecretInput('')
        setShowSecret(false)
        setEnterError(null)
        setGenerated(null)
        setCopied(null)
      }
      onOpenChange(nextOpen)
    },
    [onOpenChange],
  )

  return (
    <Dialog open={open} onOpenChange={handleOpenChange}>
      <DialogContent className="sm:max-w-md">
        <DialogHeader>
          <DialogTitle>Connect Wallet</DialogTitle>
          <DialogDescription>
            Enter an existing XRPL secret key or generate a new keypair.
          </DialogDescription>
        </DialogHeader>

        <Alert className="border-amber-500/30 bg-amber-500/10">
          <AlertTriangle className="h-4 w-4 text-amber-400" />
          <AlertDescription className="text-amber-400 text-xs">
            Only use devnet / testnet keys. Never enter mainnet secrets here.
          </AlertDescription>
        </Alert>

        <Tabs defaultValue="enter" className="mt-2">
          <TabsList className="w-full">
            <TabsTrigger value="enter" className="flex-1">
              Enter Key
            </TabsTrigger>
            <TabsTrigger value="generate" className="flex-1">
              Generate New
            </TabsTrigger>
          </TabsList>

          {/* ---------- Enter Key Tab ---------- */}
          <TabsContent value="enter" className="space-y-4 mt-4">
            <div className="space-y-2">
              <Label htmlFor="secret-input">Secret Key</Label>
              <div className="relative">
                <Input
                  id="secret-input"
                  type={showSecret ? 'text' : 'password'}
                  value={secretInput}
                  onChange={(e) => {
                    setSecretInput(e.target.value)
                    setEnterError(null)
                  }}
                  onKeyDown={(e) => {
                    if (e.key === 'Enter') handleEnterSubmit()
                  }}
                  placeholder="sEdV... or seed..."
                  className="pr-10 font-mono text-sm"
                />
                <button
                  type="button"
                  onClick={() => setShowSecret(!showSecret)}
                  className="absolute right-2 top-1/2 -translate-y-1/2 text-muted-foreground hover:text-foreground"
                >
                  {showSecret ? (
                    <EyeOff className="h-4 w-4" />
                  ) : (
                    <Eye className="h-4 w-4" />
                  )}
                </button>
              </div>
              {enterError && (
                <p className="text-xs text-destructive">{enterError}</p>
              )}
            </div>
            <Button
              className="w-full"
              onClick={handleEnterSubmit}
              disabled={!secretInput.trim() || isConnecting}
            >
              {isConnecting ? 'Connecting...' : 'Connect'}
            </Button>
          </TabsContent>

          {/* ---------- Generate Tab ---------- */}
          <TabsContent value="generate" className="space-y-4 mt-4">
            {!generated ? (
              <Button
                variant="outline"
                className="w-full"
                onClick={handleGenerate}
              >
                Generate Keypair
              </Button>
            ) : (
              <div className="space-y-3">
                <div className="space-y-1">
                  <Label className="text-xs text-muted-foreground">
                    Address
                  </Label>
                  <div className="flex items-center gap-2">
                    <Input
                      readOnly
                      value={generated.address}
                      className="font-mono text-xs"
                    />
                    <Button
                      variant="ghost"
                      size="icon"
                      onClick={() =>
                        handleCopy(generated.address, 'address')
                      }
                    >
                      <Copy className="h-4 w-4" />
                    </Button>
                  </div>
                  {copied === 'address' && (
                    <p className="text-xs text-green-400">Copied!</p>
                  )}
                </div>

                <div className="space-y-1">
                  <Label className="text-xs text-muted-foreground">
                    Secret Key
                  </Label>
                  <div className="flex items-center gap-2">
                    <Input
                      readOnly
                      value={generated.secret}
                      className="font-mono text-xs"
                    />
                    <Button
                      variant="ghost"
                      size="icon"
                      onClick={() =>
                        handleCopy(generated.secret, 'secret')
                      }
                    >
                      <Copy className="h-4 w-4" />
                    </Button>
                  </div>
                  {copied === 'secret' && (
                    <p className="text-xs text-green-400">Copied!</p>
                  )}
                </div>

                <Alert className="border-amber-500/30 bg-amber-500/10">
                  <AlertTriangle className="h-4 w-4 text-amber-400" />
                  <AlertDescription className="text-amber-400 text-xs">
                    Save your secret key now. It will NOT be stored after
                    you close this dialog.
                  </AlertDescription>
                </Alert>

                <Button
                  className="w-full"
                  onClick={handleConnectGenerated}
                  disabled={isConnecting}
                >
                  {isConnecting
                    ? 'Connecting...'
                    : 'Connect with this wallet'}
                </Button>
              </div>
            )}
          </TabsContent>
        </Tabs>
      </DialogContent>
    </Dialog>
  )
}
