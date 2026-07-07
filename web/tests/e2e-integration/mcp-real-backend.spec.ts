import { test, expect } from '@playwright/test'

/** Fase 7 Onda 7 (A1): prova a fronteira por EXECUÇÃO contra o `forge
 * dashboard` real. `.forge/mcp.toml` (ver scripts/run-integration-server.mjs)
 * declara 2 servidores: "vivo" aponta pro fixture MCP REAL (mesmo bin que
 * `forge-tools/tests/mcp_integration.rs` usa via handshake de verdade) e
 * "morto" aponta pra um comando inexistente — a tela precisa mostrar os dois
 * status corretamente no MESMO probe. O override é gravado aqui via a rota
 * REAL `/api/permissions/rules` (mesmo caminho que a tela de Skills usa),
 * não injetado direto no sqlite — prova que o preview de política da tela
 * MCP lê o mesmo store que a matriz de permissões grava.
 *
 * O override é revogado no `finally` — o `rules.db` é compartilhado por todo
 * o `webServer` da suíte (`permissions-real-backend.spec.ts` roda no MESMO
 * processo e assume a lista de overrides vazia no início do seu teste); sem
 * a limpeza, esta prova deixaria rastro cross-spec.
 */
test('console MCP mostra status real dos 2 servidores e o preview de política reflete um override real', async ({
  page,
}) => {
  // Override real ANTES de abrir a tela: mcp__vivo__echo sempre "allow" pro build.
  const setRule = await page.request.post('/api/permissions/rules', {
    data: { profile: 'build', tool: 'mcp__vivo__echo', decision: 'allow' },
  })
  expect(setRule.ok()).toBe(true)
  const { id: ruleId } = await setRule.json()

  try {
    await page.goto('/')
    await page.getByRole('button', { name: '◨ Administrador' }).click()
    await page.getByRole('button', { name: 'Console MCP' }).click()
    await expect(page.getByRole('heading', { name: 'Console MCP' })).toBeVisible()

    // Exatamente 1 servidor online (handshake MCP real) e 1 offline (comando
    // inexistente) — únicos no probe, então o texto global já é inequívoco.
    await expect(page.getByText('vivo', { exact: true })).toBeVisible({ timeout: 15_000 })
    await expect(page.getByText('online', { exact: true })).toBeVisible()
    await expect(page.getByText('morto', { exact: true })).toBeVisible()
    await expect(page.getByText('offline', { exact: true })).toBeVisible()

    // A tool "echo" (única que o fixture anuncia), namespaced.
    const echoRow = page.locator('tr', { hasText: 'mcp__vivo__echo' })
    await expect(echoRow).toBeVisible()
    // Override real vence: build vira "allow" (não o "ask" default do perfil);
    // plan não tem override, cai no default real (ask).
    await expect(echoRow.getByText('allow', { exact: true })).toBeVisible()
    await expect(echoRow.getByText('ask', { exact: true })).toBeVisible()
  } finally {
    await page.request.delete(`/api/permissions/rules/${ruleId}`)
  }
})
