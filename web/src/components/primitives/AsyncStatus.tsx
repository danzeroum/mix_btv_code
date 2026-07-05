import type { ReactNode } from 'react'
import type { AsyncState } from '../../hooks/useAsyncAction'
import { Button } from './Button'

/** Wrapper padrão para idle -> loading -> success | error. Nenhuma ação
 * assíncrona da UI deve renderizar sem passar por aqui (ou pelo Toast, para
 * ações "fire and forget").
 */
export function AsyncStatus<T>({
  state,
  onRetry,
  children,
  idleFallback,
}: {
  state: AsyncState<T>
  onRetry?: () => void
  children: (data: T) => ReactNode
  idleFallback?: ReactNode
}) {
  switch (state.status) {
    case 'idle':
      return <>{idleFallback ?? null}</>
    case 'loading':
      return (
        <div className="row" style={{ color: 'var(--muted)', fontSize: 12 }}>
          <span className="cursor-blink">▸</span> carregando…
        </div>
      )
    case 'error':
      return (
        <div className="row" style={{ color: 'var(--red)', fontSize: 12 }}>
          <span>✗ {state.error.message}</span>
          {onRetry && (
            <Button variant="ghost" onClick={onRetry}>
              tentar de novo
            </Button>
          )}
        </div>
      )
    case 'success':
      return <>{children(state.data)}</>
  }
}
