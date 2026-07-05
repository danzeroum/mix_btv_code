import { simulateLatency } from './client'

export type ToolCallStatus = 'running' | 'ok' | 'error'

export interface TranscriptTurn {
  id: string
  kind: 'user' | 'agent' | 'tool' | 'diff' | 'lint'
  text: string
  toolStatus?: ToolCallStatus
}

export interface SessionHeader {
  model: string
  agent: string
  provider: string
  cacheOn: boolean
  sessionId: string
}

export const INITIAL_TRANSCRIPT: TranscriptTurn[] = [
  { id: 't1', kind: 'user', text: 'implemente um validador de CPF em utils/cpf.py, com testes' },
  { id: 't2', kind: 'agent', text: 'vou ler a estrutura do projeto e criar o validador com testes.' },
  { id: 't3', kind: 'tool', text: '⚒ read  utils/', toolStatus: 'ok' },
  { id: 't4', kind: 'tool', text: '⚒ edit  utils/cpf.py', toolStatus: 'ok' },
  {
    id: 't5',
    kind: 'diff',
    text: '+ def validar_cpf(cpf: str) -> bool:\n+     ...\n- # TODO: implementar validador',
  },
  { id: 't6', kind: 'tool', text: '⚒ bash  $ python -m pytest tests/test_cpf.py', toolStatus: 'ok' },
  { id: 't7', kind: 'agent', text: '5 passed in 0.08s — validador de CPF pronto, com 5 casos de teste.' },
]

/** // TODO: backend Fase 5 — POST /api/session/:id/message, streaming SSE do agente real. */
export async function streamAgent(message: string): Promise<TranscriptTurn[]> {
  await simulateLatency(500)
  return [
    { id: `u-${Date.now()}`, kind: 'user', text: message },
    { id: `a-${Date.now()}`, kind: 'agent', text: 'entendido — analisando o pedido (resposta simulada).' },
  ]
}

export interface ToolPolicy {
  tool: string
  policy: 'allow' | 'ask'
}

export const TOOL_POLICIES: ToolPolicy[] = [
  { tool: 'read', policy: 'allow' },
  { tool: 'grep', policy: 'allow' },
  { tool: 'edit', policy: 'ask' },
  { tool: 'bash', policy: 'ask' },
  { tool: 'webfetch', policy: 'ask' },
]

/** // TODO: backend Fase 5 — persiste a política de permissões por ferramenta no forge-core. */
export async function toggleToolPolicy(tool: string): Promise<ToolPolicy> {
  await simulateLatency(200)
  const found = TOOL_POLICIES.find((p) => p.tool === tool)
  if (!found) throw new Error(`ferramenta desconhecida: ${tool}`)
  found.policy = found.policy === 'allow' ? 'ask' : 'allow'
  return found
}

export const SESSION_HEADER: SessionHeader = {
  model: 'claude-sonnet-5',
  agent: 'build',
  provider: 'anthropic',
  cacheOn: true,
  sessionId: 's7f3a1',
}
