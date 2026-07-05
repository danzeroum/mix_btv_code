import { useState } from 'react'
import { Card } from '../../primitives/Card'
import { Button } from '../../primitives/Button'
import { Badge } from '../../primitives/Badge'
import { Gauge } from '../../primitives/Gauge'
import { ProgressBar } from '../../primitives/ProgressBar'
import { useAsyncAction } from '../../../hooks/useAsyncAction'
import { useToast } from '../../primitives/Toast'
import { REVIEWERS, VALUE_GATE, VALUE_SCORE, VERIFY_STEPS, runVerify } from '../../../api/verify'

export function Verify() {
  const toast = useToast()
  const [steps, setSteps] = useState(VERIFY_STEPS)
  const [expandedStep, setExpandedStep] = useState<string | null>(null)
  const [expandedReviewer, setExpandedReviewer] = useState<string | null>(null)
  const verify = useAsyncAction(runVerify)

  async function handleRun() {
    try {
      const result = await verify.run()
      setSteps(result)
      toast.push('success', 'pipeline /verify concluído')
    } catch {
      toast.push('error', 'falha ao rodar /verify')
    }
  }

  return (
    <div className="grid grid-2">
      <Card>
        <div className="row" style={{ justifyContent: 'space-between' }}>
          <strong>Pipeline /verify</strong>
          <Button onClick={() => void handleRun()} disabled={verify.state.status === 'loading'}>
            {verify.state.status === 'loading' ? 'rodando…' : 'rodar /verify'}
          </Button>
        </div>
        <div className="stack" style={{ marginTop: 10 }}>
          {steps.map((s) => (
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
                  <span style={{ color: s.ok ? 'var(--ok)' : 'var(--red)' }}>{s.ok ? '✓' : '✗'}</span> {s.name}
                </span>
                <span style={{ color: 'var(--muted)', fontSize: 12 }}>
                  {s.detail} {expandedStep === s.name ? '▾' : '▸'}
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
                  {JSON.stringify(s.evidence, null, 2)}
                </pre>
              )}
            </div>
          ))}
        </div>
        <p style={{ fontSize: 11, color: 'var(--faint)', marginTop: 10 }}>
          self-hosting: este pipeline roda sobre o próprio Forge (Fase 5).
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
