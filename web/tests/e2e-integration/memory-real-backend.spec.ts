import { test, expect } from '@playwright/test'

/** Fase 7 Onda 8 (A3): prova a fronteira por EXECUÇÃO contra o `forge
 * dashboard` real com o `MemoryService` (sidecar Python real, ADR 0022) por
 * trás. O corpus (ver scripts/run-integration-server.mjs) é semeado DIRETO
 * no caminho que o sidecar real usa, sob um agente dedicado
 * (`e2e-memory-agent`) — nenhum outro spec desta suíte toca
 * `.forge/squad-memory`, então a asserção de contagem é robusta independente
 * da ordem de execução dos arquivos de teste.
 */
test('mapa de memória mostra o agente semeado e a busca léxica recupera por termo', async ({ page }) => {
  await page.goto('/')
  await page.getByRole('button', { name: '◨ Administrador' }).click()
  await page.getByRole('button', { name: 'Memória do squad' }).click()
  await expect(page.getByRole('heading', { name: 'Memória do squad' })).toBeVisible({ timeout: 10_000 })

  // O mapa agrupado por agente reflete o corpus real (contagem/decisão reais).
  const row = page.locator('tr', { hasText: 'e2e-memory-agent' })
  await expect(row).toBeVisible()
  await expect(row.getByText(/arquitetura do gateway/)).toBeVisible()

  // Busca léxica real: termos em comum recuperam a memória semeada.
  await page.getByPlaceholder('o que o squad já decidiu sobre...').fill('plano de arquitetura')
  await page.getByRole('button', { name: 'buscar' }).click()
  await expect(page.getByText(/score 0\./)).toBeVisible({ timeout: 10_000 })
  await expect(page.getByText('e2e-memory-agent').first()).toBeVisible()
})
