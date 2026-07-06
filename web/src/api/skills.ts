import { fetchJson } from './client'
import type { SkillEntry } from '../types/domain'

/**
 * Fase 6 Onda 3: lista as skills com o status REAL do vetter, do endpoint
 * `/api/skills` (forge-server → `forge-verify::vetter::list_skill_statuses`).
 * O status é read-only: o vetter decide (fail-closed), o usuário não sobrepõe.
 * Fase 7 Onda 2: sem fallback silencioso — uma falha de rede/backend vira
 * erro real (`AsyncStatus`), não um array mock disfarçado de dado real.
 */
export async function fetchSkills(): Promise<SkillEntry[]> {
  return fetchJson<SkillEntry[]>('/api/skills')
}
