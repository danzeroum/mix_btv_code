/** Único módulo que fala com um backend REAL: forge-server (crates/forge-server/src/lib.rs). */

export interface Summary {
  total_events: number
  cache_hit_rate: number | null
  by_name: Record<string, number>
}

export interface EventRow {
  ts: string
  name: string
  session_id: string
  props: unknown
}

export async function getSummary(): Promise<Summary> {
  const r = await fetch('/api/summary')
  if (!r.ok) throw new Error(`GET /api/summary falhou (${r.status})`)
  return r.json() as Promise<Summary>
}

export async function getEvents(limit = 50): Promise<EventRow[]> {
  const r = await fetch(`/api/events?limit=${limit}`)
  if (!r.ok) throw new Error(`GET /api/events falhou (${r.status})`)
  return r.json() as Promise<EventRow[]>
}
