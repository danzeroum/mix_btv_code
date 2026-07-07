import { test, expect } from '@playwright/test'

/** Fase 7 Onda 12 (piso): prova a fronteira por EXECUÇÃO. O processo do
 * dashboard sobe com só `ANTHROPIC_API_KEY` setada (ver
 * scripts/run-integration-server.mjs, que remove as 3 chaves do env
 * herdado antes de redefinir só essa) — a tela reflete exatamente isso via
 * `GET /api/providers` (`Gateway::from_env`), não um status fabricado.
 * Tetos de rate limit vêm de `GET /api/ratelimit` (Onda 10/A4), reusado, não
 * reconstruído.
 */
test('tela de providers mostra quais providers têm key configurada de verdade', async ({ page }) => {
  await page.goto('/')
  await page.getByRole('button', { name: '◨ Administrador' }).click()
  await page.getByRole('button', { name: 'Providers & Limites' }).click()
  await expect(page.getByRole('heading', { name: 'Providers & rate limits' })).toBeVisible({ timeout: 10_000 })

  // Ordem fixa de fallback (anthropic -> deepseek -> openai): os 3 primeiros
  // `.mono` da tela são os ids dos providers (a legenda do rodapé, que
  // também usa `.mono` para "forge run"/"chat", vem depois no DOM).
  const monoSpans = page.locator('span.mono')
  await expect(monoSpans.nth(0)).toHaveText('anthropic')
  await expect(monoSpans.nth(1)).toHaveText('deepseek')
  await expect(monoSpans.nth(2)).toHaveText('openai')

  // Só a env var de anthropic foi definida pelo seed — exatamente 1
  // "configurado" e 2 "sem key", nunca os 3 "configurado" (o que
  // aconteceria se a tela fabricasse o status em vez de ler o real).
  await expect(page.getByText('configurado', { exact: true })).toHaveCount(1)
  await expect(page.getByText('sem key', { exact: true })).toHaveCount(2)

  const smallRow = page.locator('tr', { hasText: 'small' })
  await expect(smallRow).toBeVisible()
  await expect(smallRow.getByText('60', { exact: true })).toBeVisible()
})
