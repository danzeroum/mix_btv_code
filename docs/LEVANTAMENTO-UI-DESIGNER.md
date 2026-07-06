# Levantamento de UI para o Designer — telas faltantes (pós-Fase 6)

> Objetivo: mapear o que **falta de tela** (usuário ou admin) para o designer
> criar, localizando cada item no repositório e dando o contexto de dados/ações.
> Baseado numa varredura completa do frontend (`web/src/`) × capacidades do
> backend (Rust `crates/` + Python `python/`).

## TL;DR — a nuance que muda tudo

O frontend **já existe e é bem-acabado**: 13 telas registradas, duas personas
(usuário/admin), sistema de componentes próprio e temas. **Mas só 3 rotas HTTP
reais existem** (`GET /api/summary`, `/api/events`, `/api/skills` em
`crates/forge-server/src/lib.rs:30-46`). O resto das telas são **cascas visuais
completas sobre dados mock** (`web/src/api/*.ts` com `simulateLatency()`).

Então "falta tela?" se divide em **dois grupos** — e só o Grupo A é trabalho de
**design novo**:

- **Grupo A — telas que NÃO existem (design novo do designer):** funcionalidades
  da Fase 6 que ganharam backend mas nenhuma tela. **É aqui que o designer atua.**
- **Grupo B — telas que existem mas são mock:** o backend é real, falta a rota
  HTTP/wiring. Já estão **desenhadas** — é trabalho de *engenharia*, não de design.
  Listo para o designer não achar que estão "prontas para produção".

---

## Como o frontend está montado (orientação para o designer)

- **App:** SPA React/TS em `web/src/`. Navegação por estado (não por URL) —
  `dispatch({ type: 'SET_SCREEN' })` em `web/src/state/AppContext.tsx`.
- **Registro de telas (3 arquivos):** `web/src/lib/nav.ts` (menus por persona),
  `web/src/lib/screenMeta.ts` (título/kicker/accent/ícone por tela),
  `web/src/lib/screenComponents.tsx` (id→componente). O union de ids está em
  `web/src/types/domain.ts:3-15`. **Adicionar uma tela = tocar nesses 3 + criar o
  componente.**
- **Personas:** `USER_NAV` (8 telas) e `ADMIN_NAV` (5 telas) em `nav.ts`. O toggle
  é `web/src/components/shell/PersonaToggle.tsx`.
- **Vocabulário de componentes já existente (reusar, não recriar)** —
  `web/src/components/primitives/`:
  | Componente | Papel |
  |---|---|
  | `Button` | 3 variantes (primary/ghost/danger) |
  | `Card` | painel, com `accentBorder` para status |
  | `StatTile` | KPI/número grande |
  | `Badge` | pílula de status colorida |
  | `Gauge` | mostrador circular SVG (valor × limiar) — usado no Verify |
  | `ProgressBar` | barra horizontal 0–1 |
  | `Table` | tabela tipada (`columns/rows/onRowClick`) |
  | `Modal` | diálogo centralizado com backdrop |
  | `Toast` | notificações (`useToast`) — canal de feedback do app |
  | `AsyncStatus` | wrapper idle/loading/error/success (padrão das telas de dados) |
  - Hooks: `useAsyncAction` (ação one-shot com estados), `usePolling` (refetch a
    cada N ms — a Telemetria usa 5s). Tema/accent já são controláveis pelo usuário
    (`ThemeSwitcher`/`AccentSwitcher`, tokens em `web/src/styles/themes.ts`).
- **A referência de estilo/telas:** `web/README.md` documenta o design system.

> **Padrão a seguir para toda tela nova:** `AsyncStatus` por cima de um módulo
> `web/src/api/<x>.ts` que faz `fetch('/api/<x>')`; estados de erro reais (a
> Telemetria é o modelo — `web/src/api/telemetry.ts`, sem fallback mock).

---

## GRUPO A — telas que faltam criar (o trabalho do designer)

Ordenadas por prontidão de backend (as primeiras já têm dados reais a mostrar).

### A1. Console MCP (admin) — *parcialmente existe como mock, precisa de tela real*

- **Para quê:** gerenciar servidores MCP (Model Context Protocol) — processos
  externos que expõem ferramentas ao agente. Hoje a tela `skills` tem só um
  "pontinho de saúde" mock (`filesystem/git/postgres`), que **não** reflete a
  realidade: não mostra os servidores configurados, nem as tools que eles
  anunciam, nem a permissão por chamada.
- **Persona:** admin (hoje mora dentro de "Skills, MCP & permissões").
- **Dados a mostrar (backend real):**
  - Servidores declarados em `.forge/mcp.toml` (`[[server]] id/command/args`) —
    loader em `crates/forge-cli/src/skills.rs::load_mcp_servers`.
  - Por servidor: status de conexão, e a **lista de tools anunciadas** —
    `McpToolMeta { name, description, input_schema }`
    (`crates/forge-tools/src/mcp.rs:26-30`).
  - O nome namespaced que o agente vê (`mcp__<server>__<tool>`) e o **escopo de
    permissão** por chamada (`mcp:<server>/<tool> <preview>`, `mcp.rs:52-56`) —
    cada chamada MCP passa pelo gate (nunca auto-aprovada).
- **Ações:** ver tools de um servidor (expandir), (re)conectar. Não há "aprovar
  servidor" — a confiança é a declaração no `.toml` (ADR 0012).
- **Onde o mock está hoje (substituir):** `web/src/api/skills.ts:11-15`
  (`MCP_SERVERS`) e `:59-65` (`reconnectMcp`, TODO); tela em
  `web/src/components/screens/admin/Skills.tsx:86-103`.
- **Backend a expor:** falta `GET /api/mcp` (servidores + tools + status) em
  `forge-server`. Tipo frontend `McpServer` (`web/src/types/domain.ts`) só tem
  `{id, status}` — precisa ganhar as tools.

### A2. Experimentos / A-B testing (admin) — *tela nova, criar do zero*

- **Para quê:** o relatório de A/B testing (`forge experiment`) — comparar duas
  variantes (de prompt/modelo/tier) por taxa de sucesso, com **veredito honesto**.
  Já é proposta no app ("A/B de prompts" em `Sugestoes.tsx:11`).
- **Persona:** admin (ou usuário, se for A/B de prompts — decidir com o produto).
- **Dados a mostrar (backend real, `crates/forge-schemas/src/experiment.rs`):**
  `ExperimentReport { experiment, metric, variants[], verdict, winner?, p_value,
  produced_at }`; cada variante `VariantStats { variant, n, successes, rate }`.
  - Duas barras de taxa (variante A × B), com `n`/`successes`.
  - **Badge de veredito** — 3 estados honestos (`ExperimentVerdict`):
    `Significant` (com **vencedor**), `Inconclusive` ("sem significância" — **sem**
    vencedor), `InsufficientData` (< 20 amostras/variante).
  - `p_value` vs `α = 0.05` (`experiment.rs:22`). Nunca fabricar vencedor: a UI
    deve mostrar "sem significância" com a mesma dignidade de um vencedor.
- **⚠️ Caveat honesto (o designer PRECISA saber):** **nenhum código de produção
  ainda escreve** a telemetria de atribuição (`props.experiment/variant/success`)
  que o relatório lê — só testes e o `examples/seed_telemetry.rs`. Ou seja: a tela
  mostraria só dados semeados até instrumentarmos a atribuição. Desenhar a tela é
  válido; deixar claro no handoff que o pipeline de dados vem depois.
- **Backend a expor:** `forge experiment` é CLI-only (`main.rs:178`); falta
  `GET /api/experiment/<nome>` + `api/experiment.ts`.

### A3. Mapa de memória do squad / RAG (usuário) — *tela nova*

- **Para quê:** visualizar o que cada agente do squad "lembra" e buscar memórias
  por similaridade. Já é proposta no app ("Mapa de memória do squad" em
  `Sugestoes.tsx:12`).
- **Persona:** usuário (relacionada à tela `squad`).
- **Dados a mostrar (backend real, `python/packages/forge-squad/.../memory.py`):**
  - Memórias episódicas — registros `{ timestamp, agent, decision, confidence }`
    (`memory.py:72-88`), persistidos em `.forge/squad-memory/agent_memories.jsonl`.
    Agrupar por agente; mostrar confiança.
  - **Caixa de busca (recall):** o usuário digita uma consulta → lista ranqueada
    por similaridade TF-IDF, `{ ids, documents, metadatas:[{agent,timestamp}],
    scores }` (`memory.py:110-131`, retriever em `recall.py`). Mostrar o `score`
    (0–1) como barra — é recuperação **léxica** (ADR 0013), então o design pode ser
    honesto sobre "por termos", não "por sentido".
- **Backend a expor:** hoje só Python interno; falta uma rota (via gRPC/servidor)
  para o front ler o corpus + rodar recall.

### A4. Status/quota de rate-limit (admin) — *tela nova, mas backend não está pronto*

- **Para quê:** mostrar o consumo de rate-limit por tier (Small/Medium/Large)
  contra o teto. Hoje a tela `providers` mostra `used/cap` **fabricados**
  (`web/src/api/providers.ts:11-15`).
- **Persona:** admin.
- **Dados:** limites por tier — `RateLimiter::for_tier` (Small 60 / Medium 30 /
  Large 15 por 600s, `crates/forge-llm/src/rate_limit.rs:48`).
- **⚠️ Caveat:** ao contrário das outras, esta **nem tem backend pronto** — o
  `RateLimiter` não expõe getter de uso/restante (`poll` é privado, `:59`).
  Precisa de superfície nova no Rust **antes** de virar tela real. O designer pode
  desenhar o alvo; a engenharia precisa expor os números.

### A5. Breakdown de telemetria por modelo (admin) — *extensão da Telemetria*

- **Para quê:** a Telemetria hoje mostra totais e cache-hit global, mas **todo
  evento `llm.call`/`cache.*` já carrega `props.model`** — dá para um gráfico de
  volume e de cache-hit **por modelo**, que não existe.
- **Persona:** admin (extensão de `telemetria`).
- **Dados:** os eventos já têm o dado (`props.model`); falta um `summary` que
  agrupe por modelo (`telemetry.rs:83-109` só agrupa por nome). Extensão pequena
  de backend + um card/tabela a mais na tela existente.

### A6. Gestão de skills de terceiro + status do sandbox (admin) — *extensão do Skills*

- **Para quê:** a tela `skills` mostra só o **status read-only do vetter**. Falta:
  (a) o **ciclo de vida** de uma skill de terceiro (instalar em `.forge/skills/` →
  vetar → habilitar/remover), e (b) o **status do sandbox** (daemon Docker no ar?
  perfil de confinamento).
- **Persona:** admin.
- **Dados (backend real):** perfil do sandbox — `Sandbox { image, mount,
  network_disabled, mem_limit_mb, cpu_quota, timeout }`
  (`crates/forge-tools/src/sandbox.rs:30-52`; padrão: rede off, 512MB, 0.5 cpu,
  30s). Fail-closed se o daemon cair (`SandboxError::DaemonUnavailable`). O status
  read-only do vetter já vem de `GET /api/skills` (real).

### A7. Status/config LSP (admin) — *opcional, agente-facing*

- **Para quê:** o cliente LSP dá definição/referências/diagnósticos aos agentes,
  mas não tem superfície de usuário. Uma tela **fina** de status ajudaria: quais
  language servers estão declarados (`.forge/lsp.toml`), se o processo subiu/
  indexou, contagem de diagnósticos.
- **Persona:** admin (baixa prioridade — é ferramenta do agente).
- **Backend:** `crates/forge-tools/src/lsp.rs` (`LspQuery{Definition,References,
  Diagnostics}`, sessão preguiçosa). Sem UI hoje.

---

## GRUPO B — telas que existem mas são mock (contexto, não design novo)

Estas **já estão desenhadas**; o que falta é a rota HTTP/wiring do backend (na
maioria, TODOs "Fase 5" no código). O designer só precisa saber que **não estão
prontas para produção** — o dado é fabricado com `simulateLatency()`.

| Tela | Persona | Backend real existe em | Falta |
|---|---|---|---|
| `sessao` (Sessão de código) | user | `forge run/chat` (`forge-cli/src/main.rs`) | `POST /api/session/:id/message` (SSE do agente) — `api/session.ts:34` |
| `prompts` (Biblioteca) | user | `forge-store::PromptLibrary` (`prompt_library.rs`) | rota HTTP; hoje só CLI `/prompt` — `api/prompts.ts:19` |
| `squad` (Squad ao vivo) | user | `SquadService` gRPC (Python) | `POST /api/squad/run` (stream `SquadEvent`) — `api/squad.ts:29` |
| `designer` (Squad Designer) | user | — (editor client-side) | `POST squad.workflow.v1` → schema → ledger — `api/designer.ts:9` |
| `ledger` (Auditoria) | admin | `forge-store::ledger` (`ledger.rs`) | `GET /api/ledger` — `api/ledger.ts:16` |
| `verify` (Verificação) | admin | `forge verify` (`forge-verify`) | rota que dispara o pipeline real — `api/verify.ts:28` |
| `providers` (Providers & Limites) | admin | `forge-llm` gateway/rate-limit | rota de config + introspection de limite — `api/providers.ts:16` |
| `permissao` (Permissão) | user | `forge-core::permission` (`permission.rs`) | resolver via `PermissionClient` + ledger — `api/permissions.ts:10` |
| `onboarding`, `modelo` | user | parcial | persistência (Fase 5/6 TODOs) |

> O `designer` (editor visual de workflow do squad, `screens/user/Designer/`) é o
> mais elaborado do Grupo B — todo client-side, com `salvar` mock
> (`api/designer.ts` devolve `seq 248` fixo). Um alvo de design forte se o produto
> priorizar "desenhar o squad visualmente".

---

## Onde o próprio app já registra estas lacunas

A tela **`sugestoes`** (`web/src/components/screens/user/Sugestoes.tsx:6-13`) é um
roadmap dentro do app — cartões de telas propostas. **Duas delas já têm backend da
Fase 6** e viraram os itens A2/A3 acima: *"A/B de prompts"* e *"Mapa de memória do
squad"*. As outras (revisor de diff, replay de sessão, aprovação em lote, modo
watch) são ideias sem backend ainda — boas sementes, mas não priorizadas aqui.

---

## Recomendação de priorização (para conversar com o produto)

1. **A2 Experimentos** e **A3 Memória do squad** — já propostas no app, backend da
   Fase 6 pronto (com o caveat de instrumentação da A2). Maior "novidade visível".
2. **A1 Console MCP** — substitui um mock que já engana (mostra saúde falsa);
   backend pronto.
3. **A5 Breakdown por modelo** — extensão barata da Telemetria, dado já capturado.
4. **A6 Sandbox/skills de terceiro** e **A7 LSP** — completam o tema "código de
   terceiro / contexto de código" da Fase 6.
5. **A4 Rate-limit** — depende de backend novo; desenhar o alvo, sinalizar a
   dependência.

E, em paralelo (engenharia, não design): ligar o **Grupo B** às rotas reais — o
maior salto de "parece pronto" para "é pronto".
