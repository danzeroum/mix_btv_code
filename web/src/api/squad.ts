/**
 * Fase 7 Onda 4: cliente do squad ao vivo — `POST /api/squad/run` dispara
 * `SquadService.ExecuteTask` (via `SquadPool`, capacidade 1 nesta entrega —
 * ver `crates/forge-cli/src/squad_agent.rs`) e `GET /api/squad/:id/events`
 * transmite `SquadEvent` cru como SSE, **sem DTO espelho**: o formato aqui é
 * exatamente o que `forge_proto::squad::SquadEvent` produz via
 * `#[derive(serde::Serialize)]` (ver `forge-proto/build.rs`) — union
 * externally-tagged pelo nome da variante Rust (`Proposal`/`Consensus`/
 * `Handoff`/`Hitl`/`Step`/`Error`), não um envelope autoral como o de
 * `stream.ts` (sessão).
 */
import { ApiError, fetchJson } from './client'

export interface SquadProposal {
  agent: string
  confidence: number
  /** JSON cru específico do agente (arquitetura/plano/código/veredito) — schema varia por `agent`. */
  content_json: string
}

export interface SquadConsensus {
  decision_maker: string
  strength: number
  decision_json: string
  requires_human: boolean
}

/** Espelha `forge.squad.v1.Handoff.Phase` — `phase` chega como i32 cru (enum proto3, sem rename). */
export const HANDOFF_PHASE_LABELS = ['desconhecido', 'iniciado', 'confirmado', 'concluído', 'erro'] as const

export interface SquadHandoff {
  phase: 0 | 1 | 2 | 3 | 4
  from_agent: string
  to_agent: string
  contract: string
  payload_digest: string
}

export interface SquadHitl {
  reason: string
  confidence: number
}

export interface SquadStep {
  step_id: string
  success: boolean
  summary: string
}

/** Mensagem de conversa do squad ao vivo (proto `ChatMessage`). `author_role`
 * distingue o membro-agente do membro-humano. */
export interface SquadChatMessage {
  author: string
  author_role: 'AGENT' | 'HUMAN' | 'SYSTEM'
  text: string
  in_reply_to?: string
}

export type SquadEventPayload =
  | { Proposal: SquadProposal }
  | { Consensus: SquadConsensus }
  | { Handoff: SquadHandoff }
  | { Hitl: SquadHitl }
  | { Step: SquadStep }
  | { Error: string }
  | { Chat: SquadChatMessage }

export interface SquadEventEnvelope {
  task_id: string
  ts: string
  payload: SquadEventPayload | null
}

export interface RunSquadResponse {
  task_id: string
}

export async function runSquad(task: string): Promise<RunSquadResponse> {
  return fetchJson<RunSquadResponse>('/api/squad/run', {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify({ task }),
  })
}

export async function resolveHitl(taskId: string, allow: boolean): Promise<void> {
  await fetchJson(`/api/squad/${encodeURIComponent(taskId)}/hitl`, {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify({ allow }),
  })
}

/** Envia uma mensagem do usuário à squad viva — o humano como MEMBRO, não só
 * aprovador. Responde `202 Accepted` sem corpo, então NÃO usamos `fetchJson`
 * (que tentaria `.json()` num corpo vazio — bug corrigido na Onda 15). */
export async function postSquadMessage(taskId: string, text: string): Promise<void> {
  const response = await fetch(`/api/squad/${encodeURIComponent(taskId)}/message`, {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify({ text }),
  })
  if (!response.ok) {
    throw new ApiError(`falha ao enviar mensagem (${response.status})`, `http_${response.status}`)
  }
}

/** Kill-switch (Fase 3): para um squad em execução imediatamente. Responde
 * `200` sem corpo. */
export async function emergencyStopSquad(taskId: string, reason?: string): Promise<void> {
  const response = await fetch(`/api/squad/${encodeURIComponent(taskId)}/emergency-stop`, {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify({ reason }),
  })
  if (!response.ok) {
    throw new ApiError(`falha ao interromper (${response.status})`, `http_${response.status}`)
  }
}

export interface SquadEventHandlers {
  onEvent: (event: SquadEventEnvelope) => void
  onConnectionError?: () => void
}

/**
 * Abre o SSE da tarefa de squad. Devolve uma função de limpeza (fecha a
 * conexão). Diferente de `connectSessionEvents`: uma tarefa de squad é
 * **finita** (o stream do servidor termina sozinho quando a tarefa acaba —
 * ver `SquadHub::finish_task`), então quem chama deve fechar a conexão no
 * primeiro `onConnectionError` em vez de deixar o `EventSource` nativo
 * reconectar para sempre contra uma tarefa que já terminou.
 */
export function connectSquadEvents(taskId: string, handlers: SquadEventHandlers): () => void {
  const source = new EventSource(`/api/squad/${encodeURIComponent(taskId)}/events`)
  source.onmessage = (ev) => {
    try {
      const parsed = JSON.parse(ev.data) as SquadEventEnvelope
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
