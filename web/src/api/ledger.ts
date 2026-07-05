import { simulateLatency } from './client'
import type { LedgerEntry } from '../types/domain'

export const LEDGER_ENTRIES: LedgerEntry[] = [
  { seq: 247, ts: '18:32:07', actor: 'build', actorColor: 'ok', action: 'tool.bash · pytest', hashPrev: '9f3a', hashCurr: 'c1e8' },
  { seq: 246, ts: '18:30:41', actor: 'build', actorColor: 'ok', action: 'tool.edit · cpf_validator.py', hashPrev: 'a02b', hashCurr: '9f3a' },
  { seq: 245, ts: '18:12:09', actor: 'humano', actorColor: 'wire', action: 'permission.grant · bash', hashPrev: '4e10', hashCurr: 'a02b' },
  { seq: 244, ts: '18:02:55', actor: 'build', actorColor: 'ok', action: 'llm.call · sonnet-5', hashPrev: '5de9', hashCurr: '4e10' },
  { seq: 243, ts: '17:58:03', actor: 'humano', actorColor: 'wire', action: 'override · gate value_score', hashPrev: '3ab0', hashCurr: '5de9', flag: 'override' },
  { seq: 242, ts: '17:44:31', actor: 'auditor', actorColor: 'py', action: 'auditor review.reject · score 0.63', hashPrev: 'f117', hashCurr: '3ab0' },
  { seq: 241, ts: '17:30:02', actor: 'build', actorColor: 'ok', action: 'compaction · época 2', hashPrev: '0c8d', hashCurr: 'f117' },
]

export const LEDGER_TOTAL = 247

/** // TODO: backend Fase 5 — GET /api/ledger, lê o hash-chain real de crates/forge-store/src/ledger.rs. */
export async function getLedger(): Promise<LedgerEntry[]> {
  await simulateLatency(300)
  return LEDGER_ENTRIES
}

/** // TODO: backend Fase 5 — recomputa o hash-chain no forge-store e retorna o resultado real. */
export async function verifyChain(): Promise<{ ok: boolean; verified: number }> {
  await simulateLatency(600)
  return { ok: true, verified: LEDGER_TOTAL }
}
