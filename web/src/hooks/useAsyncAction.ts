import { useCallback, useState } from 'react'

export type AsyncState<T> =
  | { status: 'idle' }
  | { status: 'loading' }
  | { status: 'success'; data: T }
  | { status: 'error'; error: Error }

export function useAsyncAction<Args extends unknown[], T>(fn: (...args: Args) => Promise<T>) {
  const [state, setState] = useState<AsyncState<T>>({ status: 'idle' })

  const run = useCallback(
    async (...args: Args) => {
      setState({ status: 'loading' })
      try {
        const data = await fn(...args)
        setState({ status: 'success', data })
        return data
      } catch (e) {
        const error = e instanceof Error ? e : new Error(String(e))
        setState({ status: 'error', error })
        throw error
      }
    },
    [fn],
  )

  const reset = useCallback(() => setState({ status: 'idle' }), [])

  return { state, run, reset }
}
