'use client'

import {
  createContext,
  useContext,
  useEffect,
  useState,
  useRef,
  useCallback,
  type ReactNode,
} from 'react'
import { fetchAllVaults, type VaultEntry, type VaultKey } from '@/services/vaultRegistry'

interface VaultRegistryValue {
  vaults: Map<VaultKey, VaultEntry>
  isLoading: boolean
  error: string | null
  refetch: () => void
}

const VaultRegistryContext = createContext<VaultRegistryValue>({
  vaults: new Map(),
  isLoading: false,
  error: null,
  refetch: () => {},
})

interface VaultRegistryProviderProps {
  children: ReactNode
  walletAddress: string | null
}

export function VaultRegistryProvider({
  children,
  walletAddress,
}: VaultRegistryProviderProps) {
  const [vaults, setVaults] = useState<Map<VaultKey, VaultEntry>>(new Map())
  const [isLoading, setIsLoading] = useState(false)
  const [error, setError] = useState<string | null>(null)

  const fetchIdRef = useRef(0)

  const load = useCallback(async () => {
    const id = ++fetchIdRef.current
    setIsLoading(true)
    setError(null)

    try {
      const result = await fetchAllVaults()
      if (fetchIdRef.current !== id) return
      setVaults(result)
    } catch (err) {
      if (fetchIdRef.current !== id) return
      setError(
        err instanceof Error
          ? err.message
          : 'Failed to load vault registry',
      )
      setVaults(new Map())
    } finally {
      if (fetchIdRef.current === id) setIsLoading(false)
    }
  }, [])

  useEffect(() => {
    if (walletAddress) {
      load()
    } else {
      fetchIdRef.current++
      setVaults(new Map())
      setError(null)
      setIsLoading(false)
    }
  }, [walletAddress, load])

  return (
    <VaultRegistryContext.Provider value={{ vaults, isLoading, error, refetch: load }}>
      {children}
    </VaultRegistryContext.Provider>
  )
}

export function useVaultRegistry(): VaultRegistryValue {
  return useContext(VaultRegistryContext)
}
