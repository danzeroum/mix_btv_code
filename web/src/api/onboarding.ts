/**
 * Fase 7 Onda 13 (Modelo & Onboarding): `ENV_KEYS`/`DOCTOR_OUTPUT` (arrays
 * estáticos, sempre "tudo verde" exceto os fallbacks marcados de propósito)
 * saem do mock — `GET /api/doctor` (`forge-cli`, `doctor_console.rs`) agrega
 * as 4 checagens reais (providers do gateway, `uv --version`, ping Docker,
 * git). A checagem de providers aqui é só o resumo agregado (`N/3
 * configurado(s)`) — nunca um preview de key mascarada (o backend nunca
 * expôs isso, "keys só no Rust", e esta onda não muda essa fronteira). O
 * detalhe por provider individual mora na tela Providers (Onda 12) — não
 * duplicado aqui; esta onda foi deliberadamente aberta sem depender do
 * merge da Onda 12 (branches sem sobreposição de arquivo), então não
 * importa `api/providers.ts` daquela onda.
 */
import { fetchJson } from './client'

export interface DoctorCheck {
  id: 'providers' | 'uv' | 'docker' | 'git'
  ok: boolean
  detail: string
}

export async function fetchDoctor(): Promise<DoctorCheck[]> {
  const view = await fetchJson<{ checks: DoctorCheck[] }>('/api/doctor')
  return view.checks
}

export async function copyToClipboard(text: string): Promise<void> {
  if (navigator.clipboard) await navigator.clipboard.writeText(text)
}
