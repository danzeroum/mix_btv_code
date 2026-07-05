import { simulateLatency } from './client'
import type { PermissionDecision, PermissionRequest } from '../types/domain'

export const PENDING_PERMISSION: PermissionRequest = {
  id: 'perm-1',
  tool: 'bash',
  scope: '$ python -m pytest tests/test_users.py',
}

/** // TODO: backend Fase 5 — resolve via forge-core PermissionClient, grava entrada no ledger real. */
export async function resolvePermission(
  _requestId: string,
  _decision: PermissionDecision,
): Promise<{ ledgerSeq: number }> {
  await simulateLatency(250)
  return { ledgerSeq: 248 }
}
