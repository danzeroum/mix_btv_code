# Forge (mix_btv_code)

CLI/TUI de coding agent unificando opencode + prompte + BuildToValue.
Núcleo **Rust** (workspace cargo em `crates/`) + sidecar **Python** (workspace uv em
`python/`), integrados por gRPC sobre Unix Domain Socket (ativação na Fase 3).

## Comandos

```sh
cargo test --workspace                 # testes Rust (inclui paridade de hash)
cargo clippy --workspace -- -D warnings
cargo fmt --all --check
cd python && uv sync && uv run pytest  # testes Python
just test | just lint | just verify    # atalhos (requer just)
```

## Regra de fronteira (ADR 0001 — docs/adr/)

- **Rust**: tudo que toca disco/rede/processo/segredo ou roda a cada keystroke
  (CLI/TUI, sessões, gateway LLM, ferramentas, permissões, verify, storage).
  API keys existem SÓ no processo Rust.
- **Python**: tudo que decide o próximo passo por raciocínio de agente
  (squad, PromptForge, review, eval). Python NUNCA chama provedores LLM
  diretamente — sempre via `CoreService.Generate` (gRPC).
- Sem PyO3 no caminho principal (tokio×asyncio); sidecar supervisado com
  fallback progressivo: squad → agente-único → safe-mode read-only.

## Regras de contrato

- Fonte única em `schemas/` (protos gRPC + `*.v1.schema.json` + fixtures).
- Mudança breaking = novo arquivo `.v2` + ADR novo; protos evoluem só aditivamente.
- O hash de cache de prompt (`prompt-cache-key.v1`) tem implementação dupla:
  `crates/forge-schemas/src/canonical.rs` (Rust) × 
  `python/packages/forge-promptforge/src/forge_promptforge/hashing.py` (Python).
  Qualquer mudança exige regenerar `schemas/fixtures/` (`scripts/gen_fixtures.py`)
  e os testes de paridade dos DOIS lados devem passar.
- Ledger é append-only com hash-chain (`crates/forge-store/src/ledger.rs`) —
  nunca UPDATE/DELETE; overrides são novas entradas marcadas.

## Roadmap e estado

Plano completo em `docs/PLANO-PLATAFORMA-FORGE.md` (6 fases). Estado atual:
**Fase 5 concluída** (ver histórico completo em `docs/DECISOES.md`;
próximo marco: Fase 6 — LSP/MCP, plugins de terceiros com sandbox, RAG,
A/B testing, k6).

**Fase 5 — verificação, review e governança (ADRs 0008–0010), 6 ondas:**
`/verify` (`crates/forge-verify`) roda um pipeline configurável
(timeout + kill de *grupo* de processos, parsers reais para
cargo test/clippy/ruff) e produz `verification-evidence.v1`; `forge
verify` grava a evidência em disco e sai com código ≠0 em veredito
`Fail`. O squad roda `/verify` antes de cada tarefa e anexa a evidência
ao `SquadTask` (ADR 0008); o auditor Python julga sobre ela e
reprova automaticamente, sem chamar o gateway, quando ausente/inválida
(fail-closed). `forge_review` pondera quatro reviewers, mas
`gates.evaluate` sobrepõe a média com regras duras (finding crítico,
veredito fail, piso de segurança) que nenhuma média alta salva;
`certification.certify` produz o artefato com hash de evidência,
registrável no ledger. O skill-vetter (`forge-verify::vetter`, ADR 0009)
aplica a mesma máquina a uma skill e decide `Vet`/`Block` de forma dura
e fail-closed. A fase fecha com self-hosting real (ADR 0010): o job
`verify` do CI roda `forge verify` sobre o próprio workspace e falha o
build em veredito `Fail`.

**Fase 4 — squad multi-agente + gRPC bidirecional (ADRs 0004–0007):** o
sidecar Python `forge_squad` roda o `UnifiedOrchestrator` (5 agentes
reais, consenso ponderado, planejamento adaptativo, HITL) e expõe
`SquadService.ExecuteTask → stream SquadEvent`; os agentes chamam de volta
o `CoreService` Rust (`Generate`/`RequestPermission`) — keys só no Rust, o
Python só conhece o UDS. `forge squad` renderiza os eventos ao vivo,
registra o consenso no ledger e degrada em 3 níveis (squad → agente-único
→ safe-mode). Regra "Nada Fake" aplicada onda a onda: onde a origem
escondia fabricação atrás de um default, o porte deriva tudo de raciocínio
real (ver ADR 0005/0007). Dois achados de interop registrados: o
`grpc.default_authority` (grpc-python↔tonic sobre UDS) e o `process_group`
kill (`uv run` orfanava o Python). Provado por testes cross-process reais
(`squad_e2e.rs` + o `kill -9` do fallback).

Fase 3 (ADR 0003): ativação real do gRPC — contrato em
`schemas/proto/promptforge.proto`; `forge-proto/build.rs` compila via
tonic-build com protoc vendorizado (`protoc-bin-vendored`, sem exigir
protoc de sistema); `scripts/gen_proto_py.py` gera os stubs Python
(grpcio-tools, não betterproto — mais maduro). `forge-sidecar`:
`SidecarSupervisor::spawn`+`wait_ready` sobe `uv run python -m
forge_promptforge.server` e espera o health check; `SidecarClient` fala
`PromptForgeService` sobre UDS. `forge-cli/src/sidecar.rs::try_start()`
degrada para `None` sem quebrar `run`/`chat`/`tui` se o sidecar não
estiver disponível. Servidor real em
`python/packages/forge-promptforge/src/forge_promptforge/server.py`.
Testes em duas camadas: mock Rust (`client_over_uds.rs`, sempre roda) e
processo Python real (`python_sidecar.rs`, pula graciosamente sem
`uv`/workspace — o CI instala `uv` para exercitar o caminho real).

Fechamento da Fase 3: `forge-llm::RateLimiter` (sliding window tier-gated,
decorator `RateLimitedGenerator` em `forge-cli/src/rate_limit_gen.rs`,
composto por baixo do `CachedGenerator` — hit de cache nunca consome vaga);
`forge-store::{Telemetry, PromptLibrary}` (offline-first, SQLite local,
falhas nunca derrubam o caminho principal); `/prompt
save|library|use|fav|rm` no chat (biblioteca de prompts); `forge-server`
(axum) + comando `forge dashboard` servindo `.forge/telemetry.db` num
painel local em `127.0.0.1`.

## Convenções

- Código e comentários em português (padrão do projeto); identificadores em inglês.
- Testes unitários junto do módulo (Rust `#[cfg(test)]`; Python `tests/` por pacote).
- CI em `.github/workflows/ci.yml`: cargo test/clippy/fmt + pytest + gitleaks
  (bloqueante) + cargo-deny + job `verify` (self-hosting: `forge verify` sobre
  o próprio workspace, Fase 5 Onda 6).
