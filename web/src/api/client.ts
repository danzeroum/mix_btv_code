/** Utilitários compartilhados pela camada api/*. Todo módulo mock usa
 * `simulateLatency()` para nunca parecer instantâneo/travado, e lança
 * `ApiError` para exercitar o estado `error` de `useAsyncAction`.
 */

export class ApiError extends Error {
  code?: string
  constructor(message: string, code?: string) {
    super(message)
    this.name = 'ApiError'
    this.code = code
  }
}

export function simulateLatency(ms = 300 + Math.random() * 400): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms))
}

/** Lança ApiError com a taxa dada — use em ações que devem, às vezes, exercitar o caminho de erro. */
export function maybeFail(rate: number, message: string): void {
  if (Math.random() < rate) {
    throw new ApiError(message)
  }
}
