# Registro de decisões da junção (sessão de 2026-07-05)

Histórico do que foi discutido e decidido ao unificar os três repositórios na
plataforma Forge. Complementa o plano (`PLANO-PLATAFORMA-FORGE.md`) e o ADR 0001.

## As origens e o que cada uma contribui

1. **danzeroum/opencode** (fork TypeScript do coding agent OpenCode) — runtime de
   sessão durável (System Context, Context Epochs, compaction em fronteiras seguras —
   spec em `CONTEXT.md` do repo), agentes selecionáveis (build/plan/general),
   permissões por ferramenta/escopo, ferramentas (grep/edit/bash/webfetch/LSP/MCP),
   TUI. Contribuições próprias do fork: **ModelTier** (classificação small/medium/large
   por id de modelo, comportamento tier-gated: prompt enxuto, menos ferramentas,
   compaction ~75%, step-discipline) e o **pipeline de verificação determinística**
   (`/verify`: typecheck→test→lint→SAST com evidência JSON; filosofia "o LLM orquestra;
   ferramentas determinísticas verificam"), skill-vetter e CI de segurança.
2. **danzeroum/prompte** (ferramenta web de engenharia de prompts, JS/Node) —
   geradores declarativos `{name, fields, build}`, base de conhecimento aditiva
   (3 níveis), quality linter ("ESLint de prompts"), **cache por hash** (JSON canônico
   de chaves ordenadas + sha256; contrato hash cliente == servidor, `api/src/hash.js`),
   rate limiting auth-aware, proxy LLM seguro com fallback (keys só no servidor),
   biblioteca de prompts, telemetria offline-first com dashboard.
3. **danzeroum/BuildToValue_AI_Agent_Specialization** (metodologia BuildToFlip v6 +
   protótipo Python) — squad de agentes especializados
   (Architect/Developer/Auditor/Designer/Ops + Supervisor/Exploration/Recovery),
   UnifiedOrchestrator (recall→plano→propostas→consenso→execução→auditoria→ledger→
   aprendizado), **consenso ponderado por expertise**, planejamento hierárquico,
   LearningRouter, memória com esquecimento inteligente, HITL/autonomia progressiva,
   **fallback progressivo 3 níveis**, ledger append-only, "Nada Fake", review por
   valor (4 reviewers, value_score > 0.7), quality gates e certificação.

## Decisões de produto (do usuário)

- **Produto final**: CLI/TUI de coding agent (`forge`) cujo motor é o squad
  multi-agente, com camada de prompts/qualidade do prompte.
- **Escopo**: 100% das ideias dos 3 repos, roadmap completo em 6 fases longas
  (~44–56 semanas), cada fase terminando em software usável.
- **Linguagens por design**: Rust + Python (pedido original da junção).
- **Sede**: inicialmente workspace `platform/` no BuildToValue; em seguida o usuário
  criou o repositório dedicado **mix_btv_code** — o trabalho passa a viver aqui, com o
  workspace promovido à raiz e commits direto na `main`.

## Decisões de arquitetura (ADR 0001)

- **Regra de fronteira**: Rust = tudo que toca disco/rede/processo/segredo ou roda a
  cada keystroke; Python = tudo que decide o próximo passo por raciocínio de agente.
- **Integração**: gRPC bidirecional sobre Unix Domain Socket (`tonic`/`prost` ×
  `betterproto`/`grpclib`). PyO3 rejeitado no caminho principal (conflito
  tokio×asyncio, isolamento de falhas). Crash do sidecar aciona o fallback
  progressivo do BuildToValue: squad → agente-único → safe-mode read-only.
- **Segurança**: API keys só no processo Rust (princípio do proxy do prompte);
  permissões não contornáveis pelo Python; skill-vetter determinístico; gitleaks
  bloqueante no CI.
- **Contratos**: fonte única em `schemas/` — protobuf no wire, JSON Schema
  (`*.v1.schema.json`) para documentos auditáveis, golden fixtures de paridade
  cross-language; breaking → `.v2` + ADR.

## O que já foi entregue (scaffold da Fase 1)

- Workspace cargo (10 crates) + uv (5 pacotes), compilando com **26 testes Rust +
  13 Python verdes**, clippy/fmt limpos.
- Contratos: 3 protos gRPC (`core`, `squad`, `llm`), 6 JSON Schemas, fixtures de
  paridade do hash de cache validadas pelos dois lados.
- Portes reais: ModelTier (de `model-tier.ts`, com exclusões substituindo lookaheads),
  motor de permissões com perfis build/plan/general, ledger hash-chain com detecção de
  adulteração testada, `/verify` mínimo com evidência JSON, contrato de ferramenta com
  truncamento UTF-8 seguro, consenso ponderado migrado e tipado (pydantic, gatilho
  HITL < 0.7), primeiros geradores declarativos, quality linter, value_score do review.
- Operação: justfile, CI, ADR 0001, `scripts/gen_fixtures.py`.

## Estado dos repositórios de origem (referência histórica)

Branch `claude/multi-repo-implementation-plan-brp6w4` em cada um:

- **opencode**: documento do plano mergeado na `dev` via **PR #196** (squash
  `9b478e5`), CI verde (typecheck, unit, gitleaks, semgrep, compliance, standards,
  nix-eval).
- **prompte**: documento do plano commitado (`ed7419d`), sem PR.
- **BuildToValue**: plano + scaffold `platform/` (`a18282e`) + roadmap visual
  (`41efdb6`), sem PR. O conteúdo foi migrado para este repositório.

## Nota técnica: o roadmap visual

`docs/roadmap-forge.html` é a versão autocontida (React 18.3.1, ReactDOM e o runtime
DC embutidos) do roadmap interativo. Durante o merge foi encontrado e corrigido um bug
real: o runtime DC re-parseia o texto da própria página e corta o template a partir do
primeiro `<x-dc>` literal — que passou a existir dentro do próprio runtime embutido
(string de erro `"has no <x-dc> block"`). A correção quebra o literal em concatenação
(`"<x-dc" + ">"`). Verificado no Chromium headless via `file://` e HTTP: render,
expansão de fases, filtros da matriz (21 ideias) e acordeões funcionando.

## Próximos marcos (Fase 1)

Entregue em 2026-07-05 (segundo commit da main): loop de agente real no
`forge run` — gateway HTTP com streaming SSE e fallback (Anthropic/OpenAI/
DeepSeek, keys por env), agregadores de stream testados com fixtures (sem
rede), ferramentas read/grep/edit/bash sob o motor de permissões (grep
respeita .gitignore; edit exige trecho único; bash com timeout), loop
genérico sobre `Generator` (testes com gerador roteirizado cobrem edição
fim-a-fim, negação de permissão e limite de passos) e sessão com ledger
hash-chain em `.forge/forge.db`.

Fase 1 concluída (terceiro commit da main): `forge chat` (REPL multi-turno
via `continue_run`, histórico carregado entre turnos — testado) e cache de
prompt ligado ao gateway (`CachedGenerator` decorando o `Gateway`; chave =
`request_hash` do envelope canônico modelo+system+tools+histórico; hit
devolve o turno sem rede e marca provider `+cache` no ledger — testado com
gerador contador). Total: 51 testes Rust + 13 Python.

O critério de aceite com API real (`forge run` editando um repo de verdade)
fica pendente de uma API key configurada pelo usuário — toda a cadeia até a
borda HTTP está coberta por testes.

Fase 2 na sequência: sessões duráveis (System Context/Epochs/compaction),
TUI ratatui, tier-gating completo.

## Porte seletivo da branch rust-migration (2026-07-05)

Avaliada a `rust-migration` do opencode (~40k linhas Rust em 14 crates,
migração strangler-fig do backend TS): **decidido não copiar integralmente**
— traria o monorepo TS e a maquinaria de coexistência TS↔Rust que o Forge
não precisa. Portados os módulos coerentes (detalhes no ADR 0002):

- **EventStore** (`opencode-db`/`opencode-events`) → `forge-store::events`
  (rusqlite, WAL, concorrência otimista por `(aggregate_id, seq)`).
- **Sessões duráveis** → `forge-core::session::DurableSession`: conversa
  como agregado de eventos `message.1`, replay reconstrói o histórico,
  conflito detecta escritores concorrentes. CLI: `--session <id>` retoma;
  toda execução é persistida em `.forge/sessions.db` (primeiro marco da
  **Fase 2**).
- **grep com libs do ripgrep** → `forge-tools::grep` (Searcher + ignore).
- **edit `replace_all`** → `forge-tools::edit`.
- **deny.toml + cargo-deny** → gate de supply-chain no CI.

Não portados: proxy reverso/`openapi-diff` (contrato legado), verificador
de journal de migrations, crates acoplados ao opencode.

## Fase 2 — epochs, compaction e TUI (2026-07-05)

- **Context Epochs + compaction**: `compaction.rs` no forge-core — estimativa
  de tokens chars/4 (tokenizer BPE real segue won't-do, herdado do fork),
  política tier-gated (small compacta a ~75% da janela, demais a ~90%),
  fronteira segura = último turno do assistente sem tool_use pendente
  (nunca corta par tool_use/tool_result). O resumo é gerado pelo próprio
  modelo sem ferramentas; `DurableSession::compact` grava `epoch.started.1`
  + a baseline resumida num único append atômico e o replay recomeça da
  última época. CLI: compaction automática antes de cada turno e `/compact`
  manual no chat; `--context-window` configura a janela.
- **TUI ratatui**: crate `forge-tui` com estado e render puros (testados via
  TestBackend: transcript, streaming, modal de permissão, scroll) e comando
  `forge tui` no CLI — loop de agente numa task tokio, UI na thread
  principal, canais para eventos e resolver de permissão bloqueante
  respondido pelo modal (s/n). Sessão durável e ledger integrados.

## Fase 2 concluída: diff viewer, seletor de modelo/agente e Managed Tool Output Files (2026-07-05)

- **Managed Tool Output Files**: `forge-tools::bound_output_managed` — quando
  o output de uma ferramenta excede `DEFAULT_OUTPUT_LIMIT` (32 KiB), o
  conteúdo completo é gravado em `.forge/tool-outputs/<id>.txt` e o
  `tool_result` devolvido ao modelo inclui o caminho ("use read para
  consultar"). `read`, `grep` e `bash` usam a versão gerenciada; testado
  fim-a-fim pelo loop de agente (comando que gera >32 KiB via `bash`).
- **Diff viewer**: `forge-tools::diff` calcula o diff de linhas do `edit`
  pela simplificação maior-prefixo-comum/maior-sufixo-comum (exata para
  edições localizadas, o caso comum), com janela de contexto de 2 linhas.
  O `ToolOutput` ganhou o campo `diff`, propagado por
  `LoopEvent::ToolFinished` até a TUI, que renderiza um bloco colorido
  (`Item::Diff` em `forge-tui`, vermelho/verde/cinza) logo após o resultado
  da ferramenta.
- **Seletor de modelo/agente na TUI**: `Ctrl+M` percorre uma lista curada de
  modelos (cobrindo os três tiers e providers) e `Ctrl+G` alterna
  build/plan. A troca é um `UiCommand` consumido pela task do agente, que
  reconstrói o `AgentLoop` (barato, sem I/O) antes do turno seguinte — sem
  precisar reiniciar o processo.

Com isso a Fase 2 do roadmap está completa: sessões duráveis, epochs +
compaction, TUI (transcript, diff, permissões, seletor) e Managed Tool
Output Files. 81 testes Rust + 13 Python verdes. Próximo marco: Fase 3
(ativação do gRPC com o sidecar Python PromptForge).

## Fase 3 iniciada: primeira ativação do gRPC — PromptForgeService (2026-07-05)

ADR 0003. Contrato `schemas/proto/promptforge.proto` (Health/Lint/Render/
ListGenerators) — nenhum RPC gera texto de LLM, mantendo a regra de ouro
do ADR 0001.

**Desvio deliberado do ADR 0001**: Python usa `grpcio`/`grpcio-tools` em
vez de `betterproto`/`grpclib` — mais maduro e mantido; o script
`scripts/gen_proto_py.py` corrige o import absoluto que o
`grpc_tools.protoc` gera por padrão para relativo, já que os stubs vivem
dentro do pacote `forge_proto`.

**Geração sem toolchain de sistema**: `forge-proto/build.rs` usa
`protoc-bin-vendored` (protoc vendorizado) em vez de exigir `protoc`
instalado — funciona em qualquer máquina com Rust.

**forge-sidecar** (novo crate): `SidecarClient` conecta via
`tonic::transport::Endpoint::connect_with_connector` sobre
`tokio::net::UnixStream`; `SidecarSupervisor::spawn` sobe `uv run python
-m forge_promptforge.server --socket <path>` e `wait_ready` faz poll do
socket + health check até um timeout, detectando também se o processo
morreu antes (stderr incluído no erro). `kill_on_drop` mata o processo
quando o supervisor é dropado.

**Degradação graciosa total** (fallback progressivo do BuildToValue
aplicado aqui): `forge-cli::sidecar::try_start()` devolve `None` se o
workspace Python, `uv`, ou o health check falharem — `run`/`chat`/`tui`
continuam funcionando por completo, só sem lint consultivo e sem
`/prompt`. Nunca é fatal.

**Servidor Python real**: `forge_promptforge/server.py` — `grpc.aio`
sobre `unix://`, implementando o servicer com os módulos puros já
existentes (`lint_prompt`, `GENERATORS`), sem duplicar lógica.

**CLI**: lint automático (aviso não bloqueante) antes de cada turno em
`run`/`chat`/`tui`; comando `/prompt` no chat lista e renderiza geradores
via sidecar.

**Verificação em duas camadas**: `forge-sidecar/tests/client_over_uds.rs`
(servidor mock Rust sobre UDS, sempre roda) e
`forge-sidecar/tests/python_sidecar.rs` (processo Python real via `uv
run`, valida health/lint/render/list_generators fim-a-fim; pula
graciosamente sem `uv`/workspace). CI (`rust` job) instala `uv` e roda
`uv sync` antes dos testes para exercitar o caminho real — confirmado
localmente com o processo Python de verdade respondendo por gRPC.

Total: 84 testes Rust + 19 Python verdes; clippy `-D warnings` e rustfmt
limpos.

## Fase 3 concluída: rate limiting, telemetria, biblioteca de prompts e dashboard (2026-07-05)

- **Rate limiting tier-gated** (`forge-llm::rate_limit`): sliding window em
  memória (`VecDeque<Instant>` atrás de um `Mutex`), limites default mais
  conservadores para modelos caros (`ModelTier::Small` 60/10min,
  `Medium` 30/10min, `Large` 15/10min), `acquire()` async espera até a
  janela liberar ou devolve `RateLimitError` se a espera excede
  `max_wait`. Testado com `#[tokio::test(start_paused = true)]` para
  determinismo sem `sleep` real. Decorator `RateLimitedGenerator`
  (`forge-cli/src/rate_limit_gen.rs`) fica por baixo do
  `CachedGenerator` na cadeia — um hit de cache nunca consome uma vaga
  do limitador. `GatewayError` ganhou a variante `RateLimited(String)`.
- **Telemetria offline-first** (`forge-store::telemetry`, origem: prompte):
  `TelemetryStore` grava eventos (`name`, `session_id`, `props`, `ts`) em
  SQLite; `TelemetrySummary` agrega por nome e calcula
  `cache_hit_rate = hits/(hits+misses)`. Handle `Telemetry` clonável
  (`Arc<Mutex<TelemetryStore>>`) cujo `record()` nunca propaga erro —
  só avisa no stderr. `CachedGenerator` e `RateLimitedGenerator` gravam
  `cache.hit`/`cache.miss`/`llm.call`. Nada sai da máquina do usuário.
- **Biblioteca de prompts** (`forge-store::prompt_library`, origem:
  prompte `library.js`/`savedPrompts.js`): `PromptLibrary` sobre SQLite
  com `save`/`list(tag)`/`get`/`toggle_favorite`/`delete`, todos
  idempotentes/seguros para ids inexistentes. Ligada ao chat via
  `/prompt save <nome> [tags=a,b] <gerador> chave=valor...` (renderiza
  pelo sidecar e grava), `/prompt library [tag]`, `/prompt use <id>`,
  `/prompt fav <id>`, `/prompt rm <id>` — funcionam mesmo sem sidecar
  (só `save` e o render bruto exigem o gerador Python ativo).
- **Dashboard de métricas** (`forge-server`, origem: prompte): API axum
  local (`GET /`, `GET /api/summary`, `GET /api/events?limit=N`) servindo
  uma página HTML autocontida (fetch + auto-refresh a cada 5s) sobre o
  mesmo `Telemetry` handle. Comando `forge dashboard [--port]` abre
  `.forge/telemetry.db` e sobe o servidor em `127.0.0.1` (nunca expõe a
  rede). Testado com `axum::Router::oneshot` (sem bind de socket real)
  e smoke-testado de ponta a ponta com o binário real via `curl`.

Total: 101 testes Rust + 19 Python verdes; clippy `-D warnings` e rustfmt
limpos. Fase 3 do roadmap está completa. Próximo marco: Fase 4 (squad
multi-agente via sidecar Python, consenso ponderado do BuildToValue).

## Fase 4 planejada: lineage canônica do squad resolvida antes do porte (2026-07-05)

Antes de começar a migração, inspecionamos `src/` do
`BuildToValue_AI_Agent_Specialization` arquivo a arquivo (via GitHub API,
sem clone) para montar o checklist de porte ordenado por dependência.
Achado central, registrado no **ADR 0004**: o repositório tem **três
orquestradores** (`orchestrator.py::AgentOrchestrator`,
`protocols/squad_orchestrator.py::SquadOrchestrator`,
`orchestration/unified_orchestrator.py::UnifiedOrchestrator`) e **duas
hierarquias de agente** (`core/safe_agent_base.py::SafeAgentBase` e
`agents/base_agent.py::BaseAgent`) — gerações sucessivas da mesma ideia,
não peças complementares. Só `UnifiedOrchestrator` + `BaseAgent` são
canônicos: confirmado por leitura direta de `unified_orchestrator.py`, que
instancia os 5 agentes reais (architect/developer/auditor/designer/ops,
todos herdando de `BaseAgent`) e chama consenso/planning/routing/memória/
paralelo/sandbox/avaliação — batendo exatamente com o fluxo do plano.
`SafeAgentBase` não tem nada a migrar como código (`execute()` devolve
`f"executed::{task}"`, simulado; seus guardrails são todos `return
bool(pattern)`, sem lógica real, e mesmo que houvesse não pertenceriam ao
Python pela regra de fronteira do ADR 0001). Também resolvida a colisão de
nomes entre `evaluation/continuous_eval.py` e `continuous_evaluator.py`
(duas classes `ContinuousEvaluator` diferentes — só a segunda é importada
pelo orquestrador canônico).

Achado que corrige o próprio plano: são **5 agentes wireados**, não os 8
que a tabela de mapeamento 100% lista. Exploration/Recovery não têm
chamador algum (Recovery é o método `_attempt_recovery` do orquestrador,
não uma classe). Supervisor é diferente: tem um chamador real
(`src/main.py`, o único entrypoint `__main__` do repo), mas com uma
interface incompatível com `BaseAgent` (`run()` num `@dataclass` solto,
não `execute()` numa ABC) — não é "sem uso", é uma peça de arquitetura
separada (coordenador acima do squad) que não pertence à Onda 2. A tabela
ganhou uma nota de rodapé apontando para o ADR 0004. Duas adaptações de
interface já mapeadas para a Onda 4 do checklist: o check manual de consenso
`< 0.7` vira `consensus.requires_human` (property já pronta no
`ConsensusResult` pydantic migrado em `forge_squad/consensus.py`), e as
propostas dos agentes precisam ser envolvidas em `Proposal(...)` antes de
`reach_consensus` (que já espera o tipo pydantic, não dicts soltos).

Checklist de porte por onda (tasks #35–38 do tracker da sessão): onda 1
módulos-folha (memory/routing/chains/utils/sandbox), onda 2 `BaseAgent` +
5 agentes reais, onda 3 planning/parallel/hitl, onda 4
`UnifiedOrchestrator` + ativação real do `SquadService` bidirecional.
`orchestrator.py`, `safe_agent_base.py`, `squad_orchestrator.py` e
`continuous_eval.py` ficam fora do porte; `rag_tool.py`/`mcp_server.py`
adiados para a Fase 6.

## Fase 4 — Onda 1 portada: módulos-folha do squad (2026-07-05)

Antes de portar, a mesma checagem de "tem chamador real?" aplicada no ADR
0004 foi repetida para o material da Onda 1. Achado: `utils/observability.py`,
`utils/tool_utils.py` e `safety/guardrails.py` **não têm importador algum**
em lugar nenhum do BuildToValue de origem — nem no `UnifiedOrchestrator`,
nem nos 5 agentes, nem no demo `main.py` (confirmado por leitura direta dos
7 arquivos, já que a busca por código do GitHub voltou resultados
inconsistentes com índice provavelmente desatualizado logo após o merge).
Mesmo padrão do `safe_agent_base.py` descartado: código órfão, não portado.
`utils/memory_utils.py::MemoryStore` só é usado pelo demo `main.py` — também
ficou de fora. Achado extra relevante: `agents/developer_agent.py` importa
`from buildtovalue import BuildToValueReviewSystem` — acoplamento direto ao
review system (Fase 5) dentro de um agente da Fase 4; fica registrado para a
Onda 2 resolver (stub/adiar a chamada de review até `forge_review` existir).

Portado para `forge_squad` (com testes, 17 novos, 36 no total do workspace
Python): `security.py` (`SecurityConfig` — confirmado dependência real via
`secure_executor.py`, importado pelo `UnifiedOrchestrator` através do
sandbox), `sandbox.py` (`SecureToolSandbox` + `DockerSandbox` stub — Docker
de verdade é Fase 6), `memory.py` (`AgentMemorySystem`, diretório de
armazenamento renomeado de `.buildtoflip/ledger` para a convenção `.forge/`
do resto da plataforma), `forgetting.py` (`IntelligentForgetting` +
`MemoryStore`), `routing.py` (`LearningRouter`), `chains.py` (`ChainStep` +
`ResilientPromptChain`). Um bug de código morto da fonte original foi
identificado e documentado (não corrigido, só registrado): em
`SecureToolSandbox._validate_security`, o `ValueError` de
`_validate_params_safety` é inalcançável, porque `SecurityConfig.validate_tool_call`
já varre a mesma lista `FORBIDDEN_PATTERNS` contra `str(params)` primeiro e
sempre levanta `SecurityError` antes (nuance: isso vale na prática, para
qualquer padrão realista — as duas checagens serializam os params de
formas diferentes, `str()` × `json.dumps()`, então não é uma prova formal
para todo input possível).

## Fase 4 — Onda 2 iniciada: ADR 0005 e ArchitectAgent real (2026-07-05)

Dois obstáculos de sequenciamento identificados antes de portar os
agentes: (1) `core.proto`/`llm.proto` ainda não têm stubs gerados nem em
Rust (`forge-proto/build.rs`) nem em Python (`gen_proto_py.py` só compila
`promptforge.proto`) — a ativação do gRPC real é Onda 4, então agentes que
dependessem do client real ficariam bloqueados fora de ordem; (2)
`developer_agent.py` importa `BuildToValueReviewSystem` de um pacote
externo `buildtovalue`, acoplando a Fase 5 dentro de um agente da Fase 4.

**ADR 0005** resolve os dois com o mesmo movimento — injeção de
dependência na fronteira do agente: `forge_squad.gateway.GatewayClient`
(`Protocol` async `generate(LlmRequest) -> LlmResponse`, pydantic,
espelhando `llm.proto` sem depender dos stubs gerados) +
`ScriptedGatewayClient` (fake roteirizado para teste, mesmo princípio do
gerador roteirizado já usado nos testes Rust do loop de agente);
`BaseAgent.attach_gateway()` no mesmo padrão de `attach_memory()`; e
`review_system` como dependência opcional (`None` por padrão) em vez de
instanciado direto — fica pendente para quando `forge_review` existir.

`BaseAgent` portado (`forge_squad/agents/base.py`, fiel à origem +
`attach_gateway`). `ArchitectAgent` portado como implementação de
referência (`forge_squad/agents/architect.py`): na origem,
`reason_with_cot` era 100% heurística fixa — os "passos" de Chain-of-
Thought eram literais constantes, sempre os mesmos independente do
problema recebido. A versão portada chama `self.gateway.generate(...)`
de verdade, pedindo ao modelo um JSON estruturado
(`problem_analysis`/`constraints`/`applicable_patterns`/`trade_offs`/
`recommendation`/`confidence`), com parsing defensivo — bloco JSON
extraído via regex (tolera cercas de código markdown ao redor), e
qualquer falha de parsing cai num fallback de confiança 0.0 em vez de
lançar. `create_plan`/`create_adr` continuam deterministas sobre o
resultado real do raciocínio.

12 testes novos (48 no total do workspace Python): `ScriptedGatewayClient`
(ordem das respostas, esgotamento), `BaseAgent` (injeção de dependências,
`validate_confidence`, `log_decision` sem memória anexada) e
`ArchitectAgent` (execução real com gateway roteirizado, erro claro sem
gateway anexado, fallback defensivo em resposta sem JSON, parsing
tolerante a texto ao redor do JSON, histórico de raciocínio acumulando
entre chamadas).

Restam da Onda 2: `developer_agent.py` (ReAct loop real +
`review_system` opcional), `auditor_agent.py`, `designer_agent.py`,
`ops_agent.py` — mesmo padrão do `ArchitectAgent`.

## Fase 4 — Onda 2 corrigida: `create_plan` ainda fabricava a maior parte do plano (2026-07-05)

Revisão do molde antes de replicá-lo 4x encontrou uma falha real: a
primeira versão do `ArchitectAgent` tornou `reason_with_cot` uma chamada
de verdade ao gateway, mas `create_plan` — o método que o orquestrador
executa e o auditor audita — continuava devolvendo `architecture`,
`components`, `risks`, `mitigations` e `estimated_effort` como
**constantes fixas**, com só um componente extra (`"Caching Layer"`)
condicionado a uma checagem de string em `recommendation`. Ou seja: dois
problemas completamente diferentes produziam o mesmo plano. A docstring
chamava isso de "bookkeeping mecânico sobre uma decisão real", o que era
falso — era heurística disfarçada de decisão, atrás de uma chamada real
no método vizinho. O risco não era cosmético: se os 4 agentes restantes
copiassem esse molde, o squad pareceria real (chamadas LLM de verdade no
raciocínio) e produziria saída fabricada — especialmente grave no
`auditor_agent`, onde uma aprovação hardcoded seria um carimbo
automático, o oposto da tese "o LLM orquestra; ferramentas
determinísticas verificam".

Corrigido (ADR 0005 atualizada com o histórico): `_SYSTEM_PROMPT` passou
a pedir `architecture`/`components`/`risks`/`mitigations`/
`estimated_effort` ao modelo, com instrução explícita de refletir o
problema recebido; `create_plan` lê todos os campos de `reasoning`, zero
constantes. Fallback defensivo devolve campos vazios (não um plano
genérico) quando o parsing falha. Novo teste
(`test_dois_problemas_diferentes_produzem_planos_diferentes`) trava
exatamente o que a versão anterior não conseguiria passar: dois
problemas, dois planos diferentes. 33 testes no `forge-squad` (49 no
workspace Python).

Revisão adicional do teste de derivação: a asserção original usava `!=`
(prova só que a saída varia, não que é fiel à resposta do modelo — um
`create_plan` que transformasse os valores em vez de repassá-los ainda
passaria). Trocada por igualdade contra os valores exatos da resposta
roteirizada. Esse virou o padrão de teste adotado nos 4 agentes
seguintes.

## Fase 4 — Onda 2 completa: developer, auditor, designer e ops portados (2026-07-05)

Últimos 4 agentes portados no molde corrigido do `ArchitectAgent` — todo
campo de saída deriva do gateway real, testes provam igualdade (não só
diferença), fallback honesto (vazio/não-aprovado, nunca fabricado).

- **`developer_agent`**: o "loop ReAct" original era 100% roteirizado
  (`think`/`decide_action`/`execute_action` devolviam strings canned por
  keyword matching). Como `CoreService.RunTool` também não existe ainda
  (Onda 4), um loop real de múltiplas iterações executando ferramentas
  não é possível hoje — fingir iterações que não fariam nada de verdade
  seria trocar uma fabricação por outra. Decisão registrada no ADR 0005:
  uma chamada real ao gateway que implementa a tarefa inteira, documentada
  como limitação de escopo atual. `review_system` vira
  `Optional[ReviewSystem]` (Protocol mínimo local) — sem ele,
  `generate_code` devolve o código sem revisão.
- **`auditor_agent`** — o mais arriscado dos quatro: um veredito hardcoded
  aqui é um carimbo automático que destrói a tese "o LLM orquestra;
  ferramentas determinísticas verificam". `check_security`/`check_quality`
  continuam determinísticos (busca de padrão / limiares — legítimos, tipo
  linter), mas viram evidência de entrada para uma chamada real ao
  gateway, que produz o veredito. Testado que ausência de achados
  críticos não força aprovação (`issues == []` e `passed=False` ainda é
  um resultado válido) e que o fallback defensivo nunca aprova por
  engano. Consumo de evidência do `/verify` completo é Fase 5 — não
  antecipado aqui.
- **`designer_agent`/`ops_agent`**: mesmo molde; único resquício
  determinístico é uma guarda de domínio (pattern/strategy do modelo
  precisa estar entre as opções suportadas) — validação de escolha
  externa, não fabricação.

57 testes no `forge-squad`, 73 no workspace Python. Onda 2 da Fase 4
está completa. Próximo: Onda 3 (planning/parallel/hitl).

## Fase 4 — Onda 3 completa: planning, parallel e hitl (2026-07-05)

Nit cosmético resolvido antes de seguir: `DeveloperAgent.react_loop`
renomeado para `implement_task` — o método deixou de ser um loop (é uma
chamada única ao gateway, decisão documentada no ADR 0005), então o nome
antigo descrevia o que ele deixou de fazer.

**ADR 0006** registra as duas colisões de nome encontradas em
`planning/` (mesmo padrão do ADR 0004): `planning/adaptive_planner.py::AdaptivePlanner`
é o único com chamador real (`unified_orchestrator.py`);
`planning/adaptive_replanner.py::AdaptivePlanner` (nome igual, interface
diferente, `_execute_plan` sempre `{"success": True}`) e
`planning/hierarchical_planner.py::HierarchicalPlanner` (confiança
sempre fixa) não têm chamador algum — descartados. O `AdaptivePlanner`
real tinha o mesmo bug já corrigido no `ArchitectAgent.create_plan`:
`_decompose_task` devolvia passos com descrições fixas para qualquer
tarefa. Corrigido do mesmo jeito — decomposição real via gateway, testes
provando igualdade entre tarefas diferentes.

`parallel/resource_manager.py::ParallelResourceManager` portado sem
nenhuma chamada de gateway — é infraestrutura determinística legítima
(semáforo + `asyncio.gather`), forçar uma decisão de LLM ali seria
fabricar onde já existe resposta mecânica.

`hitl/progressive_autonomy.py::ProgressiveAutonomyManager` tinha dois
placeholders: `_request_human_approval` sempre aprovava (carimbo
automático) e `_execute_action` sempre "tinha sucesso" — mas esse
segundo resultado nunca era lido pelo único chamador real, que só
verifica o portão booleano `executed`. Era fabricação morta. Corrigido
com o mesmo padrão do `GatewayClient` (`core.proto`/`RequestPermission`
também sem stubs gerados): `PermissionClient` Protocol +
`ScriptedPermissionClient`. `execute_with_autonomy` virou só o portão —
não finge executar nada; `record_action` fica público para quem executa
de verdade chamar depois com o resultado real. Mudança deliberada de
contrato, não migração literal — preservar o original teria preservado
a fabricação.

19 testes novos (92 no total do workspace Python). Onda 3 da Fase 4 está
completa. Próximo: Onda 4 (`UnifiedOrchestrator` + `SquadService`
bidirecional — a peça que finalmente conecta `GatewayClient`/
`PermissionClient` aos RPCs de verdade).

## Fase 4 — Onda 4a: stubs de proto ativados (core/squad/llm) (2026-07-05)

Passo-porteiro da Onda 4: até aqui, `forge-proto/build.rs` e
`gen_proto_py.py` compilavam **só `promptforge.proto`** (verificado três
vezes ao longo da migração — era o motivo do padrão GatewayClient/
PermissionClient existir). Agora os quatro protos são gerados nos dois
lados.

Decisões técnicas do lado Rust: `core.proto` importa `llm.proto`, e o
prost gera referências entre pacotes por caminho relativo
(`super::super::llm::v1::LlmRequest`) — então a estrutura de módulos em
`forge-proto/src/lib.rs` teve que espelhar a hierarquia de pacotes
(`forge::core::v1`, `forge::llm::v1`, ...), não módulos planos. Aliases
curtos (`pub mod core { pub use crate::forge::core::v1::*; }`) mantêm
caminhos estáveis para os consumidores; em particular `pub mod promptforge`
foi preservado como estava, porque `forge-sidecar` já dependia dele
(confirmado: seus testes, incluindo o do processo Python real, continuam
verdes sem tocar em uma linha do crate).

Do lado Python: o `gen_proto_py.py` só reescrevia imports absolutos →
relativos nos `*_pb2_grpc.py`. Como `core_pb2.py` agora tem `import
llm_pb2` (por `core.proto` importar `llm.proto`), a reescrita passou a
cobrir todos os `*_pb2*.py`. Verificado por round-trip real: importar
`core_pb2`/`squad_pb2`/`llm_pb2` e construir mensagens
(`LlmRequest`/`SquadTask`/`PermissionRequest`), com `CoreServiceStub` e
`SquadServiceServicer` presentes.

Nenhuma mudança breaking: os protos novos são aditivos (a regra do plano
"breaking → `.v2` + ADR" não é acionada). 92 testes Python + workspace
Rust seguem verdes; clippy/fmt limpos. Próximo: 4b (porte do
`UnifiedOrchestrator` com as adaptações dos ADR 0004/0006) e 4c
(`SquadService` real + impls gRPC de `GatewayClient`/`PermissionClient`).

## Fase 4 — Onda 4b: UnifiedOrchestrator + ContinuousEvaluator (2026-07-05)

Capstone do porte (ADR 0007). O orquestrador é **coordenação
determinística** — não chama o gateway, compõe os agentes que chamam. As
adaptações mapeadas nos ADR 0004/0006 aterrissaram: consenso via
`requires_human` (não o `< 0.7` manual), propostas em `Proposal(...)`,
5 agentes reais com `attach_gateway`+`attach_memory`, planner com
`attach_gateway`, autonomia com `attach_permission_client`, e
`record_action` chamado com o resultado real **após** a execução (o
portão não registra mais — ADR 0006).

Dois achados novos, mesmo padrão de rigor das ondas anteriores:

1. **`ContinuousEvaluator` fabricava a nota técnica**: na origem,
   `evaluate_technical_quality` devolvia `result.get("technical_score", 0.8)`,
   mas nenhum agente produz `technical_score` — sempre 0.8, e o portão de
   replanejamento (`< 0.6`) nunca disparava. Mesmo bug do `create_plan`.
   Corrigido: deriva de `confidence`/`success` reais; `improvement` é
   delta contra a média histórica; `business_score` fabricado (0.7)
   removido.
2. **`requires_human` sumia na serialização**: é uma `@property`, e
   `model_dump()` só serializa campos — então o sinal de HITL era perdido
   no dict de resultado que a TUI vai consumir. Corrigido com um helper
   que injeta a property explicitamente. Pego pelo teste (`KeyError`),
   não em produção.

Testado com `RoutingGatewayClient` (fake que roteia por `requester` —
fluxo multi-agente determinístico) e `ScriptedPermissionClient`: 5
agentes, consenso forte dispensa HITL, consenso fraco dispara o portão
(aprovação segue / negação aborta com `success=False`), `record_action`
com resultado real (trust 0.5→0.52 sucesso, 0.5→0.4 rejeição). 11 testes
novos (103 no workspace Python). Próximo: 4c (`SquadService` Python real
+ impls gRPC das duas interfaces + `forge squad` no Rust com fallback de
3 níveis).

## Fase 4 — Onda 4c: laço gRPC bidirecional do lado Python (2026-07-05)

Todo o lado Python do laço bidirecional, testado com round-trips gRPC
reais (sem mock): `GrpcGatewayClient`/`GrpcPermissionClient`
(`grpc_clients.py`) implementam os Protocols do ADR 0005 falando
`CoreService.Generate`/`RequestPermission`; o `SquadServicer`
(`server.py`) roda o `UnifiedOrchestrator` e streama `SquadEvent` ao vivo.

- **Streaming honesto, não pós-fato**: o orquestrador ganhou um
  `event_sink` async opcional (aditivo, default None — os 103 testes
  anteriores seguem verdes) que emite proposta/consenso/hitl/handoff/step
  **conforme acontecem**; o servidor drena esses eventos por uma
  `asyncio.Queue` e os yield como `SquadEvent`. Reconstruir os eventos do
  resultado final seria teatro — a fila garante que são os eventos reais
  da execução.
- **Defesa contra o default-zero do proto3** (o alerta desta rodada):
  `Consensus.requires_human` é `@property` no pydantic e **campo** no
  proto — o mapeamento seta o campo à mão nos dois sentidos. Travado por
  teste (`test_requires_human_true_sobrevive_ao_mapeamento_proto`): com
  consenso fraco, o `SquadEvent.consensus.requires_human` chega `True` do
  outro lado do socket; se o mapeamento omitisse o campo, viria `False`
  silenciosamente. O mesmo default-zero funciona a **favor** no
  `PermissionDecision`: enum ausente = `DECISION_UNSPECIFIED` (0) →
  `approved=False`, fail-closed correto.
- **ADR 0005 cobrado**: o teste bidirecional
  (`test_squad_server.py`) sobe um `CoreService` fake (papel do Rust) + o
  `SquadService` real + um cliente, e prova que os agentes obtêm o LLM de
  volta via `CoreService` (`GrpcGatewayClient` fechou o laço — os
  `Scripted*` não existem ali) e que o HITL negado aborta o stream antes
  de qualquer step.

9 testes novos (112 no workspace Python): 5 do laço bidirecional, 3 do
mapeamento dos clients (agregação de chunks, temperature opcional,
allow/deny→approved), 1 da ordem de emissão de eventos. `forge-squad`
ganhou `grpcio`+`forge-proto-py` como deps (só o servidor/clients
precisam; o núcleo segue só pydantic). Próximo: 4d (lado Rust —
`CoreService` server sobre o `Gateway`, `SquadService` client,
`forge squad`, fallback de 3 níveis, teste cross-process real Rust↔Python
e os critérios de aceite da Fase 4).

## Fase 4 — Onda 4d (parte 1): laço bidirecional Rust↔Python real, e2e cross-process (2026-07-05)

O lado Rust do laço fechou, provado por um teste **cross-process real**
(`squad_e2e.rs`, nos moldes do `python_sidecar.rs`): o Rust sobe o
`CoreService`, spawna o `forge_squad.server` Python de verdade
(`uv run`), chama `ExecuteTask` e coleta o stream de `SquadEvent`. A
execução mostra as 3 propostas com conteúdo real vindo dos callbacks
`Generate`, o consenso com `strength 0.786` e `requires_human=false`
preservado através de todo o caminho (pydantic→proto→wire→Rust), handoffs
start/complete e o step — o critério de aceite `handoff.*` da Fase 4
exercitado de ponta a ponta.

Peças (crate `forge-sidecar`): `core_server.rs` (`CoreService` server com
um `CoreBackend` injetável — `Gateway` real em produção, roteirizado em
teste; `Generate` faz a ponte com o gerador, `RunTool`/`AppendLedger`/
`Recall`/`Remember` devolvem `Unimplemented` honestamente porque o
orquestrador atual não os chama), `squad_client.rs` (`SquadClient` +
`SquadSupervisor` que spawna o Python com os dois sockets). Isolei o
server com um round-trip Rust↔Rust em processo (`core_server_inprocess.rs`)
antes de culpar a interop — passou, o que provou que o problema seguinte
era de interop, não da implementação.

Dois achados reais de interop, ambos corrigidos:

1. **`grpc.default_authority`**: o grpc-python, sobre UDS, deriva um
   `:authority` do path do socket que o servidor tonic (h2) rejeita como
   `PROTOCOL_ERROR` → `RST_STREAM` na primeira chamada `Generate`. Fixar
   `("grpc.default_authority", "localhost")` no canal Python resolve —
   essa é a primeira vez que um cliente Python fala com um servidor Rust
   (as direções anteriores eram todas Rust-cliente → Python-servidor).
   Isolado metodicamente: o round-trip Rust↔Rust passava, então o culpado
   era o cliente Python.
2. **stderr do filho na falha**: o `SquadSupervisor` (como o
   `SidecarSupervisor`, cujo comentário prometia isso mas não entregava)
   agora lê o stderr do processo Python quando ele morre antes de ficar
   pronto e inclui no erro — sem isso, o diagnóstico do RST_STREAM teria
   sido às cegas.

`forge-sidecar` ganhou `tokio-stream` como dep regular (server precisa de
`UnixListenerStream`/`ReceiverStream`). 5 testes do crate verdes (o
`python_sidecar.rs` da Fase 3 segue passando — sem regressão). Restante da
Onda 4d: `forge squad` no CLI (CoreService sobre o `Gateway` real +
resolver de permissão na TUI) e o fallback de 3 níveis (squad →
agente-único → safe-mode).

## Fase 4 — Onda 4d (parte 2): `forge squad` + fallback de 3 níveis (2026-07-05)

O comando `forge squad "..."` fecha a Fase 4. `forge-cli/src/squad.rs`:
`GatewayCoreBackend` implementa o `CoreBackend` sobre o `Gateway` real
(parseia `messages_json` → `GenerateRequest`, agrega o turno, devolve
`(texto, usage)`) — as API keys ficam só no Rust (ADR 0001, confirmado por
grep: zero referência a key de provider no Python, que só conhece o UDS).
`request_permission` resolve o HITL no terminal (auto-aprova com `--yes`),
fechando o caminho humano. O consenso é registrado no ledger via
`session.note("squad.consensus", ...)` conforme o evento chega — critério
literal da Fase 4.

**Fallback progressivo de 3 níveis**, exercitado de verdade num smoke com
key inválida: L1 squad (o Python subiu, o laço rodou, o `Generate` do
planner bateu no Gateway com a key falsa → `error` → `SquadRun::Failed`)
→ L2 agente-único (`run_once`, mesma falha de gateway) → L3 safe-mode
read-only (mensagem, nenhuma ação de escrita, saída limpa). A cascata
inteira num exit 0, sem panic.

**Achado real de robustez — o `kill -9`**: o teste de injeção de falha
(`kill_do_sidecar_dispara_fallback`) revelou que `uv run` spawna o Python
como filho, então `Child::kill()` matava só o wrapper `uv` e deixava o
servidor Python **órfão** (rodando) — o `kill_on_drop` tinha o mesmo
vazamento latente. Corrigido colocando o `uv` como líder do próprio grupo
de processos (`process_group(0)`) e matando o **grupo** inteiro via
`libc::kill(-pid, SIGKILL)`. Só depois disso o `kill -9` de fato quebra o
stream → `Failed` → fallback. Sem essa correção o critério de aceite do
`kill -9` passaria falsamente (o squad sobrevivia e completava).

104 testes Rust (o e2e + o kill, ambos cross-process reais) + 112 Python,
zero falhas. clippy/fmt limpos. **Fase 4 concluída** — o gRPC bidirecional
tonic × grpc-python sobre UDS deixou de ser a aposta mais arriscada do
plano e virou fato testado de ponta a ponta.

## Fase 5 — verificação, review e governança, 6 ondas (2026-07-05/06)

Resumo consolidado; cada onda teve seu próprio ciclo commit→PR→merge com
verificação independente antes de avançar (ADRs 0008–0010 têm o detalhe
de contrato de cada decisão não-óbvia).

- **Onda 1 — `/verify` determinístico completo**: reescrita de
  `crates/forge-verify` com timeout real por passo e kill de **grupo** de
  processos (`process_group(0)` + `libc::kill(-pid, SIGKILL)` — a mesma
  lição da Fase 4d aplicada aqui, provada com `pgrep` confirmando ausência
  de processo órfão), parsers para `cargo test`/clippy JSON/ruff JSON
  construídos contra saída real capturada (não schema adivinhado),
  `forge.toml` configurável com fallback para `default_steps()`, e um
  teste golden que valida uma `VerificationEvidence` real (com findings
  preenchidos) contra `verification-evidence.v1.schema.json`, incluindo
  caso negativo.
- **Onda 2 — `forge verify` no CLI**: comando real (`--config/--out/
  --format`) que roda o pipeline, grava a evidência em
  `.forge/evidence/<run_id>.json` e sai com `process::exit(1)` em
  veredito `Fail` — o gate central que as ondas seguintes (e a Onda 6)
  consomem. 6 testes cross-process reais contra o binário compilado.
- **Onda 3 — auditor consome evidência real (ADR 0008)**: `SquadTask`
  ganha `verification_evidence_json` (campo proto aditivo); `forge squad`
  roda `/verify` antes de cada tarefa e anexa a evidência; `server.py`
  distingue explicitamente "ausente/inválida" de "válida" (a armadilha do
  default silencioso de proto3 que já mordeu em `Consensus.requires_human`
  na Fase 4c); o orquestrador reprova automaticamente **sem chamar o
  gateway** quando a evidência falta — fail-closed provado por contagem
  de chamadas ao LLM, não só por valor de saída.
- **Onda 4 — `forge_review` + gates + certificação**: quatro reviewers
  ponderam um `value_score`, mas `gates.evaluate` sobrepõe essa média com
  regras duras (finding crítico, veredito `Fail`, piso de segurança) que
  nenhuma média alta salva — provado com médias altas (~0.9) e uma
  condição de gate simultânea. `certification.certify` produz o artefato
  com o hash da evidência, reusando `canonical_json`/sha256 do
  `prompt-cache-key` (não uma segunda implementação de hash); o
  `LedgerStore` já registra esse payload livremente, cadeia íntegra.
  Desvio registrado: `.buildtovalue/review/orchestrator.py` (fonte de
  porte prevista) não estava disponível neste ambiente — os reviewers
  technical/security são código novo, documentado como tal, sem fabricar
  heurísticas de performance/value por falta de sinal determinístico.
- **Onda 5 — skill-vetter (ADR 0009)**: `forge-verify::vetter` aponta a
  mesma máquina de evidência para o diretório de uma skill (manifesto
  mínimo `skill.toml`), soma checagens de padrão de comando perigoso e de
  permissão declarada incoerente com sinais de uso no código, e decide
  `Vet`/`Block` de forma dura — qualquer finding crítico ou veredito
  `Fail` bloqueia, testado com uma skill boa e uma maliciosa (≥2
  findings). Fail-closed explícito: manifesto ausente/inválido bloqueia
  sempre. Decisão registrada: o vetter reimplementa a regra de gate da
  Onda 4 em Rust puro em vez de importar `forge_review` (Python), para
  não puxar uma dependência Python num crate que é motor determinístico
  puro.
- **Onda 6 — self-hosting no CI + fixtures golden + reconciliação (ADR
  0010)**: job `verify` novo em `.github/workflows/ci.yml` (separado do
  job `rust` de propósito — não arrisca o gate que já funciona) roda
  `forge verify` sobre o próprio workspace e anexa a evidência como
  artefato do run; o exit code do `forge verify` já é o gate — nenhum
  encanamento de status check adicional foi necessário. Provado
  localmente, não só declarado: `forge verify` sobre o workspace real deu
  `verdict: pass`/exit 0 (~32s); um teste quebrado propositalmente
  (inserido e revertido na mesma sessão) fez o veredito virar `fail` e o
  exit code virar 1. Fixtures golden novas para os 4 schemas que ainda
  não tinham (`handoff-event`, `ledger-entry`, `telemetry-event`,
  `prompt-template`), cada uma com caso inválido — `prompt-template`
  registrado honestamente como schema sem tipo Rust/Python associado
  ainda (só protegido contra drift de sintaxe, não paridade de tipo).

145 testes Rust + 135 Python, zero falhas, clippy/fmt limpos.
**Fase 5 concluída** — a cobrança de evidência que dependia de disciplina
manual (a verificação onda a onda desta própria conversa) passa a morar
estruturalmente no pipeline: `/verify` gera, o squad e o review a
consomem, o vetter a aplica a skills, e o CI a exige da própria
plataforma.

## Fase 6 — Ecossistema e escala, 9 ondas (2026-07-06)

A fase em que a plataforma passa a rodar **código que não é dela** — contido — e
a escalar. A ordem foi ditada por segurança (runtime → sandbox → terceiros, nunca
o inverso). PRs estratégicas, uma por onda, cada uma verde no CI antes do merge.

- **Onda 1 — runtime de skill:** `SkillTool` (`forge-tools`) implementa `Tool`; o
  `build_registry` (ponto único) veta e registra skills built-in de `<root>/skills/`.
  Uma skill `Block` nunca é registrada. Built-ins são vetados mesmo assim
  (dogfooding). (ADR 0011.)
- **Onda 2 — sandbox Docker real (bollard, Rust):** contêiner com limites (mount,
  rede off, timeout com kill de grupo, memória), imagem puxada se ausente, rodando
  como o uid dono do mount. Fail-closed para terceiro. Os quatro vetores de
  contenção são testes que **mordem**, `#[ignore]` local e rodados de verdade no
  job `sandbox` do CI (Docker real; guarda que falha sem daemon). (ADR 0011.)
- **Onda 3 — skill de terceiro ponta-a-ponta (critério nº 1):** `<root>/.forge/
  skills/` untrusted, vetado e registrado como `sandboxed`. Maliciosa bloqueada;
  vetada roda confinada. Tela `skills` vira read-only (badge do vetter +
  "re-vetar"); veredito ao ledger (`skill.vetting`). (ADR 0011.)
- **Onda 4 — cliente MCP (`rmcp`):** conecta a servidores externos, lista tools, as
  expõe namespaced (`mcp__<server>__<tool>`) sob o motor de permissões. Confiança
  no servidor declarado; cada chamada pede permissão. Fail-soft. Cross-process
  fixture real (`ECHO:mundo`). (ADR 0012.)
- **Onda 5 — cliente LSP:** framing JSON-RPC/`Content-Length` **hand-rolled** (só
  `serde_json`, zero-dep), sessão persistente preguiçosa. Expõe definição/
  referências/diagnósticos. Provado contra o **rust-analyzer REAL** no CI (job
  `sandbox`, componente instalada) por igualdade de posição (`lib.rs:0:7`), além do
  fixture hermético que sempre roda.
- **Onda 6 — RAG:** `recall_similar`, antes no-op (`_FallbackCollection` devolvia
  vazio), vira TF-IDF léxico local em Python puro (zero-dep, offline). Ground truth
  de dois tópicos disjuntos: recupera **exatamente** os relevantes. Honesto sobre
  ser léxico, não neural. (ADR 0013.)
- **Onda 7 — A/B testing (critério nº 2):** `forge experiment` agrega a telemetria
  por variante (`json_extract`), roda um teste z de duas proporções hand-rolled
  (sem crate de estatística) e emite veredito derivado dos dados: vencedor só com
  significância, senão "sem significância". Schema `experiment.v1` (Rust-only).
  Provado e2e (exp-sig → vencedor; exp-tie → sem significância). (ADR 0014.)
- **Onda 8 — bench + k6 + infra (critério nº 3):** benches criterion nos caminhos
  quentes (hash canônico, épocas de contexto, gateway) rodam no CI (job `bench`) e
  produzem baseline; um `ScriptedGenerator` sem key foi promovido a tipo público.
  O bin `loadgen` (`forge-server`) embrulha-o num endpoint HTTP; o **k6** martela e
  valida o **P95** (job `k6`: `p(95)<100` → medido ~3.5ms, 20 VUs, 107k requests,
  0% falha). `infra/` (terraform/ansible) é esqueleto honesto — produto local-first,
  sem alvo de deploy real ainda.
- **Onda 9 — fecho:** ADRs 0011–0014 formalizados; README × PLANO × CLAUDE.md
  reconciliados para "6 fases concluídas"; a pendência de exercício do
  consenso→ledger (Fase 4) re-declarada com um caminho de fechamento agora
  **determinístico** (e2e roteirizado sem key, viável pós-Onda 8).

194 testes Rust + 145 Python, zero falhas, clippy/fmt limpos. **Fase 6 concluída
— roadmap das 6 fases completo.** A plataforma que começou como um fork do
opencode agora se verifica com a própria ferramenta, contém código de terceiro
num sandbox real, enxerga o código por LSP, recupera memória por similaridade,
compara variantes com estatística honesta e valida a própria latência sob carga.
O que vier depois é produto novo, não plano antigo.

## Fase 7 — o navegador como forma primária de uso, 15 ondas (2026-07-06/07)

Primeiro produto pós-roadmap: o frontend (`web/`), 95% vitrine sobre 3 rotas GET,
liga cada tela a backend real. Emenda no meio do caminho reverteu um recorte
inicial que deixava o Grupo A do levantamento de design (`docs/
LEVANTAMENTO-UI-DESIGNER.md`) quase todo de fora — a versão final fecha 7/7.
PRs estratégicas, uma por onda, CI verde antes do merge.

- **Onda 1 — fundação web:** DTO de evento + SSE (`forge-cli::web_agent`), ponte
  de permissão real, guarda de `Origin`/`Host` (ADR 0015/0016).
- **Onda 2 — sessão e permissão:** timeout de permissão pendente fail-closed
  (ADR 0017); matriz de permissão persistida + trilha de auditoria (ADR 0018).
- **Onda 3 — sidecar Python como serviço de longa duração**, supervisionado,
  substitui o spawn-por-chamada (ADR 0019).
- **Onda 4 — squad ao vivo pelo navegador:** agentes mudando de estado em tempo
  real, gate HITL bloqueando até a UI resolver.
- **Onda 5 — biblioteca de prompts real** (CRUD + render), mesma rota que o chat
  REPL já usa.
- **Onda 6 — ledger real:** leitura paginada + filtro por ator.
- **Onda 7 — Console MCP (A1) + Uso por modelo (A5):** telas dedicadas (não
  cartão embutido), preview de política via `PermissionEngine::evaluate` real.
- **Onda 8 — Mapa de memória do squad + busca RAG (A3):** `MemoryService` novo,
  ponte Python↔Rust na direção certa (Rust chama, Python serve) — o
  `CoreService.Recall/Remember` que o protótipo citava era stub abandonado e
  direção errada. Recall léxico TF-IDF, rotulado honesto ("RAG" na nav, "léxico,
  não semântico" no rodapé). `forgetting.py` confirmado código morto — sem
  coluna de tendência de esquecimento fabricada. (ADR 0022.)
- **Onda 9 — Experimentos A/B (A2):** `experiment.v1` (já existia, Fase 6) ganha
  rota HTTP; banner explícito que a atribuição por telemetria ainda não roda em
  produção (dados semeados).
- **Onda 10 — Rate limits (A4) + Sandbox & skills de terceiro (A6) + Language
  servers (A7):** três telas admin pequenas. Achado de corretude: o getter de uso
  do `RateLimiter` não pode chamar `poll()` (muta, consumiria vaga real só de
  abrir a tela). Zero probe indevido no LSP — a mesma prova que já existia
  (registro sem subir processo) continua valendo.
- **Onda 11 — `/verify` real em background**, progresso via polling, 409 em
  execução concorrente.
- **Onda 12 — Providers:** reflete `Gateway::from_env` de verdade (ordem fixa de
  fallback anthropic→deepseek→openai); degrau de mutação descartado —
  `FallbackChain` é código morto, `Gateway::generate` nunca o consulta.
- **Onda 13 — Modelo & Onboarding:** `model`/`agent` (campos que existiam desde a
  Onda 1, nunca populados pelo frontend) passam a chegar de verdade à sessão —
  fronteira provada por comportamento observável (override de permissão muda o
  resultado conforme o agente enviado). Doctor agrega checagens reais
  (providers/uv/docker/git). Autonomia por tarefa (`max_autonomy_level`)
  deliberadamente NÃO wireada — ignorada ponta-a-ponta pelo orquestrador hoje,
  wire-la seria só "o campo viajou" sem efeito. (ADR 0021.)
- **Onda 14 — Designer salva honesto:** `squad.workflow.v1` novo (schema +
  fixture golden, padrão de `experiment.v1`) valida o grafo antes de gravar no
  ledger; grafo malformado nunca toca o ledger. Corrigiu os 2 lados da mentira
  do mock antigo (`seq` fabricado E a promessa de aplicação real).
- **Onda 15 — fecho:** `--web-agent` vira padrão (`--no-web-agent` para o modo
  só-leitura); varredura encontrou e removeu 2 resíduos mock que nenhuma onda
  anterior tinha coberto (toggle de política de ferramenta fake na tela Sessão,
  cabeçalho de sessão com provider/cache hardcoded) e um bug real só visível
  escrevendo a 1ª cobertura de browser da tela Sessão (`fetchJson` chamando
  `.json()` num corpo vazio de `202 Accepted`, fazendo toda mensagem enviada
  parecer falha mesmo quando o servidor respondia com sucesso). Grupo A e Grupo
  B do levantamento de design reconciliados como fechados.

**Fase 7 concluída.** O produto que só existia como CLI/TUI agora tem o
navegador como forma primária de uso, com prova executável (Playwright contra
o `forge dashboard` real) em vez de leitura de código — inclusive achando e
corrigindo um bug de produção que só a cobertura de browser revelava.
