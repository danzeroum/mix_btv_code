import { useState } from 'react'
import { useAppDispatch, useAppState } from '../../../state/AppContext'
import { useAsyncAction } from '../../../hooks/useAsyncAction'
import { useToast } from '../../primitives/Toast'
import { primaryModelName } from '../../../api/models'
import {
  INITIAL_TRANSCRIPT,
  SESSION_HEADER,
  TOOL_POLICIES,
  streamAgent,
  toggleToolPolicy,
  type TranscriptTurn,
} from '../../../api/session'

const PREFIX_COLOR: Record<TranscriptTurn['kind'], string> = {
  user: 'var(--py)',
  agent: 'var(--amber)',
  tool: 'var(--muted)',
  lint: 'var(--wire)',
  diff: 'var(--ink)',
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
  const [transcript, setTranscript] = useState(INITIAL_TRANSCRIPT)
  const [input, setInput] = useState('')
  const [policies, setPolicies] = useState(TOOL_POLICIES)
  const send = useAsyncAction(streamAgent)

  async function handleSend() {
    if (!input.trim()) return
    const text = input
    setInput('')
    try {
      const newTurns = await send.run(text)
      setTranscript((prev) => [...prev, ...newTurns])
    } catch {
      toast.push('error', 'falha ao enviar mensagem (mock)')
    }
  }

  async function handleTogglePolicy(tool: string) {
    try {
      const updated = await toggleToolPolicy(tool)
      setPolicies((prev) => prev.map((p) => (p.tool === updated.tool ? updated : p)))
    } catch {
      toast.push('error', 'falha ao alternar política da ferramenta')
    }
  }

  return (
    <div style={{ display: 'flex', gap: 16, height: 'calc(100% - 50px)' }}>
      <div style={{ flex: 1, display: 'flex', flexDirection: 'column', minWidth: 0 }}>
        <div className="mono" style={{ fontSize: 11.5, color: 'var(--faint)', marginBottom: 8 }}>
          {primaryModelName(modelTier)} · agente {agentProfile} · {SESSION_HEADER.provider} · cache{' '}
          {SESSION_HEADER.cacheOn ? 'on' : 'off'} · sessão {SESSION_HEADER.sessionId}
        </div>

        <div className="stack" style={{ flex: 1, overflow: 'auto', paddingRight: 8 }}>
          {transcript.map((t) => (
            <TurnView key={t.id} turn={t} />
          ))}
          {send.state.status === 'loading' && (
            <div className="mono cursor-blink" style={{ color: 'var(--amber)', fontSize: 13 }}>
              forge ▸ …
            </div>
          )}
        </div>

        <div
          className="mono"
          style={{ borderTop: '1px solid var(--line)', padding: '8px 0', fontSize: 11.5, color: 'var(--faint)' }}
        >
          ⋯ concluído em 6 passos · 8 mensagens persistidas · ledger íntegro: 12 entradas ✓ · cache hit 41%
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
          <div style={{ fontSize: 11, color: 'var(--faint)', marginBottom: 6 }}>FERRAMENTAS</div>
          {policies.map((p) => (
            <div key={p.tool} className="row" style={{ justifyContent: 'space-between', padding: '4px 0' }}>
              <button
                onClick={() => dispatch({ type: 'SET_SCREEN', screen: 'skills' })}
                title="abrir política em Skills & Permissões"
                style={{ background: 'transparent', border: 'none', padding: 0, color: 'var(--ink)', fontSize: 12 }}
              >
                {p.tool}
              </button>
              <button
                onClick={() => void handleTogglePolicy(p.tool)}
                title="alternar allow/ask"
                style={{
                  background: 'transparent',
                  border: 'none',
                  padding: 0,
                  fontSize: 12,
                  color: p.policy === 'allow' ? 'var(--ok)' : 'var(--amber)',
                }}
              >
                {p.policy}
              </button>
            </div>
          ))}
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
