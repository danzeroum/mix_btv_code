# Forge — Plataforma Unificada (Rust + Python)

**mix_btv_code** é o repositório-sede da plataforma **Forge**: um CLI/TUI de coding
agent que unifica as ideias de três repositórios num único sistema construído por
design em Rust e Python.

| Origem | O que traz |
|---|---|
| [opencode](https://github.com/danzeroum/opencode) | Runtime de sessão durável, agentes selecionáveis, permissões, ferramentas, TUI, **ModelTier** e **verificação determinística** (fork) |
| [prompte](https://github.com/danzeroum/prompte) | Geradores de prompt, knowledge base, quality linter, cache por hash, gateway LLM com fallback, telemetria |
| [BuildToValue](https://github.com/danzeroum/BuildToValue_AI_Agent_Specialization) | Squad multi-agente: orquestração, consenso ponderado, planejamento, HITL, fallback progressivo, ledger, review por valor |

- **Plano completo** (arquitetura, mapeamento 100%, roadmap 6 fases): [`docs/PLANO-PLATAFORMA-FORGE.md`](docs/PLANO-PLATAFORMA-FORGE.md)
- **Roadmap visual interativo**: [`docs/roadmap-forge.html`](docs/roadmap-forge.html) (autocontido — abra no navegador)
- **Decisão arquitetural central**: [`docs/adr/0001-arquitetura-rust-python-grpc.md`](docs/adr/0001-arquitetura-rust-python-grpc.md)
- **Histórico de decisões da junção**: [`docs/DECISOES.md`](docs/DECISOES.md)

## Layout

- `crates/` — núcleo Rust: `forge-cli` (binário `forge`), `forge-core`
  (sessões/permissões), `forge-llm` (gateway + ModelTier), `forge-tools`,
  `forge-verify`, `forge-store` (SQLite/ledger), `forge-schemas`,
  `forge-tui`/`forge-server`/`forge-proto` (fases 2–3).
- `python/` — sidecar de orquestração (uv workspace): `forge-squad`
  (consenso/planejamento/HITL), `forge-promptforge` (geradores/linter/hash),
  `forge-review`, `forge-eval`, `forge-proto-py`.
- `schemas/` — fonte única de contratos: protos gRPC, JSON Schemas versionados
  (`*.v1.schema.json`) e fixtures de paridade cross-language.

## Desenvolvimento

```sh
just test      # cargo test + pytest
just lint      # clippy + rustfmt
just verify    # test + lint (evidência JSON completa na Fase 5)
```

Sem `just`: `cargo test --workspace` e `cd python && uv sync && uv run pytest`.

## Estado

**Fase 1 concluída**: `forge run` (tarefa única) e `forge chat` (REPL
multi-turno) executam o loop de agente real — gateway LLM com streaming SSE
(Anthropic/OpenAI/DeepSeek, fallback automático, keys por env), cache de
prompts por hash (`prompt-cache-key.v1`, desative com `--no-cache`),
ferramentas read/grep/edit/bash sob permissão interativa e cada turno
registrado no ledger append-only (`.forge/forge.db`).

```sh
export ANTHROPIC_API_KEY=...   # ou DEEPSEEK_API_KEY / OPENAI_API_KEY
cargo run -p forge-cli -- run "corrija o teste X" --model claude-sonnet-5
cargo run -p forge-cli -- chat
```

**Fase 2 concluída** — sessões duráveis, Context Epochs + compaction
tier-gated, TUI ratatui completa (diff colorido, seletor de modelo/agente)
e Managed Tool Output Files.

**Fase 3 concluída** — ativação real do gRPC: o sidecar Python
`forge_promptforge` expõe `PromptForgeService` (Lint/Render/ListGenerators)
sobre Unix Domain Socket; `forge-sidecar` sobe e supervisiona o processo
(`SidecarSupervisor`) e fala com ele (`SidecarClient`), com **degradação
graciosa total** — sem `uv`/workspace Python, `run`/`chat`/`tui` seguem
funcionando normalmente, só sem lint/geradores. `forge chat` ganhou
`/prompt` (lista e renderiza geradores, `save`/`library`/`use`/`fav`/`rm`
para a biblioteca de prompts) e um aviso consultivo de lint por turno.
Rate limiting tier-gated (`forge-llm::RateLimiter`) e telemetria
offline-first (`forge-store::Telemetry`) decoram o gateway; `forge
dashboard` sobe um painel local (axum) sobre `.forge/telemetry.db`.

```sh
export ANTHROPIC_API_KEY=...
cargo run -p forge-cli -- tui --model claude-sonnet-5   # sidecar sobe sozinho se `uv` estiver disponível
cargo run -p forge-cli -- dashboard                     # painel de telemetria em http://127.0.0.1:7878
```

**Fase 4 concluída** — squad multi-agente como motor, com o **gRPC
bidirecional** ativado de ponta a ponta (ADRs 0004–0007). O sidecar Python
`forge_squad` roda o `UnifiedOrchestrator` (5 agentes reais —
architect/developer/auditor/designer/ops — consenso ponderado, planejamento
adaptativo, HITL/autonomia progressiva) e streama `SquadEvent`s ao vivo
(propostas → consenso → handoffs → steps); os agentes obtêm LLM e decisões
de permissão de volta do Rust via `CoreService` (as API keys ficam só no
Rust — o Python só conhece o UDS). `forge squad "..."` degrada em **3
níveis** se o squad falhar: squad → agente-único → safe-mode read-only. O
laço inteiro é coberto por um teste cross-process real (Rust ⇄ Python) e um
teste de `kill -9` que prova o fallback.

```sh
export ANTHROPIC_API_KEY=...
cargo run -p forge-cli -- squad "adicione paginação ao endpoint de pedidos"
```

Princípio "Nada Fake" mantido a cada onda: onde a origem escondia
fabricação atrás de um default (`create_plan` com constantes, o veredito
do auditor por fórmula, o evaluator com `technical_score` fixo, a aprovação
HITL sempre `true`), a versão portada deriva tudo de raciocínio real do
modelo, com fallback honesto quando o parsing falha.

**Fase 5 concluída** — verificação, review e governança, em 6 ondas
(ADRs 0008–0010). `/verify` (`crates/forge-verify`) roda um pipeline
configurável (`forge.toml`) de passos com timeout e kill de grupo de
processos, parsers reais para `cargo test`/clippy/ruff, e produz
`verification-evidence.v1`; `forge verify` grava a evidência em disco e
sai com código ≠0 em veredito `Fail` — o gate que o CI (job `verify`,
Onda 6) e o squad (Onda 3) consomem. O squad passa a rodar `/verify`
antes de cada tarefa e anexa a evidência ao `SquadTask`
(`verification_evidence_json`, ADR 0008); o auditor Python julga sobre
ela e reprova automaticamente — sem chamar o LLM — quando a evidência
está ausente ou inválida (fail-closed, provado por contagem de chamadas
ao gateway). `forge_review` (Python) pondera quatro reviewers num
`value_score`, mas `gates.evaluate` sobrepõe essa média com regras duras
— finding crítico, veredito `Fail`, piso de segurança — que nenhuma média
alta "salva"; `certification.certify` produz o artefato com o hash da
evidência (mesmo `canonical_json`/sha256 do `prompt-cache-key`), que o
ledger já registra livremente. O skill-vetter (`forge-verify::vetter`,
ADR 0009) aplica a mesma máquina de evidência ao diretório de uma skill e
decide `Vet`/`Block` de forma dura e fail-closed. A fase fecha com o
self-hosting literal: um job de CI roda `forge verify` sobre o próprio
workspace e falha o build no veredito `Fail` (provado com um teste
quebrado propositalmente e revertido) — a cobrança de evidência que era
manual nesta fase passa a morar no pipeline.

Próximo marco: Fase 6 (LSP/MCP, plugins de terceiros com sandbox, RAG,
A/B testing, k6).
