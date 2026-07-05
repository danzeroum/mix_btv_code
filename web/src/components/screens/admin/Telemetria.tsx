import { StatTile } from '../../primitives/StatTile'
import { Card } from '../../primitives/Card'
import { ProgressBar } from '../../primitives/ProgressBar'
import { Table } from '../../primitives/Table'
import { AsyncStatus } from '../../primitives/AsyncStatus'
import { usePolling } from '../../../hooks/usePolling'
import { getEvents, getSummary, type EventRow, type Summary } from '../../../api/telemetry'

async function loadAll(): Promise<{ summary: Summary; events: EventRow[] }> {
  const [summary, events] = await Promise.all([getSummary(), getEvents(50)])
  return { summary, events }
}

export function Telemetria() {
  const state = usePolling(loadAll, 5000)

  return (
    <AsyncStatus state={state} idleFallback={<div>carregando telemetria…</div>}>
      {({ summary, events }) => {
        const rate = summary.cache_hit_rate == null ? 'n/a' : `${(summary.cache_hit_rate * 100).toFixed(1)}%`
        const bars = Object.entries(summary.by_name).sort((a, b) => b[1] - a[1])
        const max = bars.length ? bars[0][1] : 1

        return (
          <div className="stack">
            <div className="row">
              <StatTile value={String(summary.total_events)} label="eventos totais" />
              <StatTile value={rate} label="cache hit rate" />
              <StatTile value={String(summary.by_name['llm.call'] ?? 0)} label="chamadas llm" />
              <StatTile value={String(summary.by_name['tool.result'] ?? 0)} label="execuções de ferramenta" />
            </div>

            <div className="grid" style={{ gridTemplateColumns: '1fr 1.6fr' }}>
              <Card>
                <strong>eventos por tipo</strong>
                <div className="stack" style={{ marginTop: 8 }}>
                  {bars.map(([name, count]) => (
                    <div key={name}>
                      <div className="row" style={{ justifyContent: 'space-between', fontSize: 12 }}>
                        <span>{name}</span>
                        <span className="mono">{count}</span>
                      </div>
                      <ProgressBar value={count / max} color="var(--teal)" />
                    </div>
                  ))}
                </div>
              </Card>

              <Card>
                <strong>eventos recentes</strong>
                <div style={{ marginTop: 8, maxHeight: 360, overflow: 'auto' }}>
                  <Table
                    rowKey={(e) => `${e.ts}-${e.name}`}
                    rows={events}
                    columns={[
                      { key: 'ts', header: 'ts', render: (e) => <span className="mono">{e.ts}</span> },
                      { key: 'name', header: 'nome', render: (e) => e.name },
                      { key: 'sid', header: 'sessão', render: (e) => e.session_id },
                      {
                        key: 'props',
                        header: 'props',
                        render: (e) => <span className="mono" style={{ fontSize: 11 }}>{JSON.stringify(e.props)}</span>,
                      },
                    ]}
                  />
                </div>
              </Card>
            </div>

            <div style={{ fontSize: 11, color: 'var(--faint)' }}>
              offline-first · escuta só em 127.0.0.1 · atualiza a cada 5s
            </div>
          </div>
        )
      }}
    </AsyncStatus>
  )
}
