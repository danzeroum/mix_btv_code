import { useReducer } from 'react'
import { Board } from './Board'
import { Palette } from './Palette'
import { PropertiesPanel } from './PropertiesPanel'
import { Toolbar } from './Toolbar'
import { designerReducer, initDesignerState } from './reducer'

export function Designer() {
  const [state, dispatch] = useReducer(designerReducer, undefined, initDesignerState)

  return (
    <div className="stack">
      <Toolbar state={state} dispatch={dispatch} />
      {state.wfSaved && (
        <div style={{ fontSize: 12, color: 'var(--ok)', border: '1px solid var(--ok)', borderRadius: 8, padding: 8 }}>
          ✓ squad.workflow.v1 salvo → schema validado → ledger seq {state.lastSavedSeq} → aplicação real ao
          orquestrador é trabalho futuro (os 5 agentes fixos continuam decidindo)
        </div>
      )}
      <div style={{ display: 'flex', gap: 16 }}>
        <div style={{ width: 130, flexShrink: 0 }}>
          <Palette dispatch={dispatch} />
        </div>
        <Board state={state} dispatch={dispatch} />
        <div style={{ width: 220, flexShrink: 0 }}>
          <PropertiesPanel state={state} dispatch={dispatch} />
        </div>
      </div>
    </div>
  )
}
