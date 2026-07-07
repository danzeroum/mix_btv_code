import { useEffect, useState } from 'react'
import { Card } from '../../primitives/Card'
import { Badge } from '../../primitives/Badge'
import { Button } from '../../primitives/Button'
import { Table } from '../../primitives/Table'
import { AsyncStatus } from '../../primitives/AsyncStatus'
import { useAsyncAction } from '../../../hooks/useAsyncAction'
import { getLedger, verifyChain } from '../../../api/ledger'
import type { LedgerEntry } from '../../../types/domain'

/** Cor por prefixo de `actor` — `web:*` (override feito pelo navegador),
 * `forge-cli:*` (sessão de CLI/TUI/squad); qualquer outro (ex.: futuro
 * agente Python) cai no terceiro tom. Puramente cosmético no cliente — o
 * dado real é a string `actor` em si, não essa cor. */
function actorColor(actor: string): string {
  if (actor.startsWith('web:')) return 'var(--wire)'
  if (actor.startsWith('forge-cli:')) return 'var(--ok)'
  return 'var(--py)'
}

function shortHash(hash: string): string {
  return hash ? hash.slice(0, 8) : '(gênese)'
}

function compactPayload(payload: unknown): string {
  const json = JSON.stringify(payload)
  return json.length > 60 ? `${json.slice(0, 60)}…` : json
}

export function Ledger() {
  const [entries, setEntries] = useState<LedgerEntry[]>([])
  const [actors, setActors] = useState<string[]>([])
  const [actorFilter, setActorFilter] = useState<string>('todos')
  const [selected, setSelected] = useState<LedgerEntry | null>(null)
  const listState = useAsyncAction(getLedger)
  const verify = useAsyncAction(verifyChain)

  async function load(actor?: string) {
    const result = await listState.run(50, actor)
    setEntries(result)
    // Lista de atores só é (re)derivada da busca SEM filtro — trocar de
    // filtro não pode fazer os outros botões desaparecerem.
    if (!actor) {
      setActors(Array.from(new Set(result.map((e) => e.actor))))
    }
    return result
  }

  useEffect(() => {
    void load()
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [])

  async function handleActorFilter(actor: string) {
    setActorFilter(actor)
    // Refaz a busca no backend (não corta a lista já carregada) — um ator
    // raro fora da janela padrão de 50 só aparece assim.
    await load(actor === 'todos' ? undefined : actor)
  }

  return (
    <div className="stack">
      <Card accentBorder={verify.state.status === 'success' && !verify.state.data.ok ? 'var(--red)' : 'var(--line2)'}>
        {verify.state.status === 'success' ? (
          verify.state.data.ok ? (
            <strong style={{ color: 'var(--ok)' }}>
              ✓ cadeia íntegra — {verify.state.data.verified} entradas verificadas
            </strong>
          ) : (
            <strong style={{ color: 'var(--red)' }}>✗ cadeia corrompida — {verify.state.data.error}</strong>
          )
        ) : verify.state.status === 'error' ? (
          <strong style={{ color: 'var(--red)' }}>✗ falha ao verificar: {verify.state.error.message}</strong>
        ) : (
          <span style={{ color: 'var(--muted)' }}>
            {entries.length} entrada(s) carregada(s) · integridade ainda não verificada nesta sessão
          </span>
        )}
        <div style={{ marginTop: 8 }}>
          <Button onClick={() => void verify.run()} disabled={verify.state.status === 'loading'}>
            {verify.state.status === 'loading' ? 'verificando…' : 'verificar integridade'}
          </Button>
        </div>
      </Card>

      <div className="row">
        {['todos', ...actors].map((a) => (
          <button
            key={a}
            onClick={() => void handleActorFilter(a)}
            style={{
              fontSize: 12,
              padding: '4px 10px',
              borderRadius: 6,
              border: `1px solid ${a === actorFilter ? 'var(--ink)' : 'var(--line)'}`,
              background: a === actorFilter ? 'var(--panel2)' : 'transparent',
              color: 'var(--ink)',
            }}
          >
            {a}
          </button>
        ))}
      </div>

      <AsyncStatus state={listState.state} onRetry={() => void load(actorFilter === 'todos' ? undefined : actorFilter)}>
        {() => (
          <Table
            rowKey={(e) => String(e.seq)}
            rows={entries}
            onRowClick={(e) => setSelected(e)}
            columns={[
              { key: 'seq', header: 'seq', render: (e) => e.seq },
              { key: 'ts', header: 'ts', render: (e) => <span className="mono">{e.ts}</span> },
              {
                key: 'actor',
                header: 'ator',
                render: (e) => <span style={{ color: actorColor(e.actor) }}>{e.actor}</span>,
              },
              { key: 'kind', header: 'evento', render: (e) => e.kind },
              {
                key: 'payload',
                header: 'payload',
                render: (e) => (
                  <span className="mono" style={{ fontSize: 11, color: 'var(--muted)' }}>
                    {compactPayload(e.payload)}
                  </span>
                ),
              },
              {
                key: 'hash',
                header: 'hash',
                render: (e) => (
                  <span className="mono">
                    {shortHash(e.prev_hash)}→{shortHash(e.entry_hash)}
                  </span>
                ),
              },
              {
                key: 'flags',
                header: 'flags',
                render: (e) => (
                  <span className="row" style={{ gap: 4 }}>
                    {e.override?.marked && <Badge color="var(--wire)">override</Badge>}
                    {e.fake_marker && <Badge color="var(--amber)">fake: {e.fake_marker}</Badge>}
                  </span>
                ),
              },
            ]}
          />
        )}
      </AsyncStatus>

      {selected && (
        <Card accentBorder="var(--line2)">
          <div className="row" style={{ justifyContent: 'space-between' }}>
            <strong>entrada #{selected.seq}</strong>
            <button onClick={() => setSelected(null)} style={{ background: 'none', border: 'none', color: 'var(--muted)' }}>
              ✕ fechar
            </button>
          </div>
          <pre
            className="mono"
            style={{ background: '#0a0d12', border: '1px solid var(--line)', borderRadius: 6, padding: 8, fontSize: 11, marginTop: 8 }}
          >
            {JSON.stringify(selected, null, 2)}
          </pre>
        </Card>
      )}
    </div>
  )
}
