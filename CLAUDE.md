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
**Fase 3 concluída** (ver histórico completo em `docs/DECISOES.md`).
Ativação real do gRPC (ADR 0003): contrato em
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
painel local em `127.0.0.1`. Próximo marco: Fase 4 (squad multi-agente).

## Convenções

- Código e comentários em português (padrão do projeto); identificadores em inglês.
- Testes unitários junto do módulo (Rust `#[cfg(test)]`; Python `tests/` por pacote).
- CI em `.github/workflows/ci.yml`: cargo test/clippy/fmt + pytest + gitleaks (bloqueante).
