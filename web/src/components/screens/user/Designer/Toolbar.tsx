import type { Dispatch } from 'react'
import { Button } from '../../../primitives/Button'
import { useAsyncAction } from '../../../../hooks/useAsyncAction'
import { useToast } from '../../../primitives/Toast'
import { ApiError } from '../../../../api/client'
import { saveWorkflow } from '../../../../api/designer'
import type { DesignerAction, DesignerState } from './reducer'

export function Toolbar({ state, dispatch }: { state: DesignerState; dispatch: Dispatch<DesignerAction> }) {
  const toast = useToast()
  const save = useAsyncAction(saveWorkflow)

  async function handleSave() {
    try {
      const result = await save.run({ nodes: state.nodes, edges: state.edges })
      dispatch({ type: 'MARK_SAVED', seq: result.seq })
      toast.push('success', `${result.workflowId} salvo → schema validado → ledger seq ${result.seq}`)
    } catch (e) {
      toast.push('error', e instanceof ApiError ? e.message : 'falha ao salvar workflow')
    }
  }

  return (
    <div className="row" style={{ justifyContent: 'space-between' }}>
      <div className="row">
        <button
          onClick={() => dispatch({ type: 'SET_MODE', mode: 'select' })}
          style={modeBtn(state.mode === 'select')}
        >
          ▢ selecionar
        </button>
        <button
          onClick={() => dispatch({ type: 'SET_MODE', mode: 'connect' })}
          style={modeBtn(state.mode === 'connect')}
        >
          ↳ conectar
        </button>
      </div>
      <div className="row">
        <Button onClick={() => dispatch({ type: 'RESET' })}>↺ reset</Button>
        <Button
          variant={state.wfSaved ? 'ghost' : 'primary'}
          onClick={() => void handleSave()}
          disabled={save.state.status === 'loading'}
        >
          {save.state.status === 'loading' ? 'salvando…' : state.wfSaved ? '✓ salvo' : 'salvar'}
        </Button>
      </div>
    </div>
  )
}

function modeBtn(active: boolean): React.CSSProperties {
  return {
    border: `1px solid ${active ? 'var(--ink)' : 'var(--line)'}`,
    background: active ? 'var(--panel2)' : 'transparent',
    color: 'var(--ink)',
    borderRadius: 7,
    padding: '6px 12px',
    fontSize: 12,
  }
}
