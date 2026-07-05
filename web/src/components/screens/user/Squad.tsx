import { useState } from 'react'
import { Card } from '../../primitives/Card'
import { Button } from '../../primitives/Button'
import { ProgressBar } from '../../primitives/ProgressBar'
import { useAsyncAction } from '../../../hooks/useAsyncAction'
import { useToast } from '../../primitives/Toast'
import { CONSENSUS, SQUAD_AGENTS, resolveHITL, runSquad } from '../../../api/squad'
import type { SquadAgentState } from '../../../types/domain'

const STATE_COLOR: Record<SquadAgentState, string> = {
  concluido: 'var(--ok)',
  executando: 'var(--amber)',
  aguardando: 'var(--muted)',
  ocioso: 'var(--faint)',
}

export function Squad() {
  const toast = useToast()
  const [gateResolved, setGateResolved] = useState(false)
  const [task, setTask] = useState('migre o módulo de pagamentos para o novo gateway')
  const [agents, setAgents] = useState(SQUAD_AGENTS)
  const run = useAsyncAction(runSquad)

  async function handleRunSquad() {
    if (!task.trim()) return
    try {
      const result = await run.run(task)
      setAgents(result)
      toast.push('success', 'squad executado — agentes atualizados')
    } catch {
      toast.push('error', 'falha ao executar o squad')
    }
  }

  async function handleGate(approve: boolean) {
    try {
      const result = await resolveHITL(approve)
      setGateResolved(true)
      toast.push('success', `${approve ? 'aprovado' : 'rejeitado'} · trust ${result.trustDelta >= 0 ? '+' : ''}${result.trustDelta}`)
    } catch {
      toast.push('error', 'falha ao resolver gate HITL')
    }
  }

  return (
    <div className="stack">
      <div className="row mono" style={{ color: 'var(--muted)' }}>
        <span>&gt; forge squad</span>
        <input
          value={task}
          onChange={(e) => setTask(e.target.value)}
          onKeyDown={(e) => e.key === 'Enter' && void handleRunSquad()}
          style={{ flex: 1, background: 'transparent', border: '1px solid var(--line)', borderRadius: 6, color: 'var(--ink)', padding: '4px 8px' }}
        />
        <Button onClick={() => void handleRunSquad()} disabled={run.state.status === 'loading'}>
          {run.state.status === 'loading' ? 'executando…' : 'rodar'}
        </Button>
      </div>

      <div className="grid" style={{ gridTemplateColumns: '1.4fr 1fr' }}>
        <div className="stack">
          {agents.map((a) => (
            <Card key={a.id}>
              <div className="row" style={{ justifyContent: 'space-between' }}>
                <span className="row">
                  <span style={{ width: 8, height: 8, borderRadius: '50%', background: STATE_COLOR[a.state] }} />
                  <strong>{a.name}</strong>
                  <span style={{ fontSize: 12, color: 'var(--muted)' }}>{a.state}</span>
                </span>
                <span className="mono" style={{ fontSize: 12, color: 'var(--muted)' }}>
                  conf {a.confidence == null ? '—' : a.confidence.toFixed(2)}
                </span>
              </div>
              <p style={{ fontSize: 13, color: 'var(--muted)', marginTop: 6 }}>{a.task}</p>
            </Card>
          ))}
        </div>

        <div className="stack">
          <Card>
            <strong>Consenso ponderado</strong>
            <div style={{ fontSize: 22, fontWeight: 700, margin: '6px 0' }}>{CONSENSUS.strength.toFixed(2)}</div>
            <ProgressBar value={CONSENSUS.strength} />
            <p style={{ fontSize: 12, color: 'var(--muted)', marginTop: 8 }}>
              decisão: {CONSENSUS.decisionMaker} · divergência:{' '}
              {CONSENSUS.dissent.map((d) => `${d.agent} ${d.score.toFixed(2)}`).join(', ')}
            </p>
          </Card>

          <Card accentBorder="var(--amber)">
            <strong>Gate HITL</strong>
            <p style={{ fontSize: 13, color: 'var(--muted)' }}>ação crítica requer aprovação humana.</p>
            {gateResolved ? (
              <div style={{ color: 'var(--ok)', fontSize: 13 }}>✓ resolvido</div>
            ) : (
              <div className="row">
                <Button variant="danger" onClick={() => void handleGate(false)}>
                  Rejeitar
                </Button>
                <Button variant="primary" onClick={() => void handleGate(true)}>
                  Aprovar
                </Button>
              </div>
            )}
          </Card>

          <Card>
            <strong>Fallback progressivo</strong>
            <p style={{ fontSize: 13, color: 'var(--muted)' }}>squad → agente-único → safe-mode</p>
            <div className="row" style={{ fontSize: 12, color: 'var(--ok)' }}>
              <span className="pulse-dot" /> sidecar saudável
            </div>
          </Card>
        </div>
      </div>
    </div>
  )
}
