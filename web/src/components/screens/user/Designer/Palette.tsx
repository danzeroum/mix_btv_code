import type { Dispatch } from 'react'
import { TEMPLATE_KEYS } from './templates'
import type { DesignerAction } from './reducer'

export function Palette({ dispatch }: { dispatch: Dispatch<DesignerAction> }) {
  return (
    <div className="stack" role="group" aria-label="paleta de blocos">
      <div style={{ fontSize: 11, color: 'var(--faint)' }}>BLOCOS</div>
      {TEMPLATE_KEYS.map((key) => (
        <button
          key={key}
          onClick={() => dispatch({ type: 'ADD_NODE', templateKey: key })}
          style={{
            border: '1px solid var(--line)',
            background: 'var(--panel)',
            color: 'var(--ink)',
            borderRadius: 7,
            padding: '7px 10px',
            fontSize: 12,
            textAlign: 'left',
          }}
        >
          + {key}
        </button>
      ))}
    </div>
  )
}
