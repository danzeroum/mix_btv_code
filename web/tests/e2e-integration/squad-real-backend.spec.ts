import { test, expect } from '@playwright/test'

/** Fase 7 Onda 4: prova a fronteira browser → `POST /api/squad/run` → SSE
 * real → `SquadService.ExecuteTask` (squad Python real, sem API key via
 * `FORGE_SCRIPTED=1` — ver `run-integration-server.mjs`). O consenso
 * roteirizado usa confiança 0.5 uniforme de propósito (mesma receita do
 * teste Rust `run_squad_via_http_com_gate_hitl_real_e_ledger`) — fraco o
 * bastante para exigir HITL de verdade. O registro no ledger em si já é
 * provado a nível Rust por aquele teste; aqui o que importa é a UI real:
 * agentes aparecendo ao vivo (não um array estático), o gate bloqueando de
 * verdade até a UI resolver, e o stream encerrando sozinho (a correção desta
 * onda para o SSE que nunca terminava).
 */
test('squad ao vivo: agentes aparecem em tempo real, gate HITL bloqueia até a UI resolver, stream encerra sozinho', async ({
  page,
}) => {
  await page.goto('/')
  await page.getByRole('button', { name: 'Squad ao vivo' }).click()
  await expect(page.getByRole('heading', { name: 'Squad ao vivo' })).toBeVisible()

  await page.getByRole('button', { name: 'rodar' }).click()

  // Cada agente aparece conforme sua Proposal real chega pelo SSE — prova
  // que a lista é derivada de eventos ao vivo, não o array mock antigo
  // (SQUAD_AGENTS fixo com os 5 agentes sempre presentes). Timeout maior
  // só neste primeiro — cobre o pior caso de `SquadPool::acquire` (até 30s
  // de `wait_ready`, ver `default_squad_pool`) + o pipeline de /verify
  // antes dele; os agentes seguintes chegam no mesmo stream já aberto.
  await expect(page.getByText('architect', { exact: true })).toBeVisible({ timeout: 45_000 })
  await expect(page.getByText('developer', { exact: true })).toBeVisible({ timeout: 20_000 })
  await expect(page.getByText('auditor', { exact: true })).toBeVisible({ timeout: 20_000 })

  // Consenso roteirizado (confiança 0.5 uniforme) é fraco de propósito —
  // "consenso fraco — HITL", não um veredito fabricado de sucesso.
  await expect(page.getByText('consenso fraco — HITL')).toBeVisible({ timeout: 10_000 })

  // O gate aparece com o motivo/confiança REAIS emitidos pelo orquestrador
  // (HitlEscalation), não um texto estático ("ação crítica requer...").
  await expect(page.getByText(/weak_consensus/)).toBeVisible({ timeout: 10_000 })

  // Antes de resolver, o orquestrador está genuinamente bloqueado — nenhum
  // handoff/passo de execução apareceu ainda (match exato: "squad em
  // execução" do card "Fallback progressivo" já está na tela e não deve
  // ser confundido com o título do card de execução).
  await expect(page.getByText('Execução', { exact: true })).toHaveCount(0)

  await page.getByRole('button', { name: 'Aprovar' }).click()

  // Só depois do clique real na UI o orquestrador retoma e a fase de ops
  // aparece — prova que o gate bloqueava de verdade, não era cosmético.
  await expect(page.getByText('Execução', { exact: true })).toBeVisible({ timeout: 10_000 })
  await expect(page.getByText(/ops step 1/)).toBeVisible({ timeout: 10_000 })

  // O stream termina sozinho quando a tarefa acaba (correção desta onda —
  // antes disto, a conexão SSE ficava pendurada para sempre mesmo com o
  // squad já concluído). O texto aparece em 2 lugares (task_id + status) —
  // `.first()` basta, o que importa é que pelo menos uma reflita o fim real.
  await expect(page.getByText('stream encerrado').first()).toBeVisible({ timeout: 10_000 })
})
