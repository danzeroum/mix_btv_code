/**
 * Fase 7 Onda 2: estado da sessão de código, vivo para a aba inteira — mora
 * acima da troca de tela (montado em `App.tsx`, ao lado de `AppProvider`)
 * porque o pedido de permissão pode chegar enquanto o usuário está na tela
 * `Sessão` mas precisa aparecer na tela `Permissão` depois de navegar. Se
 * isto morasse dentro do componente `Sessao`, a troca de tela desmontaria a
 * conexão SSE e perderíamos o pedido pendente.
 */
import { createContext, useCallback, useContext, useEffect, useRef, useState, type ReactNode } from 'react'
import { ApiError, fetchJson } from '../api/client'
import { connectSessionEvents, newSessionId, type SessionEvent } from '../api/stream'
import type { TranscriptTurn } from '../api/session'

export interface PendingPermission {
  requestId: string
  tool: string
  scope: string
}

interface SessionContextValue {
  sessionId: string
  transcript: TranscriptTurn[]
  streamingText: string
  pending: PendingPermission | null
  busy: boolean
  lastError: string | null
  /** Contagem real do último `Session::verify()` — `null` antes do 1º turno concluir. */
  ledgerVerified: number | null
  /**
   * `opts.model`/`opts.agent` (Fase 7 Onda 13) vão direto no corpo de
   * `POST .../message` — os mesmos campos que `SendMessageBody` (Rust) já
   * aceita desde a Onda 1, antes nunca populados pelo cliente. Parâmetro
   * por chamada, não estado persistido (mirroring do CLI: `--model`/
   * `--agent` são flags por invocação, não uma preferência salva).
   */
  sendMessage: (message: string, opts?: { model?: string; agent?: string }) => Promise<void>
  resolvePermission: (allow: boolean) => Promise<void>
}

const SessionStateContext = createContext<SessionContextValue | null>(null)

let turnCounter = 0
function nextTurnId(prefix: string): string {
  turnCounter += 1
  return `${prefix}-${turnCounter}`
}

export function SessionProvider({ children }: { children: ReactNode }) {
  const [sessionId] = useState(newSessionId)
  const [transcript, setTranscript] = useState<TranscriptTurn[]>([])
  const [streamingText, setStreamingText] = useState('')
  const [pending, setPending] = useState<PendingPermission | null>(null)
  const [busy, setBusy] = useState(false)
  const [lastError, setLastError] = useState<string | null>(null)
  const [ledgerVerified, setLedgerVerified] = useState<number | null>(null)
  const streamingRef = useRef('')

  useEffect(() => {
    const handleEvent = (event: SessionEvent) => {
      switch (event.type) {
        case 'text_delta':
          streamingRef.current += event.text
          setStreamingText(streamingRef.current)
          break
        case 'turn_completed': {
          const text = streamingRef.current
          streamingRef.current = ''
          setStreamingText('')
          if (text) {
            setTranscript((prev) => [...prev, { id: nextTurnId('a'), kind: 'agent', text }])
          }
          break
        }
        case 'tool_started':
          setTranscript((prev) => [
            ...prev,
            { id: nextTurnId('t'), kind: 'tool', text: `⚒ ${event.name}  ${event.scope}`, toolStatus: 'running' },
          ])
          break
        case 'tool_finished':
          setTranscript((prev) => [
            ...prev,
            {
              id: nextTurnId('t'),
              kind: 'tool',
              text: `⚒ ${event.name}  ${event.summary}`,
              toolStatus: event.ok ? 'ok' : 'error',
            },
          ])
          break
        case 'tool_denied':
          setTranscript((prev) => [
            ...prev,
            { id: nextTurnId('t'), kind: 'tool', text: `⚒ ${event.name}  negado`, toolStatus: 'error' },
          ])
          break
        case 'permission_requested':
          setPending({ requestId: event.request_id, tool: event.tool, scope: event.scope })
          break
        case 'done':
          setBusy(false)
          setPending(null)
          setLedgerVerified(event.ledger_verified)
          break
        case 'error':
          setBusy(false)
          setPending(null)
          setLastError(event.message)
          break
      }
    }
    const disconnect = connectSessionEvents(sessionId, { onEvent: handleEvent })
    return disconnect
  }, [sessionId])

  const sendMessage = useCallback(
    async (message: string, opts?: { model?: string; agent?: string }) => {
      setLastError(null)
      setBusy(true)
      setTranscript((prev) => [...prev, { id: nextTurnId('u'), kind: 'user', text: message }])
      try {
        await fetchJson(`/api/session/${encodeURIComponent(sessionId)}/message`, {
          method: 'POST',
          headers: { 'content-type': 'application/json' },
          body: JSON.stringify({ message, ...opts }),
        })
      } catch (e) {
        setBusy(false)
        setLastError(e instanceof ApiError ? e.message : 'falha ao enviar mensagem')
        throw e
      }
    },
    [sessionId],
  )

  const resolvePermission = useCallback(
    async (allow: boolean) => {
      if (!pending) return
      await fetchJson(`/api/session/${encodeURIComponent(sessionId)}/permission`, {
        method: 'POST',
        headers: { 'content-type': 'application/json' },
        body: JSON.stringify({ request_id: pending.requestId, allow }),
      })
      setPending(null)
    },
    [sessionId, pending],
  )

  return (
    <SessionStateContext.Provider
      value={{
        sessionId,
        transcript,
        streamingText,
        pending,
        busy,
        lastError,
        ledgerVerified,
        sendMessage,
        resolvePermission,
      }}
    >
      {children}
    </SessionStateContext.Provider>
  )
}

export function useSession(): SessionContextValue {
  const ctx = useContext(SessionStateContext)
  if (!ctx) throw new Error('useSession deve ser usado dentro de <SessionProvider>')
  return ctx
}
