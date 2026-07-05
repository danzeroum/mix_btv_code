import { defineConfig } from 'vitest/config'
import react from '@vitejs/plugin-react'

// https://vite.dev/config/
export default defineConfig({
  plugins: [react()],
  server: {
    proxy: {
      // Aponta para `forge dashboard` real rodando localmente (telemetria é o
      // único domínio com backend de verdade nesta fase).
      '/api': 'http://127.0.0.1:7878',
    },
  },
  test: {
    environment: 'jsdom',
    exclude: ['**/node_modules/**', '**/tests/e2e/**'],
  },
})
