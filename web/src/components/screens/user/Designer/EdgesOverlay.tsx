import type { DesignerEdge, DesignerNode } from '../../../../types/domain'
import { computeEdges } from './geometry'
import { BOARD_HEIGHT, BOARD_WIDTH } from './templates'

export function EdgesOverlay({ nodes, edges }: { nodes: DesignerNode[]; edges: DesignerEdge[] }) {
  const computed = computeEdges(nodes, edges)

  return (
    <svg
      width={BOARD_WIDTH}
      height={BOARD_HEIGHT}
      style={{ position: 'absolute', inset: 0, pointerEvents: 'none' }}
    >
      <defs>
        <marker id="farr" markerWidth="8" markerHeight="8" refX="6" refY="4" orient="auto">
          <path d="M0,0 L8,4 L0,8 Z" fill="var(--muted)" />
        </marker>
        <marker id="farrA" markerWidth="8" markerHeight="8" refX="6" refY="4" orient="auto">
          <path d="M0,0 L8,4 L0,8 Z" fill="var(--amber)" />
        </marker>
      </defs>
      {computed.map((e) => (
        <g key={e.key}>
          <line
            x1={e.x1}
            y1={e.y1}
            x2={e.x2}
            y2={e.y2}
            stroke={e.amber ? 'var(--amber)' : 'var(--muted)'}
            strokeWidth={1.5}
            strokeDasharray={e.amber ? '4 3' : undefined}
            markerEnd={e.amber ? 'url(#farrA)' : 'url(#farr)'}
          />
          {e.label && (
            <text x={e.labelX} y={(e.labelY ?? 0) - 4} fontSize={10} fill="var(--faint)">
              {e.label}
            </text>
          )}
        </g>
      ))}
    </svg>
  )
}
