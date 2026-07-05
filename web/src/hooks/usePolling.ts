import { useEffect, useRef, useState } from 'react'
import type { AsyncState } from './useAsyncAction'

/** Repete `fn` a cada `intervalMs`, reaproveitando o mesmo shape de useAsyncAction
 * para que o consumidor mostre um indicador sutil de "atualizando" em vez de um
 * flash de loading a cada tick (só o primeiro fetch fica em `loading`).
 */
export function usePolling<T>(fn: () => Promise<T>, intervalMs: number) {
  const [state, setState] = useState<AsyncState<T>>({ status: 'idle' })
  const fnRef = useRef(fn)
  fnRef.current = fn

  useEffect(() => {
    let cancelled = false

    async function tick(isFirst: boolean) {
      if (isFirst) setState({ status: 'loading' })
      try {
        const data = await fnRef.current()
        if (!cancelled) setState({ status: 'success', data })
      } catch (e) {
        if (!cancelled) {
          const error = e instanceof Error ? e : new Error(String(e))
          setState({ status: 'error', error })
        }
      }
    }

    void tick(true)
    const id = setInterval(() => void tick(false), intervalMs)
    return () => {
      cancelled = true
      clearInterval(id)
    }
  }, [intervalMs])

  return state
}
