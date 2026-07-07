# Pendências e decisões da execução autônoma (Fase 6, Ondas 3-tail → 9)

> Log das decisões que tomei sozinho e das dúvidas que quero que você revise.
> Cada item diz se é **decisão** (segui em frente) ou **dúvida** (precisa do seu
> olhar). Ordenado por onda.

## Onda 3 — cauda (`/api/skills` + tela + ledger)

- **[decisão] Tela `skills` vira read-only + "re-vetar".** O status do vetter é
  determinístico e **não sobreponível** pelo usuário (é a régua fail-closed da
  fase — deixar o usuário "aprovar" uma skill bloqueada anularia a segurança).
  Então troquei os botões `aprovar`/`bloquear` (que o mock permitia) por: badge
  read-only do status real + um botão `re-vetar` que re-busca `/api/skills`
  (re-roda o vetter no servidor). O `vetSkill` mock virou `fetchSkills` real.
- **[decisão] `/api/skills` é GET read-only.** Enumera `skills/` (builtin) +
  `.forge/skills/` (third-party), veta cada uma via
  `forge-verify::vetter::list_skill_statuses`, devolve `[{id,status,detail}]`.
  Sem endpoint de ação (vet/block) — não há o que "acionar", o vetter decide.
- **[dúvida] Ledger `skill.vetting` re-veta (double-vet).** Registro o veredito
  no ledger em `run_once` reusando `list_skill_statuses` — mas isso re-veta as
  skills (o `build_registry` já vetou ao carregar). Para built-ins (sem
  `[[verify]]`) o custo é nulo; para uma skill de terceiro com passos
  `[[verify]]` que rodam subprocessos, roda-os 2×. Aceitei por simplicidade e
  zero-ripple. **Futuro:** `load_skills` devolver as decisões e registrar sem
  re-vetar. Além disso, só `run_once` registra hoje; `chat`/`tui` não (fácil de
  estender com o mesmo helper, deixei fora para não alargar o diff).

## Onda 4 — MCP (rmcp)

- **[decisão] Conexão por chamada (connect-per-call).** `McpTool::run` reconecta
  ao servidor (spawn do processo), chama, encerra — a cada invocação. Simples e
  sem estado compartilhado, espelha o sandbox. **Futuro (otimização):** sessão
  persistente (conecta uma vez, reusa a conexão) via um handle numa thread de
  runtime dedicada. Vale para servidores MCP caros de subir.
- **[decisão] Política de confiança MCP (o ADR planejado da onda).** O servidor
  é declarado pelo usuário (em `.forge/mcp.toml`) = confiança explícita; **cada
  chamada** passa pelo permission-engine (nomes `mcp__<server>__<tool>` não
  batem em nenhuma regra → default `Ask` → pergunta ao usuário). Não há vetting
  estilo-skill do servidor. Isto é o conteúdo do ADR 0011 (MCP) — **falta
  formalizar o arquivo em `docs/adr/`** (item da Onda 9).
- **[decisão] Namespacing `mcp__<server>__<tool>` + guarda de colisão.** Uma
  tool MCP não sombreia built-in/skill; registro do mesmo servidor 2× não
  duplica. Fail-soft: `.forge/mcp.toml` ausente/inválido ou servidor que não
  sobe → loga e segue (não derruba o CLI).
- **[decisão] `render_content` extrai só texto.** O resultado MCP pode ter
  blocos não-texto (imagem, resource_link); hoje concateno só os `text`. Refinar
  quando uma tool MCP real devolver conteúdo rico.
- **[dúvida/defer] Frontend MCP não ligado.** `MCP_SERVERS`/`reconnectMcp`
  seguem mock. O wiring real (`/api/mcp` + `fetchMcpServers`) espelha o que fiz
  no `/api/skills` da cauda da Onda 3 — deixei para depois para não inflar a PR.
- **[nota] `rmcp` v2.1.0** entrou como dep direta de `forge-tools` (features
  `client,server,transport-child-process,transport-io`), não em
  `[workspace.dependencies]`. Dep pesada, mas é a lib nomeada pelo PLANO. Passou
  no `cargo deny` local? — verificar no CI (job `deny`). **Resolvido:** passou no
  job `deny` da PR #14 (merge 83a61c4).

## Onda 5 — LSP (rust-analyzer/pyright)

- **[decisão] Zero dependência nova — framing LSP hand-rolled.** O protocolo LSP
  é JSON-RPC com framing `Content-Length` sobre stdio, simples o bastante para
  escrever à mão (só `serde_json`, que já é dep). **Não** puxei `lsp-types`/
  `lsp-server`/`async-lsp` — mantém o `cargo deny` leve e nos dá controle total.
  Provado por um probe contra o rust-analyzer REAL antes de escrever o módulo (o
  framing bate exatamente; a definição de um símbolo volta na posição certa).
- **[decisão] Sessão persistente preguiçosa (≠ connect-per-call do MCP).** O
  language server é caro de subir (rust-analyzer indexa o workspace, ~1-3s). Ao
  contrário do MCP (conecta por chamada), a sessão LSP sobe **uma vez** no
  primeiro uso e as consultas seguintes reusam o processo já indexado
  (`Arc<LspSession>` compartilhada pelas 3 tools do server). Processo morto no
  `Drop` (lição do process-group da Fase 4 — nada de órfão).
- **[decisão] Registro é lazy — não sobe o server no load.** `register_lsp_server`
  só registra as 3 tools (`lsp__<id>__{definition,references,diagnostics}`); o
  processo sobe no primeiro `run`. Então um comando LSP inválido em `.forge/
  lsp.toml` **não** derruba nem trava o `build_registry` (fail-soft): só falha na
  primeira invocação daquela tool. As posições são **0-indexed** (convenção LSP),
  documentado no schema/descrição das tools.
- **[decisão] Prova em duas camadas.** (1) Teste **hermético** com server fixture
  (`forge_lsp_fixture`, sempre roda, sem depender do rust-analyzer instalado) —
  prova framing/handshake/ida-e-volta do cliente. (2) Teste contra o
  **rust-analyzer REAL** (`#[ignore]`, roda no job `sandbox` do CI que instala a
  componente; guarda que FALHA se ela faltar) — prova a semântica: a definição de
  `alvo` volta em `lib.rs:0:7` por igualdade, referências incluem o call-site,
  diagnósticos pegam um erro de sintaxe. Mesma postura anti-falso-positivo do
  sandbox (Onda 2).
- **[dúvida/limitação] Leitura síncrona sob o lock (sem reader de fundo).** Entre
  consultas, notificações do server (`$/progress`, `publishDiagnostics`) ficam no
  buffer do pipe do SO até a próxima consulta drená-las. Para o fixture e uso
  típico é seguro (buffer de 64KB); um projeto gigante com enxurrada de
  notificações poderia, em teoria, encher o buffer entre consultas. **Futuro
  (endurecimento):** thread de fundo drenando stdout num canal. Aceitei a versão
  simples porque a consulta drena tudo ao ler até o próprio id.
- **[dúvida/limitação] Diagnósticos são best-effort/assíncronos.** O LSP empurra
  `publishDiagnostics` após o `didOpen`, sem sinal claro de "assentou". Bombeio
  round-trips baratos (`documentSymbol`) até aparecer um diagnóstico ou estourar
  o orçamento (`DIAG_BUDGET` 12s; sai em ~3s após a 1ª notificação se vier
  vazio). Arquivo limpo → devolve "sem diagnósticos" (honesto). Testei com erro
  de **sintaxe** (reportado nativamente, rápido) e não de tipo (que dependeria de
  `cargo check`/flycheck, mais lento e flaky).
- **[dúvida/defer] Frontend LSP não ligado.** Não há mock de LSP no frontend a
  ligar (diferente do MCP/skills); as consultas LSP são tools que o agente usa no
  loop, não um painel. Sem trabalho de UI nesta onda.
- **[nota] rust-analyzer é uma componente do rustup**, não vem por padrão. O job
  `sandbox` do CI roda `rustup component add rust-analyzer` antes do
  `--include-ignored`. Local: idem para exercitar o caminho real. **Resolvido:**
  passou no CI (PR #15, merge 03ce513) — o log do job `sandbox` mostra os dois
  testes reais `... ok`, `0 ignored`.
- **[dúvida — achado de dogfooding real, pós-merge]** Testando manualmente com a
  API da DeepSeek (VPS do usuário, fora do CI), o agente **nunca invocou**
  `lsp__rust__definition` mesmo quando o prompt pedia explicitamente ("usando
  LSP, onde é definido X? não abra outros arquivos, use a ferramenta de LSP") e
  o modelo **anunciou a intenção de usar o LSP duas vezes** — nas duas ele
  recuou pra `grep`+`read`. Causa raiz confirmada no código, não é só "o modelo
  não quis": o `input_schema` da tool exige `{file, line, character}` — a
  **coluna exata** de onde o símbolo aparece — mas `grep` (`crates/forge-tools/
  src/grep.rs:29,74-79`) devolve só `caminho:linha:conteúdo`, **nunca a
  coluna**. Pra montar uma chamada válida de LSP a partir de um cold-start ("sei
  que X é usado em algum lugar, ache a definição"), o modelo precisaria contar
  caracteres manualmente na linha — um passo de raciocínio extra, propenso a
  erro, que grepar/ler a definição direto evita com o mesmo resultado final.
  Não é bug (a tool funciona quando chamada — provado no CI), é fricção de
  design: a tool de **posição** não compõe bem com as tools de **conteúdo** que
  o agente já tem à mão. Numa das tentativas o modelo também gastou 2min num
  `cargo doc --open` (timeout) — escolha pesada e inadequada (--open tentaria
  abrir navegador num container headless) em vez de um grep direto.
  **Ideias de mitigação (não implementadas, deixo pra você decidir):** (a)
  `grep` passar a expor coluna (ripgrep suporta `--column`) — daria ao modelo os
  dados pra montar a chamada LSP sem contar na mão; (b) a tool LSP aceitar um
  modo de conveniência por **nome de símbolo** (ela mesma acha a 1ª ocorrência e
  resolve a posição), tirando o peso de encontrar a coluna do agente. Nenhuma
  das duas é urgente — o agente sempre chegou na resposta certa via grep/read;
  é sobre a ferramenta LSP ser efetivamente usada, não sobre correção do
  resultado.

## Onda 6 — RAG (recuperação semântica da memória)

- **[decisão — vai ao ADR da Onda 9] Embedder = TF-IDF local léxico, zero-dep.**
  O ambiente Python NÃO tem nenhuma lib de ML (sem numpy/sklearn/sentence-
  transformers/torch/chromadb — chromadb nunca foi dep declarada). Escolhi um
  índice **TF-IDF esparso em puro Python** (`recall.py`, só stdlib) sobre embeddings
  neurais porque: (a) **offline-first** de verdade (nada sai da máquina, sem baixar
  modelo), (b) zero-dependência (não infla `uv.lock` nem arrisca supply-chain),
  (c) o boundary rule (ADR 0001) permite computação **local** no Python — só
  proíbe chamar *provedores LLM*/ter keys lá. É recuperação **real** (substitui o
  no-op provado), mas **léxica**, não neural: casa por termos distintivos, não por
  sinônimo/paráfrase. **Honestidade:** um teste (`test_topico_oposto`) inicialmente
  falhou justamente porque "sandbox" e "contêiner/docker" são sinônimos que o
  TF-IDF não liga — reescrevi a ground truth para relevância determinável
  lexicalmente (o teste justo para um retriever léxico) e documentei o limite.
- **[dúvida — para o seu olhar] Léxico é suficiente para "semântico"?** O PLANO
  diz "recuperação semântica". TF-IDF é o teto honesto sem um modelo local
  (embeddings neurais exigiriam bundlar um modelo — conflita com offline/leveza —
  ou passar pelo gateway Rust `CoreService.Generate`, o que viraria uma chamada de
  rede por recall). Entreguei o retriever real e leve; **upgrade para embeddings
  neurais é uma onda/ADR futura** se você quiser semântica de sinônimo. Anotado
  como a decisão do "ADR do embedder do RAG (Onda 6)" que a Onda 9 formaliza.
- **[decisão] O índice vive derivado do corpus persistido (`.forge/squad-memory/
  agent_memories.jsonl`).** O JSONL episódico é a fonte da verdade; o índice
  TF-IDF é reconstruído a cada `recall_similar` (corpus pequeno — dezenas/
  centenas; custo desprezível). Funciona **entre sessões** (o JSONL persiste) e
  dentro da sessão (o `remember_decision` grava na hora). **Futuro:** índice
  materializado/incremental se o corpus crescer muito.
- **[decisão] Fronteira = correção da recuperação, não consumo no orquestrador.**
  A fronteira do PLANO é "o recall recupera exatamente as k relevantes" — provei
  com ground-truth de 2 tópicos disjuntos (igualdade de conjunto) + o corpus
  vazio honesto + o caminho antes-vazio agora recuperando. O `orchestrator.py:107`
  já chamava `recall_similar` e registrava a contagem (`context_recall_count`,
  antes **sempre 0** pelo no-op; agora real). **Deixei o orquestrador intacto**:
  alimentar o contexto recuperado no *planejamento/prompts* é uma decisão de
  raciocínio do squad (como memórias passadas devem influenciar o plano?), fora
  desta fronteira — follow-up scoped, não mexi na lógica delicada de consenso.
- **[decisão] Scaffolding chromadb mantido, inativo.** `remember_decision` ainda
  chama `self.collection.add` (o `_FallbackCollection` no-op) — o recall não
  depende mais dele (lê o JSONL). Não removi o ramo chromadb (é um sink
  alternativo para um futuro vector DB real); documentei que está inativo. Limpeza
  ou fiação a um vector DB de verdade é candidata à Onda 9 ou onda futura.

## Onda 7 — A/B testing via telemetria (critério nº 2)

- **[decisão] O relatório A/B vive no Rust.** É agregação **determinística** sobre
  a telemetria SQLite (Rust-owned: "storage" é Rust pela ADR 0001), não raciocínio
  de agente — mesmo tipo de `summary`/`dashboard`/`verify`. O Python nem acessa a
  telemetria. Novo `forge experiment <nome>` (espelha `dashboard`/`verify`), nova
  consulta `TelemetryStore::experiment_variants` (`json_extract` da extensão JSON1
  do SQLite bundled — `summary` só agrupava por nome), e o tipo/estatística em
  `forge-schemas::experiment`.
- **[decisão] Atribuição por props, sem mudar o storage.** Um evento entra no
  experimento com `props.experiment` + `props.variant` + `props.success` (o
  `record` já aceita `Value` arbitrário — nada a mudar na escrita). A consulta
  agrupa por variante e conta sucessos via `json_extract(props,'$.success')=1`.
- **[decisão] Significância hand-rolled (sem crate de estatística).** O workspace
  não tem `statrs`/`statistical`/`rand_distr` etc. Implementei o **teste z de
  duas proporções** (variância pooled) com CDF normal via aproximação de `erf`
  (Abramowitz-Stegun 7.1.26, |erro| ≤ 1.5e-7) em Rust puro (~15 linhas). Suficiente
  para um p-valor de decisão; precedente de matemática pequena embutida:
  `cache_hit_rate` e `derive_verdict`. **Nota:** o teste de igualdade de p-valor
  usa folga 1e-6 (não 1e-9) porque erf(0) da aproximação ≈ 1e-9, não exato.
- **[decisão] Veredito honesto derivado dos dados (a régua Nada Fake).** Três
  estados: `Significant` (p<α, **com** vencedor = maior taxa), `Inconclusive`
  (amostra ok mas p≥α → **sem vencedor**, "sem significância"), `InsufficientData`
  (< `MIN_SAMPLES`=20 por variante → não conclui). O vencedor **só** existe quando
  Significant — nunca fabricado. Mesma postura de `verification::derive_verdict`.
  Provado ponta-a-ponta: seed real de telemetria → `exp-sig` (90%×50%) dá
  "VENCEDOR A p≈7e-10"; `exp-tie` (50%×52%) dá "SEM SIGNIFICÂNCIA p=0.78".
- **[decisão] `experiment.v1` é Rust-only (sem paridade Python).** Segue o
  precedente `telemetry-event.v1`: schema hand-written + tipo `schemars` + fixture
  golden (valid significativo / invalid sem `verdict`) + teste em
  `schema_fixtures.rs`. Só `prompt-cache-key.v1` exige dupla implementação
  (CLAUDE.md), então `gen_fixtures.py` **não** foi tocado. O ADR de schema novo
  (`experiment.v1`) é formalizado na Onda 9.
- **[decisão/limite] A/B é entre exatamente DUAS variantes.** `forge experiment`
  falha (exit≠0, mensagem clara) se o experimento não tem 2 variantes na
  telemetria. A/B multivariante (>2, com correção de comparações múltiplas) é onda
  futura. A métrica hoje é `success_rate` (taxa de sucesso binária); outras
  métricas (latência P95, custo) são extensão futura.

## Onda 8 — bench criterion + k6 + infra (critério nº 3) — parte 1 (benches)

> Dividi a Onda 8 em duas PRs estratégicas: **8a** (esta) = benches criterion +
> `ScriptedGenerator` (Rust puro, baixo risco); **8b** = endpoint de carga + k6 +
> `infra/` (encanamento de CI, risco isolado). O `ScriptedGenerator` é a fundação
> compartilhada (o "generator sem key" que o k6 também usará).

- **[decisão] `ScriptedGenerator` promovido a tipo público de `forge-llm`.** Antes
  o gerador roteirizado só existia como test double em `#[cfg(test)]`
  (`agent_loop.rs`). Promovi um `ScriptedGenerator::echo(text)` — implementa o
  `Generator` **real**, sem provider, sem key, determinístico e reusável (imutável;
  clona o turno por chamada, então aguenta carga concorrente sem esgotar). É o
  "generator scripted, sem key real" que o PLANO pede para bench e k6.
- **[decisão] 3 benches nos caminhos quentes nomeados pelo PLANO.** `request_hash`
  (hash canônico de cache, ~2.2µs), `estimate_tokens`/`needs_compaction` (épocas de
  contexto, ~300ns), `scripted_generate` (overhead do gateway sem rede, ~390ns).
  `criterion` entrou como dev-dep de workspace; cada crate ganhou um `[[bench]]`
  com `harness = false`. Rodam e produzem baseline (provado local).
- **[decisão] Job `bench` de CI separado, tempos reduzidos.** Roda `cargo bench`
  de verdade (não só compila) com `--measurement-time 1` para provar que os benches
  RODAM sem bit-rot e produzem baseline — **não** crava regressão (não há baseline
  armazenado entre runs; comparação histórica é trabalho futuro, ex.: bencher.dev
  ou o critcmp com artefato). Job separado porque o profile `bench` é caro e não
  deve arriscar o gate `rust` (mesma lógica do `sandbox`).
- **[nota] `criterion` é dep pesada** (traz plotters p/ html_reports), mas só como
  **dev-dependency** — não entra no binário `forge`. Verifiquei que passa no
  `cargo deny` local? — a conferir no CI (job `deny`) como as outras deps pesadas.
  **Resolvido:** passou no `deny` da PR #18 (merge 4edbeb4).
- **[nota] job `bench` do CI falhou na 1ª tentativa** e foi corrigido: `cargo
  bench` sem `--bench` também roda os unittests/testes de integração sob o libtest,
  que rejeita as flags do criterion (`--warm-up-time`). Fix: targetar cada
  `--bench` (commit 1c4a241). Lição para o futuro: bench job sempre com `--bench`
  explícito.

## Onda 8 — parte 2 (k6 + infra)

- **[decisão] Endpoint de carga é um `[[bin]]` do `forge-server` (`loadgen`).** O
  k6 precisa de um alvo HTTP; `forge-server` já tem axum. O bin embrulha o
  `ScriptedGenerator` (sem key) e expõe `POST /generate` + `GET /health`, escutando
  só em `127.0.0.1`. Adicionei `forge-llm` às deps do `forge-server` **só para o
  bin** (a lib do dashboard não usa) — pequeno acoplamento, documentado no
  Cargo.toml. Alternativa (crate dedicado `forge-loadgen`) ficou como opção; o bin
  foi o mínimo que reusa o axum existente.
- **[decisão] O que o k6 mede = overhead do NOSSO lado, não latência de rede.** O
  `ScriptedGenerator` responde in-process sem provider. Então o P95 medido é o
  custo do caminho (axum + agregação + streaming) sob concorrência — que é
  justamente o que se quer garantir (regressão/contenção nossa, ex.: lock do rate
  limiter). **Honesto:** não é o P95 de uma chamada real de LLM (isso dependeria de
  rede/provider); é o P95 do gateway sem a rede. Documentado no script e no
  README. Provado local (hammer concorrente: p95 ~15ms, bem sob o limiar de 100ms).
- **[decisão] Threshold `p(95)<100ms` no script k6.** Generoso para não ser flaky
  num runner de CI, apertado para pegar regressão grosseira. O k6 sai ≠0 se
  estourar — o gate é real (mesma postura anti-decorativo do sandbox/LSP). Job `k6`
  separado no CI: instala k6 (`grafana/setup-k6-action`), sobe o `loadgen`, espera
  o `/health`, roda o script. **Não pude rodar o k6 local** (não instalado); validei
  o endpoint + a viabilidade do threshold com um hammer Python concorrente.
- **[decisão] `infra/` é esqueleto honesto, não terraform decorativo.** O produto é
  local-first (só `127.0.0.1`, sem Dockerfile/cloud), então **não há alvo de deploy
  real**. Entreguei `infra/README.md` (estado honesto), `terraform/main.tf` e
  `ansible/playbook.yml` como **esqueletos marcados** (providers/recursos
  comentados até haver alvo) + o `k6/` que é o único artefato executado. É a "Nota
  honesta" da Onda 8 do PLANO exercida: esqueleto marcado > decoração.
- **[resolvido] critério nº 3 provado no CI:** o job `k6` da PR #19 rodou o k6 de
  verdade — `✓ 'p(95)<100' p(95)=3.51ms`, `✓ 'rate<0.01' rate=0.00%`, 107.837
  requests, 0% falha. Não é número decorativo.

## Onda 9 — fecho (o que fiz e o que fica para o seu olhar)

**Feito (fecho do roadmap):** 4 ADRs novos formalizados — `0011` (skills como
`dyn Tool` + vetting + sandbox, Ondas 1-3), `0012` (confiança MCP, Onda 4), `0013`
(embedder do RAG léxico local, Onda 6), `0014` (`experiment.v1`, Onda 7). README ×
CLAUDE.md × PLANO-PLATAFORMA-FORGE.md × DECISOES.md reconciliados para "6 fases
concluídas — roadmap completo" (grep confirma a mesma história). Contagens
atualizadas (194 Rust + 145 Python). LSP (Onda 5) e benches/k6/infra (Onda 8)
ficaram como prosa no DECISOES/CLAUDE, sem ADR dedicado — são decisões de
implementação/tooling, não de contrato (não mudam schema nem fronteira); se você
preferir um ADR para o LSP ou para o k6/infra, é rápido de adicionar.

**Itens abertos para você analisar (as dúvidas que acumulei):**

1. **[dúvida — a maior] Consenso→ledger (pendência de exercício da Fase 4).**
   Re-declarei no PLANO. O código existe/compila/tem unit test; falta o exercício
   ponta-a-ponta. **Novidade:** pós-Onda 8, com o `ScriptedGenerator` (Rust, sem
   key) + o `ScriptedGatewayClient` (Python já existente), dá para escrever um
   **e2e de `forge squad` roteirizado, SEM key**, que dirige o squad até um evento
   `Consensus` e assere `squad.consensus` no ledger — virando um teste de regressão
   permanente. **Não escrevi** esse e2e (cross-process novo, no último passo, risco
   de flaky no fecho verde). Quer que eu faça numa próxima iteração? É a forma mais
   honesta de fechar a pendência sem key.
2. **[dúvida] RAG léxico é suficiente?** Onda 6 entregou TF-IDF léxico (real, zero-
   dep, offline). Não faz ponte de sinônimo (isso é neural). Se você quer semântica
   de verdade, é uma onda futura (modelo local ou gateway Rust). Documentado no ADR
   0013.
3. **[dúvida/defer] Frontends não ligados:** MCP (`/api/mcp`) e LSP não têm UI; A/B
   não tem tela. São tools/CLI hoje. Wiring de frontend espelha o que fiz no
   `/api/skills` — deixei para não inflar as PRs.
4. **[dúvida] Double-vet no ledger (`skill.vetting`)** e consumo do recall no
   planejamento do squad: follow-ups registrados nas seções das Ondas 3 e 6 acima.

Nada aqui bloqueia declarar o roadmap concluído — são refinamentos e um exercício,
não lacunas de código.

# Fase 7 — frontend como forma primária de uso (Ondas 1-2)

## Onda 2 — remanescente (matriz de permissão persistida + trilha de auditoria)

- **[decisão] Onda 2 não estava fechada quando cheguei nela.** A PR #25 (mergeada)
  entregou a sessão real via SSE + a ponte de permissão ao vivo, mas deixou de fora
  o resto do escopo da própria Onda 2 do PLANO: matriz build/plan×tool persistida,
  trilha de auditoria no ledger, o terceiro estado "sempre" do `Permissao.tsx`, e o
  fallback silencioso do `fetchSkills`. Fechei esse resto nesta entrega antes de
  seguir para a Onda 3 — a Onda 7 (Console MCP) depende explicitamente do MESMO
  `RuleStore` para o preview de política, então deixar essa base pela metade
  quebraria uma onda futura, não só um item isolado.
- **[decisão] `RuleStore` mora em `forge-store`, mas com um `RuleDecision` PRÓPRIO**
  (não o `Decision` de `forge-core`). `forge-core` já depende de `forge-store`
  (aresta pré-existente) — se `forge-store` dependesse de `forge-core` de volta,
  seria um ciclo. A conversão `RuleRecord → forge_core::Rule` mora em `forge-cli`,
  que já depende dos dois.
- **[decisão] `PermissionEngine::overlay` combina overrides persistidos com o
  default do perfil (`BUILD`/`PLAN`), overrides checados primeiro.** Achei e
  documentei uma inconsistência real entre o mock antigo e o perfil de verdade: o
  mock (`PERMISSION_MATRIX` em `skills.ts`) mostrava `plan`+`bash` como "deny"; o
  perfil real (`PermissionEngine::read_only()`) é "ask". A matriz nova reflete o
  perfil REAL (fonte única em `forge_core::{BUILD,PLAN}`), não o valor fabricado —
  a UI muda de "deny" para "ask" nessa célula, o que é uma correção, não uma
  regressão.
- **[decisão] "sempre" grava um override com `scope_prefix` = o escopo EXATO do
  pedido pendente, não um "allow" genérico para o tool inteiro.** A matriz (tela
  Skills) já cobre o caso "qualquer escopo"; misturar os dois mecanismos no mesmo
  botão pareceu mais confuso que dois botões com contratos distintos e explícitos.
  Se você preferir que "sempre" também ofereça um modo "qualquer escopo deste
  tool", é uma extensão pequena (chamar `setRule` sem `scopePrefix`).
- **[decisão] Modal de confirmação nos cliques da matriz.** O PLANO pede
  explicitamente "o escopo da rule (tool + scope_prefix) aparece explícito no
  modal antes de confirmar — nunca um clique único e opaco". Isso muda a UX
  anterior (clique único cicla a decisão) para clique → modal → confirmar. Achei
  que vale a pena dado que é a mutação mais sensível do plano (afrouxar permissão
  pelo navegador), mas é uma fricção a mais que você pode preferir remover se achar
  exagero para a matriz coarse (a "sempre" já mostra o escopo no PRÓPRIO modal do
  pedido de permissão, sem precisar de um segundo modal).
- **[dúvida] Cobertura Playwright da Onda 2 original (PR #25) ficou incompleta.**
  A fronteira do PLANO para a Onda 2 pede 4 testes Playwright: (1) pedido de
  `bash` real → tela Permissão → Permitir → ledger íntegro; (2) duas abas
  concorrentes → 409 claro; (3) editar matriz → ledger → revogar; (4) skills fora
  do ar → erro explícito. A PR #25 só entregou (1) e (2) como testes Rust
  (`reqwest` contra um servidor axum efêmero), não como Playwright de navegador —
  prova real, mas não a MESMA prova que o PLANO pede. Nesta entrega eu fechei (3) e
  (4) como Playwright de verdade (`web/tests/e2e-integration/permissions-real-backend.spec.ts`,
  rodando contra `--web-agent` real). **Não voltei** para escrever Playwright de
  navegador para (1)/(2) — é trabalho já revisado e mergeado, e reabri-lo pareceu
  fora do escopo desta entrega (que é fechar a Onda 2, não auditar a cobertura de
  teste da Onda 1). Se você quiser essa cobertura de navegador para (1)/(2)
  também, é um item pontual e localizado — me avise.
- **[nota] `web/scripts/run-integration-server.mjs` ganhou `--web-agent`** (a
  suíte de integração de telemetria e a nova de permissões agora sobem o MESMO
  servidor com o agente web ligado; puramente aditivo, não mudei nenhuma rota
  existente).

## Onda 3 — sidecar Python como serviço de longa duração

Sem pendência nova aqui — o desenho (singleton para PromptForge, pool pequeno
para squad, restart-on-crash via health-check) está integralmente registrado no
ADR 0019, sem decisão em aberto que precisasse deste arquivo.

## Onda 4 — squad ao vivo

- **[dúvida] Capacidade 1 do `SquadPool` no agente web bloqueia silenciosamente
  uma segunda tarefa concorrente, sem aviso claro na UI.** `SquadTask`/
  `PermissionRequest` não carregam identificador de tarefa no proto atual —
  rodar >1 squad concorrente pelo mesmo `CoreService` compartilhado não teria
  como demultiplexar de qual tarefa uma chamada `Generate`/`RequestPermission`
  veio (comentário de módulo em `squad_agent.rs`). Capacidade 1 evita fingir uma
  concorrência insegura, mas tem um efeito colateral real: `POST /api/squad/run`
  sempre devolve `202` com um `task_id` novo na hora (não enfileira a
  aceitação), mas a tarefa em si fica presa em `pool.acquire()` até o slot
  único liberar — quem abrir uma segunda aba durante uma execução vê a tela
  "task_id sqN · ao vivo" sem NENHUMA proposta aparecer por um tempo
  indeterminado, sem indicação de "na fila". Resolver de verdade (correlação de
  tarefa no proto + `core_socket` por slot) é escopo maior, fora desta onda. Se
  quiser, o remendo barato para a próxima iteração é a UI mostrar "aguardando
  slot livre" quando nenhum evento chegar depois de N segundos do `202`.
- **[decisão] Frontend fecha o `EventSource` no primeiro `onerror`, não deixa o
  navegador reconectar sozinho.** Diferente de `connectSessionEvents` (sessão de
  chat, vida útil da aba inteira, reconectar faz sentido), uma tarefa de squad é
  finita — o stream termina sozinho quando a tarefa acaba. Sem fechar
  explicitamente, o `EventSource` nativo reconectaria para sempre contra uma
  tarefa já terminada (replay do snapshot a cada retry, na prática inofensivo
  mas um loop sem propósito). Não dá para distinguir, pela API do
  `EventSource`, "terminou de verdade" de "conexão caiu de verdade" — tratei os
  dois igual (rótulo neutro "stream encerrado"), em vez de arriscar uma
  mensagem "concluído com sucesso" que a API não sustenta.
- **[nota] Preview do `content_json` de cada proposta é JSON bruto
  reformatado (`JSON.stringify(JSON.parse(...), null, 2)`), não campos
  estruturados por tipo de agente.** O schema varia por agente (architect vs.
  developer vs. auditor vs. designer vs. ops) — parsear campos específicos
  seria uma tela por agente. Achei prematuro para esta onda; se quiser um
  resumo mais rico por agente (ex.: só `recommendation`/`architecture` do
  architect), é uma extensão pontual depois que os campos reais de cada agente
  estiverem estáveis.
- **[nota] Achado de depuração, já resolvido — registrado para não confundir
  quem olhar o histórico:** durante o desenvolvimento desta onda, o processo
  bugado do stream SSE que nunca terminava (`SquadTaskState.tx` sem
  `Option`/`finish_task`) foi corrigido; junto com isso, encontrei dezenas de
  processos `uv`/`forge_squad.server` órfãos rodando no ambiente. Investiguei a
  fundo (reprodução isolada, fora do workspace) antes de concluir: o mecanismo
  de limpeza (`Drop` com `libc::kill` no grupo de processos, ADR 0019) funciona
  corretamente — confirmado com um processo `uv run python -m
  forge_squad.server` real, dentro de uma task `axum::serve` detached, dropado
  ao fim de um `#[tokio::test]`: tanto o `uv` quanto o `python` forkado morrem.
  Os órfãos encontrados eram resíduo de tentativas ANTERIORES desta mesma
  sessão de depuração, quando o teste ainda travava e precisou ser morto via
  `timeout ... ` (SIGKILL no processo Rust, que pula TODOS os destructors,
  inclusive o `Drop` que mata o grupo) — não um bug novo, não uma lacuna no
  desenho da Onda 3.

## Onda 5 — Prompts (CRUD + render)

- **[decisão] Branch criada a partir do `main` pós-Onda-3, não empilhada sobre a
  Onda 4 (squad ao vivo).** A Onda 5 só depende de Onda 1 (fundação, já mergeada)
  e Onda 3 (sidecar-serviço, já mergeada) — a CI da Onda 4 ainda não tinha fechado
  quando comecei esta entrega. Consequência real, já resolvida: quando a Onda 4
  mergeou primeiro, o rebase desta branch sobre o `main` novo deu conflito em
  8 arquivos (`main.rs`, `squad.rs`, `web_agent.rs`, `forge-proto/{Cargo.toml,
  build.rs}`, `forge-sidecar/supervisor.rs`, `domain.ts`, este arquivo) — todos
  mecânicos (as duas ondas tocaram as mesmas linhas de formas complementares,
  não conflitantes de verdade): `run_dashboard` passou a mesclar
  `squad_router.merge(prompt_router)` num só `extra: Router`; `supervisor.rs`
  ficou com as DUAS correções (o `Drop`/group-kill da Onda 4 + o
  `create_dir_all`/captura de stderr desta onda, que já convergiam pro mesmo
  código); `domain.ts` perdeu os tipos das duas telas. Nenhuma perda de
  funcionalidade das duas ondas — só reconciliação de texto/assinatura.
- **[achado real, corrigido] `SidecarSupervisor::spawn` nunca criava o
  diretório-pai do socket** (`crates/forge-sidecar/src/supervisor.rs`) — mesma
  classe de bug já achada e corrigida para o `SquadSupervisor` (Onda 4, branch
  separada, ainda não mergeada): sem isso, o bind gRPC do lado Python falhava
  com "No such file or directory" sempre que o diretório do socket (`.forge/`)
  ainda não existisse — o que é exatamente o caso normal de um workspace novo
  ou de um `SidecarService` construído contra um diretório de teste. Descoberto
  ao escrever o teste de paridade HTTP↔gRPC desta onda (o teste existente de
  `SidecarService` usa um socket solto em `/tmp`, não sob um `.forge/` recém-criado,
  então nunca expôs o gap). Corrigido na raiz (`create_dir_all` no `spawn`,
  igual ao `SquadSupervisor`) — quando a Onda 4 mergear, as duas correções
  devem ser idênticas/triviais de reconciliar. Também aproveitei para dar ao
  `SidecarSupervisor::wait_ready` a mesma captura de stderr no erro que o
  `SquadSupervisor` já tinha (o doc-comment já prometia isso, o código não
  cumpria — inconsistência real, agora corrigida).
- **[decisão] `PromptLibrary` não ganhou um wrapper `Arc<Mutex<_>>` DENTRO de
  `forge-store`** (ao contrário do que o próprio PLANO sugere, espelhando
  `Telemetry`). Em vez disso, `forge-server::AppState` guarda
  `Arc<Mutex<PromptLibrary>>` diretamente — mesmo efeito prático (aberta uma
  vez, compartilhada entre requisições, sem reabrir conexão por chamada), sem
  precisar mexer no tipo público de `forge-store` nem nos usos existentes do
  CLI (`/prompt` do chat REPL, que usa `PromptLibrary` direto). Se você preferir
  o wrapper dentro de `forge-store` por consistência com `Telemetry`, é um
  refactor pequeno e localizado.
- **[decisão] Guarda de `Origin`/`Host` duplicada em `forge-server`.** Como
  `forge-server` ganhou aqui suas primeiras rotas mutáveis (`POST`/`DELETE
  /api/prompts*`), preciso da mesma proteção de CSRF/DNS-rebinding que
  `forge-cli`'s `web_agent.rs` já aplica no router mesclado — mas não posso
  importá-la de lá (direção de dependência é a oposta: `forge-cli` depende de
  `forge-server`, não o contrário). Dupliquei a função (idêntica,
  `require_local_origin`/`is_local_origin`) em vez de tentar uma extração
  cross-crate nesta entrega — se isso incomodar, dá para mover as duas para um
  crate `forge-web-guard` minúsculo depois.
- **[decisão] Render não instrumenta `fields` com validação de campos
  obrigatórios no lado Rust.** `GeneratorField.required` vem do sidecar
  (`GET /api/prompt/generators`) e a UI marca campos obrigatórios com `*`, mas
  a rota `POST /api/prompt/render` não valida no Rust antes de chamar o
  sidecar — se faltar um campo que o gerador precisa, o erro vem do lado
  Python (RPC falha, vira `502` com a mensagem do gRPC). Aceitável: o sidecar
  já é a fonte de verdade de quais campos cada gerador realmente usa
  (templates podem mudar sem o Rust saber), duplicar a validação seria uma
  segunda fonte de verdade discordante.
- **[nota] Sem teste Playwright de navegador para esta onda — de propósito,
  não por omissão.** A fronteira do PLANO para a Onda 5 pede especificamente
  "teste HTTP direto" (CRUD) e "paridade com chamada gRPC direta" (render), ao
  contrário da Onda 4 que pedia Playwright explicitamente. Os dois testes
  Rust novos (`forge-server`'s CRUD e `forge-cli::prompt_render`'s paridade)
  cobrem exatamente isso. Se quiser cobertura de navegador também, é aditivo.
- **[nota] `PromptGenerator`/`SavedPrompt` saíram de `types/domain.ts`** para
  `api/prompts.ts` — mesmo padrão já usado para os tipos do squad (Onda 4):
  tipos específicos de uma tela moram no módulo de API dela, não no arquivo de
  domínio compartilhado.
- **[decisão] Botão "fav ★" do painel lateral (que operava sobre "o gerador
  ativo", resolvendo indiretamente para uma entrada salva) foi removido.** Com
  ids reais (antes eram strings fabricadas no mock), favoritar por nome de
  gerador é ambíguo se houver mais de uma entrada salva com o mesmo gerador —
  o botão "fav" por entrada na lista da biblioteca (que já existia, operando
  sobre um id direto) é o único caminho agora, sem ambiguidade.

## Onda 6 — Ledger (leitura paginada + filtro ator)

- **[achado real, corrigido] `LedgerStore::open` não ligava WAL**, ao contrário
  de `EventStore::open`/`RuleStore::open` (que já ligam). Era um bug de
  concorrência latente já **nomeado** no próprio código antes desta onda — o
  comentário de `RuleStore::open` já dizia "ao contrário do `LedgerStore`
  legado (bug conhecido, fechado só na Onda 6)". CLI (`forge run`/`chat`/
  `squad`) e o dashboard web tocam `.forge/forge.db` ao mesmo tempo; sem WAL,
  bastava uma escrita do CLI coincidir com uma leitura do dashboard para dar
  "database is locked". Corrigido com o mesmo `pragma_update(None,
  "journal_mode", "WAL")` que os outros dois stores já usam; teste dedicado
  abre um arquivo real (não `open_in_memory`, que não suporta WAL) e confirma
  `PRAGMA journal_mode` = `"wal"`.
- **[decisão] `LedgerStore::recent(limit, actor)` filtra por `actor` dentro do
  MESMO `WHERE`/`ORDER BY`/`LIMIT` do SQL, não em Rust depois.** `actor` não é
  coluna própria (mora dentro do `body` JSON), então usei
  `json_extract(body, '$.actor') = ?2` combinado com `ORDER BY seq DESC LIMIT
  ?1` numa única query — mesmo padrão de paginação de `TelemetryStore::recent`.
  Se filtrasse só depois de truncar para as N mais recentes, um ator raro que
  não estivesse entre as últimas N apareceria como "sem entradas" mesmo tendo
  histórico de verdade. Teste dedicado prova isso: semeia 1 entrada de um ator
  raro, depois 5 de outro ator, confirma que um `LIMIT 3` sem filtro NÃO veria
  o raro, mas o mesmo `LIMIT 3` COM filtro o encontra.
- **[decisão] `POST /api/ledger/verify` sempre devolve HTTP 200; `ok:false` no
  corpo sinaliza cadeia corrompida, não um status de erro.** A requisição em
  si teve sucesso — o que ela relata é que o *dado* está adulterado. Mantém o
  contrato que o mock antigo já modelava (`{ok, verified}`) e evita que o
  frontend precise distinguir "erro de rede/servidor" de "corrupção
  detectada" só pelo status code.
- **[decisão] `serve_with_agent` (forge-cli/web_agent.rs) ganhou
  `#[allow(clippy::too_many_arguments)]`.** Já estava em 7 argumentos (limite
  do clippy); adicionar o handle do ledger foi para 8. É uma função só de
  encaminhamento (empacota os handles que `main.rs` mantém abertos + compõe os
  routers) — não tem lógica que uma struct de agrupamento tornaria mais clara,
  e o padrão do projeto não introduz abstração além do que a tarefa pede. Se
  mais um handle for somado numa onda futura (ex.: Onda 7 MCP), vale
  reconsiderar uma struct `DashboardHandles` compartilhada entre
  `forge-server`/`forge-cli` neste ponto.
- **[decisão] `LedgerEntry` do frontend (`types/domain.ts`) foi reescrita para
  o formato real da wire** (`seq, prev_hash, entry_hash, kind, actor, payload,
  override?, fake_marker?, ts`) — o mock antigo tinha campos fabricados que não
  existem no backend (`actorColor`, `action`, `hashPrev`/`hashCurr` truncados
  a 4 chars, `flag`). `actorColor` e o hash truncado para exibição viraram
  derivações no CLIENTE (`actorColor()`/`shortHash()` em `Ledger.tsx`), nunca
  campos da wire — `actorColor` é um heurístico por prefixo real de `actor`
  (`web:*` → override feito pelo navegador, `forge-cli:*` → sessão de CLI/TUI/
  squad, resto → terceiro tom), não uma cor arbitrária por sessão de teste
  como o mock antigo tinha.
- **[decisão] Filtro por ator na tela refaz a busca no backend a cada troca de
  botão, não corta a lista já carregada no cliente.** Consequência direta do
  ponto acima sobre `LedgerStore::recent`: se a tela cortasse client-side, o
  mesmo problema (ator raro fora da janela recente "sumindo") reapareceria na
  UI mesmo com o backend correto. A lista de botões de ator (`actors`) só é
  re-derivada da busca SEM filtro, para não perder os outros botões quando um
  filtro específico está ativo.
- **[decisão] Banner de integridade não afirma "cadeia íntegra" por padrão —
  só depois que o usuário clica em "verificar integridade" e a resposta real
  chega.** O mock antigo mostrava a claim fixa mesmo sem nunca ter chamado
  `verifyChain()`. Antes do primeiro clique, o banner mostra só a contagem de
  entradas carregadas + "integridade ainda não verificada nesta sessão" —
  consistente com a régua "Nada Fake" (não reivindicar um veredito que ainda
  não rodou).
- **[nota] Novo exemplo `forge-store/examples/seed_ledger.rs`** (mirror de
  `seed_telemetry.rs`, já existente) — semeia entradas reais via
  `LedgerStore::append` para o e2e de integração poder provar a fronteira
  "tela mostra o que foi gravado por fora do browser" sem SQL cru. Usado por
  `web/scripts/run-integration-server.mjs` (2 chamadas, formando uma cadeia
  real de 2 entradas) e pelo novo `ledger-real-backend.spec.ts`.
- **[decisão] O teste Playwright novo filtra por um ator dedicado
  (`e2e-ledger-seed`) que nenhum outro spec desta suíte usa.** Squad e
  permissões também escrevem no MESMO `.forge/forge.db` compartilhado pelo
  `webServer` único da config de integração (`fullyParallel: false`) — sem um
  ator exclusivo, a asserção de contagem exata de linhas ficaria dependente da
  ordem de execução dos arquivos de teste. Com o filtro, a asserção
  (exatamente 2 linhas, hash/seq batendo) é robusta independente de quantas
  outras entradas os demais specs acumularem no mesmo arquivo.

## Onda 7 — Console MCP (A1) + Uso por modelo (A5)

- **[decisão] `CARGO_BIN_EXE_forge_mcp_fixture` não está disponível no teste de
  `forge-cli`** (confirmado empiricamente — o cargo só expõe essa env var para
  o PRÓPRIO pacote que declara o `[[bin]]`, não para pacotes dependentes). O
  teste de `mcp_console.rs` builda o fixture explicitamente
  (`cargo build -p forge-tools --bin forge_mcp_fixture`) e localiza o binário
  no `target/` compartilhado do workspace via `CARGO_MANIFEST_DIR/../../target/
  debug/forge_mcp_fixture` — mesmo idioma já usado em vários testes do repo
  para alcançar a raiz do workspace a partir de qualquer crate (`sidecar.rs`,
  `skills.rs`, `parity.rs`, etc.), só que aplicado a um binário em vez de um
  diretório. Custo extra: ~25s na primeira execução (compila as deps do
  `rmcp` server), ~2s depois (cache do cargo).
- **[decisão] `read_mcp_server_configs` extraído de `skills.rs::load_mcp_servers`
  para reuso.** Antes, o parsing de `.forge/mcp.toml` só existia embutido
  dentro de `load_mcp_servers` (que já registra as tools no `ToolRegistry`).
  O console MCP só precisa ENUMERAR os servidores declarados (para sondar e
  exibir, sem registrar nada) — extraí a leitura pura (`pub(crate) fn
  read_mcp_server_configs`), `load_mcp_servers` agora só itera sobre o que ela
  devolve. Mesmo padrão de parsing de `load_lsp_servers` (structs locais
  `McpConfigFile`/`ServerEntry`) permanece intocado, só ganhou um dono
  compartilhado.
- **[decisão] Preview de política do console MCP usa `web_agent::
  load_rule_overrides` (agora `pub(crate)`), não os perfis const puros.**
  `AgentProfile::BUILD`/`PLAN` não têm regra nenhuma para `mcp__*` — sem
  consultar o MESMO store de `Rule` que a Onda 2 já usa para a matriz de
  permissões, o preview seria sempre "ask", nunca refletindo uma decisão real
  do usuário. Mostra as DUAS colunas (build/plan), mesmo padrão visual da
  matriz existente em Skills.tsx.
- **[decisão] Probe de servidor MCP roda em `spawn_blocking` + `tokio::time::
  timeout` de 5s.** `list_tools_blocking` já usa uma thread+runtime própria
  internamente (ponte sync→async do `forge-tools::mcp`) — chamável de dentro
  de um handler async só via `spawn_blocking` para não bloquear o executor.
  Um servidor lento/travado vira "offline" com mensagem explícita após 5s, em
  vez de travar o dashboard. Caveat aceito, não corrigido nesta onda: se o
  timeout estoura, a thread bloqueada dentro de `list_tools_blocking` não é
  cancelada (rmcp/threads não são cancel-safe) — pode vazar uma thread por
  probe malsucedido. Risco pré-existente da própria `list_tools_blocking`
  (usada assim antes desta onda também), não introduzido aqui; registrado
  para não ser esquecido caso vire um problema de verdade em produção.
- **[decisão] `ModelUsage`/`ModelTier` não viram uma segunda fonte de verdade
  entre `forge-store` e `forge-llm`.** `forge-store::TelemetryStore::
  model_usage()` só agrega contagens brutas por `props.model` (sem saber o
  que é um "tier") — `forge-server` (que já depende de `forge-llm`, hoje só
  para o bin `loadgen`) é quem chama `tier_from_id` para anexar a coluna
  `tier` na resposta HTTP. `forge-store` continua sem depender de
  `forge-llm`.
- **[decisão] Rota `GET /api/models/usage` foi ao ar sem instrumentar produção
  além do que já existia.** `RateLimitedGenerator`/`CachedGenerator` já
  gravavam `props.model` em `llm.call`/`cache.hit`/`cache.miss` desde antes
  desta fase — a onda só soma uma consulta nova (`model_usage`) sobre dados
  já reais, não fabrica nem precisa semear nada de novo em produção.
- **[decisão] Skills.tsx perdeu o card mock "Servidores MCP"
  (`MCP_SERVERS`/`reconnectMcp` de `skills.ts`) e o título/kicker da tela
  caiu de "Skills, MCP & permissões" para "Skills & permissões"** (MCP tem
  tela própria agora) — consequência: o teste Playwright pré-existente
  `permissions-real-backend.spec.ts` que checava o heading antigo foi
  atualizado para o texto novo (mudança mecânica, mesma tela, só o título).
- **[decisão] Teste Playwright do console MCP revoga o override que cria, no
  `finally`.** `rules.db` é compartilhado por todo o `webServer` da suíte de
  integração — `permissions-real-backend.spec.ts` assume a lista de overrides
  vazia no início do seu próprio teste. Sem a limpeza, a ordem de execução dos
  specs (não determinística entre os 2 workers do Playwright) faria esse
  teste falhar de forma intermitente. Mesmo motivo pelo qual o seed de
  telemetria do A5 usa um `session_id` dedicado (`e2e-model-usage`, não
  `e2e-integration`) — evita inflar a contagem que o teste de telemetria já
  soma.
- **[nota] Ambas as telas novas (`mcp`, `modelos`) ganharam Playwright real-
  backend** (ao contrário da Onda 5, que documentou explicitamente pular
  Playwright) — decisão de proporcionalidade: são telas 100% novas e
  read-only, e o custo de semear um servidor MCP fixture real + eventos de
  telemetria com `model` já estava pago pelos testes Rust; estender ao
  browser foi incremental.

## Onda 8 — Mapa de memória do squad + busca RAG (A3, ADR 0022)

- **[decisão] `MemoryService` novo, não `CoreService.Recall/Remember`.**
  Confirmado no código antes de implementar: `core_server.rs`'s
  `recall`/`remember` são `Status::unimplemented("... memória é local ao
  Python no orquestrador atual")` — stub abandonado da Fase 4, direção
  errada (`CoreService` é servido pelo Rust, chamado pelo Python; memória
  precisa do oposto). Detalhes completos e alternativas descartadas
  (estender `SquadService`) em ADR 0022 — não repetido aqui.
- **[achado real, corrigido] o próprio handoff de design erra a direção do
  RPC** (cita "CoreService.Recall" na cópia de carregamento do protótipo) —
  a implementação e a cópia da UI real usam `MemoryService.Recall`, não
  repetem o engano.
- **[decisão] supervisão singleton (`MemoryService`, mirror de
  `SidecarService`), não `SquadPool`.** `Recall`/`List` são leituras
  stateless — um pool misturaria disputa de recurso entre leitura de
  memória e execução real de squad à toa. Ver ADR 0022 para o raciocínio
  completo.
- **[achado real, corrigido durante a implementação] `MemorySupervisor`
  precisa concordar com `SquadServicer` sobre ONDE o corpus mora.**
  `forge_squad.server`'s `SquadServicer` (quem de fato ESCREVE memória via
  `remember_decision`) nunca recebe `--memory-dir` — cai no default de
  `AgentMemorySystem()` (`.forge/squad-memory` relativo ao `current_dir` do
  processo, que é o `python_workspace_dir`). Meu primeiro rascunho do teste
  de `MemoryService` tentou contornar isso com uma env var fictícia que o
  supervisor não lia — não funcionava (o processo Python simplesmente
  ignorava). Corrigido adicionando `memory_dir: Option<&Path>` de verdade a
  `MemorySupervisor::spawn`/`MemoryService::new`, propagado como
  `--memory-dir` (flag que `memory_server.py` já aceitava) só quando
  `Some` — produção passa `None` (mesma resolução relativa do squad real,
  documentado no ADR), testes passam `Some(dir)` para um corpus isolado.
- **[nota] `list_memories(agent?, limit)` (`memory.py`) é só filtro +
  ordenação + corte sobre `_load_corpus()` já existente** — zero lógica de
  indexação nova, como planejado. O agrupamento por agente (contagem,
  decisão mais recente, maior confiança) mora em `memory_server.py`'s `List`
  RPC (não em `memory.py`), já que é apresentação da resposta gRPC, não uma
  capacidade do núcleo de memória em si.
- **[decisão] descope explícito de `forgetting.py`** (código morto,
  confirmado por grep — só o teste unitário dele chama) — o mapa de memória
  não tem coluna de tendência de esquecimento; só campos que o código
  realmente calcula. Ver ADR 0022.
- **[decisão] tela registrada como admin (`memoria`), não user**, mesmo
  categoria das outras 2 telas do Grupo A desta fase (`mcp`, `modelos`) —
  consistência com o agrupamento original do levantamento, não uma
  reclassificação individual.
- **[decisão] cartão "Mapa de memória do squad" de `Sugestoes.tsx` foi
  retargetado** (`relatedScreen: 'squad'` → `'memoria'`) e marcado "✓
  entregue" (badge nova, `delivered?: boolean` no array de propostas — não
  existia nenhuma tela ainda marcada assim). Âncora do cartão trocada de
  `forge_squad.memory / forgetting` para `forge_squad.memory + recall.py
  (TF-IDF)` — `forgetting.py` nunca foi o que ficou de pé.
- **[nota] `CARGO_BIN_EXE_forge_mcp_fixture` (Onda 7) não é o único caso de
  cross-crate fixture nesta fase — `memory_server.py` tem o equivalente
  Python:** os testes de `memory_client.rs`/`service.rs`/`memory_console.rs`
  usam `uv run` real (mesmo padrão já usado por `SidecarService`/`SquadPool`
  desde a Onda 3), não precisam de nenhum workaround — só o caso Rust→Rust
  cross-crate (Onda 7) tinha esse problema específico.
- **[decisão] seed do corpus de memória para o Playwright grava DIRETO em
  `python/.forge/squad-memory/agent_memories.jsonl`** (o caminho real que
  `MemoryService`/`SquadServicer` usam em produção, dado `memory_dir: None`)
  — não um diretório efêmero. Ator dedicado (`e2e-memory-agent`) evita
  colisão; **sem cleanup no fim da suíte** (tentei via `rmSync` no handler
  de saída do processo, mas a limpeza não é confiável — o `cargo run` filho
  pode não terminar a tempo do sinal de saída do próprio script Node
  chegar). Aceitável: esse arquivo JÁ acumula dados reais entre execuções
  hoje (o teste do squad real chama um orquestrador de verdade, que grava
  via `remember_decision` no MESMO arquivo) — `.forge/` é gitignored, e
  `writeFileSync` (não `appendFileSync`) no seed é idempotente por execução,
  o que basta para a asserção (que procura só o agente dedicado, nunca uma
  contagem global).

## Onda 9 — Experimentos A/B (A2)

- **[decisão] `GET /api/experiment/:nome` foi direto em `forge-server`, não no
  router mesclado de `forge-cli`.** Mesma classe de posicionamento que A5
  (Uso por modelo): só precisa do que `forge-server` já depende
  (`forge-store` para `experiment_variants`, `forge-schemas` para
  `ExperimentReport`/`VariantStats`) — nenhuma dependência de
  `forge-tools`/`forge-core`/`forge-sidecar`. `forge-schemas` virou
  dependência REAL do crate (antes só `dev-dependencies`, usada só para
  montar `LedgerEntry` nos testes) — o handler constrói `VariantStats`/
  `ExperimentReport` em código de produção, não só em teste.
- **[decisão] 404 vs 422 — dois jeitos distintos de "não dá pra responder",
  não um genérico.** `experiment_variants` devolve `Vec<(variante, n,
  sucessos)>`; `0` variantes (nome sem nenhum evento) é `404` (o experimento
  não existe); `1` ou `3+` variantes é `422` (o experimento existe — tem
  eventos reais — mas não está no formato de A/B estrito de 2 lados que
  `ExperimentReport::from_two_variants` exige). A wording do plano ("422 se
  alguma variante tem 0 amostras") não é literalmente alcançável: o `GROUP BY`
  da consulta só devolve grupos que already têm ≥1 linha, então uma variante
  com `n=0` não pode aparecer no resultado — reli o `Fronteira` (a fonte mais
  concreta: "seed com 1 variante só → 422; nome inexistente → 404") e segui
  essa leitura operacional, não a prosa da onda.
- **[decisão] `>2` variantes cai no mesmo `422` que `1` variante.** Não
  testado explicitamente no fronteira do plano, mas é a extensão óbvia: um
  A/B estrito (`from_two_variants`) não sabe o que fazer com 3+ lados, e
  escolher 2 arbitrariamente para "salvar" a resposta seria fabricar um
  recorte que o usuário não pediu — a régua Nada Fake também vale para
  "silenciosamente ignorar dado real".
- **[decisão] Nenhuma instrumentação de produção nesta onda — decisão
  explícita, não esquecimento.** Confirmado de novo (grep) antes de
  implementar: nenhum caminho de produção grava `props.experiment`/`variant`/
  `success` hoje, só `examples/seed_telemetry.rs` e os testes. A tela carrega
  um banner permanente dizendo isso — os relatórios que ela mostra hoje são
  sempre sobre dados semeados, nunca tráfego real, até uma fase futura
  instrumentar de verdade um ponto de decisão (ex.: variantes de prompt do
  PromptForge, squad vs. agente único).
- **[decisão] `Sugestoes.tsx`: card "A/B de prompts" retargetado para
  `experimentos`, `delivered: true`.** Existia desde antes desta fase
  apontando para `relatedScreen: 'prompts'` (a biblioteca de prompts, que não
  tem nada de A/B) — claramente um placeholder que citava o módulo errado
  (`forge_promptforge.hashing`, sem relação com `experiment.v1`). Corrigido
  para apontar pro módulo real (`forge_schemas::experiment`, ADR 0014) e
  marcado entregue, mesmo padrão visual (`Badge` "✓ entregue") que a Onda 8
  já usou pro card de memória.
- **[nota] Onda 9 foi implementada e commitada numa branch nova
  (`claude/fase-7-onda-9`) a partir de `origin/main` recém-atualizado — NÃO
  em cima da branch da Onda 8 (`claude/fase-7-onda-8`, PR #32 ainda aberta no
  momento em que esta onda começou).** Cheguei a escrever as mudanças desta
  onda em cima do checkout local da Onda 8 por hábito (a mesma armadilha já
  registrada duas vezes nas notas da Fase 7 anterior) — pego ANTES de
  commitar desta vez, via `git stash` + `checkout -b` a partir de
  `origin/main` + `stash pop` + resolução manual dos conflitos nos arquivos
  de registro de tela compartilhados (`nav.ts`/`screenMeta.ts`/
  `screenComponents.tsx`/`Shell.tsx`/`types/domain.ts`/`Sugestoes.tsx`/
  `run-integration-server.mjs`), mantendo só os trechos da Onda 9 e
  descartando os da Onda 8 (que voltam sozinhos quando a Onda 8 mergear e
  esta branch rebasear). Zero dependência funcional real entre as duas
  ondas — só compartilhavam linhas nos mesmos arquivos de registro.
- **[nota] Seed de experimento no Playwright: 40 chamadas a
  `seed_telemetry`, não uma flag de lote nova.** `MIN_SAMPLES = 20` em
  `forge_schemas::experiment` é um piso real (abaixo dele o veredito vira
  `InsufficientData`), então provar `Significant` por execução exige ≥20
  eventos por variante — sem atalho. Preferi 40 invocações simples do
  exemplo genérico já existente (mesmo padrão de todo o resto do script) a
  inventar um modo de lote no `seed_telemetry.rs` só para este caso; custo
  medido: suíte inteira (9 specs, incluindo o build do fixture MCP) ainda
  roda em ~30s.

## Onda 10 — Rate limits (A4) + Sandbox & skills de terceiro (A6) + Language servers (A7)

- **[decisão] DTO de `/api/ratelimit` é `{tier,cap,window_secs}`, sem
  "models".** O texto original da onda no plano-mestre sugeria um campo
  "models" no DTO — mas `ModelTier` classifica por REGEX
  (`forge_llm::model_tier::rules()`), não por uma lista enumerável de ids.
  Inventar uma lista de exemplo (ex.: "haiku, gpt-4o-mini") pra preencher
  "models" seria fabricar dado que o backend não tem — a régua Nada Fake
  também cobre "completar um campo do DTO com algo plausível". Removido do
  DTO; a tela mostra só tier/cap/window, que são 100% reais.
- **[achado real, mais fundamental que só o efeito colateral do `poll()`]
  `/api/ratelimit` não pode mostrar USO ao vivo em NENHUMA hipótese —
  não é só que `poll()` muta ao checar.** O `forge dashboard` é um processo
  SEPARADO de qualquer `forge run`/`chat`/`tui` que realmente consome vagas
  de rate limit — não existe `RateLimiter` compartilhado entre os dois
  processos para ler. Cada requisição à rota constrói um `RateLimiter` novo
  e vazio via `for_tier()`; o "uso" só existiria se o MESMO processo tivesse
  o limitador vivo. O getter não-mutante (`max_requests()`/`window()`) que a
  onda pedia foi construído (é barato e correto), mas ele só expõe a
  CONFIGURAÇÃO, nunca um contador real de uso — documentado explicitamente
  na doc do handler e no banner da tela, não deixado implícito.
- **[decisão] `Sandbox::ping()` é função associada (`Sandbox::ping()`, não
  `&self`), com `ping_with(docker: &bollard::Docker)` companheiro — mesmo
  padrão de `run`/`run_with`.** Reachability do daemon não depende de
  nenhum campo do perfil (image/mount/limites); a versão `_with` existe só
  pra testabilidade determinística (endpoint morto), mesmo motivo de
  `run_with` já existir do jeito que existe.
- **[decisão] Teste de `/api/sandbox` não afirma um valor fixo para
  `ping`.** Confirmado empiricamente: este container de dev NÃO tem
  `/var/run/docker.sock` (nenhum daemon), mas o runner `ubuntu-latest` do
  GitHub Actions tipicamente TEM Docker rodando por padrão — os dois jobs
  que tocam este código (`rust`/`verify` e `web`) não instalam Docker
  explicitamente (só o job dedicado `sandbox` faz isso, com
  `--include-ignored`), então o resultado real de `Sandbox::ping()` nesses
  jobs é genuinely ambíguo. Hard-codar `assert_eq!(ping, false)` seria
  flaky-por-ambiente. A propriedade fail-closed determinística (daemon
  inalcançável → `false`, nunca panic) já está provada isoladamente em
  `forge_tools::sandbox`'s `ping_com_daemon_inalcancavel_e_false` (aponta
  pra um endpoint deliberadamente morto, `http://127.0.0.1:1`) — o teste da
  rota HTTP e o do Playwright checam só que `ping` é um bool bem-formado e
  que o perfil bate com as constantes reais.
- **[decisão] `SkillStatus` ganhou o campo `source` (não só como sufixo de
  `detail`).** A tela de sandbox precisa filtrar "só as de terceiro" sem
  fazer parsing de string sobre `detail` (frágil, ex.: quebraria se a
  descrição da skill contivesse a palavra "third-party"). Mudança aditiva:
  `detail` não mudou de conteúdo, `Skills.tsx` (tela existente) não
  precisou de nenhuma alteração.
- **[decisão] `/api/lsp` não tem NENHUM campo de status/liveness — todo
  servidor é sempre "declarado, não iniciado".** Considerei se "status
  refletem só uso real já ocorrido na sessão" (texto do plano) significava
  introspectar se a MESMA sessão do dashboard já usou aquele language
  server via `LspSession` cacheada — mas nada no `ToolRegistry`/`lsp.rs`
  hoje expõe "quais sessões LSP já foram usadas" como estado consultável, e
  construir isso não estava no orçamento desta onda (seria uma peça de
  wiring nova, não mecânica). Ficou como visualizador de config puro: zero
  probe (a onda também proíbe isso explicitamente), zero liveness
  fabricada. Registrado como possível trabalho futuro, não como esquecimento.
- **[decisão] `read_lsp_server_configs` extraído de `skills.rs::
  load_lsp_servers`** — mesmo padrão de `read_mcp_server_configs` (Onda 7):
  a função de carregamento real (`load_lsp_servers`) e o console de exibição
  (`lsp_console.rs`) compartilham o parsing puro; `load_lsp_servers` virou
  um loop fino sobre o que a nova função devolve. `LspServerConfig` ganhou
  `#[derive(Serialize)]` (mesmo achado de derive faltando que
  `McpServerConfig`/`ModelTier` tiveram na Onda 7).
- **[nota] as 3 telas (`ratelimit`, `sandbox`, `lsp`) entraram como itens de
  nav PRÓPRIOS**, não cartões embutidos em `providers`/`skills` — mesma
  decisão da Onda 7 (A1/A5) de reverter o encolhimento original do plano.
  `providers.ts`'s `RATE_LIMITS` fabricado **não foi tocado** nesta onda —
  aposentá-lo é trabalho explícito da Onda 12 (Providers), que reusa a
  mesma leitura de tetos por tier que esta onda construiu.

## Onda 11 — Verify (job em background)

- **[decisão] `/api/verify/run`+`/api/verify/:id` foram direto em
  `forge-server`, não no router mesclado de `forge-cli`** — "zero
  dependência" no próprio título da onda no plano-mestre: `forge-verify` já
  é dependência de `forge-server` desde sempre (usado por `/api/skills`'s
  `list_skill_statuses`), então nenhuma dependência nova entrou.
  `run_verify_pipeline` (a função equivalente já existente em
  `forge-cli/src/main.rs`, compartilhada entre `forge verify` e `forge
  squad`) não foi importada — não dá (direção de dependência oposta,
  mesma regra que já vale para `ErrorBody`/`now_rfc3339`) — a resolução de
  `forge.toml`/`default_steps()` e os helpers `git_sha`/`novo run_id` foram
  duplicados em `forge-server`, mesmo padrão já estabelecido no resto do
  crate.
- **[decisão] `run_pipeline_with_progress` novo em `forge-verify`, `run_pipeline`
  virou este mesmo laço com callback vazio.** Evita duplicar a lógica de
  execução — só o refactor mínimo pra abrir um ponto de progresso por passo,
  sem mudar o comportamento de `run_pipeline` (mesmos testes existentes
  continuam passando, mais 1 teste novo provando que o callback é chamado
  na ordem certa com `(passo, total)` corretos).
- **[decisão] Estado do job é 1 slot em memória (`Arc<Mutex<Option<VerifyJob>>>`),
  não um parâmetro de `router()`.** Diferente de telemetria/ledger/prompt
  library (stores externos, passados de fora, sobrevivem a reinício), o job
  de verify não tem por que persistir — é união com o próprio processo do
  dashboard. Construído inline dentro de `router()` (`Arc::new(Mutex::new(None))`),
  sem alargar a assinatura pública da função nem os call-sites existentes em
  `forge-cli`/testes.
- **[decisão] `POST` concorrente com job ativo é `409` com o `run_id` já em
  andamento — não um job novo, nem erro genérico.** Provado pela fronteira:
  2 POSTs em sequência rápida contra um pipeline de 1 passo de 500ms — o
  segundo chega bem antes do primeiro terminar e recebe exatamente o mesmo
  `run_id` do primeiro. Cliente (`api/verify.ts::startVerifyRun`) trata 202 e
  409 de forma idêntica — os dois dão um `run_id` pra acompanhar, a UI não
  precisa saber se foi ela que iniciou o job ou se só "entrou" num já rodando.
- **[decisão/risco aceito] `spawn_blocking`'s `JoinHandle` não é aguardado —
  um panic dentro do pipeline deixaria o job preso em "running" pra
  sempre, sem crash visível em lugar nenhum.** Considerei
  `std::panic::catch_unwind` em volta da chamada, mas todo caminho de falha
  já documentado em `run_step`/`exec::run_with_timeout` devolve `Result`,
  nunca panica (testes existentes: `programa_inexistente_falha_sem_panicar`,
  `passo_com_timeout_estourado_falha_com_finding_e_exit_code_sentinela`) —
  um panic só viria de um bug de verdade no meu código novo de glue, que os
  testes desta onda já exercitam. Aceitei o risco residual em vez de
  adicionar `catch_unwind` só por precaução; documentado aqui, não escondido.
- **[decisão] Teste de fronteira do progresso usa polling REAL (sleep de
  20ms entre tentativas, até 200 iterações), não tempo mockado.** O job roda
  de verdade em `spawn_blocking` (thread real, subprocessos reais via `sh -c
  sleep`) — diferente de `rate_limit.rs`'s testes (`#[tokio::test(start_paused
  = true)]`, tempo virtual), aqui não dá pra pausar o relógio porque o
  progresso depende de um subprocesso do SO de verdade terminando. Passos de
  50ms (3 passos = ~150ms) mantêm o teste rápido sem ficar flaky.
- **[decisão] Frontend: `VerifyPoller` como subcomponente que só existe
  enquanto há um `run_id` ativo** (`{activeRunId && <VerifyPoller .../>}`),
  em vez de tentar fazer `usePolling` (hook existente, reusado sem
  modificação) parar sozinho. Montar/desmontar via o `run_id` no estado do
  pai é o jeito idiomático de "start/stop" um hook que só sabe repetir para
  sempre — quando o job termina (`status: "done"`), o pai zera `activeRunId`,
  o subcomponente desmonta, o `setInterval` é limpo pelo cleanup do próprio
  `usePolling`.
- **[decisão] `VerifyStep` (tipo mock antigo) removido de `types/domain.ts`**
  — substituído por `VerificationEvidence`/`VerificationStep`/`Finding` reais
  em `api/verify.ts` (mesmo padrão de `api/experiments.ts`: DTO que espelha
  `forge_schemas` mora no módulo de api, não em `domain.ts`). `ReviewerScore`
  continua em `domain.ts` — ainda usado pela seção mock de "Review por
  valor", que esta onda não tocou (fora de escopo, ver próximo item).
- **[não-escopo explícito] "Review por valor" (`REVIEWERS`/`VALUE_SCORE`/
  `VALUE_GATE`) continua mock.** O plano-mestre desta onda só cobre o job de
  `/verify` em si; ligar `forge_review`'s `gates`/`certification` reais é
  trabalho não pedido aqui — mantive os mocks intactos (só migrados de
  `api/verify.ts` pro mesmo arquivo, já que o módulo inteiro foi reescrito)
  com um comentário explícito marcando o não-escopo, não deixado implícito.
