import { test, expect } from '@playwright/test'

/** Fase 7 Onda 10 (A7): prova a fronteira por EXECUÇÃO. `.forge/lsp.toml`
 * (ver scripts/run-integration-server.mjs) declara um comando LSP
 * INEXISTENTE — a tela mostra o servidor declarado, sempre "declarado, não
 * iniciado", sem que nenhum processo suba (mesma prova que
 * `skills.rs`'s `lsp_server_declarado_registra_tres_consultas_lazy` já faz
 * no nível do registry, agora pelo browser: a página carrega normalmente,
 * sem travar/parar esperando um probe que este design deliberadamente não
 * faz).
 */
test('tela de language servers mostra o declarado sem subir processo', async ({ page }) => {
  await page.goto('/')
  await page.getByRole('button', { name: '◨ Administrador' }).click()
  await page.getByRole('button', { name: 'Language servers' }).click()
  await expect(page.getByRole('heading', { name: 'Language servers' })).toBeVisible({ timeout: 10_000 })

  const row = page.locator('tr', { hasText: 'rust' })
  await expect(row).toBeVisible()
  await expect(row.getByText('comando-lsp-inexistente-xyz')).toBeVisible()
  await expect(row.getByText('--stdio')).toBeVisible()
  await expect(row.getByText('declarado, não iniciado')).toBeVisible()
})
