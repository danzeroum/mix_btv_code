import { useEffect, useState } from 'react'
import { useAppDispatch, useAppState } from '../../../state/AppContext'
import { useSession } from '../../../state/SessionContext'
import { useToast } from '../../primitives/Toast'
import { useAsyncAction } from '../../../hooks/useAsyncAction'
import { AsyncStatus } from '../../primitives/AsyncStatus'
import { primaryModelName } from '../../../api/models'
import { fetchMatrix } from '../../../api/permissions'
import { fetchProviders } from '../../../api/providers'
import type { PermissionMatrixDecision } from '../../../types/domain'
import type { TranscriptTurn } from '../../../api/session'

const PREFIX_COLOR: Record<TranscriptTurn['kind'], string> = {
  user: 'var(--py)',
  agent: 'var(--amber)',
  tool: 'var(--muted)',
  lint: 'var(--wire)',
  diff: 'var(--ink)',
}

/** Mesmas cores de `Skills.tsx`'s `DECISION_COLOR` — 3 entradas, não vale
 * cross-importar de uma tela pra outra por isso. */
const DECISION_COLOR: Record<PermissionMatrixDecision, string> = {
  allow: 'var(--ok)',
  ask: 'var(--amber)',
  deny: 'var(--red)',
}

function TurnView({ turn }: { turn: TranscriptTurn }) {
  if (turn.kind === 'diff') {
    return (
      <pre
        className="mono"
        style={{
          background: '#0a0d12',
          border: '1px solid var(--line)',
          borderRadius: 8,
          padding: 10,
          fontSize: 12,
          overflowX: 'auto',
        }}
      >
        {turn.text.split('\n').map((line, i) => (
          <div
            key={i}
            style={{
              color: line.startsWith('+') ? 'var(--ok)' : line.startsWith('-') ? 'var(--red)' : 'var(--faint)',
            }}
          >
            {line}
          </div>
        ))}
      </pre>
    )
  }
  const icon = turn.kind === 'tool' ? (turn.toolStatus === 'error' ? '✗' : '✓') + ' ' : ''
  const label = turn.kind === 'user' ? 'você ▸' : turn.kind === 'agent' ? 'forge ▸' : ''
  return (
    <div className="mono" style={{ fontSize: 13, lineHeight: 1.65 }}>
      {label && <span style={{ color: PREFIX_COLOR[turn.kind], fontWeight: 600 }}>{label} </span>}
      {turn.kind === 'tool' && <span style={{ color: PREFIX_COLOR.tool }}>{icon}</span>}
      <span>{turn.text}</span>
    </div>
  )
}

export function Sessao() {
  const dispatch = useAppDispatch()
  const toast = useToast()
  const { modelTier, agentProfile } = useAppState()
  const { sessionId, transcript, streamingText, busy, ledgerVerified, lastError, sendMessage } = useSession()
  const [input, setInput] = useState('')
  const matrixState = useAsyncAction(fetchMatrix)
  const providersState = useAsyncAction(fetchProviders)

  useEffect(() => {
    void matrixState.run()
    void providersState.run()
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [])

  async function handleSend() {
    if (!input.trim() || busy) return
    const text = input
    setInput('')
    try {
      await sendMessage(text, { model: primaryModelName(modelTier), agent: agentProfile })
    } catch {
      toast.push('error', 'falha ao enviar mensagem')
    }
  }

  const activeProvider =
    providersState.state.status === 'success'
      ? (providersState.state.data.find((p) => p.configured)?.id ?? 'nenhum provider configurado')
      : '…'

  return (
    <div style={{ display: 'flex', gap: 16, height: 'calc(100% - 50px)' }}>
      <div style={{ flex: 1, display: 'flex', flexDirection: 'column', minWidth: 0 }}>
        <div className="mono" style={{ fontSize: 11.5, color: 'var(--faint)', marginBottom: 8 }}>
          {primaryModelName(modelTier)} · agente {agentProfile} · {activeProvider} · sessão {sessionId.slice(0, 8)}
        </div>

        <div className="stack" style={{ flex: 1, overflow: 'auto', paddingRight: 8 }}>
          {transcript.map((t) => (
            <TurnView key={t.id} turn={t} />
          ))}
          {busy && (
            <div className="mono cursor-blink" style={{ color: 'var(--amber)', fontSize: 13 }}>
              forge ▸ {streamingText || '…'}
            </div>
          )}
        </div>

        <div
          className="mono"
          style={{ borderTop: '1px solid var(--line)', padding: '8px 0', fontSize: 11.5, color: 'var(--faint)' }}
        >
          {lastError
            ? `✗ erro: ${lastError}`
            : ledgerVerified != null
              ? `⋯ ledger íntegro: ${ledgerVerified} entrada(s) ✓`
              : '⋯ nenhum turno concluído ainda nesta sessão'}
        </div>

        <div className="row" style={{ borderTop: '1px solid var(--line)', paddingTop: 8 }}>
          <span style={{ color: 'var(--amber)' }}>›</span>
          <input
            value={input}
            onChange={(e) => setInput(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === 'Enter') void handleSend()
              if (e.key === 'Tab') {
                e.preventDefault()
                dispatch({ type: 'SET_SCREEN', screen: 'modelo' })
              }
            }}
            placeholder="mensagem para o agente…"
            style={{
              flex: 1,
              background: 'transparent',
              border: 'none',
              color: 'var(--ink)',
              fontSize: 13,
              outline: 'none',
            }}
          />
          <span style={{ fontSize: 11, color: 'var(--faint)' }}>Enter envia · Esc sai · Tab modelo</span>
        </div>
      </div>

      <aside style={{ width: 210, flexShrink: 0 }} className="stack">
        <div>
          <div style={{ fontSize: 11, color: 'var(--faint)', marginBottom: 6 }}>
            FERRAMENTAS <span style={{ color: 'var(--faint)', fontWeight: 400 }}>· perfil {agentProfile}</span>
          </div>
          <AsyncStatus state={matrixState.state} onRetry={() => void matrixState.run()}>
            {(matrix) => (
              <>
                {matrix.map((row) => (
                  <div key={row.tool} className="row" style={{ justifyContent: 'space-between', padding: '4px 0' }}>
                    <button
                      onClick={() => dispatch({ type: 'SET_SCREEN', screen: 'skills' })}
                      title="ver/mudar política em Skills & Permissões"
                      style={{ background: 'transparent', border: 'none', padding: 0, color: 'var(--ink)', fontSize: 12 }}
                    >
                      {row.tool}
                    </button>
                    <span style={{ fontSize: 12, color: DECISION_COLOR[row[agentProfile]] }}>
                      {row[agentProfile]}
                    </span>
                  </div>
                ))}
              </>
            )}
          </AsyncStatus>
        </div>

        <div>
          <div style={{ fontSize: 11, color: 'var(--faint)', marginBottom: 6 }}>CONTEXTO</div>
          <div style={{ fontSize: 12 }}>época 2 · compaction 1×</div>
          <div style={{ fontSize: 12, marginBottom: 4 }}>janela 14k/200k · 7%</div>
          <div style={{ background: 'var(--line)', borderRadius: 999, height: 6 }}>
            <div style={{ width: '7%', height: '100%', background: 'var(--rust)', borderRadius: 999 }} />
          </div>
        </div>

        <div>
          <div style={{ fontSize: 11, color: 'var(--faint)', marginBottom: 6 }}>ATALHOS</div>
          <div style={{ fontSize: 12, color: 'var(--muted)' }}>↑↓ histórico · ^C cancelar · /compact · /prompt</div>
        </div>
      </aside>
    </div>
  )
}
