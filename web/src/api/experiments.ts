/**
 * Fase 7 Onda 9 (A2): relatório de A/B sobre a telemetria real. `GET
 * /api/experiment/:nome` mora direto em `forge-server` (mesma classe de
 * posicionamento de A5 — só `forge-store`+`forge-schemas`). Nenhum código de
 * produção grava `props.experiment`/`variant`/`success` ainda — só testes e
 * `examples/seed_telemetry.rs`; a tela mostra dados semeados com um banner
 * explícito até a instrumentação existir de verdade.
 */
import { fetchJson } from './client'

export type ExperimentVerdict = 'significant' | 'inconclusive' | 'insufficient_data'

/** Espelha `forge_schemas::experiment::VariantStats`. */
export interface VariantStats {
  variant: string
  n: number
  successes: number
  rate: number
}

/** Espelha `forge_schemas::experiment::ExperimentReport` — a resposta de
 * `GET /api/experiment/:nome` é essa struct serializada direto, sem DTO
 * espelho. */
export interface ExperimentReport {
  experiment: string
  metric: string
  variants: VariantStats[]
  verdict: ExperimentVerdict
  winner?: string
  p_value: number
  produced_at: string
}

export async function fetchExperiment(nome: string): Promise<ExperimentReport> {
  return fetchJson<ExperimentReport>(`/api/experiment/${encodeURIComponent(nome)}`)
}
