/**
 * Fase 7 Onda 10 (A6): perfil de confinamento do sandbox Docker + saúde do
 * daemon. `GET /api/sandbox` mora no router mesclado de `forge-cli` (precisa
 * de `forge-tools`). Tela read-only — sem handlers de instalar/vetar/
 * habilitar/remover skill de terceiro (fora de escopo desta fase, o
 * protótipo do handoff também não os tem).
 */
import { fetchJson } from './client'

export interface SandboxProfile {
  image: string
  network_disabled: boolean
  mem_limit_mb: number
  cpu_quota: number
  timeout_secs: number
  rootfs_readonly: boolean
  cap_drop_all: boolean
  no_new_privileges: boolean
}

export interface SandboxInfo {
  profile: SandboxProfile
  /** Resultado real de `Sandbox::ping()` — `false` fail-closed sem daemon. */
  ping: boolean
}

export async function fetchSandbox(): Promise<SandboxInfo> {
  return fetchJson<SandboxInfo>('/api/sandbox')
}
