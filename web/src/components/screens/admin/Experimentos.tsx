import { useState } from 'react'
import { Card } from '../../primitives/Card'
import { Button } from '../../primitives/Button'
import { Badge } from '../../primitives/Badge'
import { StatTile } from '../../primitives/StatTile'
import { Table } from '../../primitives/Table'
import { AsyncStatus } from '../../primitives/AsyncStatus'
import { useAsyncAction } from '../../../hooks/useAsyncAction'
import { fetchExperiment, type ExperimentVerdict } from '../../../api/experiments'

function verdictBadge(verdict: ExperimentVerdict, winner?: string) {
  switch (verdict) {
    case 'significant':
      return <Badge color="var(--ok)">significativo — vencedor: {winner}</Badge>
    case 'inconclusive':
      return <Badge color="var(--muted)">sem significância</Badge>
    case 'insufficient_data':
      return <Badge color="var(--amber)">amostra insuficiente</Badge>
  }
}

export function Experimentos() {
  const [nome, setNome] = useState('')
  const state = useAsyncAction(fetchExperiment)

  async function handleBuscar() {
    if (!nome.trim()) return
    await state.run(nome.trim())
  }

  return (
    <div className="stack">
      <Card>
        <strong>Relatório de A/B</strong>
        <div className="row" style={{ marginTop: 8 }}>
          <input
            value={nome}
            onChange={(e) => setNome(e.target.value)}
            placeholder="nome do experimento (props.experiment)"
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
          <Button onClick={() => void handleBuscar()} disabled={state.state.status === 'loading' || !nome.trim()}>
            {state.state.status === 'loading' ? 'buscando…' : 'buscar'}
          </Button>
        </div>
      </Card>

      <AsyncStatus
        state={state.state}
        onRetry={() => void handleBuscar()}
        idleFallback={
          <Card>
            <span style={{ fontSize: 12, color: 'var(--faint)' }}>
              digite o nome de um experimento (o valor de <span className="mono">props.experiment</span> nos
              eventos de telemetria) para ver o relatório.
            </span>
          </Card>
        }
      >
        {(report) => (
          <div className="stack">
            <div className="row">
              <StatTile value={report.experiment} label="experimento" />
              <StatTile value={report.metric} label="métrica" />
              <StatTile value={report.p_value.toFixed(4)} label="p-valor" />
            </div>

            <div className="row" style={{ alignItems: 'center' }}>
              {verdictBadge(report.verdict, report.winner)}
            </div>

            <Table
              rowKey={(v) => v.variant}
              rows={report.variants}
              columns={[
                { key: 'variant', header: 'variante', render: (v) => <span className="mono">{v.variant}</span> },
                { key: 'n', header: 'amostra', render: (v) => v.n },
                { key: 'successes', header: 'sucessos', render: (v) => v.successes },
                { key: 'rate', header: 'taxa', render: (v) => `${(v.rate * 100).toFixed(1)}%` },
              ]}
            />
          </div>
        )}
      </AsyncStatus>

      <div style={{ fontSize: 11, color: 'var(--faint)' }}>
        veredito derivado por teste z de duas proporções (<span className="mono">experiment.v1</span>, ADR 0014) —
        nunca inventa vencedor sem significância. Atribuição por telemetria ainda em instrumentação: nenhum código de
        produção grava <span className="mono">props.experiment</span>/<span className="mono">variant</span>/
        <span className="mono">success</span> hoje — os relatórios acima refletem dados semeados para prova do
        caminho, não tráfego real ainda.
      </div>
    </div>
  )
}
