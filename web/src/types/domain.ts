export type Persona = 'user' | 'admin'

export type UserScreenId =
  | 'onboarding'
  | 'sessao'
  | 'permissao'
  | 'modelo'
  | 'prompts'
  | 'squad'
  | 'designer'
  | 'sugestoes'

export type AdminScreenId = 'telemetria' | 'ledger' | 'verify' | 'providers' | 'skills'

export type ScreenId = UserScreenId | AdminScreenId

export interface NavItem {
  id: ScreenId
  icon: string
  label: string
  hint: string
}

export type AgentProfile = 'build' | 'plan'
export type ModelTierId = 'small' | 'medium' | 'large'
export type AutonomyLevel = 'interativo' | 'automatico' | 'somente_leitura'

export interface ModelTier {
  id: ModelTierId
  models: string
  label: string
}

export interface LedgerEntry {
  seq: number
  ts: string
  actor: string
  actorColor: 'ok' | 'wire' | 'py'
  action: string
  hashPrev: string
  hashCurr: string
  flag?: 'override'
}

export interface VerifyStep {
  name: string
  detail: string
  ok: boolean
  evidence: Record<string, unknown>
}

export interface ReviewerScore {
  name: string
  score: number
  detail: string
}

export interface ProviderInfo {
  id: string
  name: string
  status: 'ativo' | 'standby'
}

export interface RateLimitTier {
  tier: ModelTierId
  used: number
  cap: number
}

export interface SkillEntry {
  id: string
  status: 'aprovado' | 'bloqueado' | 'em_analise'
  detail: string
}

export interface McpServer {
  id: string
  status: 'ok' | 'pendente'
}

export type PermissionMatrixDecision = 'allow' | 'ask' | 'deny'

export interface PermissionMatrixRow {
  tool: string
  build: PermissionMatrixDecision
  plan: PermissionMatrixDecision
}

/** Override persistido (Fase 7 Onda 2) — espelha `forge_store::RuleRecord`. */
export interface PermissionRuleRecord {
  id: number
  profile: AgentProfile
  tool: string
  scope_prefix?: string
  decision: PermissionMatrixDecision
  created_at: string
}

// --- Squad Designer ---

export type DesignerNodeKind = 'card' | 'pill'

export interface DesignerNodeParam {
  k: string
  v: string
}

export interface DesignerNode {
  id: string
  x: number
  y: number
  kind: DesignerNodeKind
  name: string
  role: string
  color: string
  icon: string
  sub: string
  params: DesignerNodeParam[]
  removable: boolean
}

export interface DesignerEdge {
  from: string
  to: string
  label?: string
}
