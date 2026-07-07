import { test, expect } from '@playwright/test'

/** Fase 7 Onda 14 (Designer, "salvar honesto"): prova a fronteira por
 * EXECUÇÃO. O grafo padrão (`initialNodes`/`initialEdges`, 8 nós + várias
 * arestas válidas) já é suficiente pra salvar — não precisa arrastar nem
 * conectar nada na tela pra exercitar `POST /api/designer/workflow` de
 * verdade. `seq` vem do MESMO `LedgerStore::append` que toda outra escrita
 * de auditoria da plataforma usa — nunca o `seq 248` fabricado que o mock
 * antigo sempre devolvia, e a cópia não promete mais "aplica na próxima
 * forge squad" (o orquestrador Python continua com os 5 agentes fixos).
 */
test('salvar o grafo padrão grava no ledger real — seq real, cópia honesta sobre aplicação', async ({ page }) => {
  await page.goto('/')
  await page.getByRole('button', { name: 'Squad Designer' }).click()
  await expect(page.getByRole('heading', { name: 'Squad Designer' })).toBeVisible()

  await page.getByRole('button', { name: 'salvar', exact: true }).click()

  const banner = page.getByText(/squad\.workflow\.v1 salvo/).first()
  await expect(banner).toBeVisible({ timeout: 10_000 })
  await expect(banner).toContainText('ledger seq')
  await expect(banner).toContainText('trabalho futuro')
  // O mock antigo prometia aplicação real e um seq fixo — nenhum dos dois
  // pode sobreviver por trás do backend real.
  await expect(banner).not.toContainText('aplica na próxima forge squad')
  await expect(page.getByText('seq 248')).toHaveCount(0)

  await expect(page.getByRole('button', { name: '✓ salvo' })).toBeVisible()
})

test('backend do designer fora do ar mostra erro explícito, não a confirmação fabricada', async ({ page }) => {
  await page.route('**/api/designer/workflow', (route) =>
    route.fulfill({ status: 500, contentType: 'application/json', body: '{"error":"boom","code":"forced_failure"}' }),
  )
  await page.goto('/')
  await page.getByRole('button', { name: 'Squad Designer' }).click()
  await expect(page.getByRole('heading', { name: 'Squad Designer' })).toBeVisible()

  await page.getByRole('button', { name: 'salvar', exact: true }).click()

  await expect(page.getByText('boom')).toBeVisible()
  await expect(page.getByText(/squad\.workflow\.v1 salvo/)).toHaveCount(0)
})
