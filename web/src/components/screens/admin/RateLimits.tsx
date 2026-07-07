import { useEffect } from 'react'
import { Table } from '../../primitives/Table'
import { AsyncStatus } from '../../primitives/AsyncStatus'
import { useAsyncAction } from '../../../hooks/useAsyncAction'
import { fetchRateLimits } from '../../../api/ratelimit'

export function RateLimits() {
  const state = useAsyncAction(fetchRateLimits)

  useEffect(() => {
    void state.run()
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [])

  return (
    <div className="stack">
      <AsyncStatus state={state.state} onRetry={() => void state.run()}>
        {(limits) => (
          <Table
            rowKey={(r) => r.tier}
            rows={limits}
            columns={[
              { key: 'tier', header: 'tier', render: (r) => r.tier },
              { key: 'cap', header: 'teto (chamadas)', render: (r) => r.cap },
              {
                key: 'window',
                header: 'janela',
                render: (r) => `${r.window_secs}s`,
              },
            ]}
          />
        )}
      </AsyncStatus>

      <div style={{ fontSize: 11, color: 'var(--faint)' }}>
        tetos configurados (<span className="mono">RateLimiter::for_tier</span>) — <strong>não é uso ao vivo</strong>:
        o dashboard é um processo separado de qualquer sessão <span className="mono">forge run</span>/
        <span className="mono">chat</span> que realmente consome vagas, então não há limitador compartilhado para
        ler. Hit de cache nunca consome vaga.
      </div>
    </div>
  )
}
