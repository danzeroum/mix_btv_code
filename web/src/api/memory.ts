/**
 * Fase 7 Onda 8 (A3): mapa de memória do squad + busca léxica (TF-IDF).
 * `GET /api/memory` + `POST /api/memory/recall` moram no router mesclado
 * de `forge-cli` (precisam de `forge-sidecar`). Rótulo/nav dizem "RAG", mas
 * a recuperação é léxica (`recall.py`, ADR 0013), não semântica — a tela
 * carrega essa tensão honesta, não some com ela.
 */
import { fetchJson } from './client'

/** Espelha `forge_proto::memory::MemorySummary` — sem coluna de tendência
 * de esquecimento (nada no código a calcula). */
export interface MemorySummary {
  agent: string
  count: number
  latest_decision_json: string
  latest_timestamp: string
  top_confidence: number
}

/** Espelha `forge_proto::memory::MemoryMatch`. */
export interface MemoryMatch {
  id: string
  agent: string
  decision_json: string
  timestamp: string
  score: number
}

export async function fetchMemoryMap(agent?: string, limit = 50): Promise<MemorySummary[]> {
  const params = new URLSearchParams({ limit: String(limit) })
  if (agent) params.set('agent', agent)
  return fetchJson<MemorySummary[]>(`/api/memory?${params.toString()}`)
}

export async function recallMemory(query: string, k = 5): Promise<MemoryMatch[]> {
  return fetchJson<MemoryMatch[]>('/api/memory/recall', {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify({ query, k }),
  })
}
