import { useEffect } from 'react'
import { Card } from '../../primitives/Card'
import { Badge } from '../../primitives/Badge'
import { Button } from '../../primitives/Button'
import { AsyncStatus } from '../../primitives/AsyncStatus'
import { useAsyncAction } from '../../../hooks/useAsyncAction'
import { fetchMcpServers } from '../../../api/mcp'
import type { PermissionMatrixDecision } from '../../../types/domain'

const DECISION_COLOR: Record<PermissionMatrixDecision, string> = {
  allow: 'var(--ok)',
  ask: 'var(--amber)',
  deny: 'var(--red)',
}

export function Mcp() {
  const state = useAsyncAction(fetchMcpServers)

  useEffect(() => {
    void state.run()
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [])

  return (
    <div className="stack">
      <div className="row" style={{ justifyContent: 'space-between' }}>
        <span style={{ fontSize: 11, color: 'var(--faint)' }}>
          servidores declarados em <span className="mono">.forge/mcp.toml</span> · sondados agora, ao vivo
        </span>
        <Button onClick={() => void state.run()} disabled={state.state.status === 'loading'}>
          {state.state.status === 'loading' ? 'sondando…' : 'atualizar'}
        </Button>
      </div>

      <AsyncStatus state={state.state} onRetry={() => void state.run()}>
        {(servers) => (
          <div className="stack">
            {servers.length === 0 && (
              <Card>
                <span style={{ color: 'var(--faint)', fontSize: 12 }}>
                  nenhum servidor MCP declarado — crie <span className="mono">.forge/mcp.toml</span> com um bloco{' '}
                  <span className="mono">[[server]]</span>.
                </span>
              </Card>
            )}
            {servers.map((s) => (
              <Card key={s.id} accentBorder={s.status === 'online' ? 'var(--line2)' : 'var(--red)'}>
                <div className="row" style={{ justifyContent: 'space-between' }}>
                  <span>
                    <strong>{s.id}</strong>{' '}
                    <span className="mono" style={{ fontSize: 11, color: 'var(--faint)' }}>
                      {s.command}
                    </span>
                  </span>
                  <Badge color={s.status === 'online' ? 'var(--ok)' : 'var(--red)'}>{s.status}</Badge>
                </div>

                {s.status === 'offline' && (
                  <div style={{ marginTop: 8, fontSize: 12, color: 'var(--red)' }}>{s.error ?? 'sem detalhe'}</div>
                )}

                {s.status === 'online' && (
                  <table style={{ width: '100%', fontSize: 12, borderCollapse: 'collapse', marginTop: 8 }}>
                    <thead>
                      <tr>
                        <th style={{ textAlign: 'left' }}>tool</th>
                        <th style={{ textAlign: 'left' }}>descrição</th>
                        <th>build</th>
                        <th>plan</th>
                      </tr>
                    </thead>
                    <tbody>
                      {s.tools.map((t) => (
                        <tr key={t.name}>
                          <td className="mono" style={{ padding: '4px 0' }}>
                            {t.name}
                          </td>
                          <td style={{ color: 'var(--faint)' }}>{t.description}</td>
                          {(['build', 'plan'] as const).map((profile) => (
                            <td key={profile} style={{ textAlign: 'center', color: DECISION_COLOR[t.policy[profile]] }}>
                              {t.policy[profile]}
                            </td>
                          ))}
                        </tr>
                      ))}
                    </tbody>
                  </table>
                )}
              </Card>
            ))}
          </div>
        )}
      </AsyncStatus>
    </div>
  )
}
