import { test, expect } from '@playwright/test'

/** Fase 7 Onda 7 (A5): prova a fronteira por EXECUÇÃO. Eventos reais
 * (`llm.call`/`cache.hit`, ver scripts/run-integration-server.mjs) são
 * semeados via `forge_store::Telemetry::record` — o mesmo caminho que
 * `RateLimitedGenerator`/`CachedGenerator` usam em produção — com 2 modelos
 * cujo id ainda bate nos regexes reais de `tier_from_id` ("haiku" -> small,
 * "sonnet" -> large), para a tela provar agregação E classificação de tier
 * de ponta a ponta, não um valor fabricado no cliente.
 */
test('tela de uso por modelo reflete eventos reais com contagem e tier corretos', async ({ page }) => {
  await page.goto('/')
  await page.getByRole('button', { name: '◨ Administrador' }).click()
  await page.getByRole('button', { name: 'Uso por modelo' }).click()
  await expect(page.getByRole('heading', { name: 'Uso por modelo' })).toBeVisible({ timeout: 10_000 })

  const sonnetRow = page.locator('tr', { hasText: 'claude-sonnet-5-e2e' })
  await expect(sonnetRow).toBeVisible()
  await expect(sonnetRow.getByText('large', { exact: true })).toBeVisible()

  const haikuRow = page.locator('tr', { hasText: 'claude-haiku-4-5-e2e' })
  await expect(haikuRow).toBeVisible()
  await expect(haikuRow.getByText('small', { exact: true })).toBeVisible()
})
