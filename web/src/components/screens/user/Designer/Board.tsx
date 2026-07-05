import { useEffect, useRef, type Dispatch } from 'react'
import { EdgesOverlay } from './EdgesOverlay'
import { NodeView } from './NodeView'
import type { DesignerAction, DesignerState } from './reducer'
import { BOARD_HEIGHT, BOARD_WIDTH } from './templates'

export function Board({ state, dispatch }: { state: DesignerState; dispatch: Dispatch<DesignerAction> }) {
  const boardRef = useRef<HTMLDivElement | null>(null)

  useEffect(() => {
    if (!state.dragId) return

    function onMove(e: MouseEvent) {
      const rect = boardRef.current?.getBoundingClientRect()
      if (!rect) return
      dispatch({ type: 'DRAG_MOVE', boardX: e.clientX - rect.left, boardY: e.clientY - rect.top })
    }
    function onUp() {
      dispatch({ type: 'DRAG_END' })
    }

    window.addEventListener('mousemove', onMove)
    window.addEventListener('mouseup', onUp)
    return () => {
      window.removeEventListener('mousemove', onMove)
      window.removeEventListener('mouseup', onUp)
    }
  }, [state.dragId, dispatch])

  function handleNodeMouseDown(id: string, e: React.MouseEvent) {
    if (state.mode === 'connect') return
    const rect = boardRef.current?.getBoundingClientRect()
    if (!rect) return
    const node = state.nodes.find((n) => n.id === id)
    if (!node) return
    const boardX = e.clientX - rect.left
    const boardY = e.clientY - rect.top
    dispatch({ type: 'DRAG_START', id, grabDX: boardX - node.x, grabDY: boardY - node.y })
  }

  function handleNodeClick(id: string) {
    if (state.mode === 'connect') {
      dispatch({ type: 'CONNECT_CLICK', id })
    } else {
      dispatch({ type: 'SELECT_NODE', id })
    }
  }

  return (
    <div
      ref={boardRef}
      role="group"
      aria-label="canvas do squad designer"
      style={{
        position: 'relative',
        width: BOARD_WIDTH,
        height: BOARD_HEIGHT,
        background: 'var(--term)',
        border: '1px solid var(--line)',
        borderRadius: 10,
        overflow: 'hidden',
      }}
    >
      <EdgesOverlay nodes={state.nodes} edges={state.edges} />
      {state.nodes.map((n) => (
        <NodeView
          key={n.id}
          node={n}
          selected={n.id === state.selectedNode}
          pending={n.id === state.pendingConnect}
          mode={state.mode}
          onMouseDown={(e) => handleNodeMouseDown(n.id, e)}
          onClick={() => handleNodeClick(n.id)}
        />
      ))}
    </div>
  )
}
