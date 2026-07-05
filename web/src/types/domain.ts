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

export type PermissionDecision = 'allow' | 'deny' | 'always'

export interface PermissionRequest {
  id: string
  tool: string
  scope: string
}

export interface SquadAgentId {
  id: 'architect' | 'developer' | 'auditor' | 'designer' | 'ops'
}

export type SquadAgentState = 'concluido' | 'executando' | 'aguardando' | 'ocioso'

export interface SquadAgent {
  id: string
  name: string
  state: SquadAgentState
  /** null quando o agente está ocioso e ainda não votou nesta rodada. */
  confidence: number | null
  task: string
}

export interface ConsensusResult {
  strength: number
  decisionMaker: string
  dissent: { agent: string; score: number }[]
}

export interface PromptGenerator {
  id: string
  name: string
}

export interface SavedPrompt {
  id: string
  name: string
  favorite: boolean
  generator: string
  tags: string[]
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
