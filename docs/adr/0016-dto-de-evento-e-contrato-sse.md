# ADR 0016 — DTO de evento owned+Serialize e contrato SSE (snapshot-then-live)

- Status: aceita
- Data: 2026-07-06

## Contexto

`forge_core::LoopEvent<'a>` (o evento do loop do agente) empresta `&str`s do
contexto de execução — correto para o caminho síncrono do CLI/TUI, mas não
serializável como está (o borrow não sobrevive a virar JSON e atravessar uma
conexão SSE). Precisamos de um tipo **owned** para o wire, e de decidir a semântica
de reconexão: um evento SSE perdido (aba fechada, rede caiu) não pode significar
"perdeu o pedido de permissão pendente para sempre".

## Decisão — `SessionEvent` owned, snapshot do log acumulado + eventos ao vivo

`SessionEvent` (`#[derive(Serialize)] #[serde(tag = "type", rename_all =
"snake_case")]`) espelha cada variante de `LoopEvent` via `From<LoopEvent<'_>>`, mais
variantes que só existem no servidor: `PermissionRequested` (pedido pendente),
`Done{ledger_verified}` (fim do turno, contagem real do ledger), `Error`. Nomes de
evento (`text_delta`, `tool_started`, `permission_requested`, `done`, ...) são o
contrato estável entre backend e frontend.

Semântica de reconexão: `SessionHub` guarda um `log: Vec<SessionEvent>` por sessão
— quem conecta em `GET /api/session/:id/events` recebe primeiro um **snapshot**
desse log acumulado (via um stream que encadeia o snapshot + o canal `broadcast`
ao vivo), depois eventos novos dali em diante. Isso cobre o caso central: um pedido
de permissão pendente já existe quando o navegador conecta (reload, ou conectou
tarde) — o snapshot o contém, o usuário ainda vê o gate. **Fora de escopo,
deliberadamente:** `Last-Event-ID`/replay fino a partir de um ponto específico —
o snapshot completo é suficiente para o caso de uso (uma sessão de código, não um
feed de alto volume).

## O que foi provado, não só declarado

- Cliente HTTP real (reqwest + `bytes_stream`) recebe a sequência de SSE e a
  decodifica por igualdade contra o esperado.
- Conectar **depois** que um pedido de permissão já existe ainda mostra o pedido
  pendente (não é só o caminho feliz de "já estava conectado antes").

## Consequências

- O contrato de nomes de evento (`snake_case`, tag `type`) é replicado
  manualmente no TypeScript (`web/src/api/stream.ts`) — sem geração de tipo
  automática entre Rust e TS nesta fase; uma mudança de nome de evento precisa
  ser espelhada nos dois lados manualmente (risco aceito, documentado aqui).
- `SessionHub` mantém o log em memória por sessão — sem persistência entre
  reinícios do processo `forge dashboard` (aceitável: uma sessão web não sobrevive
  a um restart do servidor de qualquer forma, ao contrário do ledger/`DurableSession`
  em disco, que sim sobrevivem).
