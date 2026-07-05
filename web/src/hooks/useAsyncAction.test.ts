import { describe, expect, it } from 'vitest'
import { renderHook, act } from '@testing-library/react'
import { useAsyncAction } from './useAsyncAction'

describe('useAsyncAction', () => {
  it('vai de idle a loading a success', async () => {
    const { result } = renderHook(() => useAsyncAction(async (x: number) => x * 2))
    expect(result.current.state.status).toBe('idle')

    let promise: Promise<number>
    act(() => {
      promise = result.current.run(21)
    })
    expect(result.current.state.status).toBe('loading')

    await act(async () => {
      await promise
    })
    expect(result.current.state).toEqual({ status: 'success', data: 42 })
  })

  it('vai de idle a loading a error quando a promise rejeita', async () => {
    const { result } = renderHook(() => useAsyncAction(async () => { throw new Error('boom') }))

    await act(async () => {
      await result.current.run().catch(() => {})
    })

    expect(result.current.state.status).toBe('error')
    if (result.current.state.status === 'error') {
      expect(result.current.state.error.message).toBe('boom')
    }
  })

  it('reset volta para idle', async () => {
    const { result } = renderHook(() => useAsyncAction(async () => 'ok'))
    await act(async () => {
      await result.current.run()
    })
    expect(result.current.state.status).toBe('success')

    act(() => result.current.reset())
    expect(result.current.state.status).toBe('idle')
  })
})
