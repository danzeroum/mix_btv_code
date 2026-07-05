import { simulateLatency } from './client'
import type { DesignerEdge, DesignerNode } from '../types/domain'

export interface SaveWorkflowResult {
  seq: number
  workflowId: string
}

/** // TODO: backend Fase N — POST squad.workflow.v1 → schema validado → ledger append → orquestrador aplica na próxima `forge squad`. */
export async function saveWorkflow(_graph: { nodes: DesignerNode[]; edges: DesignerEdge[] }): Promise<SaveWorkflowResult> {
  await simulateLatency(500)
  return { seq: 248, workflowId: 'squad.workflow.v1' }
}
