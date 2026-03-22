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
import { fetchAllPools, type PoolEntry, type PoolKey } from '@/services/poolRegistry'

// --------------- Context Types ---------------

interface PoolRegistryValue {
  pools: Map<PoolKey, PoolEntry>
  isLoading: boolean
  error: string | null
  /** Manually re-fetch pools (e.g. after a network switch) */
  refetch: () => void
}

// --------------- Context ---------------

const PoolRegistryContext = createContext<PoolRegistryValue>({
  pools: new Map(),
  isLoading: false,
  error: null,
  refetch: () => {},
})

// --------------- Provider ---------------

interface PoolRegistryProviderProps {
  children: ReactNode
  /**
   * The connected wallet address. When this transitions from null → non-null,
   * the registry is fetched. When it returns to null, the registry is cleared.
   */
  walletAddress: string | null
}

export function PoolRegistryProvider({
  children,
  walletAddress,
}: PoolRegistryProviderProps) {
  const [pools, setPools] = useState<Map<PoolKey, PoolEntry>>(new Map())
  const [isLoading, setIsLoading] = useState(false)
  const [error, setError] = useState<string | null>(null)

  // Prevent stale fetch results from landing if wallet disconnects mid-flight
  const fetchIdRef = useRef(0)

  const load = useCallback(async () => {
    const id = ++fetchIdRef.current
    setIsLoading(true)
    setError(null)

    try {
      const result = await fetchAllPools()
      if (fetchIdRef.current !== id) return // superseded
      setPools(result)
    } catch (err) {
      if (fetchIdRef.current !== id) return
      setError(
        err instanceof Error
          ? err.message
          : 'Failed to load pool registry',
      )
      setPools(new Map())
    } finally {
      if (fetchIdRef.current === id) setIsLoading(false)
    }
  }, [])

  // Fetch when wallet connects; clear when wallet disconnects
  useEffect(() => {
    if (walletAddress) {
      load()
    } else {
      fetchIdRef.current++ // cancel any in-flight fetch
      setPools(new Map())
      setError(null)
      setIsLoading(false)
    }
  }, [walletAddress, load])

  return (
    <PoolRegistryContext.Provider value={{ pools, isLoading, error, refetch: load }}>
      {children}
    </PoolRegistryContext.Provider>
  )
}

// --------------- Hook ---------------

export function usePoolRegistry(): PoolRegistryValue {
  return useContext(PoolRegistryContext)
}
