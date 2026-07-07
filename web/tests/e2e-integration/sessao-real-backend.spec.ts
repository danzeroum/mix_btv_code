import { test, expect } from '@playwright/test'

/** Fase 7 Onda 15 (fecho): a tela principal do produto (Sessão) era a única
 * sem prova de browser — coberta só a nível Rust
 * (`web_agent.rs::post_message_real_dispara_bash_via_modo_roteirizado` e
 * afins). Este spec dirige o fluxo completo pelo browser: mensagem → SSE
 * real (`text_delta`/`tool_started`) → pedido de permissão real (tela
 * Permissão, estado compartilhado via `SessionContext` acima da troca de
 * tela — não se perde ao navegar) → aprovar → volta pra Sessão → transcript
 * completo + `ledger íntegro: N entrada(s)` real, não fabricado. Roda em
 * modo roteirizado (`FORGE_SCRIPTED=1`, sem API key — mesmo processo real
 * que os outros specs desta suíte), dispara `bash` de verdade dentro do
 * sandbox de teste.
 */
test('mensagem real dispara bash, pede permissão, aprovar completa o turno e o ledger fica íntegro', async ({
  page,
}) => {
  await page.goto('/')
  await expect(page.getByRole('heading', { name: 'Sessão de código' })).toBeVisible()

  // Sidebar de ferramentas já reflete a matriz real (Onda 15: fim do
  // toggle fake) — bash "ask" é o default real do perfil build, sem
  // override nenhum.
  const bashRow = page.locator('div', { hasText: 'bash' }).last()
  await expect(bashRow.getByText('ask', { exact: true })).toBeVisible()

  await page.getByPlaceholder('mensagem para o agente…').fill('diga oi')
  await page.getByPlaceholder('mensagem para o agente…').press('Enter')

  await expect(page.locator('.mono', { hasText: 'você ▸ diga oi' })).toBeVisible()

  await page.getByRole('button', { name: 'Permissão' }).click()
  await expect(page.getByText('Permissão solicitada')).toBeVisible({ timeout: 10_000 })
  await expect(page.getByText('⚒ bash')).toBeVisible()
  await page.getByRole('button', { name: 'Permitir' }).click()

  await page.getByRole('button', { name: 'Sessão de código' }).click()

  await expect(page.locator('.mono', { hasText: 'forge ▸ pronto' })).toBeVisible({ timeout: 10_000 })
  // Ledger real, não um contador fabricado — bate por igualdade com uma
  // leitura independente do mesmo `.forge/forge.db` seria redundante com o
  // que `web_agent.rs`'s teste Rust já prova; aqui o que importa é que a UI
  // mostra ALGUM número real (não a linha "nenhum turno concluído" que
  // precede qualquer turno).
  await expect(page.getByText(/ledger íntegro: \d+ entrada\(s\)/)).toBeVisible({ timeout: 10_000 })
})
