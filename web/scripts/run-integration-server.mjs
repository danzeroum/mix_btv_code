// Sobe o `forge dashboard` REAL (processo Rust, sqlite de verdade) para os
// e2e de integração em tests/e2e-integration/ (telemetria, permissões,
// squad, ledger). Não é vite dev + proxy — é o binário que roda em
// produção, servindo o build real de web/dist. Semeia dados via
// forge-store::Telemetry/LedgerStore (os mesmos caminhos reais que
// llm.call/tool.result e a CLI usam), nunca SQL cru.
//
// Chamado pelo `webServer.command` de playwright.integration.config.ts;
// Playwright espera a URL de health check e mata este processo (que repassa
// o sinal ao `cargo run` filho) ao final da suíte.

import { spawn, spawnSync } from 'node:child_process'
import { mkdtempSync, mkdirSync, rmSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'

const __dirname = fileURLToPath(new URL('.', import.meta.url))
const repoRoot = resolve(__dirname, '..', '..')
const webDist = resolve(__dirname, '..', 'dist')
const port = process.env.FORGE_E2E_PORT ?? '7999'

function run(cmd, args) {
  const result = spawnSync(cmd, args, { cwd: repoRoot, stdio: 'inherit' })
  if (result.status !== 0) {
    process.exit(result.status ?? 1)
  }
}

// 1. garante que o binário do CLI e os exemplos de seed estão compilados.
run('cargo', [
  'build', '-p', 'forge-cli', '-p', 'forge-store',
  '--example', 'seed_telemetry', '--example', 'seed_ledger',
])

// 2. diretório de trabalho isolado para o dashboard (.forge/telemetry.db e
// .forge/forge.db próprios, isolados de qualquer outra execução).
const workDir = mkdtempSync(join(tmpdir(), 'forge-e2e-'))
mkdirSync(join(workDir, '.forge'), { recursive: true })
const dbPath = join(workDir, '.forge', 'telemetry.db')
const ledgerPath = join(workDir, '.forge', 'forge.db')

// 3. semeia um evento real via o mesmo Telemetry::record usado em produção.
run('cargo', [
  'run', '-q', '-p', 'forge-store', '--example', 'seed_telemetry', '--',
  dbPath, 'llm.call', 'e2e-integration', '{"provider":"anthropic"}',
])

// 3b. semeia 2 entradas reais no ledger (mesmo LedgerStore::append usado em
// produção) com um ator dedicado (e2e-ledger-seed) que nenhum outro spec
// usa — o teste de Ledger filtra por ele, então não importa a ordem em que
// os specs desta suíte rodam nem quantas outras entradas (squad/permissões)
// o mesmo forge.db acumular depois.
run('cargo', [
  'run', '-q', '-p', 'forge-store', '--example', 'seed_ledger', '--',
  ledgerPath, 'session.start', 'e2e-ledger-seed', '{"task":"e2e"}', '2026-01-01T00:00:00Z',
])
run('cargo', [
  'run', '-q', '-p', 'forge-store', '--example', 'seed_ledger', '--',
  ledgerPath, 'tool.run', 'e2e-ledger-seed', '{"tool":"bash"}', '2026-01-01T00:00:01Z',
])

// 4. sobe o dashboard real apontando pro build da SPA, servindo o evento semeado.
// --manifest-path resolve o workspace a partir de workDir (cargo não muda o
// cwd do processo filho); run_dashboard lê `.forge/telemetry.db` relativo ao
// cwd real do binário, por isso `cwd: workDir` aqui. `--web-agent` liga as
// rotas de sessão/permissão/matriz/squad (Fase 7 Ondas 1-4) por cima do
// dashboard padrão — puramente aditivo, não muda `/api/summary`/`/api/events`/
// `/api/skills` que o teste de telemetria já usa. `FORGE_SCRIPTED=1` troca o
// gerador por respostas determinísticas (sem API key) tanto na sessão de chat
// quanto no squad (`ScriptedSquadCoreBackend`, mesma confiança 0.5 uniforme
// do teste Rust — consenso fraco de propósito, exercita o gate HITL real);
// nenhum teste de integração hoje envia mensagem de chat, então isso não
// muda o comportamento observado pelos specs existentes.
const manifestPath = join(repoRoot, 'Cargo.toml')
const child = spawn(
  'cargo',
  [
    'run', '-q', '--manifest-path', manifestPath, '-p', 'forge-cli', '--',
    'dashboard', '--port', port, '--web-agent',
  ],
  {
    cwd: workDir,
    env: { ...process.env, FORGE_WEB_DIR: webDist, FORGE_SCRIPTED: '1' },
    stdio: 'inherit',
  },
)

function cleanup() {
  rmSync(workDir, { recursive: true, force: true })
}

child.on('exit', (code) => {
  cleanup()
  process.exit(code ?? 0)
})
for (const sig of ['SIGTERM', 'SIGINT']) {
  process.on(sig, () => child.kill(sig))
}
