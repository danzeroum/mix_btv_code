import { useEffect } from 'react'
import { Card } from '../../primitives/Card'
import { Table } from '../../primitives/Table'
import { AsyncStatus } from '../../primitives/AsyncStatus'
import { useAsyncAction } from '../../../hooks/useAsyncAction'
import { fetchLspServers } from '../../../api/lsp'

export function Lsp() {
  const state = useAsyncAction(fetchLspServers)

  useEffect(() => {
    void state.run()
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [])

  return (
    <div className="stack">
      <AsyncStatus state={state.state} onRetry={() => void state.run()}>
        {(servers) =>
          servers.length === 0 ? (
            <Card>
              <span style={{ fontSize: 12, color: 'var(--faint)' }}>
                nenhum language server declarado em <span className="mono">.forge/lsp.toml</span>.
              </span>
            </Card>
          ) : (
            <Table
              rowKey={(s) => s.id}
              rows={servers}
              columns={[
                { key: 'id', header: 'id', render: (s) => s.id },
                { key: 'command', header: 'comando', render: (s) => <span className="mono">{s.command}</span> },
                { key: 'args', header: 'args', render: (s) => s.args.join(' ') },
                {
                  key: 'status',
                  header: 'status',
                  render: () => <span style={{ color: 'var(--faint)' }}>declarado, não iniciado</span>,
                },
              ]}
            />
          )
        }
      </AsyncStatus>

      <div style={{ fontSize: 11, color: 'var(--faint)' }}>
        registro é preguiçoso: o processo do language server só sobe no primeiro uso real de uma sessão de código —
        esta tela nunca sonda sob demanda (isso quebraria a mesma propriedade que o registro lazy garante).
      </div>
    </div>
  )
}
