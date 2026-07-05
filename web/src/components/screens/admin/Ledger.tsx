import { useEffect, useState } from 'react'
import { Card } from '../../primitives/Card'
import { Badge } from '../../primitives/Badge'
import { Button } from '../../primitives/Button'
import { Table } from '../../primitives/Table'
import { useAsyncAction } from '../../../hooks/useAsyncAction'
import { useToast } from '../../primitives/Toast'
import { getLedger, verifyChain } from '../../../api/ledger'
import type { LedgerEntry } from '../../../types/domain'

export function Ledger() {
  const toast = useToast()
  const [entries, setEntries] = useState<LedgerEntry[]>([])
  const [actorFilter, setActorFilter] = useState<string>('todos')
  const verify = useAsyncAction(verifyChain)

  useEffect(() => {
    void getLedger().then(setEntries)
  }, [])

  const actors = ['todos', ...Array.from(new Set(entries.map((e) => e.actor)))]
  const rows = actorFilter === 'todos' ? entries : entries.filter((e) => e.actor === actorFilter)

  async function handleVerify() {
    try {
      const result = await verify.run()
      toast.push('success', `cadeia íntegra — ${result.verified} entradas verificadas`)
    } catch {
      toast.push('error', 'falha ao verificar integridade')
    }
  }

  return (
    <div className="stack">
      <Card accentBorder="var(--ok)">
        <strong>Cadeia de hash íntegra</strong> — {entries[0]?.seq ?? '—'} entradas verificadas · política Nada Fake
        <div style={{ marginTop: 8 }}>
          <Button onClick={() => void handleVerify()} disabled={verify.state.status === 'loading'}>
            {verify.state.status === 'loading' ? 'verificando…' : 'verificar integridade'}
          </Button>
        </div>
      </Card>

      <div className="row">
        {actors.map((a) => (
          <button
            key={a}
            onClick={() => setActorFilter(a)}
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

      <Table
        rowKey={(e) => String(e.seq)}
        rows={rows}
        columns={[
          { key: 'seq', header: 'seq', render: (e) => e.seq },
          { key: 'ts', header: 'ts', render: (e) => <span className="mono">{e.ts}</span> },
          { key: 'actor', header: 'ator', render: (e) => <span style={{ color: `var(--${e.actorColor})` }}>{e.actor}</span> },
          { key: 'action', header: 'ação', render: (e) => e.action },
          { key: 'hash', header: 'hash', render: (e) => <span className="mono">{e.hashPrev}→{e.hashCurr}</span> },
          { key: 'flag', header: 'flags', render: (e) => (e.flag ? <Badge color="var(--wire)">{e.flag}</Badge> : '') },
        ]}
      />
    </div>
  )
}
