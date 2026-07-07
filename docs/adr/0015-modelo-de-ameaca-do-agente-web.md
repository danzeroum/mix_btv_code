# ADR 0015 — Modelo de ameaça do agente web: local-first permanece, navegador é hostil

- Status: aceita
- Data: 2026-07-06

## Contexto

A Onda 1 da Fase 7 expõe, pela primeira vez, HTTP em `127.0.0.1` capaz de disparar
ações reais (rodar `bash`, aplicar `edit`) a partir do navegador — antes, o
dashboard (`forge-server`) só servia leitura (telemetria). A plataforma continua
local-first/single-user (sem autenticação multi-tenant, sem TLS, sem exposição além
de loopback) — isso não muda. O que muda é a superfície de ataque: o **navegador**
em si é hostil. Qualquer site aberto na mesma sessão do navegador pode tentar
requisições cross-origin contra `127.0.0.1` (CSRF), e um DNS que resolve para
`127.0.0.1` sob um domínio controlado pelo atacante (`http://127.0.0.1.evil.example`)
pode tentar se passar por local (DNS-rebinding).

## Decisão — guarda de `Origin`/`Host` em toda rota mutável, sem novo modelo de auth

Middleware (`require_local_origin`, `axum::middleware::from_fn`) aplicado a **todo
método ≠ `GET`**: se o header `Origin` estiver presente, precisa resolver para
`127.0.0.1`/`localhost`/`::1`/`[::1]` (qualquer porta) — senão `403`. Sem `Origin`
(curl, CLI, chamadas de teste) passa: só o navegador manda esse header em requisição
cross-origin, então sua ausência não é um sinal de ataque. Parsing manual de
esquema+host (sem nova dependência de `url`) para evitar o ataque de sufixo —
`127.0.0.1.evil.example` contém `127.0.0.1` como substring mas **não é** o host
`127.0.0.1`.

Não há sessão de autenticação, cookie de sessão, nem CSRF token — o modelo de ameaça
continua "processo único, usuário único, loopback apenas"; a guarda de `Origin`
fecha especificamente o vetor "outro site no mesmo navegador" e "rebinding de DNS",
não introduz uma segunda camada de identidade.

## O que foi provado, não só declarado

- Teste real: requisição `POST` com `Origin: https://evil.example` recebe `403`; a
  mesma requisição sem `Origin` passa.
- Variantes de loopback aceitas (`http://127.0.0.1:porta`, `http://localhost:porta`,
  `https://127.0.0.1`, `http://[::1]:porta`); variantes de ataque rejeitadas
  (`http://127.0.0.1.evil.example`, `http://evil.example/?u=127.0.0.1`, string vazia
  tratada como não-local).

## Consequências

- A rota que literalmente aprova `bash` (`POST /api/session/:id/permission`) e todas
  as rotas de mutação (mensagem, matriz de permissão) ficam atrás da mesma guarda —
  um site aberto no mesmo navegador não alcança nenhuma delas.
- Se a plataforma algum dia deixar de ser loopback-only (multi-usuário, rede), este
  ADR precisa ser revisitado — a guarda de `Origin` não substitui autenticação real.
