import { test, expect } from '@playwright/test'

/** Fase 7 Onda 11: prova a fronteira por EXECUÇÃO. `forge.toml` na raiz do
 * workDir (ver scripts/run-integration-server.mjs) declara 2 passos curtos
 * e determinísticos — clicar "rodar /verify" dispara `POST /api/verify/run`
 * de verdade (job em `spawn_blocking`), a tela mostra progresso real via
 * polling (`GET /api/verify/:id`) até o veredito final, sem nenhum
 * placeholder que pule direto pro fim.
 */
test('pipeline /verify roda em background e a tela acompanha até o veredito real', async ({ page }) => {
  await page.goto('/')
  await page.getByRole('button', { name: '◨ Administrador' }).click()
  await page.getByRole('button', { name: 'Verificação & Review' }).click()
  await expect(page.getByRole('heading', { name: 'Verificação & review por valor' })).toBeVisible({
    timeout: 10_000,
  })

  await page.getByRole('button', { name: 'rodar /verify' }).click()

  // Enquanto roda, o botão mostra "rodando…" e nenhum resultado antigo aparece.
  await expect(page.getByRole('button', { name: 'rodando…' })).toBeVisible()

  // Acompanha até concluir — os 2 passos semeados (ambos `sleep`, exit 0)
  // aparecem com ✓ e o veredito final é "pass".
  const passoUm = page.locator('button', { hasText: 'passo-um' })
  await expect(passoUm).toBeVisible({ timeout: 10_000 })
  const passoDois = page.locator('button', { hasText: 'passo-dois' })
  await expect(passoDois).toBeVisible()
  await expect(page.getByText('veredito:')).toBeVisible()
  await expect(page.getByText('pass', { exact: true })).toBeVisible()

  // Botão volta a ficar disponível para rodar de novo.
  await expect(page.getByRole('button', { name: 'rodar /verify' })).toBeVisible()

  // Expande um passo e confirma que o JSON real (não um placeholder) aparece.
  await passoUm.click()
  await expect(page.getByText('"exit_code": 0')).toBeVisible()
})
