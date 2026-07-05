import { simulateLatency } from './client'
import type { ModelTierId, ProviderInfo, RateLimitTier } from '../types/domain'

export let PROVIDERS: ProviderInfo[] = [
  { id: 'anthropic', name: 'Anthropic', status: 'ativo' },
  { id: 'deepseek', name: 'DeepSeek', status: 'standby' },
  { id: 'openai', name: 'OpenAI', status: 'standby' },
]

export const RATE_LIMITS: RateLimitTier[] = [
  { tier: 'small', used: 12, cap: 120 },
  { tier: 'medium', used: 34, cap: 60 },
  { tier: 'large', used: 18, cap: 30 },
]

/** // TODO: backend Fase 5 — persiste ordem de fallback no forge-llm gateway. */
export async function reorderFallback(order: string[]): Promise<ProviderInfo[]> {
  await simulateLatency(200)
  PROVIDERS = order.map((id) => PROVIDERS.find((p) => p.id === id)!).filter(Boolean)
  return PROVIDERS
}

/** // TODO: backend Fase 5 — liga/desliga provider no forge-llm gateway real. */
export async function toggleProvider(id: string): Promise<ProviderInfo[]> {
  await simulateLatency(200)
  PROVIDERS = PROVIDERS.map((p) => (p.id === id ? { ...p, status: p.status === 'ativo' ? 'standby' : 'ativo' } : p))
  return PROVIDERS
}

/** // TODO: backend Fase 5 — ajusta o RateLimiter (sliding window) do forge-llm. */
export async function setRateLimit(tier: ModelTierId, cap: number): Promise<RateLimitTier> {
  await simulateLatency(150)
  const found = RATE_LIMITS.find((r) => r.tier === tier)
  if (!found) throw new Error('tier desconhecido')
  found.cap = cap
  return found
}
