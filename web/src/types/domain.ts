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

export type AdminScreenId =
  | 'telemetria'
  | 'mcp'
  | 'modelos'
  | 'memoria'
  | 'experimentos'
  | 'ratelimit'
  | 'sandbox'
  | 'lsp'
  | 'ledger'
  | 'verify'
  | 'providers'
  | 'skills'

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

/** Espelha `forge_schemas::ledger::OverrideMark` — override é sempre um
 * campo de primeira classe na entrada em si, nunca inferido no cliente. */
export interface LedgerOverrideMark {
  marked: boolean
  reason?: string
}

/** Espelha `forge_schemas::ledger::LedgerEntry` — a resposta de `GET
 * /api/ledger` é essa struct serializada direto, sem DTO espelho. */
export interface LedgerEntry {
  seq: number
  prev_hash: string
  entry_hash: string
  kind: string
  actor: string
  payload: unknown
  override?: LedgerOverrideMark
  fake_marker?: string
  ts: string
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
  /** "builtin"/"third-party" (Fase 7 Onda 10) — a tela de sandbox filtra por
   * ele em vez de fazer parsing de `detail`. */
  source: string
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
