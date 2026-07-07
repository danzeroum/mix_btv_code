import { useEffect, useMemo, useRef, useState } from 'react'
import { Card } from '../../primitives/Card'
import { Button } from '../../primitives/Button'
import { ProgressBar } from '../../primitives/ProgressBar'
import { useAsyncAction } from '../../../hooks/useAsyncAction'
import { useToast } from '../../primitives/Toast'
import {
  connectSquadEvents,
  resolveHitl,
  runSquad,
  HANDOFF_PHASE_LABELS,
  type SquadEventEnvelope,
  type SquadHandoff,
  type SquadStep,
} from '../../../api/squad'

function formatJsonPreview(raw: string): string {
  try {
    return JSON.stringify(JSON.parse(raw), null, 2)
  } catch {
    return raw
  }
}

export function Squad() {
  const toast = useToast()
  const [task, setTask] = useState('migre o módulo de pagamentos para o novo gateway')
  const [taskId, setTaskId] = useState<string | null>(null)
  const [events, setEvents] = useState<SquadEventEnvelope[]>([])
  const [streamEnded, setStreamEnded] = useState(false)
  const [resolvedHitlCount, setResolvedHitlCount] = useState(0)
  const disconnectRef = useRef<(() => void) | null>(null)
  const run = useAsyncAction(runSquad)

  // Desmonte da tela (troca de aba) fecha a conexão SSE — a tarefa em si
  // continua rodando no backend, só paramos de escutar.
  useEffect(() => () => disconnectRef.current?.(), [])

  const proposals = useMemo(() => {
    const order: string[] = []
    const byAgent = new Map<string, { agent: string; confidence: number; content_json: string; ts: string }>()
    for (const e of events) {
      if (e.payload && 'Proposal' in e.payload) {
        const p = e.payload.Proposal
        if (!byAgent.has(p.agent)) order.push(p.agent)
        byAgent.set(p.agent, { ...p, ts: e.ts })
      }
    }
    return order.map((agent) => byAgent.get(agent)!)
  }, [events])

  const consensus = useMemo(() => {
    for (let i = events.length - 1; i >= 0; i -= 1) {
      const p = events[i].payload
      if (p && 'Consensus' in p) return p.Consensus
    }
    return null
  }, [events])

  const executionLog = useMemo(
    () => events.map((e, i) => ({ e, i })).filter(({ e }) => !!e.payload && ('Handoff' in e.payload || 'Step' in e.payload)),
    [events],
  )

  const hitlEvents = useMemo(
    () => events.flatMap((e) => (e.payload && 'Hitl' in e.payload ? [e.payload.Hitl] : [])),
    [events],
  )
  const pendingHitl = resolvedHitlCount < hitlEvents.length ? hitlEvents[hitlEvents.length - 1] : null

  const errorMessage = useMemo(() => {
    for (let i = events.length - 1; i >= 0; i -= 1) {
      const p = events[i].payload
      if (p && 'Error' in p) return p.Error
    }
    return null
  }, [events])

  const active = taskId !== null && !streamEnded
  const busy = active || run.state.status === 'loading'

  async function handleRunSquad() {
    if (!task.trim() || busy) return
    disconnectRef.current?.()
    disconnectRef.current = null
    setTaskId(null)
    setEvents([])
    setResolvedHitlCount(0)
    setStreamEnded(false)
    try {
      const { task_id } = await run.run(task)
      setTaskId(task_id)
      disconnectRef.current = connectSquadEvents(task_id, {
        onEvent: (event) => setEvents((prev) => [...prev, event]),
        onConnectionError: () => {
          setStreamEnded(true)
          disconnectRef.current?.()
          disconnectRef.current = null
        },
      })
      toast.push('success', 'squad disparado — acompanhando eventos ao vivo')
    } catch {
      toast.push('error', 'falha ao disparar o squad')
    }
  }

  async function handleGate(allow: boolean) {
    if (!taskId || !pendingHitl) return
    try {
      await resolveHitl(taskId, allow)
      setResolvedHitlCount((n) => n + 1)
      toast.push('success', allow ? 'aprovado' : 'rejeitado')
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
          disabled={busy}
          style={{
            flex: 1,
            background: 'transparent',
            border: '1px solid var(--line)',
            borderRadius: 6,
            color: 'var(--ink)',
            padding: '4px 8px',
          }}
        />
        <Button onClick={() => void handleRunSquad()} disabled={busy}>
          {run.state.status === 'loading' ? 'disparando…' : active ? 'executando…' : 'rodar'}
        </Button>
      </div>

      {taskId && (
        <div className="mono" style={{ fontSize: 11, color: 'var(--faint)' }}>
          task_id {taskId} · {streamEnded ? 'stream encerrado' : 'ao vivo'}
        </div>
      )}

      {errorMessage && (
        <Card accentBorder="var(--red)">
          <strong style={{ color: 'var(--red)' }}>Erro no squad</strong>
          <p style={{ fontSize: 13, color: 'var(--muted)' }}>{errorMessage}</p>
        </Card>
      )}

      <div className="grid" style={{ gridTemplateColumns: '1.4fr 1fr' }}>
        <div className="stack">
          {proposals.length === 0 && (
            <div className="mono" style={{ padding: 18, color: 'var(--faint)', fontSize: 13 }}>
              nenhuma proposta ainda — rode uma tarefa para ver os agentes reais responderem ao vivo.
            </div>
          )}
          {proposals.map((p) => (
            <Card key={p.agent}>
              <div className="row" style={{ justifyContent: 'space-between' }}>
                <span className="row">
                  <span style={{ width: 8, height: 8, borderRadius: '50%', background: 'var(--ok)' }} />
                  <strong>{p.agent}</strong>
                </span>
                <span className="mono" style={{ fontSize: 12, color: 'var(--muted)' }}>
                  conf {p.confidence.toFixed(2)}
                </span>
              </div>
              <pre
                className="mono"
                style={{
                  fontSize: 11,
                  color: 'var(--muted)',
                  marginTop: 6,
                  maxHeight: 140,
                  overflow: 'auto',
                  whiteSpace: 'pre-wrap',
                }}
              >
                {formatJsonPreview(p.content_json)}
              </pre>
            </Card>
          ))}

          {executionLog.length > 0 && (
            <Card>
              <strong>Execução</strong>
              <div className="stack" style={{ marginTop: 8 }}>
                {executionLog.map(({ e, i }) => {
                  const payload = e.payload as { Handoff: SquadHandoff } | { Step: SquadStep }
                  if ('Handoff' in payload) {
                    const h = payload.Handoff
                    return (
                      <div key={i} className="mono" style={{ fontSize: 12, color: 'var(--muted)' }}>
                        handoff {HANDOFF_PHASE_LABELS[h.phase]} · {h.from_agent} → {h.to_agent}
                      </div>
                    )
                  }
                  const s = payload.Step
                  return (
                    <div key={i} className="mono" style={{ fontSize: 12, color: s.success ? 'var(--ok)' : 'var(--red)' }}>
                      passo {s.step_id} · {s.summary} · {s.success ? 'ok' : 'falhou'}
                    </div>
                  )
                })}
              </div>
            </Card>
          )}
        </div>

        <div className="stack">
          <Card>
            <strong>Consenso ponderado</strong>
            {consensus ? (
              <>
                <div style={{ fontSize: 22, fontWeight: 700, margin: '6px 0' }}>{consensus.strength.toFixed(2)}</div>
                <ProgressBar value={consensus.strength} />
                <p style={{ fontSize: 12, color: 'var(--muted)', marginTop: 8 }}>
                  decisão: {consensus.decision_maker} ·{' '}
                  {consensus.requires_human ? 'consenso fraco — HITL' : 'consenso forte — sem humano'}
                </p>
              </>
            ) : (
              <p style={{ fontSize: 13, color: 'var(--muted)' }}>aguardando consenso…</p>
            )}
          </Card>

          <Card accentBorder="var(--amber)">
            <strong>Gate HITL</strong>
            {pendingHitl ? (
              <>
                <p style={{ fontSize: 13, color: 'var(--muted)' }}>
                  {pendingHitl.reason} · confiança {pendingHitl.confidence.toFixed(2)}
                </p>
                <div className="row">
                  <Button variant="danger" onClick={() => void handleGate(false)}>
                    Rejeitar
                  </Button>
                  <Button variant="primary" onClick={() => void handleGate(true)}>
                    Aprovar
                  </Button>
                </div>
              </>
            ) : (
              <p style={{ fontSize: 13, color: 'var(--muted)' }}>
                {hitlEvents.length > 0 ? '✓ resolvido' : 'nenhum pedido de escalonamento humano nesta rodada.'}
              </p>
            )}
          </Card>

          <Card>
            <strong>Fallback progressivo</strong>
            <p style={{ fontSize: 13, color: 'var(--muted)' }}>squad → agente-único → safe-mode</p>
            <div className="row" style={{ fontSize: 12, color: active ? 'var(--ok)' : 'var(--muted)' }}>
              <span className="pulse-dot" /> {active ? 'squad em execução' : taskId ? 'stream encerrado' : 'ocioso'}
            </div>
          </Card>
        </div>
      </div>
    </div>
  )
}
