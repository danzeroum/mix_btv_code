import { Card } from '../../primitives/Card'
import { Button } from '../../primitives/Button'
import { useAppDispatch } from '../../../state/AppContext'
import { useToast } from '../../primitives/Toast'
import { copyToClipboard, DOCTOR_OUTPUT, ENV_KEYS } from '../../../api/onboarding'

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
            {ENV_KEYS.map((k) => (
              <div key={k.name} className="row mono" style={{ fontSize: 12, justifyContent: 'space-between' }}>
                <span>{k.name}</span>
                <span style={{ color: k.detected ? 'var(--ok)' : 'var(--muted)' }}>
                  {k.detected ? `✓ definida (${k.masked})` : 'ausente · fallback'}
                </span>
              </div>
            ))}
          </div>
          <p style={{ fontSize: 12, color: 'var(--faint)', marginTop: 8 }}>
            🔑 keys vivem SÓ no processo Rust.
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
          <div style={{ color: 'var(--muted)' }}>$ forge init</div>
          {DOCTOR_OUTPUT.map((line) => (
            <div key={line} style={{ color: line.startsWith('✓') ? 'var(--ok)' : 'var(--faint)' }}>
              {line}
            </div>
          ))}
          <span className="cursor-blink">▸</span>
        </div>
      </Card>
    </div>
  )
}
