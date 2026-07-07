/**
 * Fase 7 Onda 14 (Designer, "salvar honesto"): `POST /api/designer/workflow`
 * valida o grafo contra `squad.workflow.v1` (schema + integridade de
 * arestas) e grava no ledger de verdade — `seq` real, não fabricado.
 * "Aplica na próxima forge squad" nunca foi real (o orquestrador Python
 * continua com os 5 agentes fixos, sem reescrita nesta fase) — a resposta
 * e a cópia da tela dizem só "salvo e validado".
 */
import { fetchJson } from './client'
import type { DesignerEdge, DesignerNode } from '../types/domain'

export interface SaveWorkflowResult {
  seq: number
  workflowId: string
}

export async function saveWorkflow(graph: { nodes: DesignerNode[]; edges: DesignerEdge[] }): Promise<SaveWorkflowResult> {
  const result = await fetchJson<{ seq: number; workflow_id: string }>('/api/designer/workflow', {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify(graph),
  })
  return { seq: result.seq, workflowId: result.workflow_id }
}
