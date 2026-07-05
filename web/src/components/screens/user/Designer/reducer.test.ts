import { describe, expect, it } from 'vitest'
import { designerReducer, initDesignerState } from './reducer'
import { BOARD_HEIGHT, BOARD_WIDTH, CARD_H, CARD_W } from './templates'

describe('designerReducer', () => {
  it('remover o nó task é um no-op', () => {
    const state = initDesignerState()
    const next = designerReducer(state, { type: 'REMOVE_NODE', id: 'task' })
    expect(next).toBe(state)
    expect(next.nodes.some((n) => n.id === 'task')).toBe(true)
  })

  it('remover um nó comum também remove suas arestas', () => {
    const state = initDesignerState()
    const next = designerReducer(state, { type: 'REMOVE_NODE', id: 'architect' })
    expect(next.nodes.some((n) => n.id === 'architect')).toBe(false)
    expect(next.edges.some((e) => e.from === 'architect' || e.to === 'architect')).toBe(false)
    expect(next.selectedNode).toBe('task')
    expect(next.wfSaved).toBe(false)
  })

  it('connect click: primeiro clique define pendingConnect, sem criar aresta', () => {
    const state = { ...initDesignerState(), mode: 'connect' as const }
    const next = designerReducer(state, { type: 'CONNECT_CLICK', id: 'architect' })
    expect(next.pendingConnect).toBe('architect')
    expect(next.edges.length).toBe(state.edges.length)
  })

  it('connect click: clicar no mesmo nó cancela o pendingConnect', () => {
    const state = { ...initDesignerState(), mode: 'connect' as const, pendingConnect: 'architect' }
    const next = designerReducer(state, { type: 'CONNECT_CLICK', id: 'architect' })
    expect(next.pendingConnect).toBeNull()
  })

  it('connect click: segundo clique em outro nó cria uma aresta nova', () => {
    const state = { ...initDesignerState(), mode: 'connect' as const, pendingConnect: 'ops-1' }
    const withOps = { ...state, nodes: [...state.nodes, { ...state.nodes[0], id: 'ops-1' }] }
    const next = designerReducer(withOps, { type: 'CONNECT_CLICK', id: 'auditor' })
    expect(next.edges).toContainEqual({ from: 'ops-1', to: 'auditor' })
    expect(next.pendingConnect).toBeNull()
    expect(next.wfSaved).toBe(false)
  })

  it('connect click: aresta duplicada não é criada de novo', () => {
    const state = initDesignerState()
    const existingEdge = state.edges[0]
    const withPending = { ...state, mode: 'connect' as const, pendingConnect: existingEdge.from }
    const next = designerReducer(withPending, { type: 'CONNECT_CLICK', id: existingEdge.to })
    const count = next.edges.filter((e) => e.from === existingEdge.from && e.to === existingEdge.to).length
    expect(count).toBe(1)
  })

  it('drag move é limitado (clamp) aos limites do board', () => {
    const state = { ...initDesignerState(), dragId: 'architect', grabDX: 0, grabDY: 0 }
    const next = designerReducer(state, { type: 'DRAG_MOVE', boardX: 999999, boardY: 999999 })
    const node = next.nodes.find((n) => n.id === 'architect')!
    expect(node.x).toBe(BOARD_WIDTH - CARD_W)
    expect(node.y).toBe(BOARD_HEIGHT - CARD_H)
  })

  it('drag move não deixa a posição ficar negativa', () => {
    const state = { ...initDesignerState(), dragId: 'architect', grabDX: 0, grabDY: 0 }
    const next = designerReducer(state, { type: 'DRAG_MOVE', boardX: -500, boardY: -500 })
    const node = next.nodes.find((n) => n.id === 'architect')!
    expect(node.x).toBe(0)
    expect(node.y).toBe(0)
  })

  it('add node incrementa addCount e seleciona o novo nó', () => {
    const state = initDesignerState()
    const next = designerReducer(state, { type: 'ADD_NODE', templateKey: 'Ops' })
    expect(next.nodes.length).toBe(state.nodes.length + 1)
    expect(next.addCount).toBe(1)
    expect(next.selectedNode).toBe(next.nodes[next.nodes.length - 1].id)
  })

  it('reset restaura o grafo inicial', () => {
    const state = initDesignerState()
    const mutated = designerReducer(state, { type: 'ADD_NODE', templateKey: 'Ops' })
    const reset = designerReducer(mutated, { type: 'RESET' })
    expect(reset.nodes.length).toBe(state.nodes.length)
    expect(reset.wfSaved).toBe(false)
  })
})
