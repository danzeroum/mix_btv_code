// Load-test k6 do caminho do gateway (Fase 6 Onda 8, critério de conclusão nº 3).
//
// Martela o endpoint de carga (`forge-server` bin `loadgen`), que embrulha o
// `ScriptedGenerator` — sem provider, SEM API KEY. Mede o overhead do NOSSO lado
// (axum + agregação + streaming) isolado da latência de rede do provider, sob
// concorrência, e valida o P95 sob o limiar. O k6 sai ≠0 se o threshold estourar:
// o gate é real.

import http from 'k6/http';
import { check } from 'k6';

export const options = {
  vus: 20,
  duration: '10s',
  thresholds: {
    // P95 do overhead do gateway (in-process, sem rede/key) sob o limiar.
    // Generoso o bastante para não ser flaky num runner de CI, apertado o
    // bastante para pegar regressão grosseira/contenção (ex.: lock do rate
    // limiter serializando tudo).
    http_req_duration: ['p(95)<100'],
    // Praticamente nenhuma falha tolerada.
    http_req_failed: ['rate<0.01'],
  },
};

const BASE = __ENV.FORGE_LOADGEN_URL || 'http://127.0.0.1:7900';

export default function () {
  const res = http.post(
    `${BASE}/generate`,
    JSON.stringify({ prompt: 'requisição de carga' }),
    { headers: { 'Content-Type': 'application/json' } },
  );
  check(res, {
    'status 200': (r) => r.status === 200,
    'devolveu texto': (r) => typeof r.body === 'string' && r.body.includes('resposta'),
  });
}
