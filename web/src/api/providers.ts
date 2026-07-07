/**
 * Fase 7 Onda 12 (piso): providers configurados de verdade (`GET
 * /api/providers`, `forge-server` — reusa `Gateway::from_env().available()`,
 * a MESMA leitura de env vars que uma sessão `forge run`/`chat` real usaria;
 * zero dependência nova) + limites por tier reais (reusa `GET /api/ratelimit`
 * da Onda 10/A4, `api/ratelimit.ts` — não reconstruído aqui).
 *
 * Degrau (reordenar fallback, ajustar teto do rate limiter) fica de fora
 * desta onda: `forge_llm::FallbackChain` é código morto (`Gateway::generate`
 * itera os providers configurados direto, nunca consulta
 * `FallbackChain::next_after` — confirmado lendo o código antes de expor
 * qualquer mutação) e, mesmo se fosse consultada, o `forge dashboard` é um
 * processo separado de qualquer sessão real — uma mutação aqui não afetaria
 * nenhuma sessão de verdade (mesmo achado da Onda 10 sobre "uso ao vivo" do
 * `RateLimiter`). A tela é read-only.
 */
import { fetchJson } from './client'
import type { ProviderInfo } from '../types/domain'

export async function fetchProviders(): Promise<ProviderInfo[]> {
  return fetchJson<ProviderInfo[]>('/api/providers')
}
