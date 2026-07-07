# ADR 0023 — `RunTool` ativado: squad como executor sob permission-engine

- Status: aceita
- Data: 2026-07-07

## Contexto

Um run real na VPS (`forge squad "crie uma calculadora científica... gere
um arquivo .html"`) não produziu arquivo nenhum. O diagnóstico (parecer
de engenharia externo, revisado e corrigido em dois pontos) apontou a
causa raiz: o squad planeja, delibera e audita, mas não tinha executor. A
**Onda 0** (commit `f8be5bd`, PR #42) fechou a metade imediata do
problema — o auditor parou de poder alegar que um arquivo foi
persistido sem evidência — e deixou explicitamente registrado em
`pendencias.md` o que faltava: "Onda 1 (RunTool real) e Onda 2 (loop
ReAct do developer)... a peça que faz um arquivo aparecer de verdade no
disco". Esta ADR fecha essa pendência (Ondas 1–3).

**Precedente correto: `Generate`/`RequestPermission`, não a ADR 0022.**
`RunTool(ToolCall) -> ToolResult` já existia no contrato
(`schemas/proto/core.proto`), servido pelo Rust e dormente
(`Status::unimplemented`) — a mesma forma de `Generate`/
`RequestPermission`, que já estão ativos e provados nessa direção
(`CoreService` servido pelo Rust, chamado pelo Python). A ADR 0022
não é o precedente aqui: ela tratou de um problema com a direção
**oposta** (memória mora no Python; `CoreService.Recall`/`Remember`
seriam a direção errada) e por isso construiu um `MemoryService` novo,
servido pelo Python — decisão correta para aquele caso, mas não aplicável
a `RunTool`, que já está na direção certa desde o início.

## Decisão

Ativar `RunTool` reusando o `ToolRegistry`/`PermissionEngine` que o loop
de agente único já usa (`crates/forge-core/src/agent_loop.rs`) — não um
"shuttle" de `final_output` pelo stream de eventos. Zero mudança breaking
de proto (só comentários aditivos em `ToolCall`/`ToolResult`).

**Onda 1 — executor real no Rust.** `CoreBackend` (`forge-sidecar/src/
core_server.rs`) ganha `run_tool`; o handler gRPC vira um passthrough
fino (erro de domínio — negado/falhou — vira `ToolResult` normal, nunca
`Status` de transporte, mesmo espírito de `generate`). `core_run_tool`
(`forge-cli/src/squad.rs`) é o helper compartilhado pelos três
`CoreBackend` de produção (`GatewayCoreBackend`, `WebSquadCoreBackend`,
`ScriptedSquadCoreBackend`): recalcula o escopo real via `Tool::scope`
(o `ToolCall.scope` da rede é só informativo — nunca decide permissão,
fechando um vetor onde um Python bugado/comprometido poderia declarar
escopo mais permissivo que o real), avalia via `PermissionEngine` (perfil
`BUILD` reusado — `read`/`grep` liberados, `edit`/`bash` pedem
confirmação — perguntar a cada leitura de um loop de vários passos seria
ruído desproporcional), executa em `spawn_blocking` (só a chamada
síncrona `tool.run`, não a checagem de permissão nem o `Ask` assíncrono)
e registra cada chamada no ledger (`squad.tool_run`, via novo
`session::append_entry` — uma variante de `append_override_entry` sem a
marcação de override, para logging fora do ciclo de vida de uma
`Session` de tarefa). Convenção de `exit_code`: `0` sucesso; `1` erro de
execução/args inválidos/ferramenta desconhecida (vale tentar de novo);
`-1` negado (não adianta repetir a mesma ação) — um sinal estrutural para
o loop ReAct, não só prosa pro modelo interpretar.

**Onda 2 — loop ReAct real no `developer` Python.** `ToolClient`
Protocol + `GrpcToolClient` (`grpc_clients.py`) sobre `CoreService.
RunTool` — zero codegen novo, o stub Python já existia gerado
(`core_pb2_grpc.py`) e nunca usado. `DeveloperAgent._implement_with_tools`
é o loop: o modelo alterna entre `tool_call` (executado de verdade) e
`final_answer`, até um dos dois ou até estourar `_MAX_REACT_STEPS`/
`_REACT_TIMEOUT_SECONDS` — nesse caso devolve `status: "incomplete"`
honesto, nunca fabrica sucesso. Sinal de ativação:
`bool(task.get("action")) and tool_client is not None` — separa
trabalho real do plano de proposta/avaliação sem hardcodar um
vocabulário de ações, preservando o caminho de chamada única intacto.

Achado real durante a revisão do plano, antes de qualquer linha de
código: `_can_parallelize` (`orchestrator.py`) manda um passo
`"implement"` sem `dependencies` — o caso comum de um plano de 1 passo —
para `_extract_parallel_tasks`, que chamava o developer **sem**
`"action"`. O sinal de ativação nunca disparava nesse caminho: era
exatamente a rota que reproduziria o bug original mesmo com `RunTool`/
loop ReAct prontos. Fix: `_extract_parallel_tasks` passou a propagar
`action`/`prior_results` (mesma forma do `step_task` sequencial) — testado
com um caso que falha sem o fix (verificado manualmente revertendo e
rodando antes de reaplicar).

Achado real, também de leitura antes de escrever código:
`core_generate` (`squad.rs`) só tratava o papel `"system"` como
especial — qualquer outro, incluindo `"assistant"`, colapsava em
`Role::User`. Todo caller anterior mandava só 1 mensagem system + 1
user, então isso nunca foi exercitado; o loop ReAct é o primeiro a
mandar histórico multi-turno de verdade, e a API da Anthropic exige
alternância estrita. Corrigido antes de dar problema em produção.

**Onda 3 — evidência real chega ao auditor + gate duro + observabilidade
do veredito.** Como a Onda 0 já fez `execution_results`/`prior_results`
carregarem o dict completo de cada passo, a nova chave `tool_calls` (Onda
2) chega aos dois pontos de auditoria sem nenhuma fiação nova no
orquestrador. `_claims_completion_without_write_evidence`
(`auditor.py`) é um gate duro (mesma filosofia de `forge_review/
gates.py::evaluate` — regra dura sobrepõe a média do LLM): reprova ANTES
do gateway ser chamado quando um resultado do developer alega
`status: completed` sem nenhum `tool_calls[].exit_code == 0` em
`edit`/`bash`. Só se aplica a resultados que passaram pelo loop ReAct
(carregam a chave `tool_calls`, mesmo vazia) — um resultado do caminho
antigo de chamada única nunca teve infraestrutura de ferramenta
disponível, então gateá-lo puniria a ausência de algo que nunca poderia
ter existido (achado real ao rodar a suíte: sem esta distinção, o gate
quebrava um teste legítimo da Onda 0 que usa o caminho antigo de
propósito). Um novo `_emit` em `orchestrator.py` (reusando `StepResult`,
`step_id: "final_validation"`, zero mudança de proto) torna o veredito
final observável fora do dict de retorno que `server.py` descarta —
sem isso, a definição de pronto ("o auditor julgando sobre o arquivo
real, observável") não seria verificável fora de um teste Python isolado.

## Não-escopo explícito

- Sem ferramenta de criação de arquivo dedicada. `ToolRegistry::
  default_set` = `read, grep, edit, bash`; `EditTool` exige que o arquivo
  já exista (`std::fs::read_to_string` antes de escrever). `bash`
  (heredoc/redirecionamento) é o único mecanismo para criar um arquivo do
  zero — guiado por prompt (`_REACT_SYSTEM_PROMPT`), não uma ferramenta
  nova. Documentado como restrição real, não contornado em silêncio.
- Sem mensagem de proto dedicada para o evento de validação final — reusa
  `StepResult` (`kind: "step"`, `step_id: "final_validation"`).
- Sem perfil de permissão mais fino que `BUILD` para o squad
  especificamente — a alternativa mais conservadora
  (`PermissionEngine::default()`, pergunta tudo) é uma troca de uma linha
  se a postura mais cautelosa for preferida no futuro.
- **O gate duro é backstop mecânico, não prova de materialização** — prova
  "uma chamada mutante (`edit`/`bash`) rodou sem erro do lado Rust", não
  "o arquivo X existe com o conteúdo Y". `BashTool::run` devolve `Ok`
  mesmo quando o comando shell interno falha (embute `[exit code: N]`
  como texto, não retorna `Err`) — então mesmo o `exit_code: 0` do proto
  é sobre a execução da *ferramenta*, não sobre o sucesso do *comando*.
  A convenção de comando de verificação do prompt (ex.: `sha256sum` antes
  do `final_answer`) é o que bota evidência de verdade no transcript para
  o auditor-LLM raciocinar em cima; o gate mecânico fica deliberadamente
  grosseiro para não depender de parsing frágil de texto. Complementar à
  proibição textual dos dois prompts da Onda 0, não substitui: o prompt
  reduz a chance de uma alegação falsa chegar ao payload; o gate faz uma
  alegação falsa não conseguir produzir `approved`/`passed: true` de jeito
  nenhum, mas só quando há infraestrutura de ferramenta para checar.
- Sem extração estrutural de path/hash do lado Rust para arquivos criados
  via `bash` — só a convenção de prompt acima.
- `max_autonomy_level` (ADR 0021) continua intocado e ignorado
  ponta-a-ponta — ortogonal a esta entrega.
- Sem mudança nos jobs `sandbox`/`bench`/`k6`/`deny`/`security`/`web` do
  CI — os jobs `rust` e `python` (que já rodam `cargo test --workspace`/
  `uv run pytest`) pegam os testes novos automaticamente.

## O que foi provado, não só declarado

- **Onda 1** — `crates/forge-sidecar/tests/core_server_inprocess.rs`:
  `run_tool_executa_de_verdade_e_arquivo_aparece_no_disco` (um `ToolCall`
  de `bash` sobre UDS puro, sem Python, cria um arquivo real num tempdir)
  e `run_tool_negado_pela_permissao_nao_executa` (uma negação do motor de
  permissões não executa nada, `exit_code: -1`, nenhum arquivo criado).
- **Onda 2** — `python/packages/forge-squad/tests/test_developer.py`:
  `test_execute_com_action_usa_tool_client_e_faz_tool_call_antes_do_final_answer`,
  `test_execute_sem_action_nao_usa_tools_mesmo_com_tool_client_anexado`
  (prova o sinal de ativação e que o caminho antigo sobrevive intacto),
  `test_react_loop_esgota_passos_sem_final_answer_devolve_incomplete_honesto`.
  `test_orchestrator.py::test_passo_implement_paralelizavel_ainda_ativa_tool_client_do_developer`
  prova o fix do caminho paralelo (falha sem ele — verificado manualmente).
  `crates/forge-cli/src/squad.rs` — teste inline
  `core_generate_mapeia_papel_assistant_para_role_assistant` prova o fix
  de papel via um gerador que grava as mensagens recebidas.
- **Onda 3** — `python/packages/forge-squad/tests/test_auditor.py`:
  `test_validate_results_reprova_completed_sem_tool_call_de_escrita_mesmo_com_llm_aprovando`
  e `test_audit_reprova_prior_results_sem_tool_call_de_escrita_mesmo_com_llm_aprovando`
  (o gate dispara antes do gateway ser sequer chamado — `gateway.requests
  == []`), `test_validate_results_nao_reprova_completed_com_tool_call_de_escrita_bem_sucedida`
  e `test_validate_results_nao_reprova_completed_sem_chave_tool_calls_caminho_antigo`
  (as duas contraprovas: o gate não é um martelo cego). O teste de
  fechamento, `crates/forge-sidecar/tests/squad_e2e.rs::
  squad_cria_arquivo_real_via_run_tool_ledger_e_auditor_veem_evidencia`
  — processo Python real, sem key — dirige `forge squad "crie
  scientific-calculator.html..."` e prova, nesta ordem: (1) o arquivo
  existe de verdade no workspace; (2) o ledger tem a entrada
  `squad.tool_run`; (3) um evento `Consensus` real aparece; (4) um
  `StepResult{step_id: "final_validation"}` aparece com `approved: true`
  — e o próprio backend roteirizado falha o teste alto e claro (via
  `assert!` dentro do `generate()` do lado "auditor") se o payload que
  chega ao Rust não carregar evidência real de `tool_calls`, provando que
  o auditor não está julgando no vácuo.

## Consequências

- O squad passa a tocar o filesystem — expande, não cria, uma fronteira
  de confiança: é o mesmo `PermissionEngine` que o Rust já aplica ao loop
  de agente único, agora também sob chamada do Python.
- Latência maior no developer quando o loop ReAct roda (multi-turno vs.
  chamada única) — aceitável, é o preço de materializar algo de verdade.
- O ledger cresce uma entrada por chamada de ferramenta do squad.
- Trabalho futuro explícito (não bloqueante): ferramenta de escrita
  dedicada (criar arquivo sem depender de heredoc via `bash`); perfil de
  permissão mais fino especificamente para o squad; evento de proto
  dedicado para validação (`ValidationResult`) em vez de reusar
  `StepResult`; extração estrutural de evidência (path/hash) do lado
  Rust para escritas via `bash`.
