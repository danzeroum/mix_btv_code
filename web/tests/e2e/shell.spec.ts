import { test, expect } from '@playwright/test'

test('carrega o shell com topbar, sidebar e persona usuário por padrão', async ({ page }) => {
  await page.goto('/')

  await expect(page.getByText('Superfícies do usuário')).toBeVisible()
  await expect(page.getByRole('heading', { name: 'Sessão de código' })).toBeVisible()

  await page.screenshot({ path: 'tests/e2e/__screenshots__/shell-default-theme.png' })
})

test('trocar de persona muda a sidebar e a tela padrão', async ({ page }) => {
  await page.goto('/')
  await page.getByText('◨ Administrador').click()

  await expect(page.getByText('Painéis de administração')).toBeVisible()
  await expect(page.getByRole('heading', { name: 'Telemetria' })).toBeVisible()
})
