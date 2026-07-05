# ADR 0008 — Evidência de verificação atravessa Rust→Python como campo no `SquadTask`

- Status: aceita
- Data: 2026-07-05

## Contexto

A Fase 5 Onda 3 fecha a fronteira demarcada em `auditor.py` desde a Fase 4
(comentário explícito no módulo: "consumir evidência determinística do
`/verify` completo é escopo da Fase 5"). O `AuditorAgent.validate_results`
precisa julgar sobre a `verification-evidence.v1` real que `forge-verify`
(Onda 1) produz e que `forge verify` (Onda 2) já grava em disco — no lugar
de só as heurísticas de padrão (`check_security`/`check_quality`).

A pergunta de contrato: como a evidência atravessa a fronteira gRPC
Rust→Python até chegar no auditor?

## Decisão

**Campo `string verification_evidence_json = 5` no `SquadTask`** (não um
RPC novo `CoreService.GetEvidence`).

Razões:
- A evidência é conhecida **antes** do squad rodar — é sobre o estado atual
  do código, não algo que muda durante a execução da tarefa. Não há motivo
  para um mecanismo de *pull* sob demanda quando o dado já existe no
  momento em que o `forge squad` monta o `SquadTask`.
- Menos superfície: um campo string aditivo em uma mensagem proto3 já
  existente é compatível para trás (clients antigos que não o setam
  recebem `""` no lado servidor); um RPC novo exigiria método novo em
  `CoreService`, stub novo nos dois lados (Rust `tonic` + Python
  `grpcio-tools`), e uma rota de erro adicional.
- O `task` dict já flui `server.py::ExecuteTask → orchestrator.execute_complex_task
  → agentes` sem cerimônia — acrescentar uma chave é natural; um RPC novo
  quebraria essa simplicidade ao exigir que o orquestrador (ou o auditor)
  segurasse uma referência ao canal gRPC do Core só para puxar evidência.

**Fluxo completo:** `forge squad` (Rust) roda `run_verify_pipeline` (o
mesmo helper que `forge verify` usa — extraído para não duplicar) sobre o
workspace atual, serializa a evidência (`serde_json::to_string`) e a
anexa no `SquadTask.verification_evidence_json` antes de chamar
`ExecuteTask`. `server.py` parseia o campo e monta duas chaves no `task`
dict: `verification_evidence` (o dict parseado, ou `None`) e
`verification_evidence_missing` (bool). O orquestrador, em
`execute_complex_task`, checa `verification_evidence_missing`: se `True`,
**fail-closed antes de chamar o gateway** (economiza uma chamada de LLM
que já sabemos que não pode aprovar); senão, repassa
`task.get("verification_evidence")` para `AuditorAgent.validate_results`.

## A armadilha de proto3 e como foi evitada

Campo string ausente em proto3 vira `""`, não erro — o mesmo tipo de
default silencioso que já mordeu o projeto antes (`Consensus.requires_human`,
ADR/Fase 4c: campo que é `@property` do lado Python e campo real do lado
proto, exigindo tradução explícita nos dois sentidos). Aqui o risco
equivalente seria tratar `""` (ou JSON inválido) como "sem evidência,
tudo bem" — que inverteria a régua "Nada Fake" para "ausência de prova =
aprovação". A escolha estrutural: `server.py` distingue explicitamente
"evidência ausente/inválida" (`verification_evidence_missing=True`) de
"evidência válida" (`verification_evidence=dict`), e o orquestrador trata
o primeiro caso como reprovação automática, não como sinal neutro.

Chamadas diretas ao `UnifiedOrchestrator.execute_complex_task` que nunca
passam por `server.py` (os testes existentes de `test_orchestrator.py`,
por exemplo) simplesmente não têm a chave `verification_evidence_missing`
no `task` dict — `.get(..., False)` preserva o comportamento anterior sem
exigir nenhuma mudança nesses testes.

## Consequências

- `squad.proto`: `SquadTask` ganha o campo 5 (aditivo, compatível).
  Regenerado nos dois lados (`build.rs` automático no Rust;
  `scripts/gen_proto_py.py` explícito no Python).
- `forge squad` agora roda `/verify` sobre o workspace **antes** de cada
  disparo de tarefa — custo de tempo adicional (mesma ordem de grandeza de
  `cargo test --workspace` do `default_steps()`), aceito porque é o que dá
  ao auditor algo real para julgar. Mensagem de progresso no stderr evita
  a sensação de travamento.
- `AuditorAgent.validate_results` ganha o parâmetro opcional `evidence`;
  veredito continua vindo do gateway (o modelo pesa a evidência, não
  carimba automaticamente por ela) — preserva a régua fail-closed já
  provada na Fase 4 Onda 2.
- Evidência grande demais para caber confortavelmente num campo string
  seria motivo para um `.v2` do proto (ex.: passar a hash + referência a
  artefato em disco/objeto) — não antecipado nesta onda; a evidência real
  observada é pequena (poucos KB).
