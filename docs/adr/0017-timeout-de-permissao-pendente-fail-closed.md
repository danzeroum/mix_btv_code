# ADR 0017 — Timeout de permissão pendente: fail-closed (`Deny` após prazo)

- Status: aceita
- Data: 2026-07-06

## Contexto

`WebPermissionResolver::resolve` (a ponte permissão↔HTTP) publica o pedido e
bloqueia a thread (`spawn_blocking`) esperando a resposta por um canal `mpsc`
síncrono. Se ninguém responder — aba fechada, usuário afastado, navegador
travado — essa thread ficaria bloqueada para sempre, e a thread do pool de
`spawn_blocking` nunca voltaria a ficar disponível.

## Decisão — prazo configurável, `Deny` ao expirar

`SessionHub` recebe um `permission_timeout: Duration` no construtor; o canal usa
`recv_timeout` em vez de `recv`. Ao expirar sem resposta, o resultado é `false`
(`Deny`) — fail-closed, mesmo espírito do resto da plataforma (auditor sem
evidência reprova, vetter sem manifesto bloqueia). Prazo default de 300s
(configurável via `FORGE_PERMISSION_TIMEOUT_SECS`); testes usam prazos
encurtados via `SessionHub::new` direto.

## O que foi provado, não só declarado

- Teste real: nenhuma resposta ao pedido → o resolver expira em `Deny` sozinho,
  sem travar a thread, dentro do prazo configurado em teste (encurtado).

## Consequências

- Uma ferramenta pedida e nunca respondida é **negada**, não fica presa — o loop
  do agente segue adiante tratando como uma negação normal (`ToolDenied`), não como
  um erro.
- O prazo é por-pedido, não por-sessão: cada `PermissionRequested` tem seu próprio
  relógio a partir da publicação do evento.
