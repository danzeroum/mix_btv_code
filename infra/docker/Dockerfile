# syntax=docker/dockerfile:1
#
# Imagem de TESTE / homologação do Forge — NÃO é deploy de produção.
# O produto é local-first (ver infra/README.md): isto apenas empacota o CLI
# `forge` + o sidecar Python numa imagem, para você rodar o Forge como
# FERRAMENTA dentro de um container (na sua VPS, via SSH). Não expõe um serviço
# web multiusuário; o `forge dashboard` segue amarrado a 127.0.0.1.
#
# Build (a partir da RAIZ do repositório):
#   docker build -f infra/docker/Dockerfile -t forge:test .
# Uso (shell interativo com o `forge` no PATH):
#   docker run --rm -it -e DEEPSEEK_API_KEY=sk-... -v "$PWD":/work forge:test
#   # dentro do container (com DeepSeek, --model é OBRIGATÓRIO — ver docker/README.md):
#   forge verify                                   # self-teste (mesmo comando do CI)
#   forge run --model deepseek-chat "..."          # tarefa única
#   forge chat --model deepseek-chat               # sessão interativa
#   forge squad --model deepseek-chat "..."        # squad multi-agente (usa o sidecar Python)
#
# Pegadinhas (detalhe em infra/docker/README.md): dashboard em 127.0.0.1,
# sandbox de skills de terceiro exige o socket do Docker (DinD), e a key só
# existe no processo Rust (ADR 0001).

# ---------- build (Rust) ----------
FROM rust:1-bookworm AS build
WORKDIR /src
# protoc é vendorizado (protoc-bin-vendored), TLS é rustls e SQLite é bundled:
# o build NÃO precisa de protobuf-compiler / libssl-dev / libsqlite3-dev.
COPY . .
RUN cargo build --release -p forge-cli
# clippy/rustfmt não vêm no profile default do rustup — `forge verify` (os
# default_steps: cargo test/clippy/fmt) precisa deles no RUNTIME também, senão
# falha com "No such file or directory" (o binário não existe, não é bug do
# Forge). Adiciona aqui pra copiar o toolchain inteiro abaixo.
RUN rustup component add clippy rustfmt

# ---------- build (frontend do dashboard) ----------
# web/dist NÃO é commitado (web/.gitignore) — precisa ser buildado. Sem este
# estágio, `forge dashboard` sobe mas não tem SPA pra servir (404 no `/`).
FROM node:22-bookworm-slim AS web-build
WORKDIR /web
# web/package.json não pina `packageManager` — instala o major confirmado
# (pnpm 10.x) direto, em vez de depender da resolução implícita do corepack.
RUN npm install -g pnpm@10
COPY web/package.json web/pnpm-lock.yaml ./
RUN pnpm install --frozen-lockfile
COPY web/ ./
RUN pnpm build

# ---------- runtime ----------
FROM debian:bookworm-slim AS runtime
RUN apt-get update \
 && apt-get install -y --no-install-recommends ca-certificates curl git gcc \
 && rm -rf /var/lib/apt/lists/*

# uv: o sidecar (squad/promptforge) é spawnado pelo processo Rust via `uv run`.
RUN curl -LsSf https://astral.sh/uv/install.sh | sh
ENV PATH="/root/.local/bin:${PATH}"

# Toolchain Rust completo (cargo/rustc/clippy/rustfmt), copiado do estágio de
# build — é o que permite `forge verify` (cargo test/clippy/fmt) rodar de
# verdade dentro do container. A imagem oficial `rust:*` instala em
# /usr/local/cargo e /usr/local/rustup por convenção; `gcc` (acima) é o linker
# que `cargo build`/`test` chamam via `cc`.
COPY --from=build /usr/local/cargo /usr/local/cargo
COPY --from=build /usr/local/rustup /usr/local/rustup
ENV CARGO_HOME=/usr/local/cargo \
    RUSTUP_HOME=/usr/local/rustup \
    PATH="/usr/local/cargo/bin:${PATH}"

WORKDIR /app
COPY --from=build /src/target/release/forge /usr/local/bin/forge

# Workspace Python + ambiente uv resolvido.
# CRÍTICO: FORGE_PYTHON_DIR. Sem ele, o sidecar procura um caminho de
# compile-time (`<crate>/../../python`) que NÃO existe nesta imagem, e o
# squad/promptforge degrada EM SILÊNCIO para agente-único. (Ver
# crates/forge-cli/src/sidecar.rs.)
COPY python /app/python
RUN cd /app/python && uv sync
ENV FORGE_PYTHON_DIR=/app/python

# SPA do dashboard, já buildada no estágio web-build.
COPY --from=web-build /web/dist /app/web/dist
ENV FORGE_WEB_DIR=/app/web/dist

# Diretório onde o agente atua — monte o SEU projeto aqui (-v "$PWD":/work).
WORKDIR /work
# CMD (não ENTRYPOINT): `docker run forge:test` cai num shell interativo, mas
# `docker run forge:test forge dashboard --port 7878` SUBSTITUI o CMD inteiro e
# roda o binário direto — sem isso, um ENTRYPOINT fixo prefixaria "/bin/bash" na
# frente do comando e quebraria esse uso.
CMD ["/bin/bash"]
