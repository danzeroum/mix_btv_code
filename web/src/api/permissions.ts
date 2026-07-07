/** Matriz de permissão build/plan×tool e overrides persistidos — Fase 7
 * Onda 2. Fala com o router de `forge-cli` (crates/forge-cli/src/web_agent.rs),
 * não com `forge-server` — precisa de `forge-core` (PermissionEngine/Rule).
 */
import { fetchJson } from './client'
import type { AgentProfile, PermissionMatrixDecision, PermissionMatrixRow, PermissionRuleRecord } from '../types/domain'

export async function fetchMatrix(): Promise<PermissionMatrixRow[]> {
  return fetchJson<PermissionMatrixRow[]>('/api/permissions/matrix')
}

export async function listRules(): Promise<PermissionRuleRecord[]> {
  return fetchJson<PermissionRuleRecord[]>('/api/permissions/rules')
}

/**
 * Grava um override — sem `scopePrefix`, é uma regra de matriz (vale para
 * qualquer escopo do tool); com `scopePrefix`, é uma regra "sempre" restrita
 * ao pedido de permissão que a originou.
 */
export async function setRule(
  profile: AgentProfile,
  tool: string,
  decision: PermissionMatrixDecision,
  scopePrefix?: string,
): Promise<PermissionRuleRecord> {
  return fetchJson<PermissionRuleRecord>('/api/permissions/rules', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ profile, tool, scope_prefix: scopePrefix, decision }),
  })
}

export async function revokeRule(id: number): Promise<void> {
  await fetchJson<void>(`/api/permissions/rules/${id}`, { method: 'DELETE' })
}
