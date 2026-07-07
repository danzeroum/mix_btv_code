import { test, expect } from '@playwright/test'

/** Fase 7 Onda 9 (A2): prova a fronteira por EXECUÇÃO. 2 variantes reais
 * (`props.experiment`/`variant`/`success`, ver scripts/run-integration-server.mjs)
 * são semeadas via `forge_store::Telemetry::record` com diferença grande o
 * bastante (18/20 vs 6/20) pro teste z ser significativo por construção — a
 * tela busca pelo nome, chama `GET /api/experiment/:nome` de verdade e mostra
 * o veredito derivado, não fabricado.
 */
test('tela de experimentos busca por nome e mostra o veredito real do teste z', async ({ page }) => {
  await page.goto('/')
  await page.getByRole('button', { name: '◨ Administrador' }).click()
  await page.getByRole('button', { name: 'Experimentos A/B' }).click()
  await expect(page.getByRole('heading', { name: 'Experimentos A/B' })).toBeVisible({ timeout: 10_000 })

  await page.getByPlaceholder('nome do experimento (props.experiment)').fill('e2e-experiment')
  await page.getByRole('button', { name: 'buscar' }).click()

  await expect(page.getByText('significativo — vencedor: controle')).toBeVisible({ timeout: 10_000 })
  const controleRow = page.locator('tr', { hasText: 'controle' })
  await expect(controleRow.getByText('90.0%')).toBeVisible()
  const tratamentoRow = page.locator('tr', { hasText: 'tratamento' })
  await expect(tratamentoRow.getByText('30.0%')).toBeVisible()
})
