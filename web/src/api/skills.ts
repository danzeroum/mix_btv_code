import { simulateLatency } from './client'
import type { McpServer, PermissionMatrixDecision, PermissionMatrixRow, SkillEntry } from '../types/domain'

export let SKILLS: SkillEntry[] = [
  { id: 'sql-explain', status: 'aprovado', detail: 'gera explicação de queries SQL' },
  { id: 'docker-scan', status: 'aprovado', detail: 'varre Dockerfile por vulnerabilidades' },
  { id: 'net-crawler', status: 'bloqueado', detail: 'gist · desconhecido' },
  { id: 'k6-load', status: 'em_analise', detail: 'teste de carga k6' },
]

export let MCP_SERVERS: McpServer[] = [
  { id: 'filesystem', status: 'ok' },
  { id: 'git', status: 'ok' },
  { id: 'postgres', status: 'pendente' },
]

export let PERMISSION_MATRIX: PermissionMatrixRow[] = [
  { tool: 'read', build: 'allow', plan: 'allow' },
  { tool: 'grep', build: 'allow', plan: 'allow' },
  { tool: 'edit', build: 'ask', plan: 'deny' },
  { tool: 'bash', build: 'ask', plan: 'deny' },
  { tool: 'webfetch', build: 'ask', plan: 'ask' },
]

const NEXT_DECISION: Record<PermissionMatrixDecision, PermissionMatrixDecision> = {
  allow: 'ask',
  ask: 'deny',
  deny: 'allow',
}

/**
 * Fase 6 Onda 3: lista as skills com o status REAL do vetter, do endpoint
 * `/api/skills` (forge-server → `forge-verify::vetter::list_skill_statuses`).
 * O status é read-only: o vetter decide (fail-closed), o usuário não sobrepõe.
 * Em falha (ex.: front rodando sem o backend em dev), cai no mock `SKILLS`.
 */
export async function fetchSkills(): Promise<SkillEntry[]> {
  try {
    const resp = await fetch('/api/skills')
    if (!resp.ok) return SKILLS
    return (await resp.json()) as SkillEntry[]
  } catch {
    return SKILLS
  }
}

/** // TODO: backend Fase 5 — persiste a matriz de permissões (tool × agent profile) no forge-core. */
export async function togglePermissionCell(tool: string, profile: 'build' | 'plan'): Promise<PermissionMatrixRow> {
  await simulateLatency(150)
  PERMISSION_MATRIX = PERMISSION_MATRIX.map((row) =>
    row.tool === tool ? { ...row, [profile]: NEXT_DECISION[row[profile]] } : row,
  )
  const found = PERMISSION_MATRIX.find((r) => r.tool === tool)
  if (!found) throw new Error('ferramenta não encontrada')
  return found
}

/** // TODO: backend Fase 5 — reconecta o servidor MCP real via forge-tools, atualiza a saúde do sidecar. */
export async function reconnectMcp(id: string): Promise<McpServer> {
  await simulateLatency(400)
  MCP_SERVERS = MCP_SERVERS.map((s) => (s.id === id ? { ...s, status: 'ok' } : s))
  const found = MCP_SERVERS.find((s) => s.id === id)
  if (!found) throw new Error('servidor MCP não encontrado')
  return found
}
