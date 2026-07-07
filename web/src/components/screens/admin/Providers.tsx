import { useEffect } from 'react'
import { Card } from '../../primitives/Card'
import { Badge } from '../../primitives/Badge'
import { Table } from '../../primitives/Table'
import { AsyncStatus } from '../../primitives/AsyncStatus'
import { useAsyncAction } from '../../../hooks/useAsyncAction'
import { fetchProviders } from '../../../api/providers'
import { fetchRateLimits } from '../../../api/ratelimit'

export function Providers() {
  const providersState = useAsyncAction(fetchProviders)
  const limitsState = useAsyncAction(fetchRateLimits)

  useEffect(() => {
    void providersState.run()
    void limitsState.run()
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [])

  return (
    <div className="stack">
      <div className="grid grid-2">
        <Card>
          <strong>Gateway LLM · providers configurados</strong>
          <div style={{ marginTop: 8 }}>
            <AsyncStatus state={providersState.state} onRetry={() => void providersState.run()}>
              {(providers) => (
                <div className="stack">
                  {providers.map((p) => (
                    <div key={p.id} className="row" style={{ justifyContent: 'space-between' }}>
                      <span className="row">
                        <span style={{ color: p.configured ? 'var(--ok)' : 'var(--muted)' }}>●</span>
                        <span className="mono">{p.id}</span>
                      </span>
                      <Badge color={p.configured ? 'var(--ok)' : 'var(--muted)'}>
                        {p.configured ? 'configurado' : 'sem key'}
                      </Badge>
                    </div>
                  ))}
                </div>
              )}
            </AsyncStatus>
          </div>
          <p style={{ fontSize: 11, color: 'var(--faint)', marginTop: 10 }}>
            🔑 keys só no Rust · ordem de fallback fixa (anthropic → deepseek → openai) · reflete os mesmos env vars
            que uma sessão real (<span className="mono">forge run</span>/<span className="mono">chat</span>) usaria
          </p>
        </Card>

        <Card>
          <strong>Rate limiting tier-gated</strong>
          <div style={{ marginTop: 8 }}>
            <AsyncStatus state={limitsState.state} onRetry={() => void limitsState.run()}>
              {(limits) => (
                <Table
                  rowKey={(r) => r.tier}
                  rows={limits}
                  columns={[
                    { key: 'tier', header: 'tier', render: (r) => r.tier },
                    { key: 'cap', header: 'teto', render: (r) => r.cap },
                    { key: 'window', header: 'janela', render: (r) => `${r.window_secs}s` },
                  ]}
                />
              )}
            </AsyncStatus>
          </div>
          <p style={{ fontSize: 11, color: 'var(--faint)', marginTop: 10 }}>
            tetos configurados — hit de cache nunca consome vaga. Uso ao vivo não é mostrado: o dashboard não
            compartilha processo com nenhuma sessão real (ver tela "Rate limits" para detalhes).
          </p>
        </Card>
      </div>
    </div>
  )
}
