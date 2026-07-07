/**
 * Fase 7 Onda 11: pipeline `/verify` real, rodando em background no `forge
 * dashboard`. `POST /api/verify/run` dispara o job (mesma config que `forge
 * verify`: `forge.toml` na raiz, ou `default_steps()` — espelha o job
 * `rust` do CI) e devolve um `run_id`; `GET /api/verify/:id` (polling)
 * acompanha o progresso real passo a passo até o veredito final. Execuções
 * concorrentes são serializadas: um segundo `POST` com job ativo devolve
 * `409` com o `run_id` já em andamento — o cliente trata 202 e 409 igual
 * (os dois dão um `run_id` para acompanhar via polling).
 */
import { ApiError, fetchJson } from './client'
import type { ReviewerScore } from '../types/domain'

/** "Review por valor" (`forge_review`'s gates/certification) ainda não
 * ligado nesta onda — escopo desta fase é só o job de `/verify` em
 * background (ver `docs/PLANO-FASE-7-frontend-primario.md`, Onda 11). */
export const VALUE_SCORE = 0.86
export const VALUE_GATE = 0.7

export const REVIEWERS: ReviewerScore[] = [
  { name: 'qualidade', score: 0.9, detail: 'cobertura de testes e clareza do diff acima do gate.' },
  { name: 'segurança', score: 0.84, detail: 'sem segredos expostos; permissões respeitam a matriz build/plan.' },
  { name: 'valor', score: 0.88, detail: 'entrega o pedido do usuário sem escopo extra.' },
  { name: 'manutenção', score: 0.82, detail: 'poucas abstrações novas; segue os padrões existentes do crate.' },
]

export interface Finding {
  tool: string
  severity: string
  message: string
  file?: string
  line?: number
}

export interface VerificationStep {
  name: string
  tool: string
  exit_code: number
  duration_ms: number
  findings: Finding[]
}

export type Verdict = 'pass' | 'fail' | 'skipped'

/** Espelha `forge_schemas::verification::VerificationEvidence`. */
export interface VerificationEvidence {
  run_id: string
  git_sha: string
  steps: VerificationStep[]
  verdict: Verdict
  produced_at: string
}

export interface VerifyRunStarted {
  run_id: string
}

export type VerifyStatus =
  | { status: 'running'; run_id: string; step: number; total: number }
  | { status: 'done'; run_id: string; evidence: VerificationEvidence }

export async function startVerifyRun(): Promise<VerifyRunStarted> {
  let response: Response
  try {
    response = await fetch('/api/verify/run', { method: 'POST' })
  } catch {
    throw new ApiError('falha de rede em /api/verify/run', 'network_error')
  }
  if (response.status === 202 || response.status === 409) {
    return (await response.json()) as VerifyRunStarted
  }
  throw new ApiError(`/api/verify/run respondeu ${response.status}`, `http_${response.status}`)
}

export async function fetchVerifyStatus(runId: string): Promise<VerifyStatus> {
  return fetchJson<VerifyStatus>(`/api/verify/${encodeURIComponent(runId)}`)
}
