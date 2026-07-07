import type { ModelTier, ModelTierId } from '../types/domain'

export const MODEL_TIERS: ModelTier[] = [
  { id: 'small', models: 'haiku · deepseek-chat', label: 'step-discipline' },
  { id: 'medium', models: 'gpt-4o · sonnet', label: '' },
  { id: 'large', models: 'claude-sonnet-5 · opus', label: '' },
]

/** Nome do modelo primário de um tier, para exibir no cabeçalho da sessão (ex.: "claude-sonnet-5 · opus" -> "claude-sonnet-5"). */
export function primaryModelName(tier: ModelTierId): string {
  const found = MODEL_TIERS.find((t) => t.id === tier)
  return found ? found.models.split(' · ')[0] : tier
}

/**
 * Fase 7 Onda 13: `selectTier`/`selectAgentProfile`/`selectAutonomy` (mocks
 * de `simulateLatency` que só devolviam o mesmo valor recebido) foram
 * removidos — tier/agente não são mais "selecionados" via uma chamada à
 * parte; a escolha feita em `Modelo.tsx` fica só no `AppContext` (parâmetro
 * por sessão/tarefa, mirroring do CLI — sem store de preferência novo) e é
 * aplicada de verdade quando a próxima mensagem é enviada
 * (`SessionContext::sendMessage`'s `model`/`agent`, que já chegam a
 * `SendMessageBody` real no Rust).
 *
 * `AUTONOMY_LEVELS` continua só informativo: `max_autonomy_level`
 * (`SquadTask`) é ignorado ponta-a-ponta pelo orquestrador Python hoje
 * (`ProgressiveAutonomyManager`/`agent_trust_scores` decide de verdade,
 * `hitl.py`) — descope explícito registrado na ADR 0021, não wireable sem
 * fabricar um efeito que não existe.
 */
export const AUTONOMY_LEVELS: { id: 'interativo' | 'automatico' | 'somente_leitura'; label: string; detail: string }[] = [
  {
    id: 'interativo',
    label: 'Interativo',
    detail: 'hoje: toda ferramenta "ask" pede confirmação — não há teto por tarefa.',
  },
  {
    id: 'automatico',
    label: 'Automático',
    detail: 'não implementado — o orquestrador não aceita um teto de autonomia por tarefa ainda.',
  },
  {
    id: 'somente_leitura',
    label: 'Somente leitura',
    detail: 'use o perfil de agente "plan" (acima) — edits já são negados por padrão nesse perfil.',
  },
]
