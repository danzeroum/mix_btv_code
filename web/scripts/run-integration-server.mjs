// Sobe o `forge dashboard` REAL (processo Rust, sqlite de verdade) para o
// e2e de integração de telemetria em tests/e2e-integration/. Não é vite dev
// + proxy — é o binário que roda em produção, servindo o build real de
// web/dist. Semeia um evento via forge-store::Telemetry (o mesmo caminho
// real que llm.call/tool.result usam), nunca SQL cru.
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

// 1. garante que o binário do CLI e o exemplo de seed estão compilados.
run('cargo', ['build', '-p', 'forge-cli', '-p', 'forge-store', '--example', 'seed_telemetry'])

// 2. diretório de trabalho isolado para o dashboard (.forge/telemetry.db próprio).
const workDir = mkdtempSync(join(tmpdir(), 'forge-e2e-'))
mkdirSync(join(workDir, '.forge'), { recursive: true })
const dbPath = join(workDir, '.forge', 'telemetry.db')

// 3. semeia um evento real via o mesmo Telemetry::record usado em produção.
run('cargo', [
  'run', '-q', '-p', 'forge-store', '--example', 'seed_telemetry', '--',
  dbPath, 'llm.call', 'e2e-integration', '{"provider":"anthropic"}',
])

// 4. sobe o dashboard real apontando pro build da SPA, servindo o evento semeado.
// --manifest-path resolve o workspace a partir de workDir (cargo não muda o
// cwd do processo filho); run_dashboard lê `.forge/telemetry.db` relativo ao
// cwd real do binário, por isso `cwd: workDir` aqui.
const manifestPath = join(repoRoot, 'Cargo.toml')
const child = spawn(
  'cargo',
  ['run', '-q', '--manifest-path', manifestPath, '-p', 'forge-cli', '--', 'dashboard', '--port', port],
  {
    cwd: workDir,
    env: { ...process.env, FORGE_WEB_DIR: webDist },
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
