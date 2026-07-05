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
