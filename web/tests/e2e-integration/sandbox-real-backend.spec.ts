import { test, expect } from '@playwright/test'

/** Fase 7 Onda 10 (A6): prova a fronteira por EXECUÇÃO. Perfil real de
 * `Sandbox::new` + as constantes hardcoded de `run_with` (rootfs read-only,
 * cap-drop ALL, no-new-privileges); a skill de terceiro semeada em
 * `.forge/skills/` (ver scripts/run-integration-server.mjs) aparece via
 * `/api/skills` real, filtrada por `source === 'third-party'`. **Não**
 * afirma um valor fixo para o status do daemon Docker — se há ou não um
 * daemon alcançável varia por ambiente (dev local vs. runner de CI); a
 * propriedade fail-closed determinística já está provada no nível de
 * `forge_tools::sandbox` (`ping_com_daemon_inalcancavel_e_false`).
 */
test('tela de sandbox mostra o perfil real e a skill de terceiro semeada', async ({ page }) => {
  await page.goto('/')
  await page.getByRole('button', { name: '◨ Administrador' }).click()
  await page.getByRole('button', { name: 'Sandbox & skills de terceiro' }).click()
  await expect(page.getByRole('heading', { name: 'Sandbox & skills de terceiro' })).toBeVisible({ timeout: 10_000 })

  await expect(page.getByText('python:3.11-slim')).toBeVisible()
  await expect(page.getByText('rede: desabilitada')).toBeVisible()
  await expect(page.getByText('512 MB')).toBeVisible()
  await expect(page.getByText('rootfs: read-only')).toBeVisible()
  await expect(page.getByText('cap-drop ALL')).toBeVisible()
  await expect(page.getByText('no-new-privileges')).toBeVisible()

  // Badge do daemon: "conectado" OU "indisponível" — exatamente um dos dois,
  // nunca nenhum (a rota sempre devolve um bool real).
  await expect(page.getByText(/daemon Docker (conectado|indisponível)/)).toBeVisible()

  const skillRow = page.locator('tr', { hasText: 'eco-terceiro' })
  await expect(skillRow).toBeVisible()
  await expect(skillRow.getByText('aprovado')).toBeVisible()
})
