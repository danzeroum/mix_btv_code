import { test, expect } from '@playwright/test'

/** Fase 7 Onda 2 (remanescente): a matriz de permissão e a lista de regras
 * ativas deixaram de ser mock local — este teste prova a fronteira
 * browser → fetch real → axum (`--web-agent`) → `RuleStore`/`PermissionEngine`
 * → JSON → React, incluindo o modal de confirmação obrigatório ("nunca um
 * clique único e opaco") e a revogação. O rastro no ledger em si já é
 * provado a nível Rust (`web_agent::tests::post_rule_persiste_matriz_reflete_e_ledger_audita`);
 * aqui o que importa é a UI de verdade, não uma segunda cópia do mesmo teste.
 */
test('editar uma célula da matriz grava um override real, mostra na lista de regras ativas, e revogar reverte', async ({
  page,
}) => {
  await page.goto('/')
  await page.getByRole('button', { name: '◨ Administrador' }).click()
  await page.getByRole('button', { name: 'Skills & Permissões' }).click()
  await expect(page.getByRole('heading', { name: 'Skills & permissões' })).toBeVisible()

  const bashRow = page.locator('tbody tr', { hasText: 'bash' })
  const planCell = bashRow.locator('button').nth(1)
  // Default REAL do perfil `plan` (`forge_core::PLAN`/`read_only()`): bash é
  // "ask", não "deny" — precisão sobre o mock antigo que essa tela substitui.
  await expect(planCell).toHaveText('ask')
  await expect(page.getByText('nenhum override persistido')).toBeVisible()

  await planCell.click()
  await expect(page.getByText('Confirmar mudança de permissão')).toBeVisible()
  // O escopo (tool + perfil) aparece explícito no modal antes de confirmar —
  // nunca um clique único e opaco.
  await expect(page.getByText('ferramenta: bash')).toBeVisible()
  await expect(page.getByText('perfil: plan')).toBeVisible()
  await page.getByRole('button', { name: 'confirmar' }).click()

  await expect(planCell).toHaveText('deny')
  const revokeButton = page.getByRole('button', { name: 'revogar' })
  await expect(revokeButton).toBeVisible()

  await revokeButton.click()
  await expect(page.getByText('nenhum override persistido')).toBeVisible()
  // Sem override, o efetivo volta ao default REAL do perfil (ask), não
  // "fica preso" na última escolha nem reseta para um valor fabricado.
  await expect(planCell).toHaveText('ask')
})

/** Quarto teste da fronteira da Onda 2: backend fora do ar mostra estado de
 * erro explícito na tela Skills, não o array mock antigo (`fetchSkills`
 * perdeu o fallback silencioso). Intercepta só `/api/skills` — o resto do
 * backend (dashboard, matriz) continua real.
 */
test('backend de skills fora do ar mostra erro explícito, não o array mock antigo', async ({ page }) => {
  await page.route('**/api/skills', (route) =>
    route.fulfill({ status: 500, contentType: 'application/json', body: '{"error":"boom","code":"forced_failure"}' }),
  )
  await page.goto('/')
  await page.getByRole('button', { name: '◨ Administrador' }).click()
  await page.getByRole('button', { name: 'Skills & Permissões' }).click()

  await expect(page.getByText('boom')).toBeVisible()
  await expect(page.getByRole('button', { name: 'tentar de novo' })).toBeVisible()
  // O mock antigo tinha "sql-explain" fixo — não pode sobreviver por trás do erro.
  await expect(page.getByText('sql-explain')).toHaveCount(0)
})
