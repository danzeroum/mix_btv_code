# Plano: Plataforma Unificada "Forge" — CLI/TUI de Coding Agent em Rust + Python

> **Repositório-sede da implementação**: `danzeroum/mix_btv_code` (este repositório,
> workspace na raiz). Cópias históricas deste documento existem nos repositórios de
> origem `danzeroum/opencode` (mergeada na `dev` via PR #196), `danzeroum/prompte` e
> `danzeroum/BuildToValue_AI_Agent_Specialization`, cujas ideias a plataforma unifica.

## Contexto

Três repositórios com ideias complementares são unificados num sistema
construído por design em **Python e Rust**:

1. **opencode** (fork TypeScript) — coding agent de terminal: sessões duráveis
   (System Context/Epochs/compaction), agentes selecionáveis, permissões,
   ferramentas (grep/edit/bash/LSP/MCP), TUI. O fork adiciona **ModelTier**
   (comportamento tier-gated para modelos small/medium/large) e o **pipeline de
   verificação determinística** ("o LLM orquestra; ferramentas determinísticas
   verificam").
2. **prompte** (JS/Node) — engenharia de prompts: geradores declarativos, base
   de conhecimento aditiva, quality linter ("ESLint de prompts"), cache por
   hash sha256, rate limiting, proxy LLM com fallback, biblioteca de prompts,
   telemetria offline-first.
3. **BuildToValue** (Python) — squad multi-agente: UnifiedOrchestrator,
   consenso ponderado, planejamento adaptativo, LearningRouter, memória,
   HITL/autonomia progressiva, fallback em 3 níveis, ledger append-only,
   review orientado a ROI, quality gates.

**Produto final**: CLI/TUI de coding agent (`forge`) cujo motor é o squad
multi-agente, com a camada de prompts/qualidade do prompte. **Escopo**:
roadmap completo cobrindo 100% das ideias, em fases longas.

## Arquitetura

### Divisão de linguagens (regra de fronteira)

- **Rust**: tudo que toca disco/rede/processo/segredo ou roda a cada
  keystroke — CLI/TUI, runtime de sessão, gateway LLM (keys de API só aqui),
  ferramentas determinísticas, permissões, pipeline `/verify`, skill-vetter,
  storage SQLite/ledger/telemetria.
- **Python**: tudo que decide "o que fazer a seguir" por raciocínio de
  agente — squad multi-agente completo, PromptForge
  (geradores/knowledge/linter/glossário), review system, avaliação/RAG.

### Integração Rust↔Python: gRPC bidirecional sobre Unix Domain Socket

- `tonic`/`prost` (Rust) × `betterproto`/`grpclib` (Python). Sem PyO3 no
  caminho principal (evita conflito tokio×asyncio; isolamento de falhas:
  crash do sidecar Python aciona o fallback progressivo naturalmente).
- O binário `forge` spawna e supervisiona o sidecar `forge-squadd`; sem
  sidecar → degrada para agente-único → safe-mode read-only.
- Direções: Rust→Python `SquadService.ExecuteTask(SquadTask) → stream
  SquadEvent`; Python→Rust `CoreService`: `Generate` (LLM), `RunTool`,
  `AppendLedger`, `Recall/Remember`, `RequestPermission`. **Python nunca
  chama provedores LLM diretamente** — tudo via gateway Rust.

### Estrutura do workspace (raiz do repositório)

```
mix_btv_code/
├── Cargo.toml                  # cargo workspace
├── justfile                    # just build/test/verify/gen-proto
├── crates/
│   ├── forge-cli/              # bin `forge` (clap), spawn/supervisão do sidecar
│   ├── forge-tui/              # ratatui: chat, diff, permissões, painel do squad
│   ├── forge-core/             # sessões (System Context/Epochs/compaction), agentes
│   │                           #   build/plan/general, permissões, event bus
│   ├── forge-llm/              # gateway: providers (Anthropic/OpenAI/DeepSeek),
│   │                           #   streaming, fallback, ModelTier, prompt caching,
│   │                           #   cache por hash, rate limiting
│   ├── forge-tools/            # grep (crates grep/ignore), edit/patch, bash/PTY,
│   │                           #   webfetch, LSP, MCP (rmcp), sandbox (bollard)
│   ├── forge-verify/           # pipeline /verify + skill-vetter + evidência JSON
│   ├── forge-store/            # rusqlite: sessões, ledger hash-chain, biblioteca
│   │                           #   de prompts, memória do squad, telemetria
│   ├── forge-schemas/          # tipos serde + schemars + canonicalização
│   ├── forge-proto/            # tonic-build sobre ../schemas/proto
│   └── forge-server/           # axum: API local + dashboard de métricas
├── python/  (uv workspace)
│   └── packages/
│       ├── forge-proto-py/     # stubs gerados — nunca editados à mão
│       ├── forge-squad/        # orquestrador, agentes, consenso, planning,
│       │                       #   routing, memory, hitl, fallback, parallel
│       ├── forge-promptforge/  # geradores, knowledge base, quality linter, glossário
│       ├── forge-review/       # 4 reviewers, value_score, quality gates, certificação
│       └── forge-eval/         # avaliação contínua, A/B, RAG tools
├── schemas/
│   ├── proto/                  # core.proto, squad.proto, llm.proto (fonte única)
│   ├── json/                   # *.v1.schema.json (evidência, ledger, handoff,
│   │                           #   cache-key, telemetria, prompt-template)
│   └── fixtures/               # golden files p/ testes de contrato cross-language
├── skills/                     # skills built-in + padrão de autoria
├── docs/adr/                   # ADRs (governança BuildToValue)
├── infra/                      # docker-compose, terraform, ansible, k6 (adaptados)
└── .github/workflows/          # cargo test, pytest, contrato, gitleaks, semgrep, buf
```

## Mapeamento ideia → componente (cobertura 100%)

| Origem | Ideia | Destino | Fase |
|---|---|---|---|
| opencode | Permissões por ferramenta/escopo | `forge-core::permission` | 1 |
| opencode | Providers múltiplos + catálogo + troca em sessão | `forge-llm::catalog` | 1–2 |
| opencode | grep, edit/patch, bash, webfetch | `forge-tools` | 1 |
| opencode | Sessões duráveis (System Context, Epochs, compaction) | `forge-core::context` (spec: `CONTEXT.md`) | 2 |
| opencode | Agentes build/plan/general | `forge-core::agent` | 2 |
| opencode | TUI rica | `forge-tui` | 2 |
| opencode | AGENTS.md, prompt caching, truncamento gerenciado | `forge-core` + `forge-llm` | 2 |
| fork | **ModelTier** tier-gated (prompt enxuto, menos tools, compaction ~75%, step-discipline) | `forge-llm::model_tier` (porta de `model-tier.ts`) | 2 |
| prompte | 25 geradores declarativos, knowledge base, quality linter, glossário | `forge_promptforge` | 3 |
| prompte | Cache por hash (canônico + sha256, paridade Rust×Python) | `forge-llm::prompt_cache` + `forge_promptforge.hashing` | 3 |
| prompte | Rate limiting por tier, proxy seguro com fallback | `forge-llm` | 1/3 |
| prompte | Biblioteca de prompts, histórico, telemetria + dashboard | `forge-store` + `forge-server` | 3 |
| BTV | Squad (Architect/Dev/Auditor/Designer/Ops/Supervisor/Exploration/Recovery)¹ | `forge_squad.agents` | 4 |
| BTV | UnifiedOrchestrator, consenso ponderado, planning, routing, memória, paralelo | `forge_squad.*` (migração de `src/`) | 4 |
| BTV | HITL/autonomia progressiva, fallback 3 níveis, handoff events | `forge_squad.hitl/.fallback` + `squad.proto` | 4 |
| fork | Pipeline `/verify` + evidência JSON + skill-vetter | `forge-verify` | 5 |
| BTV | Review (4 reviewers, value_score >0.7), quality gates, certificação | `forge_review` | 5 |
| BTV | Governança: ADRs, ledger hash-chain, overrides, "Nada Fake" | `forge-store::ledger` + `docs/adr/` | 1/5 |
| fork | CI segurança (gitleaks bloqueante, semgrep), commit trailers, bench | `.github/workflows/` + criterion | 5–6 |
| opencode | LSP, MCP, plugins/skills de terceiros | `forge-tools` + `skills/` | 6 |
| BTV | Sandbox Docker, RAG, avaliação contínua/A-B, infra (terraform/ansible/k6/grafana) | `forge-tools::sandbox`, `forge_eval`, `infra/` | 6 |

¹ Cobertura de ideias do repositório de origem, não o conjunto que o
`UnifiedOrchestrator` de fato instancia — são 5 agentes reais (architect/
developer/auditor/designer/ops); Supervisor/Exploration/Recovery existem
como arquivos separados mas sem chamador real (ver ADR 0004).

## Roadmap (fases longas)

- **Fase 1 — Fundação executável (~6–8 sem)**: workspaces cargo+uv, schemas
  iniciais, `forge-llm` (3 providers, streaming, fallback), `forge-tools`
  básico, loop de agente único com permissões ask/allow/deny, sessões+ledger
  no SQLite, `forge run`/`forge chat`. *Critério: editar código num repo real
  com permissão interativa; ledger registra; `just test` verde.*
- **Fase 2 — Sessões duráveis + TUI + ModelTier (~8–10 sem)**: System Context
  completo (Epochs, compaction em fronteiras seguras, baseline p/ prompt
  caching), agentes build/plan/general, ModelTier tier-gated, AGENTS.md, TUI
  ratatui completa. *Critério: sessão sobrevive a restart; compaction sem
  quebrar cache; snapshot tests da TUI (insta + TestBackend).*
- **Fase 3 — PromptForge + gateway completo (~6–8 sem)**: geradores/knowledge/
  linter/glossário no sidecar (primeira ativação do gRPC), cache por hash com
  fixtures de paridade, rate limiting, biblioteca de prompts, telemetria +
  dashboard. *Critério: hash idêntico Rust×Python nas fixtures; degradação
  graciosa sem sidecar.*
- **Fase 4 — Squad multi-agente como motor (~10–12 sem)**: migração do
  UnifiedOrchestrator/consenso/planning/routing/memória/paralelo, agentes com
  LLM real via gateway, HITL na TUI, fallback 3 níveis (incl. supervisão do
  sidecar), `forge squad "..."` + painel ao vivo. *Critério: tarefa
  multi-arquivo com consenso no ledger; kill -9 do sidecar aciona fallback;
  e2e cobre handoff.start/ack/complete/error.*
- **Fase 5 — Verificação, review e governança (~6–8 sem)**: `/verify` com
  evidência `verification-evidence.v1`, auditor consome evidência real,
  skill-vetter, `forge_review` + quality gates + certificação, CI de
  segurança, ADRs. *Critério: a plataforma passa no próprio `/verify`
  (self-hosting); PR sem evidência bloqueado.*
- **Fase 6 — Ecossistema e escala (~8–10 sem)**: LSP/MCP completos, plugins de
  terceiros vetados, sandbox Docker, RAG, A/B testing via telemetria, bench
  criterion, `infra/` completa + k6. *Critério: skill de terceiro roda após
  vetting; A/B gera relatório; k6 valida P95 do gateway.*

## Contratos-chave

- **Protobuf** (`schemas/proto/`) para o wire gRPC; geração via
  `just gen-proto`; `buf breaking` no CI.
- **JSON Schema** (`schemas/json/*.v1.schema.json`) para documentos
  persistidos/auditáveis: `verification-evidence.v1`, `handoff-event.v1`,
  `ledger-entry.v1` (hash-chain, campos `override` e `fake_marker` de 1ª
  classe), `prompt-cache-key.v1` (JSON canônico de chaves ordenadas + sha256),
  `telemetry-event.v1`, `prompt-template.v1`.
- Rust: `schemars` deriva schema dos tipos; Python: pydantic. Golden fixtures
  round-trip nos dois lados no CI. Breaking → novo `.v2` + ADR.

## Reuso do código existente

- **Migrar** (BuildToValue `src/` → `python/packages/forge-squad/`):
  `consensus/weighted_voting.py` (✅ migrado no scaffold),
  `hitl/progressive_autonomy.py`, `planning/`, `routing/learning_router.py`,
  `memory/agent_memory.py`, `parallel/resource_manager.py`, `safety/`,
  estrutura do `orchestration/unified_orchestrator.py`;
  `.buildtovalue/review/orchestrator.py` → `forge_review` (quase direta).
- **Reescrever**: `src/agents/*.py` (hoje stubs heurísticos — "Nada Fake":
  agentes reais chamam LLM via gateway; mantêm-se as interfaces);
  `secure_executor.py` → `forge-tools::sandbox` em Rust.
- **Especificação de referência** (portar ideias, não código):
  `opencode/CONTEXT.md` (spec do `forge-core::context`),
  `opencode/packages/opencode/src/provider/model-tier.ts` (✅ portado),
  `opencode/script/verify.ts` + `docs/adr/0001`, `prompte/api/src/hash.js`
  (✅ portado com paridade testada), `prompte/api/src/llm.js` (fallback),
  geradores/linter do frontend do prompte.

## Verificação

1. **Unit**: `cargo test` + `insta` (snapshots TUI/prompts); `pytest` +
   `hypothesis` (consenso/planner); `clippy -D warnings`, `ruff`,
   `mypy --strict`.
2. **Contrato cross-language**: golden fixtures round-trip; paridade de hash;
   `buf breaking`.
3. **Integração Rust↔Python**: sidecar real via UDS efêmero; injeção de falha
   (kill -9) valida fallback.
4. **LLM sem custo**: gravação/replay de chamadas (modo cassette, inspirado no
   `http-recorder` do opencode); job noturno opcional contra APIs reais.
5. **E2E**: `expectrl`/PTY dirigindo o binário em repos-fixture;
   `ratatui::TestBackend`.
6. **Self-hosting** (Fase 5+): `forge verify` roda no próprio workspace;
   evidência JSON obrigatória em PR.

## Riscos principais

| Risco | Mitigação |
|---|---|
| Complexidade do runtime de sessão subestimada | `CONTEXT.md` como spec formal; invariantes como tipos + property tests; marcos internos na Fase 2 |
| Drift de contrato Rust×Python | Fonte única em `schemas/`, geração de código, fixtures no CI, `buf breaking` |
| tokio×asyncio no mesmo processo | Resolvido por arquitetura (sidecar gRPC, nunca embedding) |
| Sidecar Python cai | Fallback progressivo de 1ª classe: agente-único → safe-mode read-only |
| Escopo 100% = nunca entregar | Cada fase termina em software usável; Fase 1 já é um coding agent funcional |
| Keys de API vazarem | Keys só no processo Rust; Python conhece só o UDS (princípio do proxy do prompte) |
| Segurança de bash/skills de terceiros | Permissões no core Rust (não contornáveis), skill-vetter bloqueante, sandbox Docker, gitleaks |

## Estado atual (Fases 1–5 concluídas; Fase 6 não iniciada)

Histórico completo, decisão a decisão, em `docs/DECISOES.md`. Resumo do que já
compila e está testado no workspace (raiz deste repositório): 145 testes Rust +
135 Python, clippy `-D warnings` e rustfmt limpos.

- **Fase 1 — fundação executável**: gateway LLM real com streaming SSE
  (Anthropic/OpenAI/DeepSeek, fallback automático), cache de prompt por hash
  (`prompt-cache-key.v1`), ferramentas read/grep/edit/bash sob permissões,
  loop de agente genérico (`forge run`/`forge chat`), ledger hash-chain.
- **Fase 2 — sessões e TUI**: `EventStore` + `DurableSession` (retomada por
  `--session`), Context Epochs + compaction tier-gated em fronteiras
  seguras, TUI ratatui (transcript, diff colorido, modal de permissão,
  seletor de modelo/agente), Managed Tool Output Files.
- **Fase 3 — PromptForge + gateway completo**: primeira ativação real do
  gRPC (`PromptForgeService` sobre Unix Domain Socket, sidecar supervisado
  com degradação graciosa total), rate limiting tier-gated
  (`forge-llm::RateLimiter`), telemetria offline-first
  (`forge-store::Telemetry`), biblioteca de prompts (`/prompt
  save|library|use|fav|rm`), dashboard de métricas (`forge-server` + `forge
  dashboard`).
- **Fase 4 — squad multi-agente como motor (ADRs 0004–0007)**: os 4 protos
  gRPC (`core`/`squad`/`llm`/`promptforge`) ativados nos dois lados; o
  sidecar Python `forge_squad` roda o `UnifiedOrchestrator` (5 agentes
  reais via gateway, consenso ponderado, `AdaptivePlanner`, `LearningRouter`,
  `AgentMemorySystem`, `ProgressiveAutonomyManager`, `ContinuousEvaluator`)
  e streama `SquadEvent`s; `CoreService` (Rust) atende os callbacks
  `Generate`/`RequestPermission` (keys só no Rust). `forge squad` renderiza
  ao vivo, grava o consenso no ledger e degrada em 3 níveis (squad →
  agente-único → safe-mode). Provado por testes cross-process reais
  (`squad_e2e.rs` + `kill -9`).
- **Fase 5 — verificação, review e governança (ADRs 0008–0010), 6 ondas**:
  `/verify` (`crates/forge-verify`) roda um pipeline configurável
  (`forge.toml`, com fallback para `default_steps()` que espelha o CI) de
  passos com timeout e kill de *grupo* de processos (não só do processo
  direto — testado com `pgrep` provando ausência de órfão), parsers reais
  para `cargo test`/clippy/ruff (construídos contra saída real capturada,
  não schema adivinhado), produzindo `verification-evidence.v1` validado
  contra o JSON Schema com um caso negativo (documento quebrado precisa
  reprovar). `forge verify` (CLI) grava a evidência em disco e sai com
  código ≠0 em veredito `Fail` — o gate central da fase. O squad
  (`forge squad`) roda `/verify` antes de cada tarefa e anexa a evidência
  ao `SquadTask.verification_evidence_json` (ADR 0008); o auditor Python
  julga sobre ela e reprova automaticamente, **sem chamar o gateway
  LLM**, quando a evidência está ausente/inválida — fail-closed provado
  por contagem de chamadas, não só por valor de saída. `forge_review`
  (Python) pondera quatro reviewers num `value_score`, mas
  `gates.evaluate` sobrepõe essa média com regras duras — finding
  crítico, veredito `Fail`, piso de segurança — que nenhuma média alta
  "salva" (provado com médias altas + condição de gate simultâneas);
  `certification.certify` produz o artefato com o hash da evidência
  (reusa `canonical_json`/sha256 do `prompt-cache-key`, não uma segunda
  implementação), que o `LedgerStore` já registra livremente, cadeia
  íntegra. O skill-vetter (`forge-verify::vetter`, ADR 0009) aplica a
  mesma máquina de evidência ao diretório de uma skill (manifesto
  `skill.toml`), soma checagens de padrão perigoso e de permissão
  declarada incoerente com o uso, e decide `Vet`/`Block` de forma dura e
  fail-closed (manifesto ausente/inválido nunca "vet por default"). A
  fase fecha com o self-hosting literal (ADR 0010): job `verify` no CI
  roda `forge verify` sobre o próprio workspace e falha o build no
  veredito `Fail` — provado localmente com um teste quebrado
  propositalmente (revertido em seguida) que fez o exit code virar 1.
  Fixtures golden novas para os schemas que ainda não tinham
  (`handoff-event`, `ledger-entry`, `telemetry-event`, `prompt-template`),
  cada uma com um caso inválido para não passar por schema permissivo
  demais.
- **Contratos**: 4 protos gRPC ativos, 6 JSON Schemas versionados e
  fixtures golden validadas — `prompt-cache-key` e `verification-evidence`
  com paridade cross-language real (Rust × Python); os demais com
  round-trip schema↔tipo Rust (não há consumidor Python desses eventos
  hoje); `prompt-template` sem tipo em nenhum lado ainda (só o schema é
  guardado contra drift de sintaxe) — registrado honestamente, não
  escondido atrás de "fixtures para todos os schemas".
- **"Nada Fake" aplicado onda a onda**: a inspeção do BuildToValue
  encontrou fabricação escondida atrás de defaults em cada camada
  (`create_plan`/`_decompose_task` com constantes, o veredito do auditor
  por fórmula de pontos, o `ContinuousEvaluator` com `technical_score`
  fixo 0.8, a aprovação HITL sempre `true`, o `_execute_action` morto) —
  todas corrigidas para derivar de raciocínio real, com fallback honesto
  (ADRs 0005/0006/0007). Lineage superada descartada por leitura direta
  do código (`AgentOrchestrator`/`SafeAgentBase`/`SquadOrchestrator`/
  `continuous_eval.py`/`adaptive_replanner.py`/`hierarchical_planner.py`,
  ADR 0004).
- **Operação**: justfile, CI GitHub Actions (cargo/pytest/gitleaks/
  cargo-deny bloqueantes + job `verify` de self-hosting, Onda 6),
  ADRs 0001–0010, script de regeneração de fixtures.

**Próximo marco: Fase 6 — ecossistema e escala.** LSP/MCP, plugins de
terceiros com sandbox Docker, RAG, A/B testing, testes de carga k6. O
skill-vetter (Fase 5 Onda 5) já existe como mecanismo — a Fase 6 é quem
constrói o runtime de carregar/executar skills de terceiros por cima
dele.

**Pendência de exercício da Fase 4** (código pronto e testado por unit;
falta só rodar com API real): o registro do consenso no ledger
(`squad.consensus` em `.forge/forge.db`, via `forge-cli/src/squad.rs`)
foi coberto por unit test do orquestrador e está wireado, mas o smoke de
`forge squad` com key inválida falhou antes do consenso. Fechar na
primeira rodada de `forge squad` com API key válida — confirmar que o
evento `squad.consensus` aparece no ledger com `decision_maker`/`strength`/
`requires_human` reais. Não é lacuna de código, é lacuna de exercício.
