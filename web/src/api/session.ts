/**
 * Fase 7 Onda 15 (fecho): `TOOL_POLICIES`/`toggleToolPolicy`/`SESSION_HEADER`
 * (mocks — política de ferramenta fake, provider/cache hardcoded) foram
 * removidos. Política de ferramenta por sessão é a MESMA matriz real que a
 * tela Skills já expõe (`fetchMatrix`, `api/permissions.ts`, Onda 2) — não
 * uma segunda cópia editável aqui; provider ativo vem de `fetchProviders`
 * (Onda 12). Ver `Sessao.tsx`.
 */
export type ToolCallStatus = 'running' | 'ok' | 'error'

export interface TranscriptTurn {
  id: string
  kind: 'user' | 'agent' | 'tool' | 'diff' | 'lint'
  text: string
  toolStatus?: ToolCallStatus
}
