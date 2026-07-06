# Plano-mestre: Fase 7 — o navegador como forma primária de uso

> Documento de execução, no formato dos planos anteriores: fatos ancorados no código
> real (verificado no main pós-Fase 6, roadmap das 6 fases concluído), ondas com
> fronteira verificável, decisões de contrato viram ADR. Produzido por 2 agentes
> Explore (arquitetura de streaming/permissão; veredito por tela das 9 telas mock),
> 1 agente Plan (ordem de ondas), e revisado por uma passada de gerência que ancorou
> 4 achados extras no código antes de aprovar (ver §1).
>
> **Este documento substitui `docs/PLANO-INTEGRACAO-FRONTEND.md`** (branch
> `claude/frontend-backend-integration-7u5mti`, nunca mergeado — nenhuma PR aberta
> para ele). Mesmo território, mesma pesquisa de base (o servidor expõe só 3 rotas
> GET, o resto é mock), mas com granularidade fina de ondas e fronteira de teste
> executável por onda, em vez de 5 ondas mais largas (a contagem exata de ondas
> deste documento está na emenda logo abaixo — mudou desde a supersessão original).
> Ideias concretas dessa outra
> varredura foram absorvidas aqui: a guarda de `Origin`/`Host`, o wiring de MCP real
> + telemetria por modelo (Onda 7), o contrato de erro `{error, code}` via
> `fetchJson()`, o truque `ScriptedGenerator`/modo roteirizado para e2e sem API key,
> e o filtro `?actor=` do ledger. O documento antigo recebeu uma nota de "superseded"
> no topo, nesta mesma entrega — nenhum dos dois deve divergir dali em diante.
>
> **Emenda (15 ondas, era 12):** o recorte original tratava o Grupo A do
> `LEVANTAMENTO-UI-DESIGNER.md` como fora de escopo, com só 2 exceções (Onda 7).
> Investigação adicional — 3 agentes Explore (handoff de design em
> `docs/design_handoff_forge_telas/`, verificação de backend por tela, mecanismo de
> registro de tela) + 1 agente Plan — mostrou que as 7 telas de Grupo A já têm
> handoff de design pronto (`README.md` §12 + protótipo HTML) dando a cada uma seu
> próprio item de navegação, não cartões embutidos. O escopo mudou: Grupo A fecha
> **7/7**. Onda 7 foi reescrita (A1 completo, não só o piso) e as Ondas 8, 9 e 10 são
> novas (A3, A2, A4+A6+A7); as ondas seguintes renumeraram (+3).

## 0. Contexto e critérios de conclusão

O roadmap original (6 fases) está concluído — `CLAUDE.md`/`docs/PLANO-PLATAFORMA-FORGE.md`
já declaram isso, e o próprio `CLAUDE.md` antecipa: "o que vier depois é produto novo,
não plano antigo". O Forge foi testado de ponta a ponta numa VPS via Docker e, depois
de navegar o dashboard web, veio o pedido: "quero o frontend todo funcional como o
CLI, ele será a forma principal de usar" — refinado depois para "faça apenas um plano
para o frontend funcionar corretamente com o máximo das funcionalidades disponíveis
no backend".

O frontend (`web/src/`) já existe, bem-acabado — 13 telas, duas personas, sistema de
componentes próprio (documentado em `docs/LEVANTAMENTO-UI-DESIGNER.md`) — mas hoje é
**95% vitrine**: `crates/forge-server` expõe só 3 rotas GET read-only; o resto lê de
`web/src/api/*.ts` com `simulateLatency()`.

A Fase 7 define quatro critérios literais de conclusão:

1. **As 13 telas existentes operam sobre dados reais** — `grep simulateLatency
   web/src/api` vazio (sobra só em fixture de teste).
2. **Toda rota mutável está protegida contra CSRF/DNS-rebinding local** — uma
   requisição `Origin: https://evil.example` para qualquer método ≠ GET recebe
   `403`; sem `Origin` (curl/CLI), passa.
3. **O job `web` (Playwright real, `web/tests/e2e-integration/`) roda verde no CI**
   — hoje o harness existe mas não é exercitado por nenhum workflow.
4. **Nenhuma tela finge fazer algo que o backend não faz** — onde o backend ainda não
   sustenta o comportamento (autonomia progressiva, mutação de providers), a tela
   declara isso explicitamente em vez de simular.
5. **As 7 telas do Grupo A (A1-A7) existem e operam sobre dados reais** — cada uma
   com sua própria rota e seu próprio item de navegação (não cartão embutido em
   tela existente), honestas onde o backend ainda não sustenta tudo (A2 mostra
   banner de dado semeado; A4 mostra banner de uso ilustrativo) em vez de fabricar.

A postura local-first/single-user (`forge-server` amarrado a `127.0.0.1`) não muda —
esta fase não é "virar SaaS multiusuário". Mas o navegador como forma **primária** de
uso introduz uma superfície de risco nova mesmo em localhost: qualquer aba aberta no
mesmo navegador pode tentar `POST http://127.0.0.1:7878/api/...` — inclusive a rota
que aprova execução de `bash`. O critério nº 2 existe por causa disso, não por
paranoia de produção.

## 1. O que foi verificado (2 Explore + 1 Plan + revisão de gerência)

**A peça dura, por que isto não é wiring simples:**

- `PermissionResolver` (`crates/forge-core/src/agent_loop.rs:40-42`) é **síncrono**
  (`fn resolve(&mut self, tool: &str, scope: &str) -> bool`), dyn-dispatched. As duas
  implementações interativas **bloqueiam a thread** até um humano responder:
  `CliResolver` (stdin, `forge-cli/src/main.rs:867-887`) e `TuiResolver`
  (`forge-cli/src/tui_app.rs:114-135`) — esta é o precedente de **forma** a replicar:
  publica o pedido num `mpsc::UnboundedSender<TuiMsg>` não-bloqueante e bloqueia em
  `std_mpsc::Receiver<bool>::recv()` até a UI (loop de render em thread própria)
  responder pelo canal pareado. O agent loop roda em `tokio::spawn` separado da UI.
- `LoopEvent<'a>` (`agent_loop.rs:15-37`) só é `Debug`, não `Serialize`, e não é
  `'static` (`&'a str`). Não existe hoje nenhum DTO owned+serializável — precisa ser
  criado (o `TuiMsg` é o precedente de forma, mas também não é serializável).
- **Achado do agente Plan:** `run_tool` (`agent_loop.rs:183-249`, a chamada exata é a
  linha 197: `Decision::Ask => resolver.resolve(name, &scope)`) roda **sincronamente
  dentro do caminho async do loop**, sem `spawn_blocking`. Inofensivo no CLI/TUI (um
  processo, um usuário); num servidor com N sessões, uma permissão pendente por
  minutos prenderia uma worker-thread do reactor Tokio — o suficiente disso trava
  requisições não relacionadas (inclusive o `/api/summary` do dashboard atual).
  Mitigação: cada sessão viva roda em `spawn_blocking`, nunca `tokio::spawn` comum —
  decisão explícita, não detalhe de implementação.
- `forge-server` (`crates/forge-server/src/lib.rs:30-46`) hoje só depende de
  `forge-store`/`forge-verify`/`forge-llm` (este último só para o bin `loadgen`, não
  o dashboard). **Não depende** de `forge-core`, `forge-tools`, `forge-sidecar`,
  `forge-proto`. `forge-cli` já depende de tudo. Isso decide onde o código novo mora
  (ver Onda 1). `axum::response::sse` já vem compilado (axum 0.8, zero dep nova);
  `tokio-stream` já pinado no workspace (feature `"sync"` a mais para
  `BroadcastStream`, se fan-out multi-aba usar `tokio::sync::broadcast`).
- Squad: CLI drena `Streaming<SquadEvent>` por polling manual
  (`forge-cli/src/squad.rs:240-269`). O HITL do squad é uma chamada gRPC do Python de
  volta ao Rust (`CoreService.RequestPermission`), hoje um `spawn_blocking` sobre
  stdin (`forge-cli/src/squad.rs:76-99`). `SquadEvent` (prost/tonic-build) não tem
  serde hoje — adicionar via `.type_attribute` é baixo risco e, diferente do
  `LoopEvent`, **não precisa de DTO espelho** (o JSON do proto vai direto no SSE).
- `DurableSession` (`.forge/sessions.db`) e o `Session`-ledger do CLI
  (`.forge/forge.db`) são stores **diferentes**, hoje abertos lado a lado por
  `prepare`/`build_loop`/`open_durable` (`forge-cli/src/main.rs:374-459`) — é a
  receita que um handler HTTP precisa replicar. Só `Telemetry` tem hoje um handle
  `Arc<Mutex<...>>` compartilhável — `LedgerStore`/`EventStore`/`PromptLibrary`
  precisam do mesmo wrapper (replicar o padrão, não inventar).
- `run_pipeline` (`forge-verify`) é síncrono; passos default somam ~8.5min
  sequenciais — não cabe num request/response HTTP síncrono.
- `Gateway::generate` usa array hardcoded, nunca consulta `FallbackChain`
  (`forge-llm/src/provider.rs:41-63`), que **existe mas é código morto**.
  `RateLimiter` não tem API de mutação nem getter de uso.
- **Achado forte:** `max_autonomy_level` do proto (`schemas/proto/squad.proto:20`),
  hardcoded `3` em `forge-cli/src/squad.rs:219`, é **recebido e nunca lido** em
  `python/packages/forge-squad` (grep confirma zero uso) — dado morto ponta a ponta.
  Plugar um seletor de UI nesse campo sem tocar o Python seria fake-wiring.
- `LedgerStore::open` (`crates/forge-store/src/ledger.rs:29`) **não liga WAL**, ao
  contrário de `EventStore::open` (`events.rs:87-89`, que liga `journal_mode=WAL` +
  `synchronous=NORMAL`) — CLI e servidor web tocando `.forge/forge.db` ao mesmo tempo
  pode dar "database is locked". Bug de concorrência latente, exposto pela primeira
  vez por esta fase.
- Já existe uma suíte Playwright real (`web/tests/e2e-integration/
  telemetry-real-backend.spec.ts` + `web/scripts/run-integration-server.mjs`) que
  sobe um `forge dashboard` real (build real, sqlite real) e prova a tela contra dado
  semeado por fora do browser — **não roda em CI hoje** (sem job `web` em
  `.github/workflows/ci.yml`). Estender esse harness, não inventar outro.
- `ScriptedGenerator` público (Fase 6 Onda 8, `crates/forge-llm/src/scripted.rs`) já
  tem `from_turn(turn)` (linha 39) aceitando um turno arbitrário com `tool_use` — mas
  tanto `echo` quanto `from_turn` devolvem **sempre o mesmo turno** a cada chamada
  (`generate` clona `self.turn`); não há sequenciamento (turno 1 ≠ turno 2 ≠ turno 3
  em chamadas sucessivas do mesmo generator). Isso já existe, mas só como o tipo
  **privado** `Scripted` dentro de `agent_loop.rs`'s `#[cfg(test)] mod tests`
  (`turns: Mutex<Vec<AssistantTurn>>`, consumido com `remove(0)` a cada chamada). A
  fronteira dos testes desta fase precisa promover esse padrão de fila para uma
  variante pública reusável (mesma promoção que `echo`/`from_turn` já fizeram) — não
  inventar sequenciamento do zero.

**Achados do Grupo A (revisão de escopo — as 7 telas A1-A7):**

- **Regra de posicionamento de rota (cross-cutting, vale para as Ondas 7-10):**
  `forge-server` continua sem depender de `forge-tools`/`forge-core`/`forge-sidecar`
  (intocado desde a Onda 1). Rotas que só precisam do que `forge-server` **já**
  depende (`forge-store`, `forge-schemas`, e `forge-llm` — já é dependência do
  Cargo, hoje usado só pelo bin `loadgen`) entram **direto em
  `crates/forge-server/src/lib.rs`**, ao lado de `/api/skills`. Rotas que precisam
  de `forge-tools`/`forge-core`/`forge-sidecar` entram no router mesclado de
  `forge-cli` (o mecanismo que a Onda 1 já cria). Por tela: **A5 e A4 →
  `forge-server` direto**; **A2 → `forge-server` direto** (mesma classe de A5);
  **A1, A3, A6, A7 → router de `forge-cli`**.
- **A1 Console MCP:** `list_tools_blocking`/`McpToolMeta` (`forge-tools/src/mcp.rs:
  65,26-30`) não derivam `Serialize` hoje. O nome namespaced (`mcp__<server>__
  <tool>`) e o preview de política (`mcp:<server>/<tool> <preview>`, `mcp.rs:
  52-56`) não vêm prontos — computar via `PermissionEngine::evaluate(name, scope)`
  (`forge-core/src/permission.rs:34`, puro/síncrono). **Dependência real:** os
  perfis const (`AgentProfile::BUILD`/`PLAN`) não têm regra `mcp__*` nenhuma —
  o preview só é significativo consultando o store de `Rule` persistida que a
  Onda 2 introduz (soft-dependência em Onda 2, não só Onda 1). `McpServer`
  (`domain.ts`) só tem `{id,status}`, precisa de `tools`. **Limpeza obrigatória:**
  o card MCP mock em `Skills.tsx:86-103` (+ estado `:24-25` + handler `:50-61` +
  import `:5`) e `MCP_SERVERS`/`reconnectMcp` em `skills.ts:11-15,58-65` são
  **removidos**, não deixados coexistir.
- **A5 Uso por modelo:** `telemetry.rs:83` (`summary()`) só agrupa por nome —
  falta consulta irmã por `json_extract(props,'$.model')`, mesmo padrão de
  `experiment_variants` (`telemetry.rs:117-139`). `model_tier::tier_from_id`
  (`model_tier.rs:61`, real/testado) dá a coluna `tier` de graça — `ModelTier`
  não deriva `Serialize` hoje. **`web/src/api/models.ts` já existe** (tela
  `modelo` de usuário) — o módulo novo chama-se `modelUsage.ts`.
- **A2 Experimentos A/B:** `ExperimentReport::from_two_variants`
  (`experiment.rs:83`) + `Telemetry::experiment_variants` já existem, só
  chamados pelo CLI (`main.rs:178`) — zero rota HTTP. Ambos os tipos já derivam
  `Serialize`+`JsonSchema`, zero DTO novo. Caveat honesto já redigido no handoff:
  nenhum código de produção escreve `props.experiment/variant/success` ainda —
  só testes e `examples/seed_telemetry.rs`; a tela mostra dados semeados com
  banner explícito (instrumentar produção **não** é trabalho desta fase).
  `Sugestoes.tsx:11` tem um cartão placeholder apontando para `prompts` — retarget
  para a tela nova.
- **A3 Mapa de memória/RAG + busca (o item mais difícil):**
  `AgentMemorySystem.recall_similar`/`remember_decision` (`memory.py:110,72`) e o
  ranqueador TF-IDF (`recall.py:89`) são reais e testados (Fase 6 Onda 6), mas
  100% em processo Python, sem superfície externa. O RPC `CoreService.Recall`/
  `Remember` (`core.proto:18-19`) está `unimplemented!` (`core_server.rs:
  111-121`) **e é a direção errada**: `CoreService` é servido pelo Rust, chamado
  pelo Python — o dado de memória mora no Python, então expor ao frontend exige
  a direção oposta, igual a `PromptForgeService`/`SquadService` (servidos pelo
  Python, chamados pelo Rust via `SidecarClient`/`SquadClient`). O próprio
  protótipo do handoff erra isso (cita "CoreService.Recall" na cópia de
  loading) — a tela real corrige, não repete o engano. **Achado novo — não-escopo
  explícito:** `forge_squad/forgetting.py` (`IntelligentForgetting.
  adaptive_forget`, `MemoryStore`) é código morto — chamado só pelo próprio teste
  unitário, nunca por `memory.py`/`orchestrator.py`/`server.py` (mesma classe do
  achado `max_autonomy_level`). O painel "memória por agente" do protótipo mostra
  uma tendência de esquecimento que nada no código computa — a tela real não
  mostra essa coluna.
- **A4 Rate limits:** `RateLimiter::for_tier` (`rate_limit.rs:48-55`) é real.
  **Armadilha de corretude:** `poll()` (`:59`) é privado e **muta** (empurra
  timestamp) como efeito colateral de checar vaga (`:70`) — um getter de uso que
  chamasse `poll()` faria **abrir o dashboard consumir uma vaga real de produção
  silenciosamente**. Precisa de método novo e separado, só poda + conta, nunca
  empurra, com teste dedicado provando efeito colateral zero. O piso (tetos reais
  + banner "uso ilustrativo") não exige esse getter — é degrau opcional.
- **A6 Sandbox & skills de terceiro:** `Sandbox::new` (`sandbox.rs:43-52`, campos
  `pub`) + `/api/skills` (já real). Ping ao Docker só existe embutido em
  `run_with` (`:151-153`, side-effect de rodar algo de verdade) — precisa de
  `Sandbox::ping()` novo, fino. Tela **read-only** — o protótipo do handoff não
  tem handler nenhum de instalar/vetar/habilitar/remover.
- **A7 Language servers/LSP:** enumeração de `.forge/lsp.toml` é comprovadamente
  livre de processo (teste existente `skills.rs:463-479` prova isso apontando um
  comando inexistente). **Armadilha de escopo:** não adicionar um probe sob
  demanda para "ver se está rodando" — quebraria a propriedade que aquele teste
  protege. A tela mostra só config declarada + uso real já ocorrido na sessão.

**Inventário do Grupo B (o escopo desta fase — 11 lacunas reais):**

| # | Tela/ação | Backend real hoje |
|---|---|---|
| 1 | Ledger (admin) | `LedgerStore` (falta leitura paginada) |
| 2 | Verify (admin) | `run_pipeline` (síncrono, 8.5min) |
| 3 | Providers (admin) | `Gateway`/`RateLimiter` (sem mutação) |
| 4 | Skills — matriz de permissão (admin) | `PermissionEngine`/`AgentProfile` (hardcoded) |
| 5 | Sessão de código (user) | `AgentLoop`/`DurableSession` |
| 6 | Permissão ao vivo (user) | `PermissionResolver` |
| 7 | Squad ao vivo (user) | `SquadService` gRPC |
| 8 | Prompts — biblioteca (user) | `PromptLibrary` |
| 9 | Prompts — render (user) | `PromptForgeService` (sidecar) |
| 10 | Modelo & Onboarding (user) | nenhuma persistência hoje |
| 11 | Designer (user) | novo: schema `squad.workflow.v1` |

O item #4 (gap na matriz de permissão de `skills.ts`) não é Grupo A, mas também não é
inédito: já aparecia en passant no `PLANO-INTEGRACAO-FRONTEND.md` concorrente (Onda 2,
que o deferia para **read-only**, adiando edição "para quando houver persistência de
config com ADR próprio"). O que este documento decide diferente, deliberadamente, é
permitir a edição — mas com trilha de auditoria (ver Onda 2) em vez de deixá-la
read-only.

**Grupo A — 7/7 fechado (revisão de escopo):** o recorte original tratava o Grupo A
do `LEVANTAMENTO-UI-DESIGNER.md` (A1 Console MCP, A2 Experimentos A/B, A3 Mapa de
memória/RAG, A4 Rate limits, A5 Uso por modelo, A6 Sandbox & skills de terceiro, A7
Language servers/LSP) como design novo fora de escopo, com só 2 exceções (A1
parcial + A5) por serem wiring puro. Um handoff de design real já existe para as 7
(`docs/design_handoff_forge_telas/README.md` §12 + protótipo HTML, na branch
`claude/frontend-backend-integration-7u5mti` — conteúdo genuinamente novo,
confirmado por `git diff`), dando a cada tela seu próprio item de navegação. Isso
muda o recorte: as 7 entram nesta fase, via Ondas 7 (A1 completo + A5), 8 (A3), 9
(A2) e 10 (A4+A6+A7) — ver §2. A cobertura de rota no handoff é desigual (só A1 e
A2 têm rota nomeada; A3 só tem um método gRPC, e é o **errado**; A4/A5/A6/A7 não
têm rota nenhuma) — definir o contrato por tela é trabalho de planejamento real
desta emenda, não é só seguir o handoff.

**Princípio de recorte:** priorizar o que o backend **já oferece** sobre inventar
capacidade nova. Isso guia duas ondas:
- **Providers** — o piso é uma tela **real, só leitura** do que `Gateway`/
  `RateLimiter` já sabem (zero engenharia nova). A mutação (reordenar fallback via
  o `FallbackChain` já existente-mas-morto; ajustar teto de rate-limit) é um degrau
  a mais, ainda modesto porque o tipo já existe — decidir o corte exato no início da
  onda, não assumir de saída.
- **Modelo & Agente** — em vez de um store de preferências persistidas novo (que não
  existe em lugar nenhum do repo), usar o que o backend já faz: `--model`/`--agent`
  **por chamada**, igual ao CLI. A UI manda esses parâmetros por sessão/tarefa; não
  inventa "seleção persistida entre sessões" a menos que o produto peça depois.

## 2. Arquitetura das ondas (a lógica da ordem)

Só há **uma dependência dura**: Onda 1 → Onda 2. Tudo mais é majoritariamente
independente (ver §5).

### Onda 1 — Fundação web

DTO owned+`Serialize` espelhando `LoopEvent` (mora em `forge-cli`, ao lado de
`TuiMsg` — `forge-core` continua UI-agnóstico); rota SSE genérica por `session_id`
(`axum::response::sse`, zero dep nova); um `PermissionResolver` novo que publica o
pedido no mesmo SSE e aguarda a resposta via `POST /api/session/:id/permission` —
mesmo desenho do `TuiResolver`, rodando em `spawn_blocking` (mitiga o esgotamento de
worker-threads). Código novo entra em `forge-cli` como um `Router` `.merge()`ado ao
`forge_server::router()` existente — **`forge-server` ganha zero dependência nova**,
o crate estável/em-produção-real (túnel SSH) fica intocado. Flag de opt-in
(`--web-agent`) até o fecho. Job `web` novo no CI já aqui (o harness Playwright já
existe, só não roda em CI).

**Segurança de mutação (bloqueante desde esta onda):** middleware único validando
`Origin`/`Host` contra localhost em todo método ≠ GET — ausência de `Origin`
permitida (curl/CLI seguem funcionando sem navegador), `Origin` de outra origem →
`403`. Esta é a rota que literalmente aprova execução de `bash`; sem essa guarda,
qualquer site aberto no mesmo navegador poderia disparar `POST
http://127.0.0.1:7878/api/session/:id/permission`.

**Contrato de erro e cliente HTTP:** `client.ts` ganha `fetchJson()` real (checa
`r.ok`, lança `ApiError` com código); toda rota nova responde erro como JSON único
`{error, code}` — fim do padrão "assume sucesso" que os módulos mock têm hoje.

**Pedidos de permissão sobrevivem a navegador fechado:** o pedido pendente vive em
estado do servidor (não só publicado uma vez no SSE) — quem conectar depois (aba
nova, ou a mesma aba reconectando) recebe o pedido ainda pendente via snapshot. Sem
isso, fechar o navegador no meio de uma aprovação perde o evento e o pedido fica
órfão até o timeout.

**Teto de sessões vivas:** cada sessão ocupa uma thread do pool `spawn_blocking`
enquanto viver — limite configurável (ex.: 8 sessões simultâneas), `429` acima do
teto.

*Decisões→ADR:* forma do DTO; contrato SSE — nomes de evento e semântica de
reconexão (**snapshot do estado atual, reconstruído do `DurableSession`, + eventos ao
vivo daí em diante**; `Last-Event-ID`/replay fino explicitamente fora de escopo);
timeout de permissão pendente sem resposta (fail-closed, `Deny` após prazo); teto de
sessões simultâneas.

*Fronteira:* servidor axum real em porta efêmera, generator sequenciado novo pede
`Ask` e encerra; cliente HTTP real (reqwest + `bytes_stream`) recebe o SSE, um `POST`
resolve, sequência de eventos verificada por igualdade contra o esperado. Segundo
teste: sem resposta, o resolver expira em `Deny` sozinho (prazo encurtado via config
de teste). Terceiro teste: requisição `POST` com `Origin: https://evil.example`
recebe `403`; a mesma requisição sem `Origin` passa. Quarto teste: conectar o SSE
**depois** do pedido de permissão já existir — o cliente ainda vê o pedido pendente
(prova o snapshot-then-live, não só o caminho feliz de "já estava conectado").

### Onda 2 — Sessão de código + Permissão ao vivo (o marco da fase)

`POST /api/session/:id/message` replica a receita `prepare`/`build_loop`/
`open_durable` e transmite via SSE; `Sessao.tsx` troca mock por `EventSource` real
(hook novo, `useEventSource`); `Permissao.tsx` reflete o pedido pendente real.
Empacotado junto (reusa `PermissionEngine`/`Rule`, já `Serialize`): a matriz de
permissão build/plan×tool (`togglePermissionCell`) vira persistida — o item #4 do
inventário, com a decisão de edição (não read-only) tomada acima.

**`skills.ts` perde o fallback silencioso:** `fetchSkills()` hoje cai em `SKILLS`
mock quando o `fetch` falha (`web/src/api/skills.ts:37-45`) — isso mascara um
backend quebrado atrás de dado falso. Vira estado de erro explícito no `AsyncStatus`
existente; mock só sobrevive em teste unitário.

**Trilha de auditoria da matriz de permissão:** afrouxar permissão pelo navegador é a
mutação mais sensível deste plano. Toda gravação/remoção de `Rule` vira uma entrada
no ledger (override marcado, mesmo padrão append-only já existente); a UI lista as
rules ativas com botão de revogar; o escopo da rule (`tool` + `scope_prefix`) aparece
explícito no modal antes de confirmar — nunca um clique único e opaco.

*Decisões→ADR:* concorrência multi-aba — sessão = ator único por `session_id`,
turnos serializados, segunda tentativa concorrente recebe `409` (não corrupção); o
terceiro estado `"always"` do frontend grava uma `Rule` de override, não só resolve
o pedido atual; mutação de política de permissão sempre deixa rastro no ledger.

*Fronteira:* Playwright real — dashboard sobe com generator sequenciado (sem key),
mensagem → pedido de `bash` real aparece na tela `Permissão` → clica "Permitir" →
texto final + "ledger íntegro: N" batem com leitura direta do `.forge/forge.db`.
Segundo teste: duas abas na mesma sessão, a segunda escrita concorrente recebe erro
claro, não corrompe o histórico. Terceiro teste: editar uma célula da matriz grava
uma `Rule`, aparece uma entrada nova no ledger, e o botão "revogar" a remove da lista
ativa. Quarto teste: backend fora do ar mostra estado de erro explícito na tela
Skills, não o array mock.

### Onda 3 — Sidecar Python como serviço de longa duração

Hoje `SidecarSupervisor`/`SquadClient` spawnam `uv run ...` com `kill_on_drop` —
ciclo de vida por-invocação-CLI. Um supervisor-serviço novo (distinto do
supervisor-CLI existente, que continua intacto) mantém o processo vivo entre
requisições, com health-check e restart-on-crash. Zero dependência de Onda 1 — é
sobre supervisão de processo, não HTTP.

*Decisões→ADR:* instância única compartilhada para PromptForge (stateless,
serializar um `render` por vez é aceitável); pool pequeno com limite para Squad
(execução longa, um processo só serializaria squads concorrentes).

*Fronteira:* supervisor real atende 3 requisições sequenciais sem reabrir o
processo (PID estável) → `SIGKILL` no meio (mesmo padrão de `squad_e2e.rs`) →
detecta a queda, sobe processo novo (PID diferente), próxima requisição atendida
sem o servidor Rust reiniciar.

### Onda 4 — Squad ao vivo *(depende de Onda 1 + Onda 3)*

`POST /api/squad/run` via `SquadService.ExecuteTask` (usa o supervisor-serviço),
transmite `SquadEvent` como SSE — sem DTO espelho (serde direto no tipo gerado pelo
proto). O gate HITL troca stdin por `POST /api/squad/:task_id/hitl`, mesma forma da
ponte de permissão (incluindo persistência de pedido pendente, ADR 0016/0017).

*Fronteira:* Playwright — squad real (Python, sem key) mostra agentes mudando de
estado ao vivo (não array estático), gate HITL resolvido pela UI, `squad.consensus`
conferido direto no ledger.

### Onda 5 — Prompts

Metade CRUD (`GET/POST /api/prompts`, listar/salvar/favoritar/remover sobre
`PromptLibrary`) é wiring puro, zero dependência — pode sair antes até da Onda 1
fechar. Metade `render` depende do supervisor-serviço (Onda 3).

*Fronteira:* CRUD — teste HTTP direto confere sqlite. Render — texto devolvido pela
rota bate com chamada gRPC direta ao mesmo sidecar (paridade, não só "200 OK").

### Onda 6 — Ledger (vitória rápida, zero dependência)

Leitura paginada nova sobre `LedgerStore` (precedente exato em
`TelemetryStore::recent`) + `GET /api/ledger?limit&actor` + `POST
/api/ledger/verify` sobre `verify_chain()` já existente. Liga WAL em
`LedgerStore::open` (bug de concorrência latente, exposto agora). O filtro `?actor=`
entra desde o primeiro corte — a tela mock já filtra por ator; entregar a rota sem
isso forçaria filtro client-side sobre um dump completo, regressão de UX disfarçada
de "ligado".

*Fronteira:* semeia N entradas via `LedgerStore::append` por fora do browser, a
tela mostra exatamente essas N (hash prev/curr por igualdade); `?actor=X` devolve só
as entradas de X; verificação mostra `ok:true, verified:N`.

### Onda 7 — Console MCP + Uso por modelo (telas dedicadas)

Fecha A1 **por completo** (não só o piso da versão anterior) e A5 — cada um como
tela **própria** (`mcp`, `modelos`), não cartão embutido em Skills/Telemetria.

- **A1** (router de `forge-cli` — precisa de `forge-tools`+`forge-core`): `GET
  /api/mcp` enumera `.forge/mcp.toml` e chama `list_tools_blocking`
  (`forge-tools/src/mcp.rs:65`) por servidor em `spawn_blocking` com timeout
  curto; nome namespaced + preview de política via `PermissionEngine::evaluate`
  consultando **o mesmo store de `Rule` persistida da Onda 2** (não os perfis
  const, que não têm regra `mcp__*`). `McpServerConfig`/`McpToolMeta` ganham
  `#[derive(Serialize)]`; `McpServer` (`domain.ts`) ganha `tools:
  McpToolInfo[]`. **Remove** o card MCP mock de `Skills.tsx:86-103`
  (+ estado/handler/import) e `MCP_SERVERS`/`reconnectMcp` de `skills.ts:
  11-15,58-65` — não deixa coexistir.
- **A5** (direto em `crates/forge-server/src/lib.rs` — só precisa de
  `forge-store`, zero dependência real de Onda 1): nova consulta em
  `TelemetryStore` agrupando por `json_extract(props,'$.model')`, mesmo padrão
  de `experiment_variants` (`telemetry.rs:117-139`); coluna `tier` via
  `model_tier::tier_from_id` (real/testado); `ModelTier` ganha
  `#[derive(Serialize)]`. Módulo frontend `web/src/api/modelUsage.ts` (não
  `models.ts`, já ocupado pela tela `modelo` de usuário).
- `nav.ts`/`screenMeta.ts`/`screenComponents.tsx`/`Shell.tsx`'s
  `ADMIN_SURFACE_SCREENS` ganham `'mcp'`, `'modelos'`, na ordem do handoff
  (depois de `telemetria`, antes de `experimentos`/`ratelimit`).

Sem ADR novo além do já previsto (A1 já se apoia no store de Rule da Onda 2/ADR
0018; A5 é mecânico).

*Fronteira A1:* fixture com 2 servidores MCP (1 respondendo, 1 fora do ar) + 1
`Rule` override persistida para um `mcp__<server>__<tool>` específico — a tela
mostra status real + a política do tool com override como `allow`/`deny` (não
"ask" constante) e um segundo tool sem override como "ask" — prova que o preview
lê o engine real, não um default mudo.
*Fronteira A5:* card bate por igualdade com agregação manual dos mesmos eventos
semeados, incluindo a coluna `tier` derivada de `tier_from_id`.

### Onda 8 — Mapa de memória do squad + busca RAG *(depende de Onda 1 + Onda 3)*

A3. `schemas/proto/memory.proto` novo (`MemoryService{Health,Recall,List}` — sem
`Remember`, quem grava é só o orquestrador via chamada direta) servido por
`python/packages/forge-squad/src/forge_squad/memory_server.py` novo (mirror de
`server.py`, supervisão **singleton**, não pool — leitura de memória é
stateless/barata, misturar com o pool de squad da Onda 3 faria uma consulta
disputar recurso com uma execução de squad real à toa). `memory.py` ganha
`list_memories(agent?, limit)` público (reusa `_load_corpus()` já existente,
zero lógica nova). `crates/forge-sidecar/src/memory_client.rs` novo (mirror de
`SidecarClient`, reusa `SidecarError`/`socket_ready`). Rota no router de
`forge-cli`: `GET /api/memory?agent=&limit=` + `POST /api/memory/recall
{query,k}`; `SidecarError::Unavailable` → `503` explícito (mesmo padrão de
degradação do PromptForge).

**Por que um serviço novo, não os RPCs já declarados:** `CoreService.Recall`/
`Remember` (`core.proto:18-19`) estão `unimplemented!` (`core_server.rs:
111-121`) e são a direção errada — `CoreService` é servido pelo Rust, chamado
pelo Python; o dado de memória mora no Python. O próprio protótipo do handoff
erra isso (cita "CoreService.Recall" na cópia de loading) — a tela real corrige,
não repete o engano. Não estender `SquadService`: quebra o precedente de
um-proto-por-concern do repo e acopla disponibilidade de leitura ao pool de
squad.

**Não-escopo explícito (achado novo):** `forge_squad/forgetting.py`
(`IntelligentForgetting.adaptive_forget`, `MemoryStore`) é código morto — só o
próprio teste unitário chama, nunca `memory.py`/`orchestrator.py`/`server.py`
(mesma classe do achado `max_autonomy_level`). O painel "memória por agente" da
tela real **não** mostra tendência de esquecimento (o protótipo mostra, mas
nada no código computa isso) — só `agent`, contagem real, e a decisão de maior
confiança/mais recente.

Copy da tela mantém a tensão honesta do próprio handoff: rótulo/nav dizem "RAG",
mas rodapé e estado vazio dizem explicitamente "recuperação léxica TF-IDF, não
semântica" (`recall.py`, ADR 0013).

*Decisões→ADR 0022:* por que `MemoryService` novo (não o stub abandonado
`CoreService.Recall/Remember`, direção errada; não `SquadService` estendido);
por que Python segue dono do dado; compromisso de honestidade léxico-não-
semântico na UI; descope explícito do `forgetting.py`.

*Fronteira:* sidecar de memória real sobe, `POST /api/memory/recall` com ground
truth de 2 tópicos de vocabulário disjunto recupera exatamente as memórias do
tópico certo (mesmo padrão da Fase 6 Onda 6); sidecar morto (`SIGKILL`) → `503`
explícito, não tela travada; `GET /api/memory` sem filtro agrupa por agente com
contagem/decisão real, nenhum campo fabricado.

### Onda 9 — Experimentos A/B

A2. `GET /api/experiment/:nome` direto em `forge-server` (mesma classe de
posicionamento de A5 — só `forge-store`+`forge-schemas`); 404 se
`experiment_variants` devolve <2 variantes, 422 se alguma tem 0 amostras
(espelha a validação do CLI, `main.rs:178,203`); `Json(report)` direto, zero DTO
novo (`ExperimentReport`/`VariantStats` já derivam `Serialize`+`JsonSchema`).
Banner obrigatório com a cópia já redigida no handoff ("atribuição por
telemetria ainda em instrumentação — dados semeados"); **sem** instrumentar
produção nesta fase — não-escopo explícito, não descuido. Retarget de
`Sugestoes.tsx:11` (`relatedScreen: 'prompts'` → `'experimentos'`, tag "✓
entregue").

Sem ADR novo (usa `experiment.v1`/ADR 0014 já existente).

*Fronteira:* seed com exatamente 2 variantes (n≥20 cada) → veredito bate com
`two_proportion_p_value` calculado à parte; seed com 1 variante só → 422; nome
inexistente → 404.

### Onda 10 — Rate limits + Sandbox & skills de terceiro + Language servers

3 telas pequenas, admin, independentes entre si (A4, A6, A7).

- **A4** (direto em `forge-server` — reusa a dependência já existente de
  `forge-llm`, hoje só usada pelo bin `loadgen`): `GET /api/ratelimit` — DTO
  `{tier,models,cap,window_secs}` sobre `for_tier()`; banner "uso ilustrativo —
  `poll` é privado e muta ao checar" (`rate_limit.rs:59,70`). Getter de uso, se
  construído, é método **separado e não-mutante** (só poda + conta o deque,
  nunca chama `poll()`) com teste dedicado provando efeito colateral zero — não
  é obrigatório para a onda fechar. Reusa a mesma leitura de tetos que a Onda 12
  (Providers) já planeja — construir uma vez.
- **A6** (router de `forge-cli` — precisa de `forge-tools`): `GET /api/sandbox`
  — perfil (`Sandbox::new` + as constantes hardcoded de `run_with`: rootfs
  read-only, cap-drop ALL, no-new-privileges, documentadas como constantes) +
  `Sandbox::ping()` novo (só `docker.ping()`, sem o resto de `run_with`) + a
  lista de skills de terceiro de `/api/skills` (já real). Tela **read-only** —
  o protótipo do handoff não tem handler nenhum de instalar/vetar/habilitar/
  remover.
- **A7** (router de `forge-cli` — precisa de `forge-tools`): `GET /api/lsp` —
  enumera `.forge/lsp.toml` (mirror do parsing de `load_lsp_servers`), **zero
  probe sob demanda** (quebraria a propriedade que `skills.rs:463-479` já prova
  segura); status/diagnósticos refletem só uso real já ocorrido na sessão.

Sem ADR novo (mecânico, mesma classe de "serialização do /verify"/"paginação do
ledger").

*Fronteira A4:* tetos batem com `for_tier()` para os 3 tiers; se o getter
existir, teste "consultar 2× não move o contador" prova ausência de efeito
colateral.
*Fronteira A6:* fixture sem daemon Docker → `ping()` honesto `false`, tela
mostra fail-closed, não "rodou".
*Fronteira A7:* `.forge/lsp.toml` com comando inexistente → tela mostra
"declarado, não iniciado" sem nenhum processo subir (mesma prova do teste
existente).

### Onda 11 — Verify (job em background, zero dependência)

`POST /api/verify/run` roda `run_pipeline` em `spawn_blocking`, devolve `run_id`;
`GET /api/verify/:id` via polling (hook `usePolling` já existe). Callback de
progresso por passo é extensão nova em `forge-verify` (hoje só devolve no fim).

**Execuções concorrentes são serializadas:** um job de verify por vez — um segundo
`POST /api/verify/run` com job ativo recebe `409` com o `run_id` corrente, em vez de
disputar o mesmo `target/` e workspace. O estado do job vive em memória
(`Arc<Mutex<...>>`) — reinício do servidor perde o job em andamento; aceitável para
um produto local-first, mas documentado explicitamente na tela (não é surpresa).

*Fronteira:* pipeline fixture com passos curtos; status muda "rodando"→"passo N de
M" conforme completam, termina no veredito certo — prova progresso real, não
placeholder. Segundo teste: dois `POST /api/verify/run` em sequência rápida — o
segundo recebe `409` com o `run_id` do primeiro, não um job novo.

### Onda 12 — Providers (piso leitura real; mutação como degrau)

Piso = view real de providers configurados + limites por tier (zero engenharia
nova) — **reusa a leitura de tetos por tier que a Onda 10/A4 já constrói**, não
reconstruir. Degrau = reordenar fallback consumindo o `FallbackChain` já
existente (hoje morto), introspecção+ajuste de teto no `RateLimiter` (precisa de
API de mutação nova — se a Onda 10 já construiu o getter não-mutante, este
degrau o reusa também). Quando A4 (Onda 10) for ao ar, `web/src/api/providers.ts`'s
`RATE_LIMITS` fabricado (`:9-13`) é aposentado — não fica como segunda fonte de
verdade discordante.

*Fronteira (piso):* a tela reflete exatamente `Gateway::available()` e as
constantes de tier — sem fabricar `used/cap`. *Fronteira (degrau, se entrar):*
reordena via POST, dispara com `ScriptedGenerator` (um provider falha de propósito)
e confere que a ordem de tentativa observada é a nova, não a antiga.

### Onda 13 — Modelo & Onboarding

Modelo/agente = parâmetro por sessão/tarefa (mirroring do CLI — sem store de
preferência novo, ver princípio de recorte em §1). `GET /api/doctor` agrega
checagens já existentes mas espalhadas (env vars do gateway, `uv --version`, ping ao
Docker via bollard, git). **Autonomia explicitamente escopada, não implementada por
padrão**: dado que `max_autonomy_level` é ignorado ponta a ponta hoje, a UI pode
mandar o valor real (deixa de ser hardcoded `3`) mas a tela **declara** que o
orquestrador ainda não respeita esse teto — a menos que o produto priorize também a
mudança Python nesta onda (decidir no início, não por composição tácita).

*Decisões→ADR:* se a mudança Python de autonomia entra nesta fase ou vira pendência
re-declarada (mesmo padrão da pendência de consenso→ledger da Fase 6).

*Fronteira:* Doctor contra fixture sem `uv` no PATH mostra o item ausente (gêmeo
negativo, não "tudo verde" sempre). Se autonomia entrar: dois `SquadTask` com
`max_autonomy_level` diferentes produzem **comportamento diferente** (aprovação
pedida num caso, não no outro) — não só "o campo viajou".

### Onda 14 — Designer (salvar honesto)

`POST /api/designer/workflow` valida contra `squad.workflow.v1` novo (JSON Schema +
tipo Rust + fixture golden, padrão de `experiment.v1`) e grava no ledger
(`LedgerStore::append` já aceita payload livre, zero mudança de ledger). Tela troca
"aplica na próxima forge squad" por "salvo e validado; aplicação real é trabalho
futuro". **Orquestrador Python continua com os 5 agentes fixos — sem reescrita
nesta fase.**

*Fronteira:* grafo salvo é lido direto do ledger e valida contra o schema (fixture
com caso inválido); grafo malformado (aresta para nó inexistente) é rejeitado com
erro claro, não salvo silenciosamente.

### Onda 15 — Fecho

README/CLAUDE.md/PLANO-PLATAFORMA-FORGE.md declaram a Fase 7 concluída (ou o estado
honesto do que ficou — nomeadamente autonomia, se descoped); ADRs 0015-0022
citados; flag `--web-agent` vira default; reconciliação explícita do
`LEVANTAMENTO-UI-DESIGNER.md` — **Grupo B fechado e Grupo A fechado 7/7** (não mais
2/7 com exceção).

**Descopes explícitos registrados nos documentos, não só no código:**
`max_autonomy_level` (já existente na Fase 7 original) **e** `forgetting.py`/a
coluna de tendência de esquecimento em A3 (achado desta emenda). Confirma que
`Sugestoes.tsx` não tem mais cartões "proposto" para telas que já viraram reais
(A2/A3 retargetados nas Ondas 8-9).

**Critério mecânico de pronto:** `grep simulateLatency web/src/api` vazio (sobra só
em fixture de teste) — adicionado à verificação desde o início da fase, não só
conferido no fecho.

*Fronteira:* documentos contam a mesma história (grep); job `web` do CI verde há N
PRs seguidos; nenhuma pendência descoped vive fora dos documentos; `docs/PLANO-INTEGRACAO-FRONTEND.md`
segue com sua nota de superseded, sem conteúdo divergente deste documento.

## 3. Decisões de contrato previstas (ADRs)

- **0015** — local-first/single-user permanece; **e** fixa o modelo de ameaça do
  navegador (guarda de `Origin`/`Host` em toda rota mutável — Onda 1).
- **0016** — DTO de evento + contrato SSE: nomes de evento, e a semântica de
  reconexão (snapshot do estado atual + eventos ao vivo daí em diante;
  `Last-Event-ID`/replay fino fora de escopo) e persistência de pedidos pendentes no
  servidor (sobrevivem a navegador fechado).
- **0017** — timeout de permissão pendente, fail-closed (`Deny` após prazo).
- **0018** — sessão-ator, concorrência multi-aba; inclui a trilha de auditoria de
  mutações da matriz de permissão (toda gravação/remoção de `Rule` vira entrada no
  ledger).
- **0019** — sidecar como serviço: instância única vs. pool, restart-on-crash.
- **0020** — topologia de processo (código novo em `forge-cli`, router aditivo,
  flag de opt-in) e teto de sessões vivas simultâneas (429 acima do limite).
- **0021** — escopo da autonomia progressiva nesta fase (`max_autonomy_level`).
- **0022** — `MemoryService` (ponte Rust↔Python para memória do squad, Onda 8):
  por que um serviço novo em vez de reviver `CoreService.Recall/Remember` (stub
  abandonado, direção errada) ou estender `SquadService` (quebra o precedente de
  um-proto-por-concern, acopla ao pool de squad); por que Python continua dono
  do dado; supervisão singleton, não pool; compromisso de honestidade
  "léxico, não semântico" carregado para a UI.

Schema novo: `squad.workflow.v1` (Designer, Onda 14).

Decisões mais mecânicas (serialização de execuções concorrentes do `/verify`,
paginação/filtro do ledger, remoção do fallback mock de `skills.ts`) não geram ADR
próprio — são detalhes de implementação de uma onda, não mudança de contrato ou
fronteira.

## 4. Riscos da fase

| Risco | Mitigação |
|---|---|
| Site aberto no mesmo navegador dispara mutação em `127.0.0.1` (CSRF/DNS-rebinding) | Middleware de `Origin`/`Host` em toda rota ≠ GET desde a Onda 1 (ADR 0015) |
| Permissão pendente nunca respondida trava um `spawn_blocking` para sempre | Timeout configurável, `Deny` default (ADR 0017) |
| Resolver síncrono em `tokio::spawn` comum esgota worker-threads sob N sessões | Sempre `spawn_blocking` por sessão viva + teto de sessões simultâneas (ADR 0020) |
| Pedido de permissão se perde se o navegador fechar antes de resolver | Estado do pedido vive no servidor, reemitido a quem conectar depois (snapshot-then-live, ADR 0016) |
| Duas abas na mesma sessão corrompem histórico | Sessão = ator único, turnos serializados, erro claro na 2ª escrita concorrente |
| Duas execuções de `/verify` disputam o mesmo `target/` e workspace | Um job por vez; segundo POST recebe `409` com o `run_id` corrente (Onda 11) |
| Sidecar Python não sobrevive a servidor de longa duração | Supervisor-serviço dedicado (Onda 3), testado com `SIGKILL`, antes de Squad/Prompts depender |
| Fase 7 quebra o `forge-server` hoje estável/em produção (túnel SSH) | Código novo em `forge-cli`, zero dep nova em `forge-server`, flag opt-in até o fecho |
| `max_autonomy_level` vira wiring de fachada (campo viaja, Python ignora) | Escopo explícito (ADR 0021); fronteira exige provar comportamento diferente |
| Suíte e2e real não roda em CI, regressão passa despercebida | Job `web` já na Onda 1, não no fecho |
| Grupo A vira scope creep além das 7 telas do handoff/levantamento | Escopo travado nas 7 telas nomeadas (A1-A7); nenhuma 8ª tela entra por composição tácita |
| Verify (8.5min) parece travado sem sinal de progresso | Progresso por passo desde o primeiro corte da Onda 11 |
| Afrouxar permissão pelo navegador sem rastro do que mudou | Toda mutação de `Rule` vira entrada no ledger + lista de revogação na UI (Onda 2) |
| Dois planos de integração vivos contam histórias diferentes | Este documento substitui `PLANO-INTEGRACAO-FRONTEND.md`, que recebe nota de superseded na mesma entrega |
| Getter de uso do `RateLimiter` chama `poll()` e consome vaga real só de abrir a tela | Método novo, separado, não-mutante; teste dedicado provando efeito colateral zero (Onda 10) |
| Botão/probe sob demanda no LSP força o lazy-start só para checar status | Escopo explícito: zero probe, só config declarada + uso real já ocorrido (Onda 10) |
| A3 vira uma ponte gRPC nova desproporcional à complexidade do problema | Reuso do padrão PromptForge (singleton, 503 fail-closed honesto); decisão registrada em ADR 0022 |
| Painel de memória mostra tendência de esquecimento fabricada (`forgetting.py` é código morto) | Coluna de decay explicitamente fora da tela (Onda 8) |
| Protótipo do handoff cita o RPC errado (`CoreService.Recall`) e a implementação repete o engano | Corrigido na Onda 8: `MemoryService.Recall`, não `CoreService.Recall` |
| Mock antigo de MCP (`Skills.tsx`) coexiste com a tela nova, 2 fontes de verdade discordantes | Remoção obrigatória listada na Onda 7, não só adição |
| `providers.ts`'s `RATE_LIMITS` fabricado sobrevive depois que A4 é real | Aposentado explicitamente quando A4 for ao ar (nota cross-onda na Onda 12) |

## 5. Sequência e paralelismo

Zero dependência entre si, podem rodar em paralelo desde o dia 1: Onda 3
(sidecar-serviço), Onda 5-CRUD (Prompts), Onda 6 (Ledger), Onda 7 (Console MCP +
Uso por modelo — A1 soft-depende de Onda 2 para o preview de política, A5 não
depende de nada), Onda 9 (Experimentos A/B), Onda 10 (Rate limits + Sandbox +
LSP), Onda 11 (Verify), Onda 12 (Providers), Onda 13 sem autonomia, Onda 14
(Designer). Onda 1→2 é a única cadeia dura. Onda 4 depende de 1+3. Onda 5-render
depende só de 3. Onda 8 (Memória/RAG) depende de 1+3. Onda 15 é sempre última, e
só depois da decisão de autonomia (Onda 13) estar resolvida ou formalmente
re-declarada.

## Verificação

- O documento cobre as 11 lacunas do inventário de Grupo B, as 7 telas de Grupo A
  (A1-A7, fechadas 7/7), as 15 ondas + fecho, as 8 ADRs previstas (0015-0022), e a
  tabela de riscos.
- `grep -nE "^### Onda ([1-9]|1[0-5]) " docs/PLANO-FASE-7-frontend-primario.md`
  lista as 15 ondas na ordem esperada.
- `grep -n "Grupo A" docs/PLANO-FASE-7-frontend-primario.md` confirma o
  fechamento 7/7, não mais a lista de exceções únicas.
- `grep -n "max_autonomy_level" docs/PLANO-FASE-7-frontend-primario.md` confirma o
  achado do dado morto está registrado, não escondido.
- `grep -n "forgetting.py\|MemoryService\|CoreService.Recall" docs/PLANO-FASE-7-frontend-primario.md`
  confirma o achado de código morto do Grupo A e a correção de direção do RPC
  estão registrados, não escondidos.
- `grep -n "ADR 0022" docs/PLANO-FASE-7-frontend-primario.md` confirma a nova ADR
  está referenciada onde a Onda 8 a cita.
- `grep -n "Origin" docs/PLANO-FASE-7-frontend-primario.md` confirma a guarda de
  CSRF/DNS-rebinding está na Onda 1, não differida.
