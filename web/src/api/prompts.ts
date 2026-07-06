/**
 * Fase 7 Onda 5: cliente da biblioteca de prompts + render. Metade CRUD
 * (`/api/prompts*`) fala com `forge-server` (só `forge-store` — mesma
 * classe de `/api/skills`); metade `render`/`generators` (`/api/prompt/*`)
 * fala com o router mesclado de `forge-cli` (precisa de `forge-sidecar`
 * para chegar no sidecar PromptForge) — duas rotas distintas, mesmo
 * `fetchJson`.
 */
import { fetchJson } from './client'

export interface SavedPrompt {
  id: number
  name: string
  generator: string
  fields: Record<string, unknown>
  rendered: string
  tags: string[]
  favorite: boolean
  created_at: string
}

/** Espelha `forge_proto::promptforge::GeneratorField` — descreve um campo que o gerador espera. */
export interface GeneratorField {
  name: string
  label: string
  required: boolean
  placeholder: string
}

/** Espelha `forge_proto::promptforge::GeneratorInfo` — a lista vem do sidecar PromptForge real, não fabricada aqui. */
export interface GeneratorInfo {
  name: string
  category: string
  fields: GeneratorField[]
}

export async function listLibrary(tag?: string): Promise<SavedPrompt[]> {
  const qs = tag ? `?tag=${encodeURIComponent(tag)}` : ''
  return fetchJson<SavedPrompt[]>(`/api/prompts${qs}`)
}

export interface SavePromptInput {
  name: string
  generator: string
  fields: Record<string, string>
  rendered: string
  tags: string[]
}

export async function savePrompt(input: SavePromptInput): Promise<SavedPrompt> {
  return fetchJson<SavedPrompt>('/api/prompts', {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify(input),
  })
}

export async function toggleFavorite(id: number): Promise<{ favorite: boolean }> {
  return fetchJson(`/api/prompts/${id}/favorite`, { method: 'POST' })
}

export async function removePrompt(id: number): Promise<void> {
  await fetchJson(`/api/prompts/${id}`, { method: 'DELETE' })
}

export async function listGenerators(): Promise<GeneratorInfo[]> {
  return fetchJson<GeneratorInfo[]>('/api/prompt/generators')
}

export async function renderPrompt(generator: string, fields: Record<string, string>): Promise<string> {
  const { prompt } = await fetchJson<{ prompt: string }>('/api/prompt/render', {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify({ generator, fields }),
  })
  return prompt
}
