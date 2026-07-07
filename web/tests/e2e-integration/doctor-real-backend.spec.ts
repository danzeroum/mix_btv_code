import { test, expect } from '@playwright/test'

/** Fase 7 Onda 13 (Modelo & Onboarding): prova a fronteira do doctor por
 * EXECUÇÃO. `GET /api/doctor` (`forge-cli`, `doctor_console.rs`) agrega 4
 * checagens reais — a tela deixa de mostrar `DOCTOR_OUTPUT` fabricado
 * (sempre "tudo verde").
 *
 * `uv`/`git` são as duas checagens com valor determinístico NESTE fixture,
 * por motivos opostos e ambos genuínos (não hardcoded pra conveniência do
 * teste): `uv` está de verdade no PATH herdado pelo processo do dashboard
 * (o job `web` do CI instala via `astral-sh/setup-uv@v5`, precondição já
 * usada pelo squad e2e) — gêmeo POSITIVO. `git`, ao contrário, é um gêmeo
 * NEGATIVO real: o dashboard roda com `cwd` num diretório temporário
 * (`run-integration-server.mjs`'s `workDir`), que nunca é um repositório
 * git — `git rev-parse HEAD` falha de verdade ali, então o doctor deve
 * mostrar "ausente", não "tudo verde" por padrão. `docker`/`providers` não
 * têm valor fixo afirmado aqui: `docker` varia por ambiente (mesma cautela
 * já usada em `sandbox-real-backend.spec.ts`); a fronteira determinística
 * de `providers` já está provada a nível Rust
 * (`doctor_agrega_as_4_checagens_com_providers_real`, com isolamento de env
 * var) — reafirmar aqui seria uma segunda cópia do mesmo teste.
 */
test('tela de onboarding mostra o doctor real — uv presente, git ausente (workDir não é repo)', async ({ page }) => {
  await page.goto('/')
  await page.getByRole('button', { name: 'Primeiros passos' }).click()
  await expect(page.getByRole('heading', { name: 'Primeiros passos' })).toBeVisible()

  const terminal = page.locator('.mono', { hasText: 'forge doctor' })
  await expect(terminal).toBeVisible({ timeout: 10_000 })

  await expect(terminal.getByText(/uv encontrado/)).toBeVisible()
  await expect(terminal.getByText(/git ausente/)).toBeVisible()

  // 4 checagens reais, nunca a lista fixa antiga (sempre 6 linhas "✓"/"○").
  const providersCard = page.getByText('providers', { exact: true })
  await expect(providersCard).toBeVisible()
})

test('backend do doctor fora do ar mostra erro explícito, não o array mock antigo', async ({ page }) => {
  await page.route('**/api/doctor', (route) =>
    route.fulfill({ status: 500, contentType: 'application/json', body: '{"error":"boom","code":"forced_failure"}' }),
  )
  await page.goto('/')
  await page.getByRole('button', { name: 'Primeiros passos' }).click()
  await expect(page.getByRole('heading', { name: 'Primeiros passos' })).toBeVisible()

  // A tela tem 2 pontos de leitura do MESMO `doctorState` (card de chaves +
  // terminal) — os dois mostram o erro de forma independente, daí `.first()`
  // em vez de exigir exatamente 1 match.
  await expect(page.getByText('boom').first()).toBeVisible()
  await expect(page.getByRole('button', { name: 'tentar de novo' }).first()).toBeVisible()
  // O mock antigo tinha essa linha fixa — não pode sobreviver por trás do erro.
  await expect(page.getByText('ledger não inicializado')).toHaveCount(0)
})
