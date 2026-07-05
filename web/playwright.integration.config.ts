import { defineConfig } from '@playwright/test'

/** Config separada da padrão (playwright.config.ts): aqui não é vite dev +
 * proxy, é o `forge dashboard` real (Rust, sqlite de verdade) — ver
 * scripts/run-integration-server.mjs. Roda via `pnpm test:e2e:integration`.
 */
export default defineConfig({
  testDir: './tests/e2e-integration',
  timeout: 30_000,
  fullyParallel: false,
  reporter: 'list',
  use: {
    baseURL: 'http://127.0.0.1:7999',
    trace: 'retain-on-failure',
  },
  webServer: {
    command: 'node scripts/run-integration-server.mjs',
    url: 'http://127.0.0.1:7999/api/summary',
    reuseExistingServer: false,
    timeout: 180_000,
  },
})
