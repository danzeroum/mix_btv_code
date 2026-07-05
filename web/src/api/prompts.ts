import { simulateLatency } from './client'
import type { PromptGenerator, SavedPrompt } from '../types/domain'

export const GENERATORS: PromptGenerator[] = [
  { id: 'code-review', name: 'code-review' },
  { id: 'test-gen', name: 'test-gen' },
  { id: 'refactor', name: 'refactor' },
  { id: 'commit-msg', name: 'commit-msg' },
  { id: 'adr-draft', name: 'adr-draft' },
]

let library: SavedPrompt[] = [
  { id: '1', name: 'review-seguranca', favorite: false, generator: 'code-review', tags: ['rust', 'security'] },
  { id: '2', name: 'review-diff-rapido', favorite: true, generator: 'code-review', tags: ['rust'] },
  { id: '3', name: 'pytest-edge', favorite: true, generator: 'test-gen', tags: ['python'] },
  { id: '4', name: 'commit-conv', favorite: false, generator: 'commit-msg', tags: ['git'] },
]

/** // TODO: backend Fase 5 — chama forge_promptforge via CoreService.Generate (gRPC), nunca LLM direto. */
export async function renderPrompt(generatorId: string): Promise<string> {
  await simulateLatency(300)
  return `> /prompt ${generatorId}\n[prompt renderizado — mock local, backend real via PromptForge]`
}

export async function listLibrary(): Promise<SavedPrompt[]> {
  await simulateLatency(150)
  return library
}

export async function savePrompt(name: string, generator: string): Promise<SavedPrompt> {
  await simulateLatency(200)
  const entry: SavedPrompt = { id: String(library.length + 1), name, favorite: false, generator, tags: [] }
  library = [...library, entry]
  return entry
}

export async function toggleFavorite(id: string): Promise<SavedPrompt> {
  await simulateLatency(120)
  library = library.map((p) => (p.id === id ? { ...p, favorite: !p.favorite } : p))
  const found = library.find((p) => p.id === id)
  if (!found) throw new Error('prompt não encontrado')
  return found
}

export async function removePrompt(id: string): Promise<void> {
  await simulateLatency(120)
  library = library.filter((p) => p.id !== id)
}
