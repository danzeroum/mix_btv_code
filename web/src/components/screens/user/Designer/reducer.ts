import type { DesignerEdge, DesignerNode } from '../../../../types/domain'
import { BOARD_HEIGHT, BOARD_WIDTH, CARD_H, CARD_W, PILL_H, PILL_W, TEMPLATES, initialEdges, initialNodes } from './templates'

export interface DesignerState {
  nodes: DesignerNode[]
  edges: DesignerEdge[]
  mode: 'select' | 'connect'
  selectedNode: string | null
  pendingConnect: string | null
  dragId: string | null
  grabDX: number
  grabDY: number
  addCount: number
  wfSaved: boolean
}

export type DesignerAction =
  | { type: 'SET_MODE'; mode: DesignerState['mode'] }
  | { type: 'DRAG_START'; id: string; grabDX: number; grabDY: number }
  | { type: 'DRAG_MOVE'; boardX: number; boardY: number }
  | { type: 'DRAG_END' }
  | { type: 'CONNECT_CLICK'; id: string }
  | { type: 'ADD_NODE'; templateKey: string }
  | { type: 'REMOVE_NODE'; id: string }
  | { type: 'SELECT_NODE'; id: string | null }
  | { type: 'RESET' }
  | { type: 'MARK_SAVED' }

export function initDesignerState(): DesignerState {
  return {
    nodes: initialNodes(),
    edges: initialEdges(),
    mode: 'select',
    selectedNode: 'developer',
    pendingConnect: null,
    dragId: null,
    grabDX: 0,
    grabDY: 0,
    addCount: 0,
    wfSaved: false,
  }
}

function clamp(x: number, y: number, kind: DesignerNode['kind']) {
  const w = kind === 'pill' ? PILL_W : CARD_W
  const h = kind === 'pill' ? PILL_H : CARD_H
  return {
    x: Math.max(0, Math.min(x, BOARD_WIDTH - w)),
    y: Math.max(0, Math.min(y, BOARD_HEIGHT - h)),
  }
}

export function designerReducer(state: DesignerState, action: DesignerAction): DesignerState {
  switch (action.type) {
    case 'SET_MODE':
      return { ...state, mode: action.mode, pendingConnect: null }

    case 'DRAG_START':
      return { ...state, dragId: action.id, selectedNode: action.id, grabDX: action.grabDX, grabDY: action.grabDY }

    case 'DRAG_MOVE': {
      if (!state.dragId) return state
      const node = state.nodes.find((n) => n.id === state.dragId)
      if (!node) return state
      const { x, y } = clamp(action.boardX - state.grabDX, action.boardY - state.grabDY, node.kind)
      return {
        ...state,
        wfSaved: false,
        nodes: state.nodes.map((n) => (n.id === state.dragId ? { ...n, x, y } : n)),
      }
    }

    case 'DRAG_END':
      return { ...state, dragId: null }

    case 'CONNECT_CLICK': {
      if (!state.pendingConnect) {
        return { ...state, pendingConnect: action.id, selectedNode: action.id }
      }
      if (state.pendingConnect === action.id) {
        return { ...state, pendingConnect: null }
      }
      const exists = state.edges.some((e) => e.from === state.pendingConnect && e.to === action.id)
      const edges = exists ? state.edges : [...state.edges, { from: state.pendingConnect, to: action.id }]
      return { ...state, edges, pendingConnect: null, selectedNode: action.id, wfSaved: exists ? state.wfSaved : false }
    }

    case 'ADD_NODE': {
      const template = TEMPLATES[action.templateKey]
      if (!template) return state
      const count = state.addCount + 1
      const id = `${action.templateKey.toLowerCase().replace(/[^a-z]/g, '')}-${count}`
      const x = 280 + (count % 3) * 22
      const y = 60 + (count % 4) * 24
      const node: DesignerNode = { id, x, y, ...template }
      return {
        ...state,
        nodes: [...state.nodes, node],
        addCount: count,
        selectedNode: id,
        wfSaved: false,
      }
    }

    case 'REMOVE_NODE': {
      if (action.id === 'task') return state
      return {
        ...state,
        nodes: state.nodes.filter((n) => n.id !== action.id),
        edges: state.edges.filter((e) => e.from !== action.id && e.to !== action.id),
        selectedNode: 'task',
        pendingConnect: state.pendingConnect === action.id ? null : state.pendingConnect,
        wfSaved: false,
      }
    }

    case 'SELECT_NODE':
      return { ...state, selectedNode: action.id }

    case 'RESET':
      return initDesignerState()

    case 'MARK_SAVED':
      return { ...state, wfSaved: true }
  }
}
