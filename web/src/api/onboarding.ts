import { simulateLatency } from './client'

export interface EnvKeyStatus {
  name: string
  detected: boolean
  masked?: string
}

export const ENV_KEYS: EnvKeyStatus[] = [
  { name: 'ANTHROPIC_API_KEY', detected: true, masked: 'sk-ant-••••9f3a' },
  { name: 'DEEPSEEK_API_KEY', detected: false },
  { name: 'OPENAI_API_KEY', detected: false },
]

export const DOCTOR_OUTPUT = [
  '✓ ANTHROPIC_API_KEY definida',
  '○ DEEPSEEK_API_KEY ausente (fallback)',
  '○ OPENAI_API_KEY ausente (fallback)',
  '✓ uv encontrado — sidecar Python disponível',
  '✓ git repositório detectado',
  '✓ ledger não inicializado — será criado na 1ª sessão',
]

/** // TODO: backend Fase 5 — grava no clipboard via IPC do terminal alvo, hoje usa a Clipboard API do navegador. */
export async function copyToClipboard(text: string): Promise<void> {
  await simulateLatency(80)
  if (navigator.clipboard) await navigator.clipboard.writeText(text)
}
