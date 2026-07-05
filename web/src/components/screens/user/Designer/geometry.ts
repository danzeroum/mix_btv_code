import type { DesignerEdge, DesignerNode } from '../../../../types/domain'
import { CARD_H, CARD_W, PILL_H, PILL_W } from './templates'

export interface Point {
  x: number
  y: number
}

export interface Box {
  cx: number
  cy: number
  hw: number
  hh: number
}

export function nodeBox(n: DesignerNode): Box {
  const w = n.kind === 'pill' ? PILL_W : CARD_W
  const h = n.kind === 'pill' ? PILL_H : CARD_H
  return { cx: n.x + w / 2, cy: n.y + h / 2, hw: w / 2, hh: h / 2 }
}

/** Ponto onde a linha entre o centro de `box` e `toward` cruza a borda de `box`.
 * Porte do `computeEdges` do protótipo: escala por max(|dx|/hw, |dy|/hh). */
export function intersectRectBorder(box: Box, toward: Point): Point {
  const dx = toward.x - box.cx
  const dy = toward.y - box.cy
  if (dx === 0 && dy === 0) return { x: box.cx, y: box.cy }
  const scale = Math.max(Math.abs(dx) / box.hw, Math.abs(dy) / box.hh)
  if (scale === 0) return { x: box.cx, y: box.cy }
  return { x: box.cx + dx / scale, y: box.cy + dy / scale }
}

export interface ComputedEdge {
  key: string
  x1: number
  y1: number
  x2: number
  y2: number
  amber: boolean
  label?: string
  labelX?: number
  labelY?: number
}

export function computeEdges(nodes: DesignerNode[], edges: DesignerEdge[]): ComputedEdge[] {
  const byId = new Map(nodes.map((n) => [n.id, n]))
  const result: ComputedEdge[] = []

  for (const e of edges) {
    const from = byId.get(e.from)
    const to = byId.get(e.to)
    if (!from || !to) continue

    const fromBox = nodeBox(from)
    const toBox = nodeBox(to)
    const p1 = intersectRectBorder(fromBox, { x: toBox.cx, y: toBox.cy })
    const p2 = intersectRectBorder(toBox, { x: fromBox.cx, y: fromBox.cy })
    const amber = from.role === 'hitl' || to.role === 'hitl' || from.id === 'hitl' || to.id === 'hitl'

    result.push({
      key: `${e.from}->${e.to}`,
      x1: p1.x,
      y1: p1.y,
      x2: p2.x,
      y2: p2.y,
      amber,
      label: e.label,
      labelX: e.label ? (p1.x + p2.x) / 2 - e.label.length * 2.7 : undefined,
      labelY: e.label ? (p1.y + p2.y) / 2 : undefined,
    })
  }

  return result
}
