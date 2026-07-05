import type { DesignerNode } from '../../../../types/domain'

export const BOARD_WIDTH = 720
export const BOARD_HEIGHT = 470
export const CARD_W = 104
export const CARD_H = 62
export const PILL_W = 60
export const PILL_H = 30

type Template = Omit<DesignerNode, 'id' | 'x' | 'y'>

/** Pesos fiéis a consensus.py::DEFAULT_AGENT_WEIGHTS e a hitl.py — não inventar números novos. */
export const TEMPLATES: Record<string, Template> = {
  Architect: {
    kind: 'card', name: 'Architect', role: 'arquitetura', color: 'var(--teal)', icon: '◈', sub: 'agente · forge_squad.agents',
    params: [
      { k: 'peso architecture', v: '0.90' },
      { k: 'peso security', v: '0.70' },
      { k: 'autonomia', v: 'nível 2' },
      { k: 'ferramentas', v: 'read · grep' },
    ],
    removable: true,
  },
  Developer: {
    kind: 'card', name: 'Developer', role: 'implementação', color: 'var(--amber)', icon: '⚒', sub: 'agente · forge_squad.agents',
    params: [
      { k: 'peso architecture', v: '0.60' },
      { k: 'peso implementation', v: '0.95' },
      { k: 'peso testing', v: '0.80' },
      { k: 'ferramentas', v: 'read · grep · edit · bash' },
    ],
    removable: true,
  },
  Auditor: {
    kind: 'card', name: 'Auditor', role: 'auditoria', color: 'var(--ok)', icon: '✓', sub: 'agente · forge_squad.agents',
    params: [
      { k: 'peso security', v: '0.95' },
      { k: 'peso quality', v: '0.85' },
      { k: 'entrada', v: 'verification-evidence.v1' },
      { k: 'ferramentas', v: 'read · grep' },
    ],
    removable: true,
  },
  Designer: {
    kind: 'card', name: 'Designer', role: 'ui/ux', color: 'var(--wire)', icon: '✎', sub: 'agente · forge_squad.agents',
    params: [{ k: 'peso ui', v: '0.95' }, { k: 'peso ux', v: '0.90' }, { k: 'autonomia', v: 'nível 1' }],
    removable: true,
  },
  Ops: {
    kind: 'card', name: 'Ops', role: 'operações', color: 'var(--rust)', icon: '⛭', sub: 'agente · forge_squad.agents',
    params: [
      { k: 'peso infrastructure', v: '0.90' },
      { k: 'peso deployment', v: '0.90' },
      { k: 'crítico', v: 'schema → HITL' },
    ],
    removable: true,
  },
  Consenso: {
    kind: 'card', name: 'Consenso', role: 'consensus', color: 'var(--py)', icon: '◇', sub: 'WeightedConsensusEngine',
    params: [{ k: 'limiar HITL', v: '0.70' }, { k: 'algoritmo', v: 'voto ponderado por peso × confiança' }],
    removable: true,
  },
  'Gate HITL': {
    kind: 'card', name: 'Gate HITL', role: 'hitl', color: 'var(--amber)', icon: '⚑', sub: 'ProgressiveAutonomyManager',
    params: [
      { k: 'níveis', v: '0 full_human_control … 3 full_autonomy' },
      { k: 'aprovação', v: 'via RequestPermission' },
      { k: 'trust +/-', v: '+0.02 sucesso · -0.10 falha' },
    ],
    removable: true,
  },
}

export const TEMPLATE_KEYS = Object.keys(TEMPLATES)

export function initialNodes(): DesignerNode[] {
  return [
    { id: 'task', x: 10, y: 220, kind: 'pill', name: 'task', role: 'entry', color: 'var(--ink)', icon: '▸', sub: 'entrada', params: [], removable: false },
    { id: 'architect', x: 140, y: 204, ...TEMPLATES.Architect },
    { id: 'developer', x: 290, y: 90, ...TEMPLATES.Developer },
    { id: 'designer', x: 290, y: 320, ...TEMPLATES.Designer },
    { id: 'consenso', x: 440, y: 204, ...TEMPLATES.Consenso },
    { id: 'auditor', x: 590, y: 90, ...TEMPLATES.Auditor },
    { id: 'hitl', x: 590, y: 204, ...TEMPLATES['Gate HITL'] },
    { id: 'ops', x: 590, y: 320, ...TEMPLATES.Ops },
  ]
}

export function initialEdges() {
  return [
    { from: 'task', to: 'architect' },
    { from: 'architect', to: 'developer', label: 'paralelo' },
    { from: 'architect', to: 'designer', label: 'paralelo' },
    { from: 'developer', to: 'consenso' },
    { from: 'designer', to: 'consenso' },
    { from: 'consenso', to: 'auditor', label: '≥ 0.7' },
    { from: 'consenso', to: 'hitl', label: '< 0.7' },
    { from: 'hitl', to: 'auditor', label: 'aprovado' },
    { from: 'auditor', to: 'ops' },
  ]
}
