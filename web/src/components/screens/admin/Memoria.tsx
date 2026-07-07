import { useEffect, useState } from 'react'
import { Card } from '../../primitives/Card'
import { Button } from '../../primitives/Button'
import { Table } from '../../primitives/Table'
import { AsyncStatus } from '../../primitives/AsyncStatus'
import { useAsyncAction } from '../../../hooks/useAsyncAction'
import { fetchMemoryMap, recallMemory, type MemoryMatch, type MemorySummary } from '../../../api/memory'

function shortJson(raw: string): string {
  return raw.length > 80 ? `${raw.slice(0, 80)}…` : raw
}

export function Memoria() {
  const mapState = useAsyncAction(fetchMemoryMap)
  const recallState = useAsyncAction(recallMemory)
  const [query, setQuery] = useState('')
  const [agentFilter, setAgentFilter] = useState<string>('todos')
  const [map, setMap] = useState<MemorySummary[]>([])

  async function loadMap(agent?: string) {
    const result = await mapState.run(agent)
    setMap(result)
  }

  useEffect(() => {
    void loadMap()
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [])

  async function handleAgentFilter(agent: string) {
    setAgentFilter(agent)
    await loadMap(agent === 'todos' ? undefined : agent)
  }

  async function handleRecall() {
    if (!query.trim()) return
    await recallState.run(query.trim(), 5)
  }

  const agents = ['todos', ...Array.from(new Set(map.map((m) => m.agent)))]

  return (
    <div className="stack">
      <Card>
        <strong>Busca (RAG léxico)</strong>
        <div className="row" style={{ marginTop: 8 }}>
          <input
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            placeholder="o que o squad já decidiu sobre..."
            style={{
              flex: 1,
              background: 'transparent',
              border: '1px solid var(--line)',
              borderRadius: 6,
              color: 'var(--ink)',
              padding: '6px 8px',
              fontSize: 12,
            }}
          />
          <Button onClick={() => void handleRecall()} disabled={recallState.state.status === 'loading' || !query.trim()}>
            {recallState.state.status === 'loading' ? 'buscando…' : 'buscar'}
          </Button>
        </div>
        {recallState.state.status !== 'idle' && (
          <div style={{ marginTop: 8 }}>
            <AsyncStatus state={recallState.state} onRetry={() => void handleRecall()}>
              {(matches: MemoryMatch[]) =>
                matches.length === 0 ? (
                  <div style={{ fontSize: 12, color: 'var(--faint)' }}>
                    nenhuma memória com termos em comum com a consulta.
                  </div>
                ) : (
                  <div className="stack">
                    {matches.map((m) => (
                      <div
                        key={m.id}
                        className="row"
                        style={{
                          justifyContent: 'space-between',
                          background: 'var(--panel)',
                          border: '1px solid var(--line)',
                          borderLeft: '3px solid var(--wire)',
                          borderRadius: 6,
                          padding: '8px 12px',
                        }}
                      >
                        <span>
                          <strong>{m.agent}</strong>{' '}
                          <span className="mono" style={{ fontSize: 11, color: 'var(--faint)' }}>
                            {shortJson(m.decision_json)}
                          </span>
                        </span>
                        <span className="mono" style={{ fontSize: 11 }}>
                          score {m.score.toFixed(3)}
                        </span>
                      </div>
                    ))}
                  </div>
                )
              }
            </AsyncStatus>
          </div>
        )}
      </Card>

      <div className="row">
        {agents.map((a) => (
          <button
            key={a}
            onClick={() => void handleAgentFilter(a)}
            style={{
              fontSize: 12,
              padding: '4px 10px',
              borderRadius: 6,
              border: `1px solid ${a === agentFilter ? 'var(--ink)' : 'var(--line)'}`,
              background: a === agentFilter ? 'var(--panel2)' : 'transparent',
              color: 'var(--ink)',
            }}
          >
            {a}
          </button>
        ))}
      </div>

      <AsyncStatus state={mapState.state} onRetry={() => void loadMap(agentFilter === 'todos' ? undefined : agentFilter)}>
        {() =>
          map.length === 0 ? (
            <Card>
              <span style={{ color: 'var(--faint)', fontSize: 12 }}>
                nenhuma memória registrada ainda — aparece assim que o squad tomar uma decisão real.
              </span>
            </Card>
          ) : (
            <Table
              rowKey={(m) => m.agent}
              rows={map}
              columns={[
                { key: 'agent', header: 'agente', render: (m) => m.agent },
                { key: 'count', header: 'memórias', render: (m) => m.count },
                {
                  key: 'latest',
                  header: 'decisão mais recente',
                  render: (m) => (
                    <span className="mono" style={{ fontSize: 11 }}>
                      {shortJson(m.latest_decision_json)}
                    </span>
                  ),
                },
                { key: 'ts', header: 'quando', render: (m) => <span className="mono">{m.latest_timestamp}</span> },
                {
                  key: 'confidence',
                  header: 'maior confiança',
                  render: (m) => m.top_confidence.toFixed(2),
                },
              ]}
            />
          )
        }
      </AsyncStatus>

      <div style={{ fontSize: 11, color: 'var(--faint)' }}>
        recuperação léxica (TF-IDF sobre termos distintivos) — não é embedding semântico. Sem coluna de
        tendência de esquecimento: nada no código calcula isso hoje.
      </div>
    </div>
  )
}
