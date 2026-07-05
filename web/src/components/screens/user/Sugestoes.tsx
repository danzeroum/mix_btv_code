import { Card } from '../../primitives/Card'
import { Button } from '../../primitives/Button'
import { useAppDispatch } from '../../../state/AppContext'

const PROPOSALS = [
  { title: 'Revisor de diff', tag: 'review', desc: 'inspeciona cada diff antes de aplicar.', anchor: 'ancora: forge-cli tool.edit' },
  { title: 'Replay de sessão', tag: 'auditoria', desc: 'reproduz uma sessão a partir do ledger.', anchor: 'ancora: ledger hash-chain + sessions.db' },
  { title: 'Aprovação em lote', tag: 'permissão', desc: 'aprova várias permissões pendentes de uma vez.', anchor: 'ancora: forge-core PermissionClient' },
  { title: 'Modo watch', tag: 'sessão', desc: 'observa arquivos e sugere ações automaticamente.', anchor: 'ancora: forge-cli watch (futuro)' },
  { title: 'A/B de prompts', tag: 'promptforge', desc: 'compara variações de prompt lado a lado.', anchor: 'ancora: forge_promptforge.hashing' },
  { title: 'Mapa de memória do squad', tag: 'squad', desc: 'visualiza o que cada agente lembra.', anchor: 'ancora: forge_squad.memory / forgetting' },
]

export function Sugestoes() {
  const dispatch = useAppDispatch()

  return (
    <div className="stack">
      <Card accentBorder="var(--py)">
        <strong>Squad Designer — desenhe o workflow, o código segue</strong>
        <p style={{ fontSize: 13, color: 'var(--muted)' }}>
          Edite o grafo de agentes e conexões visualmente; o squad real segue o desenho.
        </p>
        <Button variant="primary" onClick={() => dispatch({ type: 'SET_SCREEN', screen: 'designer' })}>
          Abrir conceito →
        </Button>
      </Card>

      <div className="grid grid-3">
        {PROPOSALS.map((p) => (
          <Card key={p.title}>
            <div className="row" style={{ justifyContent: 'space-between' }}>
              <strong>{p.title}</strong>
              <span style={{ fontSize: 11, color: 'var(--teal)' }}>{p.tag}</span>
            </div>
            <p style={{ fontSize: 13, color: 'var(--muted)' }}>{p.desc}</p>
            <div style={{ fontSize: 11, color: 'var(--faint)' }}>{p.anchor}</div>
          </Card>
        ))}
      </div>
    </div>
  )
}
