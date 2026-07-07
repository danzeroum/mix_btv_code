# ADR 0020 — Topologia de processo do agente web: onde o código mora, opt-in, teto de sessões

- Status: aceita
- Data: 2026-07-06

## Contexto

`forge-server` (o dashboard existente, Fase 3) nunca dependeu de
`forge-tools`/`forge-core`/`forge-sidecar` — só de `forge-store`/`forge-schemas`.
O agente web da Fase 7 precisa do loop do agente inteiro (`forge-core::AgentLoop`,
`forge-tools::ToolRegistry`) para rodar uma sessão de código de verdade a partir do
navegador. Onde esse código novo mora, e como ele convive com o dashboard existente
sem forçar `forge-server` a ganhar dependências que sua fronteira nunca teve?

## Decisão

- **Código novo mora em `forge-cli` (`web_agent.rs`), não em `forge-server` nem em
  `forge-core`.** `forge-cli` já depende de tudo que o loop do agente precisa
  (é o mesmo binário que roda `forge run`/`chat`/`tui`); `forge-server` continua
  sem ganhar nenhuma dependência nova — seu router é `.merge()`ado ao router novo,
  não modificado.
- **Opt-in via flag `--web-agent`** no comando `forge dashboard` — o dashboard
  padrão (sem a flag) continua exatamente como era. A composição
  (`merged_router`/`serve_with_agent`) é aditiva: o dashboard ganha as rotas novas
  por cima da guarda de `Origin`/`Host` (ADR 0015), sem duplicar nada.
- **Sempre `spawn_blocking`, nunca `tokio::spawn` comum, para a task da sessão.**
  O resolver de permissão bloqueia uma thread real esperando a resposta do
  navegador (ou o timeout, ADR 0017) — um `tokio::spawn` comum esgotaria uma
  worker-thread do reactor async sob N sessões concorrentes, degradando TODA a
  aplicação, não só a sessão travada.
- **Teto configurável de sessões vivas simultâneas** (`FORGE_MAX_SESSIONS`,
  default 8) — cada sessão ocupa uma thread do pool de `spawn_blocking` enquanto
  viver; acima do teto, `429`.

## O que foi provado, não só declarado

- Servidor axum real em porta efêmera, sessão completa via generator sequenciado
  (sem key) — SSE + permissão + ledger, ponta a ponta.
- Teto de sessões: a sessão N+1 acima do limite recebe `429`, não trava o
  processo nem estoura o pool de threads.

## Consequências

- `forge-server` permanece testável/deployável independentemente do agente web —
  quem só quer o dashboard de telemetria não paga o custo de compilar
  `forge-tools`/`forge-core` transitivamente através dele.
- O teto de sessões é um limite de RECURSO (threads de `spawn_blocking`), não um
  limite de negócio — se o produto precisar de mais que 8 sessões simultâneas em
  produção, é uma configuração, não uma mudança de código.

## Atualização (Onda 15 — fecho da Fase 7)

O bullet "opt-in via `--web-agent`" mudou: a flag virou `--no-web-agent`
(opt-**out**), com o agente web habilitado por padrão — o navegador é a
forma primária de uso desta fase, não mais um extra atrás de flag. As
outras três decisões desta ADR (código em `forge-cli`, `spawn_blocking`
sempre, teto configurável de sessões) continuam valendo sem mudança.
