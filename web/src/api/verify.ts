import { simulateLatency } from './client'
import type { ReviewerScore, VerifyStep } from '../types/domain'

export const VERIFY_STEPS: VerifyStep[] = [
  { name: 'cargo test --workspace', detail: '212 passed', ok: true },
  { name: 'cargo clippy -- -D warnings', detail: '0 warnings', ok: true },
  { name: 'cargo fmt --check', detail: 'formatado', ok: true },
  { name: 'pytest (python/)', detail: '48 passed', ok: true },
  { name: 'paridade de hash (Rust × Python)', detail: 'fixtures batem', ok: true },
  { name: 'gitleaks (bloqueante)', detail: '0 segredos', ok: true },
]

export const VALUE_SCORE = 0.86
export const VALUE_GATE = 0.7

export const REVIEWERS: ReviewerScore[] = [
  { name: 'qualidade', score: 0.9 },
  { name: 'segurança', score: 0.84 },
  { name: 'valor', score: 0.88 },
  { name: 'manutenção', score: 0.82 },
]

/** // TODO: backend Fase 5 — dispara `forge verify` real (hoje um stub no forge-cli), stream de progresso por passo. */
export async function runVerify(): Promise<VerifyStep[]> {
  await simulateLatency(800)
  return VERIFY_STEPS
}
