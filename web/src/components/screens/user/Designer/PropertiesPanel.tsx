import type { Dispatch } from 'react'
import { Button } from '../../../primitives/Button'
import type { DesignerAction, DesignerState } from './reducer'

export function PropertiesPanel({ state, dispatch }: { state: DesignerState; dispatch: Dispatch<DesignerAction> }) {
  const node = state.nodes.find((n) => n.id === state.selectedNode)

  if (!node) {
    return <div style={{ fontSize: 12, color: 'var(--faint)' }}>nenhum nó selecionado</div>
  }

  return (
    <div className="stack">
      <div style={{ fontSize: 11, color: 'var(--faint)' }}>PROPRIEDADES</div>
      <div>
        <strong style={{ color: node.color }}>
          {node.icon} {node.name}
        </strong>
        <div style={{ fontSize: 12, color: 'var(--muted)' }}>{node.sub}</div>
      </div>
      <div className="stack" style={{ gap: 4 }}>
        {node.params.map((p) => (
          <div key={p.k} className="row" style={{ justifyContent: 'space-between', fontSize: 12 }}>
            <span style={{ color: 'var(--faint)' }}>{p.k}</span>
            <span className="mono">{p.v}</span>
          </div>
        ))}
      </div>
      <Button
        variant="danger"
        disabled={!node.removable}
        onClick={() => dispatch({ type: 'REMOVE_NODE', id: node.id })}
        title={node.removable ? undefined : 'nó de entrada não pode ser removido'}
      >
        ✕ remover nó & conexões
      </Button>
    </div>
  )
}
