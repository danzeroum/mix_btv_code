/**
 * Fase 7 Onda 10 (A4): tetos de rate limit por tier. `GET /api/ratelimit`
 * mora direto em `forge-server` (mesma classe de posicionamento de A5/A2 —
 * só depende do que o crate já depende). **Não é uso ao vivo**: o dashboard
 * é um processo separado de qualquer sessão `forge run`/`chat` que realmente
 * consome vagas — não há `RateLimiter` compartilhado para ler. A tela mostra
 * isso explicitamente, não finge um "usado" que não existe.
 */
import { fetchJson } from './client'
import type { ModelTierId } from '../types/domain'

export interface RateLimitEntry {
  tier: ModelTierId
  cap: number
  window_secs: number
}

export async function fetchRateLimits(): Promise<RateLimitEntry[]> {
  return fetchJson<RateLimitEntry[]>('/api/ratelimit')
}
