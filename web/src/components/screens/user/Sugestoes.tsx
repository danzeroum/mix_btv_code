import { Card } from '../../primitives/Card'
import { Badge } from '../../primitives/Badge'
import { Button } from '../../primitives/Button'
import { useAppDispatch } from '../../../state/AppContext'
import type { ScreenId } from '../../../types/domain'

const PROPOSALS: {
  title: string
  tag: string
  desc: string
  anchor: string
  relatedScreen: ScreenId
  delivered?: boolean
}[] = [
  { title: 'Revisor de diff', tag: 'review', desc: 'inspeciona cada diff antes de aplicar.', anchor: 'ancora: forge-cli tool.edit', relatedScreen: 'sessao' },
  { title: 'Replay de sessão', tag: 'auditoria', desc: 'reproduz uma sessão a partir do ledger.', anchor: 'ancora: ledger hash-chain + sessions.db', relatedScreen: 'ledger' },
  { title: 'Aprovação em lote', tag: 'permissão', desc: 'aprova várias permissões pendentes de uma vez.', anchor: 'ancora: forge-core PermissionClient', relatedScreen: 'permissao' },
  { title: 'Modo watch', tag: 'sessão', desc: 'observa arquivos e sugere ações automaticamente.', anchor: 'ancora: forge-cli watch (futuro)', relatedScreen: 'sessao' },
  {
    title: 'A/B de prompts',
    tag: 'promptforge',
    desc: 'compara variações de prompt lado a lado.',
    anchor: 'ancora: forge_schemas::experiment (teste z, ADR 0014)',
    relatedScreen: 'experimentos',
    delivered: true,
  },
  {
    title: 'Mapa de memória do squad',
    tag: 'squad',
    desc: 'visualiza o que cada agente lembra e busca por termo (léxico, não semântico).',
    anchor: 'ancora: forge_squad.memory + recall.py (TF-IDF)',
    relatedScreen: 'memoria',
    delivered: true,
  },
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
          <button
            key={p.title}
            onClick={() => dispatch({ type: 'SET_SCREEN', screen: p.relatedScreen })}
            style={{ background: 'none', border: 'none', padding: 0, textAlign: 'left', cursor: 'pointer' }}
            title={`ir para a tela relacionada`}
          >
            <Card>
              <div className="row" style={{ justifyContent: 'space-between' }}>
                <strong>{p.title}</strong>
                <span className="row" style={{ gap: 4 }}>
                  {p.delivered && <Badge color="var(--ok)">✓ entregue</Badge>}
                  <span style={{ fontSize: 11, color: 'var(--teal)' }}>{p.tag}</span>
                </span>
              </div>
              <p style={{ fontSize: 13, color: 'var(--muted)' }}>{p.desc}</p>
              <div style={{ fontSize: 11, color: 'var(--faint)' }}>{p.anchor}</div>
            </Card>
          </button>
        ))}
      </div>
    </div>
  )
}
