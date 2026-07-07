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

## Onda 12 — Providers (piso; degrau descartado, não só adiado)

- **[decisão] Só o piso (leitura real) foi construído — o degrau (reordenar
  fallback, ajustar teto do rate limiter pela UI) foi descartado, não
  deixado para depois.** Dois motivos independentes, cada um já suficiente
  sozinho: (1) `forge_llm::FallbackChain` (`provider.rs`) é código morto —
  `Gateway::generate` itera `self.providers` direto e nunca consulta
  `FallbackChain::next_after`, confirmado lendo o código antes de expor
  qualquer mutação; mutar uma estrutura que o gateway real nunca lê seria
  fake por definição. (2) mesmo que fosse consultada, o `forge dashboard` é
  um processo separado de qualquer sessão `forge run`/`chat` real — a mesma
  descoberta já registrada na Onda 10 sobre "uso ao vivo" do `RateLimiter`.
  Uma mutação aqui não mudaria o comportamento de nenhuma sessão de
  verdade. Documentado como descope explícito, não como "não deu tempo".
- **[decisão] `KNOWN_PROVIDERS: [&str; 3] = ["anthropic", "deepseek",
  "openai"]` é uma constante de ordem fixa, não descoberta em runtime.**
  Espelha a ordem de fallback real que `Gateway::from_env` monta (comentário
  no código aponta a mesma conclusão do achado do `FallbackChain`). `GET
  /api/providers` cruza essa lista com `gateway.available()` — o mesmo
  `HashSet` que uma sessão real usaria para decidir quem está configurado —
  então `configured` nunca é fabricado no cliente.
- **[decisão] `ProviderInfo` (`web/src/types/domain.ts`) encolhido para
  `{id, configured}`**, removendo os campos mock `name`/`status:
  'ativo'|'standby'` que a tela antiga usava. `RateLimitTier` (tipo
  mock, não usado por nenhuma tela real desde a Onda 10) também foi
  removido — a tela de Providers reusa `RateLimitEntry`/`fetchRateLimits`
  de `api/ratelimit.ts` (Onda 10) direto, sem duplicar o tipo. Isso fecha a
  promessa registrada na Onda 10: "`providers.ts`'s `RATE_LIMITS` fabricado
  ... aposentá-lo é trabalho explícito da Onda 12".
- **[decisão] Tela reescrita como read-only, 2 cards** — "Gateway LLM ·
  providers configurados" (novo, real) e "Rate limiting tier-gated"
  (reaproveita `fetchRateLimits` da Onda 10 sem mudança). Nenhum toggle,
  botão de reordenar ou editor de teto — o achado acima (degrau
  descartado) já eliminava a necessidade de qualquer affordance de escrita.
- **[decisão] Teste `providers_reflete_env_vars_reais` isola env vars via
  `std::env::set_var`/`remove_var` dentro do próprio `#[tokio::test]`** —
  mesmo padrão já usado por `FORGE_SCRIPTED` em `web_agent.rs`/
  `squad_agent.rs`. Confirmado por grep que nenhum outro teste do crate lê
  `ANTHROPIC_API_KEY`/`DEEPSEEK_API_KEY`/`OPENAI_API_KEY`, então não há
  disputa com outro teste rodando em paralelo no mesmo binário; reforçado
  rodando o teste 3× seguidas sem flake antes de considerar fechado.
  `scripts/run-integration-server.mjs` aplica o mesmo isolamento no
  processo filho do `forge dashboard` real (só `ANTHROPIC_API_KEY` setada,
  as outras 2 removidas do env herdado) — a fronteira do Playwright prova
  exatamente 1 "configurado" e 2 "sem key", nunca os 3, o que aconteceria
  se a tela fabricasse o status em vez de ler o real.
- **[armadilha evitada] `page.locator('span.mono')` no Playwright bateria em
  5 elementos, não 3** — a legenda do rodapé ("`forge run`"/"`chat`")
  também usa `.mono` para destaque tipográfico, sem relação com os ids de
  provider. Resolvido com `.nth(0)`/`.nth(1)`/`.nth(2)` (ordem do DOM: os 3
  providers vêm antes da legenda), não com um seletor mais específico —
  mesma classe de armadilha já registrada para `getByText('read-only')` na
  Onda 10.

## Onda 13 — Modelo & Onboarding

- **[achado] `model`/`agent` já existiam em `SendMessageBody`
  (`web_agent.rs`) desde a Onda 1, mas o frontend nunca os populava** —
  `sendMessage(text)` só mandava `{message}`, então o `unwrap_or_else`
  sempre caía no default hardcoded (`"claude-sonnet-5"`/`"build"`),
  independente do que `Modelo.tsx` mostrasse selecionado. Zero mudança de
  backend foi necessária para o piso desta onda — só `SessionContext.tsx`'s
  `sendMessage` ganhar um parâmetro opcional `{model, agent}` e
  `Sessao.tsx`'s call site passar `{model: primaryModelName(modelTier),
  agent: agentProfile}`. **Fronteira provada, não só "o campo viaja":** um
  override real (mesmo mecanismo persistido de `RuleStore` que a matriz de
  permissão da Onda 2 usa) para `plan`+`bash` = deny faz a mensagem
  roteirizada terminar em `tool_denied` sem perguntar quando `agent: "plan"`
  é enviado, e em `permission_requested` (ask, default real de `build`, sem
  override) quando `agent` nem é mandado — dois caminhos HTTP idênticos
  exceto o campo, comportamento observável diferente
  (`post_message_respeita_o_agent_do_corpo_via_override_persistido`).
- **[decisão] `primaryModelName(tier)` (já existente, só usado até agora
  pra exibir texto no cabeçalho da sessão) virou o valor real enviado no
  campo `model`.** Não fabriquei um catálogo de model-id novo — reusei a
  string que o usuário já vê na tela (`MODEL_TIERS[tier].models.split('
  · ')[0]`). Ressalva honesta: `small`'s entrada (`"haiku"`) não é
  necessariamente um id de modelo resolvível por um provider real (ao
  contrário de `"claude-sonnet-5"`, que já aparece como default em 3 outros
  lugares do código) — isso é uma imprecisão PRÉ-EXISTENTE em `MODEL_TIERS`
  (Onda 7), não introduzida aqui; corrigir o catálogo de modelos é fora do
  escopo de "Modelo & Onboarding" (que é sobre telas, não sobre curar a
  lista de modelos suportados).
- **[decisão] `selectTier`/`selectAgentProfile`/`selectAutonomy`
  (`api/models.ts`) removidos — eram `simulateLatency(150); return
  <mesmo valor>`, uma chamada fake sem efeito.** Tier/agente não são mais
  "selecionados" via uma chamada à parte: a escolha em `Modelo.tsx` fica só
  no `AppContext` (dispatch local) e é aplicada de verdade só quando a
  próxima mensagem é enviada — parâmetro por sessão/tarefa, mirroring do
  CLI (`--model`/`--agent` são flags por invocação, não uma preferência
  persistida), não um store de preferência novo. Os toasts refletem isso
  ("aplica à próxima mensagem enviada"), não mais um "selecionado"/"ativo"
  que sugeria um round-trip imediato ao backend.
- **[decisão — ADR 0021] `max_autonomy_level` NÃO foi wireado até a UI —
  descope explícito, não esquecimento.** Confirmado nesta onda: o campo é
  ignorado ponta-a-ponta hoje (hardcoded `3` em `squad.rs`+`squad_agent.rs`,
  nunca lido por `forge_squad/server.py::ExecuteTask`; a autonomia real vem
  de `ProgressiveAutonomyManager`/`agent_trust_scores`, `hitl.py`,
  desconectado deste campo do proto). A própria fronteira que o plano exige
  para "se autonomia entrar" — comportamento observável diferente por
  nível, não só o campo viajando — não seria alcançável sem mudar o
  orquestrador Python, uma mudança arquitetural fora do escopo de uma onda
  de telas. `Modelo.tsx`'s seção de autonomia virou um bloco informativo
  (sem botões, sem estado local, sem toast de sucesso fake) com nota
  explícita "não aplicado pelo orquestrador ainda". Os dois hardcodes
  (`squad.rs`/`squad_agent.rs`) ganharam comentário apontando a ADR, não
  ficaram silenciosos. Detalhe completo na ADR 0021.
- **[decisão] `GET /api/doctor` novo (`crates/forge-cli/src/doctor_console.rs`)
  agrega 4 checagens: providers (reusa o mesmo `Gateway::from_env()
  .available()` + `KNOWN_PROVIDERS` da Onda 12, duplicado — não importado,
  mesma convenção de `git_sha`/`now_rfc3339` entre os dois crates), `uv
  --version` (novo, com PATH injetável pra teste determinístico — ver
  abaixo), ping Docker (reusa `Sandbox::ping()` da Onda 10 sem mudança), e
  git (reusa `crate::git_sha()` de `main.rs` DIRETO — mesmo crate que
  `doctor_console.rs`, então nenhuma duplicação faz sentido aqui, diferente
  do caso cross-crate forge-server/forge-cli). Mora em `forge-cli` (não
  `forge-server`) porque a checagem de Docker precisa de
  `forge_tools::sandbox` — mesma regra de posicionamento de rota das
  demais telas admin desta fase.
- **[decisão] `uv_check_with_path` checa `status.success()`, não só se o
  processo subiu — diferente do guard de teste `uv_missing()` (duplicado em
  ~7 arquivos `#[cfg(test)]` deste workspace, que só quer saber "existe pra
  pular o teste").** Este é o doctor mostrado ao usuário real: um `uv`
  presente mas quebrado (exit ≠0) deve aparecer como ausente, não como
  presente. PATH é injetável (`Option<&str>`) pro teste simular "uv
  ausente" apontando pra um PATH vazio, sem depender do PATH real do
  processo de teste — mesmo espírito de `Sandbox::ping_with` receber um
  client já configurado em vez de só `ping()`.
- **[decisão] `onboarding.ts`/`Onboarding.tsx` reescritos: `ENV_KEYS`/
  `DOCTOR_OUTPUT` (arrays estáticos, sempre "tudo verde" exceto os
  fallbacks marcados de propósito) saem, `fetchDoctor()` real entra.**
  Deliberadamente NÃO importei `fetchProviders`/`api/providers.ts` da Onda
  12 pro card "Chaves de API" — a Onda 12 ainda não tinha mergeado quando
  esta onda começou (branch aberta direto de `origin/main`, sem
  sobreposição de arquivo com `claude/fase-7-onda-12`, de propósito, pra
  não empilhar sem necessidade). O card usa o resumo agregado que o próprio
  `/api/doctor` já calcula (`"N/3 provider(s) configurado(s)"`) em vez de
  duplicar a lógica de `KNOWN_PROVIDERS` uma terceira vez só pra ter uma
  lista por-key aqui também — o detalhe por provider individual mora na
  tela Providers, não duplicado nesta.
- **[nota] `AutonomyLevel` (`web/src/types/domain.ts`) ficou sem uso**
  depois que `Modelo.tsx` trocou pra um union type literal local — deixado
  no arquivo de propósito (não removido) pra não tocar `domain.ts`, que a
  Onda 12 (ainda não mergeada) também edita por perto (`ProviderInfo`);
  remover é cosmético e pode esperar uma onda que já vá mexer nesse
  arquivo por outro motivo.
- **[decisão] Fronteira do doctor por Playwright usa 2 gêmeos genuinamente
  determinísticos, por motivos opostos — nenhum hardcoded por
  conveniência.** `uv`: gêmeo POSITIVO real (o job `web` do CI instala via
  `astral-sh/setup-uv@v5`, precondição já existente pro squad e2e — PATH
  herdado pelo processo do dashboard tem `uv` de verdade). `git`: gêmeo
  NEGATIVO real (o dashboard roda com `cwd` no `workDir` temporário de
  `run-integration-server.mjs`, que nunca é um repositório git — `git
  rev-parse HEAD` falha de verdade ali). `docker`/`providers` ficam sem
  valor afirmado: docker varia por ambiente (mesma cautela da Onda 10);
  providers já tem fronteira determinística a nível Rust
  (`doctor_agrega_as_4_checagens_com_providers_real`, com isolamento de env
  var) — reafirmar no Playwright seria uma segunda cópia do mesmo teste.

## Onda 14 — Designer (salvar honesto)

- **[decisão] `squad.workflow.v1` novo (JSON Schema + tipo Rust +
  fixture golden), mesmo padrão de `experiment.v1`** — `SquadWorkflow{nodes,
  edges}` (`crates/forge-schemas/src/workflow.rs`), com `WorkflowNodeKind`
  espelhando o union `'card'|'pill'` do TS como enum (`#[serde(rename_all =
  "snake_case")]`, mesma convenção de `ModelTier`/`ExperimentVerdict`). A
  checagem semântica (aresta referencia nó inexistente) NÃO é expressável em
  JSON Schema puro — fica em `SquadWorkflow::validate_edges` (Rust puro,
  testada isoladamente); a fixture golden só prova forma (campo obrigatório
  ausente reprova o schema), não a checagem de aresta.
- **[decisão] `POST /api/designer/workflow` direto em `forge-server`**
  (mesma classe de posicionamento de A2/A5/A12 — só precisa de
  `forge-store`/`forge-schemas`, já dependências do crate). Valida
  `validate_edges()` ANTES de qualquer escrita — grafo malformado nunca
  chega a tocar o ledger (provado por teste: ledger continua com 0 entradas
  após um 422). Grafo válido grava via o MESMO `LedgerStore::append` que
  toda outra escrita de auditoria da plataforma já usa (zero mudança de
  ledger) — `kind: "designer.workflow_saved"`, `actor: "web:designer"`,
  `payload` é o grafo serializado direto (sem DTO espelho).
- **[decisão] "Salvar honesto" tocou os 2 lados da mentira antiga, não só
  o backend**: o mock antigo (`saveWorkflow`) fabricava `seq: 248` fixo E
  a cópia da tela prometia "orquestrador aplica na próxima forge squad" —
  nunca foi real (`UnifiedOrchestrator` continua com os 5 agentes fixos,
  sem reescrita nesta fase). Os dois foram corrigidos juntos:
  `MARK_SAVED` (reducer) ganhou o campo `seq` real (antes não carregava
  nenhum), e a cópia (banner em `Designer.tsx`, toast em `Toolbar.tsx`,
  botão "salvar & aplicar" → "salvar") declara só "salvo e validado —
  aplicação real é trabalho futuro". Corrigir só um dos dois teria deixado
  a outra metade da mentira de pé.
- **[nota] Grafo padrão (`templates.ts`'s `initialNodes`/`initialEdges`,
  8 nós, várias arestas) já é válido e não-trivial** — a fronteira por
  Playwright não precisa arrastar nem conectar nada na tela: clicar
  "salvar" no estado inicial já exercita `POST /api/designer/workflow` de
  ponta a ponta com um grafo real. Testar o caso de aresta pendente
  (422) pela UI não é possível de propósito: `REMOVE_NODE` do próprio
  reducer já remove as arestas de um nó ao removê-lo — a UI estruturalmente
  não deixa construir um grafo inválido por interação normal. Esse caso
  fica só no teste Rust (`salvar_workflow_com_aresta_pendente...`), que
  constrói o corpo HTTP direto, sem passar pela UI.

## Onda 15 — Fecho

- **[achado — bug real de produção] `fetchJson` chamava `.json()` direto num
  corpo VAZIO de `202 Accepted`, e `SyntaxError` (não `ApiError`) fazia
  `sendMessage` reportar falha mesmo quando o servidor respondia com
  sucesso.** Achado escrevendo a 1ª cobertura de browser da tela Sessão
  (`sessao-real-backend.spec.ts`) — nenhum outro teste (Rust ou Playwright)
  batia esse caminho, porque `POST /api/session/:id/message` sempre foi
  fire-and-forget (202 sem corpo, resultado via SSE), e todo teste Rust
  chama o handler direto, nunca pelo `fetchJson` do browser. Sem o fix,
  **toda mensagem enviada pela tela Sessão real mostraria "falha ao
  enviar mensagem"**, apesar do turno completar normalmente por trás —
  um bug silencioso que só a cobertura de browser (não unit test, não
  leitura de código) revelava. Fix em `web/src/api/client.ts`: `fetchJson`
  lê `response.text()` primeiro e só faz `JSON.parse` se não vazio,
  substituindo o `if (status === 204)` especial-casado por uma checagem
  geral de corpo vazio (cobre 202 e qualquer outro "sucesso sem corpo"
  futuro, não só 204).
- **[decisão] Varredura dos 2 resíduos mock na tela Sessão que nenhuma onda
  anterior cobriu** (achado de uma auditoria independente contra o estado
  real do repositório, não contra relato):
  - `toggleToolPolicy`/`TOOL_POLICIES` (`api/session.ts`) — removidos, não
    religados. Eram redundantes com a matriz real que a Onda 2 já entrega
    na tela Skills (`fetchMatrix`, `GET /api/permissions/matrix`) — em vez
    de duplicar o fluxo de confirmação daquela tela numa segunda tela, a
    sidebar "FERRAMENTAS" da Sessão agora **lê** a mesma matriz real
    (read-only, por perfil ativo), com o nome da ferramenta continuando a
    abrir a tela Skills pra quem quiser mudar a política.
  - `SESSION_HEADER` (`provider`/`cacheOn` hardcoded) — `provider` passa a
    vir de `fetchProviders` (Onda 12: primeiro provider com `configured:
    true`, mesma ordem fixa de fallback). `cacheOn` foi **removido**, não
    substituído por um valor "real" fabricado: não há hoje um jeito honesto
    do frontend saber se a sessão atual está em modo roteirizado
    (`FORGE_SCRIPTED`, sem cache) ou real (com `CachedGenerator`) — inventar
    um sinal novo só pra preencher esse campo seria escopo além de uma
    varredura de resíduo; melhor remover a alegação do que fabricar uma
    nova.
- **[decisão] `--web-agent` vira `--no-web-agent` (Onda 15 — fecho, conforme
  o plano-mestre já previa).** Agente web habilitado por padrão agora;
  `run_dashboard`'s branch antes "padrão" (só leitura) virou o `else`. ADR
  0020 ganhou uma seção "Atualização" (não reescrita — o resto da decisão
  original, topologia+spawn_blocking+teto de sessões, continua valendo).
  `run-integration-server.mjs` parou de passar `--web-agent` explícito
  (seria um erro de parse agora, já que a flag mudou de nome) — achado
  ao rodar a suíte de integração depois da mudança, não antes.
- **[decisão] `simulateLatency`/`maybeFail` (`api/client.ts`) removidos —
  código morto** depois que a Onda 14 (Designer) migrou o último módulo
  `api/*.ts` que ainda os chamava. Critério mecânico `grep simulateLatency
  web/src/api` fecha vazio (só resta uma menção em comentário histórico,
  `models.ts`).
- **[decisão] Documentação do fecho**: `README.md`/`CLAUDE.md` ganharam a
  seção "Fase 7 concluída" (mesmo padrão de densidade das fases
  anteriores); `docs/DECISOES.md` ganhou uma entrada consolidada de 15
  ondas (mesmo padrão da entrada da Fase 6 — uma entrada por fase, não
  mais uma por onda, convenção que já tinha mudado antes desta fase);
  `docs/PLANO-PLATAFORMA-FORGE.md` ganhou um parágrafo-ponte (não uma
  reescrita — o documento continua explicitamente escopado às 6 fases
  originais); `docs/LEVANTAMENTO-UI-DESIGNER.md` ganhou uma nota "✅
  Fechado pela Fase 7" (mesmo padrão de nota de superseded que
  `PLANO-INTEGRACAO-FRONTEND.md` já usava), citando os dois descopes
  (`max_autonomy_level` ADR 0021, `forgetting.py` ADR 0022) como histórico,
  não escondidos.
- **[nota] `docs/PLANO-FASE-7-frontend-primario.md` não foi editado** —
  mesmo padrão já seguido em todas as ondas anteriores: o plano-mestre é
  registro histórico do que foi planejado, `pendencias.md` é quem registra
  o que de fato aconteceu onda a onda.
- **[nota] `squad-real-backend.spec.ts` (pré-existente, não tocada nesta
  onda) deu timeout uma vez rodando a suíte inteira em paralelo** (2
  workers, depois de uma sessão longa com muitos builds/testes
  seguidos) — reconfirmado verde rodando isolado logo em seguida (1.5s
  até a primeira asserção). Contenção de recurso do ambiente, não
  regressão: nada nesta onda toca squad/sidecar Python.

## Pós-Fase 7 — modelo do squad hardcoded (achado em produção, VPS real)

- **[achado — bug real, sem ADR por trás] O squad do dashboard (navegador)
  sempre mandava `"claude-sonnet-5"` pros 5 agentes Python, mesmo com
  `DEEPSEEK_API_KEY` configurada e o usuário selecionando `deepseek-chat` na
  tela Modelo.** Reportado como "selecionei deepseek na UI, cliquei, e não
  mudou nada" — confirmado batendo no código, não no relato: `RunSquadBody`
  (`squad_agent.rs`) só tem `task: String`, o frontend não tinha como mandar
  modelo pro squad; `default_squad_pool` (que constrói o pool Python de
  longa duração, capacidade 1, criado uma vez na subida do `forge
  dashboard`) tinha `"claude-sonnet-5"` literal, que vira o default de
  `ArchitectAgent`/`DeveloperAgent`/etc. (`orchestrator.py`) pra sempre,
  até o processo reiniciar.
  - **Importante: isso é diferente do `max_autonomy_level` (ADR 0021).** Uma
    investigação externa citou o comentário "Descope explícito da Onda 13
    (ADR 0021)" pra justificar o hardcode do **modelo** — mas esse
    comentário (`squad_agent.rs`, perto da construção do `SquadTask`) é
    sobre `max_autonomy_level`, um campo totalmente diferente. Conferi a
    ADR 0021 inteira: fala só de autonomia, nunca de modelo. O hardcode do
    modelo não era uma decisão documentada — era lacuna real, sem dono.
  - **O `forge squad --model` via CLI (terminal) nunca teve esse bug** —
    `squad.rs::run_squad` sempre passou `opts.model` de verdade pro
    `SquadSupervisor::spawn` (processo Python novo a cada chamada, não um
    pool persistente). O bug é específico do agente web/dashboard
    (`squad_agent.rs`), que reusa um pool entre tarefas.
  - **Mecanismo completo do "400 modelo desconhecido"** (pra não repetir o
    diagnóstico externo que citou a ADR errada e propôs um workaround que
    ele mesmo reconheceu não funcionar — setar `ANTHROPIC_API_KEY` com uma
    key falsa "pra forçar" a DeepSeek): `Gateway::from_env()`
    (`forge-llm/src/gateway.rs:44-81`) escolhe o provider **só por qual env
    var existe e não é vazia**, em ordem fixa Anthropic → DeepSeek →
    OpenAI — o nome do modelo nunca decide o provider, só vira o texto
    dentro do corpo da requisição pro provider que "ganhou" a prioridade
    (`call_provider`/`build_request_body`). Consequência: mesmo com
    `ANTHROPIC_API_KEY` falsa, o Gateway tentaria Anthropic primeiro (401),
    cairia pra DeepSeek (fallback real, o `for` tenta todos), mas o texto do
    modelo continuaria `"claude-sonnet-5"` — a DeepSeek rejeitaria do mesmo
    jeito. **E isso também é um risco pra Sessão/chat** (que já manda
    `body.model` corretamente, Onda 13): se `ANTHROPIC_API_KEY` estiver
    definida no ambiente (mesmo com valor velho/inválido), ela sempre é
    tentada primeiro, e uma seleção de `deepseek-chat` na UI mandaria esse
    texto pra Anthropic antes de cair pro DeepSeek. Vale conferir no
    ambiente da VPS se `ANTHROPIC_API_KEY` está mesmo ausente.
- **[decisão] Fix: `FORGE_SQUAD_MODEL` (env var), não trocar o literal por
  outro literal.** `default_squad_pool` agora lê `FORGE_SQUAD_MODEL` com
  fallback pra `"claude-sonnet-5"` (comportamento antigo intacto pra quem
  não configurar nada) via um helper `squad_model()` compartilhado — usado
  também no `RunOpts.model` de `run_squad_handler` (que antes só afetava o
  tier de rate-limit, não o texto do modelo — corrigido por consistência,
  não porque fosse a causa do bug) e no rótulo de `model` da sessão do
  ledger (antes hardcoded, então o ledger sempre "mentia" `claude-sonnet-5`
  mesmo quando outro modelo era usado — agora reflete o que o pool está
  configurado pra usar de verdade).
- **[dúvida/gap remanescente] Isto NÃO faz a tela Modelo controlar o squad
  por tarefa** — `FORGE_SQUAD_MODEL` é uma env var fixa no deploy (lida uma
  vez, na subida do dashboard), não um campo por-request. Fazer a seleção
  da tela Modelo valer pro squad de verdade exige adicionar `model` ao
  proto `SquadTask` (`schemas/proto/squad.proto`) e cada agente Python ler
  por tarefa em vez de herdar do pool na construção — mesma categoria de
  mudança arquitetural do `max_autonomy_level`/ADR 0021 (campo novo no
  proto + Python consumindo por chamada), não coube nesta correção pontual.
  Deixo registrado como trabalho futuro, não como algo que o fix atual já
  resolve.
- **[decisão] `docker-compose.yml`: serviço `dashboard` ganhou `environment:`**
  (`ANTHROPIC_API_KEY`/`DEEPSEEK_API_KEY`/`OPENAI_API_KEY`/`FORGE_SQUAD_MODEL`,
  mesmo padrão do serviço `forge`) — achado ao revisar o arquivo pra
  documentar a env var nova: o serviço `dashboard` não passava NENHUMA key
  pro container via `docker compose run --rm dashboard` (só funcionava se o
  usuário lembrasse de repetir `-e` na linha de comando). Sem isso, o
  dashboard subiria sem provider nenhum configurado — bug adjacente, não o
  reportado, mas bloqueava o mesmo fluxo (squad/sessão pelo navegador) então
  corrigi junto em vez de deixar half-fixed.

## Squad como executor — Onda 0 (honestidade do auditor)

- **[contexto] Um parecer de engenharia externo (baseado num run real na VPS
  — `forge squad "crie uma calculadora científica... gere um .html"` não
  produziu arquivo nenhum) diagnosticou a causa raiz: o squad não tem
  executor nem canal de artefato — `CoreService::RunTool`
  (`core_server.rs:99`) devolve `Unimplemented` de propósito, o developer
  (`developer.py`) faz uma única chamada de LLM e nunca toca disco, e o
  auditor julga sem ver o resultado real. Conferi as citações (arquivo:linha)
  contra o repo — quase todas exatas. Decisão de arquitetura endossada:
  `RunTool` real no Rust (o contrato já existe no proto, zero breaking) +
  loop ReAct no developer — não um "shuttle" do `final_output` pelo stream
  de eventos. Onda 1 (o executor) e Onda 2 (o loop) ficam para depois; esta
  entrega é só a Onda 0, que não depende delas.
- **[correção ao parecer] O auditor não é cego à saída do developer do jeito
  que o parecer generalizou.** `validate_results` (chamado incondicionalmente
  ao fim de `execute_complex_task`, `orchestrator.py:155`) **já recebia**
  `execution_results` de verdade, incluindo o `final_output` real — o
  veredito que decide `overall_success` não era cego ao conteúdo. A cegueira
  genuína era só a chamada per-step `audit()` (quando o plano inclui uma
  ação `"validate"` explícita, mapeada pro auditor via
  `_select_agent_for_step`): `step_task` (`orchestrator.py:248-252`) era
  montado só com `description`/`action`/`step` do plano, sem nenhum
  resultado anterior. Isso mudou o escopo do fix de "rotear resultados pro
  auditor" (já acontecia) para dois pontos mais específicos:
  1. `auditor.py`: `_AUDIT_SYSTEM_PROMPT` e `_VALIDATE_RESULTS_SYSTEM_PROMPT`
     ganharam proibição explícita de alegar que um arquivo foi
     criado/salvo/persistido — nenhum caminho tem evidência de filesystem
     hoje (sem `RunTool`, nada escreve disco), então nenhum dos dois pode
     honestamente afirmar isso, mesmo vendo o `final_output` real.
  2. `orchestrator.py::_execute_plan_steps`: `step_task` ganhou
     `prior_results` (os resultados reais acumulados até aquele passo, não
     um resumo sintético) — fecha a cegueira específica do `audit()`
     per-step. `auditor.py::audit()` repassa isso ao gateway como
     `prior_agent_results` quando presente (não fabrica a chave quando
     ausente — proposta inicial em `_get_squad_proposals` continua sem
     ela).
- **[decisão] `run_tool` "morto" não é herança de `CoreBackend`.** O trait
  (`core_server.rs:27-34`) só declara `generate`/`request_permission` — o
  `run_tool` que devolve `Unimplemented` (linha 99) está hardcoded direto no
  `impl<B: CoreBackend> CoreService for CoreServer<B>`, idêntico pra
  qualquer `B`. Não muda o plano da Onda 1 (que já propõe corretamente
  adicionar o método ao trait), só corrige a descrição do mecanismo — não
  há o que "herdar" hoje, porque não existe hook nenhum pra um backend
  fornecer isso.
- **[nota] Testes novos são todos deterministicos via `ScriptedGatewayClient`/
  `RoutingGatewayClient`** (nenhuma chamada de rede/LLM real) — provam a
  proibição textual do prompt, a presença/ausência condicional de
  `prior_agent_results` no payload, e o fluxo ponta a ponta (plano de 2
  passos "implement"→"validate" via `UnifiedOrchestrator` real) mostrando
  que só a chamada per-step do auditor carrega `prior_agent_results`, com o
  `final_output` real do developer dentro.
- **[gap remanescente, fora desta entrega] Onda 1 (RunTool real) e Onda 2
  (loop ReAct do developer)** ficam para PRs futuras — são a peça que faz
  um arquivo aparecer de verdade no disco. Definição de pronto proposta
  pelo parecer (e que endosso): `forge squad "crie X.html..."` produzindo
  `X.html` real no workspace, registrado no ledger, com o auditor julgando
  sobre um artefato que existe — não sobre uma alegação de texto.

---

# BuildToValue — incorporação dos repos irmãos (jul/2026, autônomo)

> Contexto: renomeação Forge→BuildToValue + roadmap em 4 fases derivado da
> análise de 11 repositórios irmãos. Handoff completo em
> `docs/handoff/desenvolvimento/HANDOFF-BUILDTOVALUE.md`. Este bloco registra
> as DÚVIDAS e decisões que precisam de revisão humana.

## Fase 1 — usuário como membro da squad

- **[entregue · Fase 1a] Chat visível + canal de entrada.** `ChatMessage` no
  `squad.proto` (tag 9, aditivo); `POST /api/squad/:id/message` injeta a fala do
  usuário (ecoada como `ChatMessage` HUMAN + enfileirada na `inbox`); o
  orquestrador dá voz aos agentes (`_emit_chat`, narração derivada do conteúdo
  REAL da proposta, nada fabricado) e narra o consenso; UI de conversa na tela
  Squad. Testes: Rust (`push_user_message_*`), Python (`test_chat_message_*`,
  `test_event_sink_*`), web (tsc/lint/vitest verdes).

- **[DÚVIDA — decisão do dono · Fase 1b] O usuário influencia o run de verdade?**
  Hoje (1a) a fala do usuário é visível e enfileirada, mas o orquestrador é um
  pipeline determinístico (plano→propostas→consenso→passos) e **ainda não
  consome** a `inbox` no meio da execução. Para o usuário virar membro PLENO
  (a fala vira `Proposal` de peso máximo dentro do consenso), é preciso:
  (a) RPC novo `CoreService.AwaitUserTurn` (`core.proto`), (b) método no trait
  `CoreBackend` (`forge-sidecar/core_server.rs`) com default no-op p/ não quebrar
  os backends existentes, (c) `WebSquadCoreBackend.await_user_turn` puxando de
  `SquadHub.take_user_message`, (d) `UserAgent` + ponto de consulta no
  `_get_squad_proposals`. É mais invasivo (mexe no contrato gRPC Core) — deixei
  como **PR 2 separada** por segurança. **Confirmar se quer 1b agora** ou se 1a
  (chat + gate) já atende "não só aprovador".

## Fase 3/4 — decisão estratégica pendente

- **[DÚVIDA — decisão do dono] Autonomia progressiva L1–L5.** A análise recomenda
  MANTER descopada (ADR 0021): o próprio SquadIAds não usa o loop dele, e a UX
  "humano é membro contínuo" torna a auto-promoção redundante. Mas era um dos 4
  diferenciais do BuildToValue original. Implementei/implementarei autonomia como
  **rótulo descritivo** (metadado consultável em `persona.v1`), NÃO como loop
  automático. Se quiser readotar um dial Manual/Assistido/Autônomo de verdade, é
  um **ADR novo explícito** que supersede o 0021 — não farei em silêncio.

- **[nota] Descopes herdados que sigo respeitando:** `max_autonomy_level` continua
  ignorado ponta-a-ponta (ADR 0021); `forge_squad/forgetting.py` segue código
  morto. Não vou "ligar o campo" sem efeito real.

## Fase 3 — confiança e governança (parcial)

- **[entregue] Kill-switch de squad (Prioridade-Zero).** `POST /api/squad/:id/
  emergency-stop` para um squad em execução: aborta a task que drena o stream,
  destrava gate HITL pendente (nega, fail-closed), publica evento de erro
  visível, encerra o SSE. Checagem `is_stopped` no loop como saída limpa. Botão
  "interromper" na tela Squad. Testes Rust (`emergency_stop_*`).
- **[pendente — maior/mais sensível, precisa de revisão antes de autonomia
  profunda] Restante da Fase 3:** HMAC por entrada no ledger (mexe em
  `forge-store/ledger.rs`, código sensível — paridade de hash/append-only);
  gates 4-estados + piso-crítico-irredutível + impacto regulatório (estende
  `gates.evaluate`/`forge-verify`); separação produzir≠revisar≠aprovar
  (motor de permissões); versionamento/expiração de template; Prompt Integrity
  validator. Recomendo revisar as fases já mergeadas (1, 2a) e o kill-switch
  antes de eu avançar nas mudanças de ledger/permissões — são o núcleo de
  segurança e merecem seu aval.

## Fase 4 — endurecimento (não iniciada)
Espinha operacional (dep-graph/health/logging), consenso ponderado por
confiança, Decisão→ADR, histerese, gate Spec-First, placar what-matters,
roteamento por problema, modo local. Fica para depois do aval das fases de
governança.
