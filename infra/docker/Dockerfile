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

# ---------- imagem única (build + runtime do Rust) ----------
# Deliberadamente UMA imagem só pro Rust, em vez de compilar em `rust:1-bookworm`
# e copiar o toolchain pra um runtime `debian:bookworm-slim` separado: essa
# mistura de bases já quebrou (linker `cc` incompatível entre o gcc/libc de uma
# imagem e o Rust linkado na outra). Aqui o mesmo ambiente que compila o binário
# é o que roda `forge verify` (cargo test/clippy/fmt) depois — zero risco de
# descompasso de ABI. Custo aceito: imagem maior; é imagem de teste, não de
# produção.
FROM rust:1-bookworm
RUN apt-get update \
 && apt-get install -y --no-install-recommends ca-certificates curl git \
 && rm -rf /var/lib/apt/lists/*
# clippy/rustfmt não vêm no profile default do rustup — `forge verify` (os
# default_steps: cargo test/clippy/fmt) precisa deles.
RUN rustup component add clippy rustfmt

WORKDIR /src
COPY . .
RUN cargo build --release -p forge-cli \
 && cp target/release/forge /usr/local/bin/forge

# uv: o sidecar (squad/promptforge) é spawnado pelo processo Rust via `uv run`.
RUN curl -LsSf https://astral.sh/uv/install.sh | sh
ENV PATH="/root/.local/bin:${PATH}"

# Workspace Python + ambiente uv resolvido.
# CRÍTICO: FORGE_PYTHON_DIR. Sem ele, o sidecar procura um caminho de
# compile-time (`<crate>/../../python`) que NÃO existe no container final (o
# repo real é montado em /work, não em /src), e o squad/promptforge degrada EM
# SILÊNCIO para agente-único. (Ver crates/forge-cli/src/sidecar.rs.)
RUN cd /src/python && uv sync
ENV FORGE_PYTHON_DIR=/src/python

# SPA do dashboard, já buildada no estágio web-build.
COPY --from=web-build /web/dist /src/web/dist
ENV FORGE_WEB_DIR=/src/web/dist

# Diretório onde o agente atua — monte o SEU projeto aqui (-v "$PWD":/work).
# `forge verify` roda `cargo test/clippy/fmt` contra O QUE ESTIVER em /work
# (o projeto montado), não contra /src (a cópia usada só pra compilar o forge).
WORKDIR /work
# CMD (não ENTRYPOINT): `docker run forge:test` cai num shell interativo, mas
# `docker run forge:test forge dashboard --port 7878` SUBSTITUI o CMD inteiro e
# roda o binário direto — sem isso, um ENTRYPOINT fixo prefixaria "/bin/bash" na
# frente do comando e quebraria esse uso.
CMD ["/bin/bash"]
