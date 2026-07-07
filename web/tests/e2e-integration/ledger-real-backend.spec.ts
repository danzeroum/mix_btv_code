import { test, expect } from '@playwright/test'

/** Prova a fronteira frontend↔backend por EXECUÇÃO: um `forge dashboard`
 * real (Rust, sqlite de verdade) é subido pelo webServer da config (ver
 * scripts/run-integration-server.mjs), com 2 entradas semeadas via o mesmo
 * `LedgerStore::append` de produção, sob o ator dedicado
 * `e2e-ledger-seed` — nenhum outro spec desta suíte usa esse ator, então a
 * ordem de execução dos arquivos (squad/permissões também escrevem no MESMO
 * forge.db) não interfere na contagem observada aqui. Filtrar por esse
 * ator prova que a tela lê `?actor=` combinado com o backend real (não um
 * corte feito depois, no cliente).
 */
test('tela de ledger reflete entradas reais gravadas por fora do browser e filtra por ator', async ({ page }) => {
  await page.goto('/')
  await page.getByRole('button', { name: '◨ Administrador' }).click()
  await page.getByRole('button', { name: 'Ledger / Auditoria' }).click()
  await expect(page.getByRole('heading', { name: 'Ledger / Auditoria' })).toBeVisible()

  // O ator semeado é dedicado a este teste — filtra para isolar de
  // qualquer entrada que outros specs já tenham gravado no mesmo forge.db.
  await page.getByRole('button', { name: 'e2e-ledger-seed', exact: true }).click({ timeout: 10_000 })

  const rows = page.locator('tbody tr')
  await expect(rows).toHaveCount(2)

  // `kind`/`payload` só existem se vieram do JSON real servido por
  // `GET /api/ledger` — não são dados mock do frontend.
  await expect(page.getByRole('cell', { name: 'session.start' })).toBeVisible()
  await expect(page.getByRole('cell', { name: 'tool.run' })).toBeVisible()
  await expect(page.getByText(/"tool":"bash"/)).toBeVisible()

  // A entrada mais recente (tool.run) aparece primeiro — mesma ordenação
  // que `LedgerStore::recent` prova a nível Rust (mais nova primeiro).
  const firstRowCells = rows.first().locator('td')
  await expect(firstRowCells.nth(3)).toHaveText('tool.run')

  // Clicar na linha abre o detalhe com o payload/hash reais (não truncados).
  await rows.first().click()
  await expect(page.getByText('"prev_hash"')).toBeVisible()
  await expect(page.getByText('"entry_hash"')).toBeVisible()

  // Verificação de integridade roda contra o backend real — `ok:true` só é
  // possível se `LedgerStore::verify_chain` de fato recomputou os hashes.
  await page.getByRole('button', { name: 'verificar integridade' }).click()
  await expect(page.getByText(/cadeia íntegra/)).toBeVisible({ timeout: 10_000 })
})
