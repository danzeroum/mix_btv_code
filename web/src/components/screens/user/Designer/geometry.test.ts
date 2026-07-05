import { describe, expect, it } from 'vitest'
import { computeEdges, intersectRectBorder, nodeBox } from './geometry'
import type { DesignerNode } from '../../../../types/domain'

function card(id: string, x: number, y: number, role = 'x'): DesignerNode {
  return { id, x, y, kind: 'card', name: id, role, color: '#fff', icon: '', sub: '', params: [], removable: true }
}

describe('intersectRectBorder', () => {
  it('encontra o ponto na borda direita quando o alvo está à direita', () => {
    const box = { cx: 50, cy: 50, hw: 20, hh: 10 }
    const p = intersectRectBorder(box, { x: 200, y: 50 })
    expect(p.x).toBeCloseTo(70)
    expect(p.y).toBeCloseTo(50)
  })

  it('encontra o ponto na borda inferior quando o alvo está abaixo', () => {
    const box = { cx: 50, cy: 50, hw: 20, hh: 10 }
    const p = intersectRectBorder(box, { x: 50, y: 200 })
    expect(p.x).toBeCloseTo(50)
    expect(p.y).toBeCloseTo(60)
  })

  it('retorna o centro quando o alvo é o próprio centro', () => {
    const box = { cx: 50, cy: 50, hw: 20, hh: 10 }
    const p = intersectRectBorder(box, { x: 50, y: 50 })
    expect(p).toEqual({ x: 50, y: 50 })
  })
})

describe('computeEdges', () => {
  it('ignora arestas cujo nó não existe', () => {
    const nodes = [card('a', 0, 0)]
    const result = computeEdges(nodes, [{ from: 'a', to: 'inexistente' }])
    expect(result).toEqual([])
  })

  it('marca como âmbar arestas que tocam um nó com role hitl', () => {
    const nodes = [card('a', 0, 0), card('hitl', 300, 0, 'hitl')]
    const result = computeEdges(nodes, [{ from: 'a', to: 'hitl' }])
    expect(result[0].amber).toBe(true)
  })

  it('não marca como âmbar arestas entre nós comuns', () => {
    const nodes = [card('a', 0, 0), card('b', 300, 0)]
    const result = computeEdges(nodes, [{ from: 'a', to: 'b' }])
    expect(result[0].amber).toBe(false)
  })

  it('calcula os pontos na borda dos dois nós, não nos centros', () => {
    const a = card('a', 0, 0)
    const b = card('b', 300, 0)
    const nodes = [a, b]
    const result = computeEdges(nodes, [{ from: 'a', to: 'b' }])
    const boxA = nodeBox(a)
    const boxB = nodeBox(b)
    expect(result[0].x1).not.toBeCloseTo(boxA.cx)
    expect(result[0].x2).not.toBeCloseTo(boxB.cx)
  })
})
