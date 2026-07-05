import { simulateLatency } from './client'
import type { McpServer, PermissionMatrixDecision, PermissionMatrixRow, SkillEntry } from '../types/domain'

export let SKILLS: SkillEntry[] = [
  { id: 'sql-explain', status: 'aprovado', detail: 'gera explicação de queries SQL' },
  { id: 'docker-scan', status: 'aprovado', detail: 'varre Dockerfile por vulnerabilidades' },
  { id: 'net-crawler', status: 'bloqueado', detail: 'gist · desconhecido' },
  { id: 'k6-load', status: 'em_analise', detail: 'teste de carga k6' },
]

export const MCP_SERVERS: McpServer[] = [
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

/** // TODO: backend Fase 5 — grava decisão de vetting no ledger + na config do skill-vetter real. */
export async function vetSkill(id: string, decision: SkillEntry['status']): Promise<SkillEntry> {
  await simulateLatency(250)
  SKILLS = SKILLS.map((s) => (s.id === id ? { ...s, status: decision } : s))
  const found = SKILLS.find((s) => s.id === id)
  if (!found) throw new Error('skill não encontrada')
  return found
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
