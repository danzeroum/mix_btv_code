import { test, expect } from '@playwright/test'

/** Fase 7 Onda 10 (A4): prova a fronteira por EXECUÇÃO. Os tetos vêm de
 * `RateLimiter::for_tier` (`rate_limit.rs`, constantes hardcoded reais: small
 * 60, medium 30, large 15, todos por janela de 600s) — a tela reflete essa
 * config real, não um valor fabricado no cliente.
 */
test('tela de rate limits mostra os tetos reais por tier', async ({ page }) => {
  await page.goto('/')
  await page.getByRole('button', { name: '◨ Administrador' }).click()
  await page.getByRole('button', { name: 'Rate limits' }).click()
  await expect(page.getByRole('heading', { name: 'Rate limits' })).toBeVisible({ timeout: 10_000 })

  const rows: Array<[string, string]> = [
    ['small', '60'],
    ['medium', '30'],
    ['large', '15'],
  ]
  for (const [tier, cap] of rows) {
    const row = page.locator('tr', { hasText: tier })
    await expect(row).toBeVisible()
    await expect(row.getByText(cap, { exact: true })).toBeVisible()
    await expect(row.getByText('600s')).toBeVisible()
  }
})
