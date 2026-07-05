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
            <div key={s.name} className="row" style={{ justifyContent: 'space-between', fontSize: 13 }}>
              <span>
                <span style={{ color: s.ok ? 'var(--ok)' : 'var(--red)' }}>{s.ok ? '✓' : '✗'}</span> {s.name}
              </span>
              <span style={{ color: 'var(--muted)', fontSize: 12 }}>{s.detail}</span>
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
              <div className="row" style={{ justifyContent: 'space-between', fontSize: 12 }}>
                <span>{r.name}</span>
                <span className="mono">{r.score.toFixed(2)}</span>
              </div>
              <ProgressBar value={r.score} />
            </div>
          ))}
        </div>
      </Card>
    </div>
  )
}
