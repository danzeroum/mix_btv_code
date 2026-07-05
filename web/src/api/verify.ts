import { simulateLatency } from './client'
import type { ReviewerScore, VerifyStep } from '../types/domain'

export const VERIFY_STEPS: VerifyStep[] = [
  { name: 'cargo test --workspace', detail: '212 passed', ok: true, evidence: { passed: 212, failed: 0, duration_s: 8.4 } },
  { name: 'cargo clippy -- -D warnings', detail: '0 warnings', ok: true, evidence: { warnings: 0, denied_lints: ['-D warnings'] } },
  { name: 'cargo fmt --check', detail: 'formatado', ok: true, evidence: { files_checked: 42, diffs: 0 } },
  { name: 'pytest (python/)', detail: '148 passed', ok: true, evidence: { passed: 148, failed: 0, packages: ['forge-promptforge', 'forge-squad'] } },
  {
    name: 'paridade de hash (Rust × Python)',
    detail: 'fixtures batem',
    ok: true,
    evidence: { fixture: 'prompt-cache-key.v1', rust_hash: '9f3ac1e8', python_hash: '9f3ac1e8' },
  },
  { name: 'gitleaks (bloqueante)', detail: '0 segredos', ok: true, evidence: { secrets_found: 0, rules_version: '8.18.0' } },
]

export const VALUE_SCORE = 0.86
export const VALUE_GATE = 0.7

export const REVIEWERS: ReviewerScore[] = [
  { name: 'qualidade', score: 0.9, detail: 'cobertura de testes e clareza do diff acima do gate.' },
  { name: 'segurança', score: 0.84, detail: 'sem segredos expostos; permissões respeitam a matriz build/plan.' },
  { name: 'valor', score: 0.88, detail: 'entrega o pedido do usuário sem escopo extra.' },
  { name: 'manutenção', score: 0.82, detail: 'poucas abstrações novas; segue os padrões existentes do crate.' },
]

/** // TODO: backend Fase 5 — dispara `forge verify` real (hoje um stub no forge-cli), stream de progresso por passo. */
export async function runVerify(): Promise<VerifyStep[]> {
  await simulateLatency(800)
  return VERIFY_STEPS
}
