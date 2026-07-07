# ADR 0018 — Sessão como ator único; auditoria de mutação da matriz de permissão

- Status: aceita
- Data: 2026-07-06

## Contexto

Duas preocupações distintas, mas ligadas por serem sobre concorrência e confiança
numa mesma sessão: (1) múltiplas abas ou requisições concorrentes na mesma
`session_id` — o que acontece se duas mensagens chegam ao mesmo tempo? (2)
afrouxar a política de permissão pelo navegador (matriz build/plan×tool, botão
"sempre") é a mutação mais sensível de todo o plano — precisa de rastro auditável,
não pode ser um clique único e opaco.

## Decisão

**Sessão-ator único:** `SessionHub` marca cada sessão com `busy: bool`. Uma
mensagem em processamento (`try_start`) faz qualquer segunda tentativa concorrente
na MESMA sessão receber `409 Conflict` imediatamente — nunca corrompe o histórico
ou intercala eventos de dois turnos. `finish_busy` libera a sessão ao fim (sucesso
ou erro), sempre.

**Auditoria da matriz de permissão:** toda gravação ou remoção de uma `Rule`
persistida (`RuleStore`, Fase 7 Onda 2 remanescente) grava, além da própria regra,
uma entrada `override`-marcada no MESMO ledger append-only que o resto da
plataforma usa (`crate::session::append_override_entry` — reusa `LedgerStore`
diretamente, sem um `session.start`/`session.end` de tarefa, já que é uma mutação
de configuração, não uma execução de agente). A UI:
- lista as regras ativas com botão de revogar (nunca uma matriz "cega" que só
  aceita escrita);
- mostra o escopo da regra (tool + `scope_prefix`) explícito num modal antes de
  confirmar — nunca um clique único e opaco;
- o terceiro estado "sempre" da ponte de permissão grava um override restrito ao
  escopo EXATO do pedido pendente (não um "allow" genérico do tool inteiro — esse
  caso é coberto pela matriz, um mecanismo distinto).

## O que foi provado, não só declarado

- Duas escritas concorrentes na mesma sessão: a segunda recebe erro claro, não
  corrompe o histórico.
- Editar uma célula da matriz grava uma `Rule`, aparece uma entrada nova no
  ledger (`ledger.verify_chain()` bate por igualdade), e o botão "revogar" a
  remove da lista ativa E do efeito real (a decisão volta ao default do perfil).
- Uma regra persistida ("build+bash=allow") muda o comportamento de uma sessão
  REAL: o loop pula `PermissionRequested` e roda a ferramenta direto — não é só
  cosmético na tela Skills.

## Consequências

- O `RuleStore` (SQLite, WAL) é uma fonte de verdade nova, distinta dos perfis
  const (`forge_core::{BUILD,PLAN}`) — `PermissionEngine::overlay` combina os
  dois (overrides sempre vencem), e é a MESMA combinação que a Onda 7 (Console
  MCP) reusa para seu preview de política.
- `409` é o único sinal de concorrência — não há fila/retry automático; o
  cliente decide se tenta de novo.
