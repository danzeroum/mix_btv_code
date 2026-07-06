/**
 * Fase 7 Onda 7 (A5): uso por modelo. `GET /api/models/usage` mora direto em
 * `forge-server` (só depende do que o crate já depende — `forge-store` +
 * `forge-llm`, este último já usado pelo bin `loadgen`). Nome do módulo não
 * é `models.ts` — já ocupado pela tela `modelo` de usuário (seleção de
 * tier/agente).
 */
import { fetchJson } from './client'
import type { ModelTierId } from '../types/domain'

export interface ModelUsageEntry {
  model: string
  tier: ModelTierId
  calls: number
  cache_hits: number
  cache_misses: number
}

export async function fetchModelUsage(): Promise<ModelUsageEntry[]> {
  return fetchJson<ModelUsageEntry[]>('/api/models/usage')
}
