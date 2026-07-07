import { useEffect } from 'react'
import { Card } from '../../primitives/Card'
import { Button } from '../../primitives/Button'
import { AsyncStatus } from '../../primitives/AsyncStatus'
import { useAppDispatch } from '../../../state/AppContext'
import { useToast } from '../../primitives/Toast'
import { useAsyncAction } from '../../../hooks/useAsyncAction'
import { copyToClipboard, fetchDoctor } from '../../../api/onboarding'

function CodeBlock({ code }: { code: string }) {
  const toast = useToast()
  return (
    <div
      className="row mono"
      style={{
        background: '#0a0d12',
        border: '1px solid var(--line)',
        borderRadius: 8,
        padding: '8px 10px',
        fontSize: 12,
        justifyContent: 'space-between',
      }}
    >
      <span>{code}</span>
      <button
        onClick={() => {
          void copyToClipboard(code).then(() => toast.push('success', 'copiado'))
        }}
        style={{ background: 'transparent', border: 'none', color: 'var(--muted)' }}
      >
        ⧉ copiar
      </button>
    </div>
  )
}

export function Onboarding() {
  const dispatch = useAppDispatch()
  const doctorState = useAsyncAction(fetchDoctor)

  useEffect(() => {
    void doctorState.run()
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [])

  return (
    <div className="grid" style={{ gridTemplateColumns: '1fr 1.1fr' }}>
      <div className="stack">
        <Card accentBorder="var(--ok)">
          <div className="row" style={{ fontWeight: 600 }}>
            ✓ Instalar
          </div>
          <p style={{ fontSize: 13, color: 'var(--muted)' }}>Via cargo ou binário.</p>
          <CodeBlock code="$ cargo install forge" />
        </Card>

        <Card accentBorder="var(--amber)">
          <div className="row" style={{ fontWeight: 600 }}>
            ◑ Chaves de API <span style={{ fontSize: 11, color: 'var(--amber)' }}>etapa atual</span>
          </div>
          <div className="stack" style={{ marginTop: 8 }}>
            <AsyncStatus state={doctorState.state} onRetry={() => void doctorState.run()}>
              {(checks) => {
                const providers = checks.find((c) => c.id === 'providers')
                return (
                  <div className="row mono" style={{ fontSize: 12, justifyContent: 'space-between' }}>
                    <span>providers</span>
                    <span style={{ color: providers?.ok ? 'var(--ok)' : 'var(--muted)' }}>
                      {providers?.detail ?? '—'}
                    </span>
                  </div>
                )
              }}
            </AsyncStatus>
          </div>
          <p style={{ fontSize: 12, color: 'var(--faint)', marginTop: 8 }}>
            🔑 keys vivem SÓ no processo Rust — detalhe por provider na tela Providers.
          </p>
        </Card>

        <Card>
          <div className="row" style={{ fontWeight: 600, color: 'var(--muted)' }}>
            Sidecar Python (opcional)
          </div>
          <p style={{ fontSize: 13, color: 'var(--muted)' }}>
            Com <code>uv</code> instalado, PromptForge e squad sobem sozinhos.
          </p>
        </Card>

        <Card>
          <div className="row" style={{ fontWeight: 600 }}>
            Primeiro comando
          </div>
          <CodeBlock code={'$ forge run "explique a estrutura deste repo" --agent plan'} />
        </Card>

        <Button variant="primary" onClick={() => dispatch({ type: 'SET_SCREEN', screen: 'sessao' })}>
          concluir setup →
        </Button>
      </div>

      <Card style={{ background: 'var(--term)' }}>
        <div className="mono" style={{ fontSize: 12.5, lineHeight: 1.9 }}>
          <div style={{ color: 'var(--muted)' }}>$ forge doctor</div>
          <AsyncStatus state={doctorState.state} onRetry={() => void doctorState.run()}>
            {(checks) => (
              <>
                {checks.map((c) => (
                  <div key={c.id} style={{ color: c.ok ? 'var(--ok)' : 'var(--faint)' }}>
                    {c.ok ? '✓' : '○'} {c.detail}
                  </div>
                ))}
              </>
            )}
          </AsyncStatus>
          <span className="cursor-blink">▸</span>
        </div>
      </Card>
    </div>
  )
}
