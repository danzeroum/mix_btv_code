/**
 * Fase 7 Onda 2: cliente do SSE de sessão (`GET /api/session/:id/events`,
 * `crates/forge-cli/src/web_agent.rs::SessionEvent`). Usa o `EventSource`
 * nativo do navegador — ele já entende o formato `data: <json>\n\n` que o
 * axum produz via `Event::json_data`, e reconecta sozinho se a conexão cair
 * (o servidor reemite o snapshot do que já aconteceu a quem conectar depois,
 * então uma reconexão não perde o pedido de permissão pendente).
 */

export type SessionEvent =
  | { type: 'text_delta'; text: string }
  | { type: 'turn_completed'; provider: string; input_tokens: number; output_tokens: number }
  | { type: 'tool_started'; name: string; scope: string }
  | { type: 'tool_finished'; name: string; ok: boolean; summary: string; diff: DiffLine[] | null }
  | { type: 'tool_denied'; name: string; scope: string }
  | { type: 'permission_requested'; request_id: string; tool: string; scope: string }
  | { type: 'done'; ledger_verified: number }
  | { type: 'error'; message: string }

export interface DiffLine {
  Context?: string
  Removed?: string
  Added?: string
}

export interface SessionEventHandlers {
  onEvent: (event: SessionEvent) => void
  onConnectionError?: () => void
}

/** Abre o SSE da sessão. Devolve uma função de limpeza (fecha a conexão). */
export function connectSessionEvents(sessionId: string, handlers: SessionEventHandlers): () => void {
  const source = new EventSource(`/api/session/${encodeURIComponent(sessionId)}/events`)
  source.onmessage = (ev) => {
    try {
      const parsed = JSON.parse(ev.data) as SessionEvent
      handlers.onEvent(parsed)
    } catch {
      // evento não-JSON (ex.: keep-alive) — ignora silenciosamente.
    }
  }
  source.onerror = () => {
    handlers.onConnectionError?.()
  }
  return () => source.close()
}

/** Gera um id de sessão novo — estável para a vida da aba (não persiste entre reloads nesta onda). */
export function newSessionId(): string {
  if (typeof crypto !== 'undefined' && 'randomUUID' in crypto) {
    return crypto.randomUUID()
  }
  return `s${Date.now().toString(16)}${Math.random().toString(16).slice(2, 8)}`
}
