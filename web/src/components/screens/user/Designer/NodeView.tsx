import type { DesignerNode } from '../../../../types/domain'
import { CARD_H, CARD_W, PILL_H, PILL_W } from './templates'

export function NodeView({
  node,
  selected,
  pending,
  mode,
  onMouseDown,
  onClick,
}: {
  node: DesignerNode
  selected: boolean
  pending: boolean
  mode: 'select' | 'connect'
  onMouseDown: (e: React.MouseEvent) => void
  onClick: () => void
}) {
  const w = node.kind === 'pill' ? PILL_W : CARD_W
  const h = node.kind === 'pill' ? PILL_H : CARD_H

  const glow = pending
    ? '0 0 0 3px #f0a13c33'
    : selected
      ? '0 0 0 3px #4d9fff22, 0 8px 24px -8px #000c'
      : 'none'

  return (
    <button
      onMouseDown={onMouseDown}
      onClick={onClick}
      style={{
        position: 'absolute',
        left: node.x,
        top: node.y,
        width: w,
        height: h,
        borderRadius: node.kind === 'pill' ? 16 : 10,
        borderWidth: 1,
        borderColor: pending ? 'var(--amber)' : selected ? 'var(--line2)' : 'var(--line)',
        background: 'var(--panel2)',
        color: 'var(--ink)',
        boxShadow: glow,
        cursor: mode === 'connect' ? 'crosshair' : 'grab',
        display: 'flex',
        flexDirection: node.kind === 'pill' ? 'row' : 'column',
        alignItems: node.kind === 'pill' ? 'center' : 'flex-start',
        justifyContent: node.kind === 'pill' ? 'center' : 'center',
        gap: 2,
        padding: node.kind === 'pill' ? 0 : '6px 8px',
        fontSize: node.kind === 'pill' ? 11 : 12,
        borderStyle: node.kind === 'pill' ? 'dashed' : 'solid',
        userSelect: 'none',
        zIndex: 1,
      }}
    >
      <span style={{ color: node.color }}>
        {node.icon} {node.name}
      </span>
      {node.kind === 'card' && (
        <span style={{ fontSize: 10, color: 'var(--muted)' }}>{node.role}</span>
      )}
    </button>
  )
}
