import { simulateLatency } from './client'
import type { AgentProfile, AutonomyLevel, ModelTier, ModelTierId } from '../types/domain'

export const MODEL_TIERS: ModelTier[] = [
  { id: 'small', models: 'haiku · deepseek-chat', label: 'step-discipline' },
  { id: 'medium', models: 'gpt-4o · sonnet', label: '' },
  { id: 'large', models: 'claude-sonnet-5 · opus', label: '' },
]

/** Nome do modelo primário de um tier, para exibir no cabeçalho da sessão (ex.: "claude-sonnet-5 · opus" -> "claude-sonnet-5"). */
export function primaryModelName(tier: ModelTierId): string {
  const found = MODEL_TIERS.find((t) => t.id === tier)
  return found ? found.models.split(' · ')[0] : tier
}

export const AUTONOMY_LEVELS: { id: AutonomyLevel; label: string; enabled: boolean }[] = [
  { id: 'interativo', label: 'Interativo', enabled: true },
  { id: 'automatico', label: 'Automático (em dev)', enabled: false },
  { id: 'somente_leitura', label: 'Somente leitura', enabled: true },
]

/** // TODO: backend Fase 5 — persiste seleção de tier no forge-core, propaga ao gateway LLM. */
export async function selectTier(tier: ModelTierId): Promise<ModelTierId> {
  await simulateLatency(150)
  return tier
}

/** // TODO: backend Fase 5 — muda o agente ativo (build/plan), recalcula matriz de permissões. */
export async function selectAgentProfile(profile: AgentProfile): Promise<AgentProfile> {
  await simulateLatency(150)
  return profile
}

/** // TODO: backend Fase 6 — liga a nível de autonomia do hitl.py (ProgressiveAutonomyManager). */
export async function selectAutonomy(level: AutonomyLevel): Promise<AutonomyLevel> {
  await simulateLatency(150)
  return level
}
