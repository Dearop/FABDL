'use client'

import { useWallet } from '@/lib/wallet-context'
import { Button } from '@/components/ui/button'
import { 
  DropdownMenu, 
  DropdownMenuContent, 
  DropdownMenuItem, 
  DropdownMenuTrigger 
} from '@/components/ui/dropdown-menu'
import { Wallet, LogOut, ChevronDown, Zap } from 'lucide-react'

function truncateAddress(address: string): string {
  if (address.length <= 12) return address
  return `${address.slice(0, 6)}...${address.slice(-4)}`
}

export function Header() {
  const { wallet, disconnectWallet } = useWallet()

  return (
    <header className="sticky top-0 z-50 w-full border-b border-border bg-background/95 backdrop-blur supports-[backdrop-filter]:bg-background/60">
      <div className="container mx-auto flex h-16 items-center justify-between px-4">
        <div className="flex items-center gap-3">
          <div className="flex h-9 w-9 items-center justify-center rounded-lg bg-primary">
            <Zap className="h-5 w-5 text-primary-foreground" />
          </div>
          <div className="flex flex-col">
            <span className="text-lg font-semibold text-foreground">XRPL AI Trading</span>
            <span className="text-xs text-muted-foreground">Powered by Local LLM</span>
          </div>
        </div>

        {wallet && (
          <DropdownMenu>
            <DropdownMenuTrigger asChild>
              <Button variant="outline" className="gap-2">
                <Wallet className="h-4 w-4" />
                <span className="hidden sm:inline">{truncateAddress(wallet.address)}</span>
                <span className="inline sm:hidden">Wallet</span>
                <ChevronDown className="h-4 w-4" />
              </Button>
            </DropdownMenuTrigger>
            <DropdownMenuContent align="end" className="w-64">
              <div className="px-3 py-2 border-b border-border">
                <p className="text-sm font-medium text-foreground">Connected Wallet</p>
                <p className="text-xs text-muted-foreground font-mono mt-1">{wallet.address}</p>
              </div>
              <div className="px-3 py-2 border-b border-border">
                <div className="flex items-center justify-between">
                  <span className="text-sm text-muted-foreground">Network</span>
                  <span className="text-sm font-medium text-foreground">{wallet.network}</span>
                </div>
                <div className="flex items-center justify-between mt-1">
                  <span className="text-sm text-muted-foreground">Balance</span>
                  <span className="text-sm font-medium text-foreground">{wallet.balance}</span>
                </div>
              </div>
              <DropdownMenuItem 
                onClick={disconnectWallet}
                className="text-destructive focus:text-destructive cursor-pointer"
              >
                <LogOut className="h-4 w-4 mr-2" />
                Disconnect
              </DropdownMenuItem>
            </DropdownMenuContent>
          </DropdownMenu>
        )}
      </div>
    </header>
  )
}
