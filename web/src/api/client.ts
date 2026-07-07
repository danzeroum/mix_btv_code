/** Utilitários compartilhados pela camada api/*. */

export class ApiError extends Error {
  code?: string
  constructor(message: string, code?: string) {
    super(message)
    this.name = 'ApiError'
    this.code = code
  }
}

/** Corpo de erro que toda rota real desta fase devolve (Fase 7 Onda 1). */
interface ApiErrorBody {
  error?: string
  code?: string
}

/**
 * Cliente HTTP real para as rotas ligadas ao backend (Fase 7). Checa `r.ok` e
 * lança `ApiError` com o `code` do corpo `{error, code}` em caso de falha —
 * fim do padrão "assume sucesso" dos módulos ainda mock. `init` aceita os
 * mesmos campos de `fetch` (method/body/headers/signal...).
 */
export async function fetchJson<T>(url: string, init?: RequestInit): Promise<T> {
  let response: Response
  try {
    response = await fetch(url, init)
  } catch {
    throw new ApiError(`falha de rede em ${url}`, 'network_error')
  }
  if (!response.ok) {
    let body: ApiErrorBody = {}
    try {
      body = (await response.json()) as ApiErrorBody
    } catch {
      // corpo não-JSON (ex.: 404 de proxy) — segue com a mensagem genérica.
    }
    throw new ApiError(
      body.error ?? `${url} respondeu ${response.status}`,
      body.code ?? `http_${response.status}`,
    )
  }
  // Corpo vazio (204, ou 202 fire-and-forget como `POST .../message`) nunca
  // deve chamar `.json()` direto — em conteúdo vazio ele lança `SyntaxError`
  // (não `ApiError`), que o chamador então confunde com falha de verdade.
  // Achado real ao escrever a 1ª cobertura de browser da tela Sessão (Onda
  // 15): sem isso, toda mensagem enviada mostrava "falha ao enviar
  // mensagem" mesmo quando o servidor respondia 202 com sucesso.
  const text = await response.text()
  return (text ? JSON.parse(text) : undefined) as T
}
