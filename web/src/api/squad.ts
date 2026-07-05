import { simulateLatency } from './client'
import type { ConsensusResult, SquadAgent } from '../types/domain'

/** Pesos fiéis a python/packages/forge-squad/src/forge_squad/consensus.py::DEFAULT_AGENT_WEIGHTS. */
export const AGENT_WEIGHTS: Record<string, Record<string, number>> = {
  architect: { architecture: 0.9, security: 0.7 },
  developer: { architecture: 0.6, implementation: 0.95, testing: 0.8 },
  auditor: { security: 0.95, quality: 0.85 },
  designer: { ui: 0.95, ux: 0.9 },
  ops: { infrastructure: 0.9, deployment: 0.9 },
}

export const HITL_ESCALATION_THRESHOLD = 0.7

export const SQUAD_AGENTS: SquadAgent[] = [
  { id: 'architect', name: 'Architect', state: 'concluido', confidence: 0.88, task: 'Definiu contrato da nova API e plano de migração incremental.' },
  { id: 'developer', name: 'Developer', state: 'executando', confidence: 0.95, task: 'Migrando payments/client.py e adaptando chamadas.' },
  { id: 'auditor', name: 'Auditor', state: 'aguardando', confidence: 0.85, task: 'Aguardando diff do Developer para revisão de segurança.' },
  { id: 'designer', name: 'Designer', state: 'ocioso', confidence: null, task: 'Sem tarefa atribuída nesta rodada.' },
  { id: 'ops', name: 'Ops', state: 'aguardando', confidence: 0.9, task: 'Aguardando aprovação do gate HITL para deploy.' },
]

export const CONSENSUS: ConsensusResult = {
  strength: 0.82,
  decisionMaker: 'developer',
  dissent: [{ agent: 'auditor', score: 0.19 }],
}

/** // TODO: backend Fase 6 — POST /api/squad/run, stream de SquadEvent via SquadService.ExecuteTask (gRPC). */
export async function runSquad(_task: string): Promise<SquadAgent[]> {
  await simulateLatency(600)
  return SQUAD_AGENTS
}

/** // TODO: backend Fase 6 — resolve o gate HITL real; aprovar +0.02 de trust, rejeitar -0.10 (hitl.py). */
export async function resolveHITL(approve: boolean): Promise<{ trustDelta: number }> {
  await simulateLatency(300)
  return { trustDelta: approve ? 0.02 : -0.1 }
}
