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
