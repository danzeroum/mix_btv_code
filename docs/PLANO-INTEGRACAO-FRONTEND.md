> **SUPERSEDED por `docs/PLANO-FASE-7-frontend-primario.md`.** Este documento
> nasceu numa branch paralela (`claude/frontend-backend-integration-7u5mti`, nunca
> mergeada) cobrindo o mesmo território — o frontend é 95% vitrine, o servidor
> expõe só 3 rotas GET — em 5 ondas mais largas. O plano de Fase 7 cobre o mesmo
> escopo com granularidade de 12 ondas e fronteira de teste executável por onda, e
> absorveu as ideias concretas daqui que ainda não estavam lá: a guarda de
> `Origin`/`Host`, o wiring de MCP real + telemetria por modelo, o contrato de erro
> `{error, code}` via `fetchJson()`, o truque `ScriptedGenerator`/modo roteirizado
> para e2e sem API key, e o filtro `?actor=` do ledger. Mantido aqui só como
> histórico de pesquisa — não desenvolver a partir deste documento.

# Plano — frontend funcionando com o máximo do backend

> Plano de integração `web/` ↔ backend (Rust `crates/` + sidecar Python),
> derivado de varredura completa dos dois lados. **Só plano — nada aqui foi
> implementado.** Complementa `docs/LEVANTAMENTO-UI-DESIGNER.md` (que olha
> pela ótica do design); este documento olha pela ótica da **engenharia**:
> o que ligar, em que ordem, com que rotas e que mudanças de backend.

## 1. Diagnóstico (estado de hoje)

**Servidor HTTP** (`crates/forge-server/src/lib.rs`): apenas 3 rotas, todas
GET/read-only, + SPA estática (`web/dist`, fallback SPA):

| Rota | Fonte real |
|---|---|
| `GET /api/summary` | `Telemetry::summary` (`.forge/telemetry.db`) |
| `GET /api/events?limit=N` | `Telemetry::recent` |
| `GET /api/skills` | `forge_verify::vetter::list_skill_statuses` |

**Frontend** (`web/src/api/*.ts`): 13 telas prontas, mas só `telemetry.ts` é
100% real e `skills.ts` é híbrido (real com fallback silencioso para mock).
Os outros 11 módulos (`session`, `squad`, `prompts`, `ledger`, `verify`,
`providers`, `permissions`, `models`, `designer`, `onboarding`, resto de
`skills`) são 100% mock via `simulateLatency()`, cada um com `// TODO`
apontando o endpoint-alvo.

**Backend rico não exposto** (existe, compila, tem teste — só falta rota):

- `forge-store::PromptLibrary` — CRUD completo de prompts (síncrono, rusqlite).
- `forge-store::LedgerStore` — hash-chain; **só tem `append`/`verify_chain`,
  falta método público de leitura** para listar entradas.
- `Telemetry::experiment_variants` + `forge-schemas::experiment::ExperimentReport`
  — relatório A/B com veredito honesto (hoje só CLI `forge experiment`).
- `forge-verify::run_pipeline` — pipeline `/verify` real (síncrono, subprocessos).
- `forge-llm::Gateway` (+ `RateLimitedGenerator` + `CachedGenerator`) — geração
  com streaming via callback `on_delta`; keys só no processo Rust (ADR 0001).
- `forge-llm::RateLimiter` — tetos por tier; **`poll` é privado, falta getter
  de uso/restante** para virar tela.
- `forge-core::{AgentLoop, DurableSession, PermissionEngine}` +
  `forge-store::EventStore` — sessões duráveis por event-sourcing.
- `forge-sidecar::SquadClient` — `ExecuteTask → stream SquadEvent` (gRPC/UDS),
  com supervisor e fallback progressivo já prontos no CLI.
- `forge-tools` — MCP (`list_tools_blocking`), sandbox (perfil + fail-closed),
  LSP (config + sessão preguiçosa).
- Python `forge_squad.memory`/`recall` — memórias episódicas (JSONL) + recall
  TF-IDF (ADR 0013), sem superfície HTTP.

**Infra que já ajuda:** proxy Vite `'/api' → 127.0.0.1:7878` (dev) e mesma
origem em produção (a SPA é servida pelo próprio forge-server) — **não precisa
de CORS**; padrão de UI `AsyncStatus` + `useAsyncAction`/`usePolling`; teste de
integração Playwright real já existente (`tests/e2e-integration/` +
`scripts/run-integration-server.mjs`) como molde.

## 2. Princípios do plano

1. **O padrão a copiar é `telemetry.ts` × handler axum × teste in-crate ×
   e2e-integration.** Cada ligação nova replica esse caminho de ponta a ponta.
2. **"Nada Fake" vale para a integração:** cada onda termina com uma prova
   executável (teste axum + um spec Playwright contra o backend real). Nada de
   tela "ligada" que na prática cai no mock.
3. **Fallback mock silencioso morre.** O fallback de `skills.ts` (falha →
   mock) mascara backend quebrado; trocar por estado de erro explícito no
   `AsyncStatus`. Mock só em teste unitário, nunca em runtime.
4. **AppState cresce pelo padrão existente:** handles clonáveis
   (`Arc<Mutex<…>>`, como `Telemetry`) para `PromptLibrary`, `LedgerStore`,
   `EventStore`; `Arc<Gateway>` para o async. Puro/síncrono (permission,
   experiment, vetter) pluga direto; I/O pesado síncrono (verify, MCP,
   tools) via `tokio::task::spawn_blocking`.
5. **Mutação exige um mínimo de segurança local.** Hoje o servidor é
   read-only em `127.0.0.1` sem auth. Ao ganhar POST/DELETE, validar o header
   `Origin`/`Host` contra localhost (mitiga CSRF/DNS-rebinding de sites
   maliciosos atingindo `127.0.0.1`). Barato e suficiente para o modelo
   local-first; token de sessão fica como endurecimento futuro.
6. **Streaming = SSE.** Sessão e squad precisam de eventos incrementais; SSE
   (axum `Sse`/`Event` + `EventSource`/`ReadableStream` no front) é o mínimo
   que resolve, sem WebSocket. O `on_delta` do `Generator` e o
   `Streaming<SquadEvent>` do tonic mapeiam 1:1 para SSE.
7. **Erro consistente:** respostas de erro JSON `{ "error": string, "code":
   string }` em todas as rotas novas; `client.ts` ganha um `fetchJson()` real
   (checa `r.ok`, lança `ApiError` com código) e aposenta
   `simulateLatency`/`maybeFail` dos módulos ligados.
8. **Contratos:** payloads novos que virem contrato (workflow do designer,
   eventos SSE do squad) nascem como `*.v1.schema.json` em `schemas/` com
   fixture, seguindo a regra de fronteira do CLAUDE.md. Protos só evoluem
   aditivamente.

## 3. Ondas de entrega

Ordenadas por relação valor/risco: primeiro leitura (barato, backend pronto),
depois mutação simples, depois streaming, por fim as pontes complexas
(HITL/squad/memória). Cada onda é mergeável sozinha.

### Onda 1 — leituras baratas (backend pronto, só falta rota)

| Entrega | Backend | Frontend |
|---|---|---|
| `GET /api/ledger?limit&actor` + `GET /api/ledger/verify` | **novo** `LedgerStore::entries(limit, actor) -> Vec<LedgerEntry>` (leitura; hoje só append/verify) + handle no AppState | `ledger.ts` real (`getLedger`, `verifyChain`); tela Ledger sem mudanças visuais |
| `GET /api/experiment/:name` | `Telemetry::experiment_variants` + `ExperimentReport::from_two_variants`; 404 se experimento ausente, 422 se ≠2 variantes (espelha o CLI) | **tela nova** Experimentos (admin): 2 barras de taxa, badge de veredito honesto (`Significant`/`Inconclusive`/`InsufficientData`), `p_value` vs α |
| `GET /api/summary?by=model` (ou campo `by_model` no summary) | extensão de `TelemetryStore::summary` agrupando por `props.model` (JSON1, como `experiment_variants` já faz) | card/tabela extra na Telemetria: volume + cache-hit por modelo |
| `GET /api/mcp` | enumerar `.forge/mcp.toml` (`load_mcp_servers`) + `list_tools_blocking` por servidor em `spawn_blocking` com timeout curto; status `up/down` honesto | substitui `MCP_SERVERS` mock em `skills.ts`; tela Skills mostra servidores reais + tools anunciadas (`mcp__<server>__<tool>`) + escopo de permissão |
| `GET /api/providers` | `Gateway::from_env().available()` (só **nomes**, nunca keys) + tetos estáticos `RateLimiter::for_tier` | `providers.ts` deixa de fabricar provider ativo/fallback; `used/cap` fabricados saem da tela até a Onda 5 (honestidade > gauge bonito) |

Caveats explícitos: a tela de Experimentos mostrará só dados semeados até
existir instrumentação de atribuição (`props.experiment/variant/success`) no
caminho de produção — registrar isso na UI ("nenhum experimento ativo") e
deixar a instrumentação como item da Onda 5.

### Onda 2 — mutações simples (primeiros POSTs)

Pré-requisito da onda: guarda de `Origin`/`Host` (princípio 5) como middleware
único aplicado a todo método ≠ GET.

| Entrega | Backend | Frontend |
|---|---|---|
| CRUD de prompts: `GET/POST /api/prompts`, `POST /api/prompts/:id/favorite`, `DELETE /api/prompts/:id` | `PromptLibrary::{list,save,toggle_favorite,delete}` (tudo pronto) + handle no AppState; mesma `.forge/prompts.db` do chat CLI | `prompts.ts` real; biblioteca compartilhada entre CLI (`/prompt`) e web — mesma fonte |
| `POST /api/verify` (dispara) + `GET /api/verify/:run_id` (status/resultado) | `run_pipeline` é longo (minutos) → **padrão job**: POST devolve `run_id`, pipeline roda em `spawn_blocking`, estado em memória (`Arc<Mutex<HashMap>>`); resultado é a `verification-evidence.v1` real | `verify.ts` real: `runVerify` vira POST + polling (`usePolling`); tela mapeia passos reais (cargo test/clippy/fmt/pytest) e o veredito derivado — fim do `VERIFY_STEPS` fixo |
| `POST /api/prompt/render` (+ `GET /api/prompt/generators`) | ponte para o sidecar PromptForge (`SidecarClient::{render,lint,list_generators}`); sem `uv`/sidecar → 503 com mensagem honesta (degradação já é padrão do `try_start`) | geradores da tela Prompts deixam de retornar string "mock local" |
| `POST /api/skills/permissions` (matriz tool×perfil) | **decisão de produto embutida:** a matriz mock hoje é editável; a real deve nascer **read-only** exibindo `PermissionEngine`/regras do perfil (editar política via web é superfície de risco — adiar edição para quando houver persistência de config com ADR próprio) | `togglePermissionCell` sai; matriz vira leitura de `GET /api/permissions/rules` |

### Onda 3 — sessão real com SSE (o maior salto de valor ao usuário)

Transforma `forge dashboard` de "painel de telemetria" em "servidor de app":
o processo passa a montar o mesmo stack do CLI (`prepare()`:
Gateway + RateLimiter + Cache + Telemetria — keys continuam só nesse processo,
ADR 0001 intacto).

- **Rotas:** `POST /api/session` (cria; body: `task_hint`, `model_tier`,
  `agent_profile`) → `DurableSession::open` sobre `EventStore`;
  `GET /api/sessions` → `EventStore::aggregates()` (listar/retomar);
  `POST /api/session/:id/message` → resposta **SSE** com eventos tipados:
  `delta` (texto incremental via `on_delta`), `tool` (início/fim, status),
  `turn` (resumo), `done`/`error`.
- **AgentLoop no servidor:** reusar `AgentLoop::run/continue_run` +
  `ToolRegistry::default_set(root)`. **Permissões nesta onda:** resolver
  `Ask` como `Deny` com evento SSE explicativo ("aprovação interativa chega
  na Onda 4") — honesto e seguro; `read`/`grep` seguem allow.
- **Persistência dupla como no CLI:** eventos no `EventStore` (retomada) e no
  ledger (`Session::record`) — auditoria igual à do terminal.
- **Frontend:** infra SSE nova (`web/src/api/stream.ts`, `ReadableStream` +
  parser de eventos), `session.ts` real, tela Sessão renderiza transcript
  real (turnos `user/agent/tool/diff` já têm componentes). `SESSION_HEADER`
  passa a vir de `GET /api/session/:id`.
- **Modelo/perfil (tela Modelo):** `modelTier`/`agentProfile` deixam de ser
  só estado local — viram parâmetros da criação de sessão. `selectAutonomy`
  continua "em dev" até a Onda 4 (é o HITL).
- **Testabilidade sem key:** modo roteirizado no servidor (env
  `FORGE_SCRIPTED=1` → `ScriptedGenerator`, mesmo truque do `loadgen` e do
  squad e2e) — permite spec Playwright do fluxo completo de sessão no CI
  sem `ANTHROPIC_API_KEY`.

### Onda 4 — squad ao vivo + permissões interativas (HITL na web)

O item de maior desenho do plano; destrava o `Ask` da Onda 3 e o gate HITL.

- **Ponte de permissão (peça central):** fila de pedidos pendentes no
  servidor — `RequestPermission`/`PermissionResolver` empurra
  `{id, tool, scope, preview}` num mapa + notifica via SSE; o front resolve
  com `POST /api/permission/:id/decision` (`allow`/`deny`), que completa o
  `oneshot` e desbloqueia o loop/squad. Timeout → `deny` (fail-closed).
  Tela Permissão (`permissions.ts`) deixa de ser demo e vira o gate real;
  a Sessão (Onda 3) troca o "Ask=Deny" por escalada de verdade.
- **`POST /api/squad/run` → SSE de `SquadEvent`:** `SquadSupervisor::spawn` +
  `SquadClient::execute_task`, retransmitindo o stream gRPC como SSE
  (tradução proto→JSON documentada como contrato `squad-event.v1` em
  `schemas/`). Reusar a degradação em 3 níveis do CLI (squad → agente-único →
  safe-mode) com evento SSE dizendo em que nível está. Consenso registrado
  no ledger com o helper existente do `forge squad`.
- **Frontend:** `squad.ts` real — estados dos 5 agentes, consenso ponderado e
  dissenso vindos dos eventos; `resolveHITL` usa a mesma ponte de permissão
  (deltas de trust reais do `hitl.py`, fim do `±0.02/−0.10` hardcoded).
- **Prova sem key:** dirigir o squad com `ScriptedGatewayClient` (Python) +
  `ScriptedGenerator` (Rust) — de quebra fecha a pendência de exercício
  consenso→ledger da Fase 4 registrada em `pendencias.md`.

### Onda 5 — memória/RAG, designer, rate-limit e acabamento

| Entrega | Backend | Frontend |
|---|---|---|
| Memória do squad: `GET /api/memory` + `POST /api/memory/recall` | listagem lê `.forge/squad-memory/agent_memories.jsonl` direto no Rust (storage é Rust, ADR 0001); recall (TF-IDF é Python, ADR 0013) via RPC **aditivo** no proto do sidecar, com 503 honesto sem sidecar | **tela nova** Mapa de memória (user): memórias por agente + busca com score em barra, rotulada "recuperação léxica" |
| Designer salva de verdade: `POST /api/workflow` | **novo contrato** `squad-workflow.v1.schema.json` em `schemas/` (+ fixture + ADR curto); validação server-side + registro no ledger (`workflow.saved`), devolvendo `seq` real (fim do `seq 248` fixo) | `designer.ts` real; abre caminho para o squad executar workflows desenhados (fase futura) |
| Rate-limit real na tela Providers | **novo getter** no `RateLimiter` (`usage() -> {used, cap, window_secs}`) — requer o limiter compartilhado do servidor de sessão (Onda 3); `GET /api/limits` | gauges `used/cap` da tela Providers voltam, agora reais |
| Instrumentação A/B | gravar `props.experiment/variant/success` num caminho de produção escolhido (candidato natural: variantes de prompt da biblioteca) — destrava a tela da Onda 1 com dados vivos | — |
| Status do sandbox: `GET /api/sandbox` | ping ao daemon Docker (bollard) com timeout curto + perfil default (`image`, rede off, 512MB, 0.5 cpu, 30s); `DaemonUnavailable` honesto | seção na tela Skills (tema "código de terceiro") |
| `GET /api/doctor` (onboarding) | checagens reais: providers detectados (booleano por env var, **nunca** valores), `uv` presente, Docker presente, versões | Onboarding troca `ENV_KEYS`/`DOCTOR_OUTPUT` fixos por diagnóstico real |
| Status LSP: `GET /api/lsp` (opcional, última prioridade) | só a config declarada em `.forge/lsp.toml` (sem subir processo — registro é lazy) | painel fino na persona admin |

## 4. Transversais (valem para todas as ondas)

- **Testes em três camadas por rota:** (1) teste axum in-crate (padrão dos 7
  existentes em `forge-server`); (2) spec em `tests/e2e-integration/` com
  seed real (estender `run-integration-server.mjs` — hoje só semeia
  telemetria; passará a semear ledger/prompts e a exportar `FORGE_SCRIPTED`);
  (3) unit vitest só para lógica de parsing/estado no front.
- **CI:** adicionar job `web` (pnpm install + vitest + build +
  `test:e2e:integration`) ao `ci.yml` — hoje o CI não exercita `web/`; sem
  isso, toda a integração fica sem gate.
- **Limpeza progressiva do mock:** a cada módulo ligado, remover
  `simulateLatency`/constantes fabricadas daquele módulo e o TODO
  correspondente. Critério de pronto da integração toda: `grep
  simulateLatency web/src/api` vazio (sobra só em fixtures de teste).
- **Docs:** cada onda atualiza `docs/DECISOES.md`; ADRs novos apenas onde há
  contrato/fronteira nova: ponte HITL-web (Onda 4), `squad-workflow.v1` e o
  RPC de recall (Onda 5). O resto é implementação sob ADRs existentes.

## 5. Riscos e decisões em aberto

1. **Segurança de mutação em localhost** — a guarda de `Origin` (Onda 2) é o
   mínimo; se o dashboard um dia escutar fora de `127.0.0.1`, auth vira
   pré-requisito duro (hoje é explicitamente fora de escopo).
2. **Ponte HITL** (Onda 4) é o maior risco de desenho — envolve `oneshot`
   entre task async do loop e handler HTTP, timeout fail-closed e reconexão
   SSE. Prototipar cedo (spike na Onda 3) antes de comprometer a tela.
3. **Processos longos** (verify, squad): o padrão job em memória perde estado
   se o servidor reiniciar — aceitável para local-first; persistir jobs é
   endurecimento futuro.
4. **Edição de política de permissão via web** foi deliberadamente adiada
   (read-only na Onda 2): afrouxar permissões por clique é superfície de
   risco que merece ADR próprio, não um toggle.
5. **A/B sem instrumentação** — a tela (Onda 1) nasce na frente dos dados
   (Onda 5); o estado vazio precisa ser honesto, nunca semear no caminho de
   produção só para a tela "ficar bonita".
6. **`skills.ts` fallback-para-mock** será removido; quem desenvolve o front
   sem backend passa a ver o estado de erro real (o proxy Vite + `forge
   dashboard` local é o fluxo de dev suportado).

## 6. Resumo executivo da sequência

1. **Onda 1** — ledger, experimentos, telemetria por modelo, MCP, providers
   (5 rotas GET; 1 método novo no `LedgerStore`; 1 tela nova).
2. **Onda 2** — prompts CRUD, verify como job, render via sidecar, guarda de
   Origin (primeiros POSTs).
3. **Onda 3** — sessão real com SSE + `ScriptedGenerator` para e2e sem key
   (o dashboard vira servidor de app).
4. **Onda 4** — squad ao vivo + ponte HITL de permissões (destrava `Ask`,
   gate HITL e fecha a pendência consenso→ledger).
5. **Onda 5** — memória/RAG, designer→ledger, rate-limit real,
   instrumentação A/B, sandbox/doctor/LSP.

Ao fim, todas as 13 telas existentes operam sobre dados reais, 3 telas novas
(Experimentos, Memória, e o console MCP dentro de Skills) expõem o que a Fase
6 construiu, e nenhum módulo de `web/src/api` fabrica dado em runtime.
