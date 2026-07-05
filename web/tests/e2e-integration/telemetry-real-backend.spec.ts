import { test, expect } from '@playwright/test'

/** Prova a fronteira frontend↔backend por EXECUÇÃO, não só por paridade de
 * tipos: um `forge dashboard` real (Rust, sqlite de verdade) é subido pelo
 * webServer da config (ver scripts/run-integration-server.mjs), com um
 * evento `llm.call`/`e2e-integration` semeado via `forge-store::Telemetry`
 * antes do browser abrir. Se a tela de Telemetria mostrar esses valores,
 * o caminho browser → HTTP real → axum → sqlite → JSON → React está
 * exercitado de ponta a ponta, com os dois processos vivos.
 */
test('tela de telemetria reflete um evento real gravado por fora do browser', async ({ page }) => {
  await page.goto('/')

  await page.getByRole('button', { name: '◨ Administrador' }).click()
  await expect(page.getByRole('heading', { name: 'Telemetria' })).toBeVisible()

  // Identificadores do evento semeado (nome + session_id) só existem se a
  // página realmente leu do /api/events do processo forge-server real —
  // não são dados mock do frontend. "llm.call" aparece 2x (barra por tipo +
  // linha da tabela), daí .first(); session_id e a prop só existem na tabela.
  await expect(page.getByText('llm.call').first()).toBeVisible({ timeout: 10_000 })
  await expect(page.getByRole('cell', { name: 'e2e-integration' })).toBeVisible()

  // A prop do evento (`{"provider":"anthropic"}`) também vem do sqlite real.
  await expect(page.getByText(/anthropic/)).toBeVisible()
})
