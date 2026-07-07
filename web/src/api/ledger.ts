/**
 * Fase 7 Onda 6: cliente do ledger append-only. `forge-server` só repassa
 * o que `forge_store::LedgerStore` já gravou (`.forge/forge.db`) — nenhuma
 * entrada é fabricada aqui.
 */
import { fetchJson } from './client'
import type { LedgerEntry } from '../types/domain'

/** `GET /api/ledger?limit=&actor=` — mais recentes primeiro. O filtro por
 * `actor`, quando presente, é resolvido no SQL do backend combinado com o
 * `limit` (não é um corte feito aqui depois de buscar): um ator raro fora
 * da janela recente ainda aparece. */
export async function getLedger(limit = 50, actor?: string): Promise<LedgerEntry[]> {
  const params = new URLSearchParams({ limit: String(limit) })
  if (actor) params.set('actor', actor)
  return fetchJson<LedgerEntry[]>(`/api/ledger?${params.toString()}`)
}

export interface VerifyResult {
  ok: boolean
  verified: number
  error?: string
}

/** `POST /api/ledger/verify` — recomputa a cadeia inteira. `ok:false` sinaliza
 * uma cadeia corrompida (não é erro HTTP — a requisição teve sucesso, o
 * *dado* que ela relata é que está adulterado). */
export async function verifyChain(): Promise<VerifyResult> {
  return fetchJson<VerifyResult>('/api/ledger/verify', { method: 'POST' })
}
