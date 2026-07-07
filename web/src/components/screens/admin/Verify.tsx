import { useCallback, useEffect, useState } from 'react'
import { Card } from '../../primitives/Card'
import { Button } from '../../primitives/Button'
import { Badge } from '../../primitives/Badge'
import { Gauge } from '../../primitives/Gauge'
import { ProgressBar } from '../../primitives/ProgressBar'
import { usePolling } from '../../../hooks/usePolling'
import { useToast } from '../../primitives/Toast'
import {
  REVIEWERS,
  VALUE_GATE,
  VALUE_SCORE,
  fetchVerifyStatus,
  startVerifyRun,
  type VerificationEvidence,
  type VerifyStatus,
} from '../../../api/verify'

function VerifyPoller({ runId, onUpdate }: { runId: string; onUpdate: (status: VerifyStatus) => void }) {
  const state = usePolling(() => fetchVerifyStatus(runId), 500)
  useEffect(() => {
    if (state.status === 'success') onUpdate(state.data)
  }, [state, onUpdate])
  return null
}

export function Verify() {
  const toast = useToast()
  const [activeRunId, setActiveRunId] = useState<string | null>(null)
  const [progress, setProgress] = useState<{ step: number; total: number } | null>(null)
  const [evidence, setEvidence] = useState<VerificationEvidence | null>(null)
  const [starting, setStarting] = useState(false)
  const [expandedStep, setExpandedStep] = useState<string | null>(null)
  const [expandedReviewer, setExpandedReviewer] = useState<string | null>(null)

  const handleStatusUpdate = useCallback(
    (status: VerifyStatus) => {
      if (status.status === 'running') {
        setProgress({ step: status.step, total: status.total })
        return
      }
      setEvidence(status.evidence)
      setProgress(null)
      setActiveRunId(null)
      toast.push(status.evidence.verdict === 'pass' ? 'success' : 'error', `pipeline /verify: ${status.evidence.verdict}`)
    },
    [toast],
  )

  async function handleRun() {
    setStarting(true)
    try {
      const { run_id } = await startVerifyRun()
      setProgress({ step: 0, total: 0 })
      setActiveRunId(run_id)
    } catch {
      toast.push('error', 'falha ao iniciar /verify')
    } finally {
      setStarting(false)
    }
  }

  const isRunning = activeRunId !== null

  return (
    <div className="grid grid-2">
      {activeRunId && <VerifyPoller runId={activeRunId} onUpdate={handleStatusUpdate} />}
      <Card>
        <div className="row" style={{ justifyContent: 'space-between' }}>
          <strong>Pipeline /verify</strong>
          <Button onClick={() => void handleRun()} disabled={starting || isRunning}>
            {isRunning ? 'rodando…' : starting ? 'iniciando…' : 'rodar /verify'}
          </Button>
        </div>
        {isRunning && (
          <div style={{ fontSize: 12, color: 'var(--muted)', marginTop: 8 }}>
            {progress && progress.total > 0 ? `passo ${progress.step} de ${progress.total}…` : 'iniciando pipeline…'}
          </div>
        )}
        {!evidence && !isRunning && (
          <div style={{ fontSize: 12, color: 'var(--faint)', marginTop: 8 }}>
            nenhuma execução ainda nesta sessão do dashboard.
          </div>
        )}
        {evidence && (
          <div className="stack" style={{ marginTop: 10 }}>
            {evidence.steps.map((s) => (
              <div key={s.name}>
                <button
                  onClick={() => setExpandedStep(expandedStep === s.name ? null : s.name)}
                  className="row"
                  style={{
                    width: '100%',
                    justifyContent: 'space-between',
                    fontSize: 13,
                    background: 'transparent',
                    border: 'none',
                    color: 'var(--ink)',
                    padding: 0,
                  }}
                >
                  <span>
                    <span style={{ color: s.exit_code === 0 ? 'var(--ok)' : 'var(--red)' }}>
                      {s.exit_code === 0 ? '✓' : '✗'}
                    </span>{' '}
                    {s.name}
                  </span>
                  <span style={{ color: 'var(--muted)', fontSize: 12 }}>
                    {s.duration_ms}ms · {s.findings.length} finding(s) {expandedStep === s.name ? '▾' : '▸'}
                  </span>
                </button>
                {expandedStep === s.name && (
                  <pre
                    className="mono"
                    style={{
                      background: '#0a0d12',
                      border: '1px solid var(--line)',
                      borderRadius: 6,
                      padding: 8,
                      fontSize: 11,
                      marginTop: 4,
                      overflowX: 'auto',
                    }}
                  >
                    {JSON.stringify(s, null, 2)}
                  </pre>
                )}
              </div>
            ))}
            <div style={{ fontSize: 11, color: 'var(--faint)' }}>
              run <span className="mono">{evidence.run_id}</span> · <span className="mono">{evidence.git_sha.slice(0, 8)}</span> ·
              veredito: <strong>{evidence.verdict}</strong>
            </div>
          </div>
        )}
        <p style={{ fontSize: 11, color: 'var(--faint)', marginTop: 10 }}>
          self-hosting: este pipeline roda sobre o próprio Forge (Fase 5). Job em memória — reinício do dashboard
          perde uma execução em andamento.
        </p>
      </Card>

      <Card>
        <strong>Review por valor</strong>
        <div style={{ display: 'flex', justifyContent: 'center', margin: '12px 0' }}>
          <Gauge value={VALUE_SCORE} gate={VALUE_GATE} label={`gate > ${VALUE_GATE.toFixed(2)}`} />
        </div>
        <div className="row" style={{ justifyContent: 'center', marginBottom: 10 }}>
          <Badge color="var(--ok)">CERTIFICADO</Badge>
        </div>
        <div className="stack">
          {REVIEWERS.map((r) => (
            <div key={r.name}>
              <button
                onClick={() => setExpandedReviewer(expandedReviewer === r.name ? null : r.name)}
                className="row"
                style={{
                  width: '100%',
                  justifyContent: 'space-between',
                  fontSize: 12,
                  background: 'transparent',
                  border: 'none',
                  color: 'var(--ink)',
                  padding: 0,
                }}
              >
                <span>{r.name}</span>
                <span className="mono">{r.score.toFixed(2)}</span>
              </button>
              <ProgressBar value={r.score} />
              {expandedReviewer === r.name && (
                <p style={{ fontSize: 11, color: 'var(--muted)', marginTop: 4 }}>{r.detail}</p>
              )}
            </div>
          ))}
        </div>
      </Card>
    </div>
  )
}
