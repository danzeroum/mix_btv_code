# Handoff: Forge — Telas de Usuário e Administrador

> Pacote de handoff para implementação em código real via Claude Code.
> Idioma do produto: **português** (identificadores em inglês, conforme `CLAUDE.md` do repo `danzeroum/mix_btv_code`).

---

## 1. Overview

**Forge** é um CLI/TUI de coding agent (núcleo Rust + sidecar Python via gRPC/UDS) que unifica
opencode + prompte + BuildToValue. Este handoff cobre a **camada visual/interativa** de todas as
superfícies de usuário e administrador:

- **Perfil Usuário** (superfícies de terminal/TUI): Primeiros passos, Sessão de código, Permissão,
  Modelo & Agente, Biblioteca de prompts, Squad ao vivo, **Squad Designer** (canvas editável) e
  Sugestões de interação.
- **Perfil Administrador** (dashboard web local `127.0.0.1`): Telemetria, Ledger/Auditoria,
  Verificação & Review, Providers & Rate limits, Skills & Permissões.
- **Sistema de temas renascentistas** (5 temas + cor de destaque editável, persistido por usuário).

O objetivo do desenvolvedor: **recriar estas telas no ambiente-alvo** e **fiar todos os eventos de
frontend** (cliques, navegação, toggles, drag, conexões, troca de tema) — **mesmo sem backend**,
usando dados mock/stubs e estado otimista. Onde o backend ainda não existe (Fases 4–6 do roadmap),
os botões devem funcionar contra stubs claramente marcados (ver §7).

---

## 2. Sobre os arquivos de design

Os arquivos em `design/` são **referências de design feitas em HTML** (um Design Component que roda
via `support.js`) — protótipos que mostram aparência e comportamento pretendidos, **não** código de
produção para copiar direto.

- `design/Forge Telas.dc.html` — protótipo completo, todas as telas + lógica interativa.
- `design/support.js` — runtime do protótipo (só para abrir o arquivo no navegador; **não** é
  dependência do app real).

Para ver rodando: abra `Forge Telas.dc.html` num navegador. Toda a lógica (estado, handlers, temas,
canvas) está na classe `Component` no fim do arquivo — leia-a como especificação de comportamento.

**Tarefa:** recriar estas telas no ambiente do codebase-alvo (React/Vue/Svelte/etc., seguindo os
padrões existentes). Se não houver frontend ainda, **React + TypeScript** é a escolha recomendada
(SPA local servida pelo `forge-server` axum para o perfil admin; o perfil usuário é conceitual —
representa a TUI ratatui e serve de referência de UX/telas para a versão web/documentação).

---

## 3. Fidelidade

**Alta fidelidade (hifi).** Cores, tipografia, espaçamento e interações são finais. Recrie
pixel-a-pixel usando os tokens da §8. As telas de terminal são estilizadas para *parecer* um
terminal — se forem viver dentro da TUI ratatui real, use-as como referência de layout/copy; se
forem para uma UI web (dashboard/documentação), recrie-as fielmente.

---

## 4. Shell global (moldura comum a todas as telas)

Layout raiz: **coluna** (`display:flex; flex-direction:column; min-height:100vh`), com id
`forge-root` (é nele que as variáveis de tema são aplicadas em runtime).

### 4.1 Topbar (sticky, `z-index:20`, `border-bottom:1px solid var(--line)`, `background:var(--bg2)`)
Da esquerda para a direita:
1. **Marca**: quadrado 26×26 `border-radius:7px` com gradiente `135deg, var(--rust) → var(--amber)`,
   letra "F" (`Space Grotesk` 700, 15px, cor `#1a1205`). Ao lado: "Forge" (Space Grotesk 700, 16px)
   + "· telas" (mono 11px, `var(--faint)`).
2. **Toggle de perfil** (segmented control): container `background:var(--panel)`,
   `border:1px solid var(--line)`, `border-radius:9px`, padding 3px. Dois botões: **▸_ Usuário** e
   **◨ Administrador**. Ativo = gradiente `135deg, var(--rust)→var(--amber)`, texto `#1a1205`,
   peso 600. Inativo = texto `var(--muted)`.
3. **Controle de tema** (`margin-left:16px`): label "TEMA" (mono 9.5px, uppercase, tracking .14em) +
   5 chips de tema (Forge, Veneziana, Ultramarino, Mármore, Afresco) + separador vertical 1px +
   7 amostras de cor de destaque (círculos 16×16).
4. **Meta** (`margin-left:auto`, mono 11.5px, `var(--faint)`): indicador de saúde (bolinha 7px
   `var(--ok)` pulsando, texto muda por perfil) + `danzeroum/mix_btv_code`.

### 4.2 Body (`display:flex; flex:1`)
- **Sidebar** (`width:262px`, `background:var(--bg2)`, `border-right:1px solid var(--line)`,
  padding `20px 14px`): heading (muda por perfil) + lista de itens de navegação. Cada item é um
  botão `display:flex; gap:11px; padding:10px; border-radius:9px`. Ativo:
  `background:var(--panel2); box-shadow:inset 2px 0 0 var(--rust)`. Ícone (mono 15px, largura 22px,
  cor própria do item) + stack de duas linhas: label (13px, 500, `white-space:nowrap; ellipsis`) e
  hint (mono 10.5px, `var(--faint)`, ellipsis). Rodapé fixo (`margin-top:auto`): legenda da regra de
  fronteira (■ Rust / ■ Python).
- **Stage** (`flex:1`, `overflow:auto`,
  `background:radial-gradient(circle at 50% -8%, var(--panel2), var(--bg) 60%)`):
  - Cabeçalho da tela: kicker (mono 11px uppercase, cor de destaque da tela) + título
    (Space Grotesk 600, 27px) + nota à direita (mono 11.5px, `var(--faint)`, max-width 360px).
  - **Janela** (a "chrome"): `border:1px solid var(--line2)`, `border-radius:12px`,
    `box-shadow:0 24px 60px -20px #000a`, `background:var(--surf)`, `min-height:560px`. Barra de
    chrome (`background:var(--bg2)`): 3 semáforos (11px: `#ff5f56/#ffbd2e/#27c93f`), pílula central
    (mono 11.5px) com ícone+título da tela, e texto à direita. Abaixo, a **superfície**:
    `background: var(--term)` para telas de terminal (perfil Usuário) ou `var(--surf)` para telas de
    browser (perfil Admin + Sugestões).

### 4.3 Navegação (itens por perfil)
**Usuário** (heading "SUPERFÍCIES DO USUÁRIO"):
`onboarding` (✦ Primeiros passos), `sessao` (▸ Sessão de código), `permissao` (⚿ Permissão),
`modelo` (◑ Modelo & Agente), `prompts` (❯ Biblioteca de prompts), `squad` (⧉ Squad ao vivo),
`designer` (⬒ Squad Designer), `sugestoes` (✧ Sugestões de interação).

**Admin** (heading "PAINÉIS DE ADMINISTRAÇÃO"):
`telemetria` (▦), `ledger` (⛓ Ledger/Auditoria), `verify` (✓ Verificação & Review),
`providers` (⇄ Providers & Limites), `skills` (⬡ Skills & Permissões).

---

## 5. Sistema de temas (fundo + estrutura de cores)

5 temas trocam **todas** as variáveis CSS (fundo, superfícies, painéis, linhas, texto e acentos).
Aplicados em runtime via `element.style.setProperty('--x', valor)` no `#forge-root`.

| Tema | Tipo | Caráter |
|---|---|---|
| **Forge** (`default`) | escuro | grafite neutro (padrão) |
| **Veneziana** | escuro | oxblood/umber, carmim+ouro+verde (a cor sobre a linha) |
| **Ultramarino** | escuro | azul-lápis profundo + ouro |
| **Mármore** | **claro** | branco de mármore, claro-escuro, ouro |
| **Afresco** | **claro** | pergaminho quente |

- **Chips de tema**: ativo = `background:rgba(128,128,128,.22); border:1px solid var(--line2); color:var(--ink)`;
  inativo = transparente, `var(--muted)`.
- **Cor de destaque** (7 amostras): sobrepõe **`--rust`** (acento primário) sobre qualquer tema.
  Cores: `null` ("do tema", círculo tracejado), `#3f6fd6` ultramarino, `#24408f` lápis-lazúli,
  `#b1372f` vermelho veneziano, `#c8972f` ouro de Ticiano, `#3f6b4f` verde, `#d8cdb4` mármore.
  Amostra ativa: `box-shadow:0 0 0 2px var(--ink)`.
- **Persistência (por usuário)**: `localStorage['forge_theme']` e `localStorage['forge_accent']`.
  Ler no mount, aplicar antes do primeiro paint; gravar a cada mudança. (No app real, isto pode
  virar preferência de usuário no perfil/config, mas localStorage é o mínimo aceitável.)

Valores completos das variáveis por tema estão na §8.3 (tabela de tokens).

---

## 6. Telas (layout, componentes, copy)

Todas as telas vivem dentro da "janela" (§4.2). Cada uma tem `kicker`, `title` e `note` no cabeçalho
e `chromeTitle`/`chromeIcon`/`chromeRight` na barra da janela — ver tabela `meta` no fim de
`Forge Telas.dc.html` para os textos exatos.

### 6.1 USUÁRIO

#### `onboarding` — Primeiros passos
- **Grid 2 colunas** (`1fr 1.1fr`), coluna esquerda com borda direita.
- **Esquerda — wizard de 4 passos** (cards empilhados, `gap:12px`):
  1. *Instalar* (concluído, borda/ícone `var(--ok)`, ✓): copy "Via cargo ou binário…" + bloco de
     código `$ cargo install forge`.
  2. *Chaves de API* (**etapa atual**, borda `var(--amber)`): 3 linhas de env var —
     `ANTHROPIC_API_KEY` (✓ definida, `sk-ant-••••9f3a`), `DEEPSEEK_API_KEY` (ausente · fallback),
     `OPENAI_API_KEY` (ausente · fallback). Aviso: "🔑 keys vivem SÓ no processo Rust…".
  3. *Sidecar Python* (opcional, cinza): "Com `uv` instalado, PromptForge e squad sobem sozinhos…".
  4. *Primeiro comando*: bloco `$ forge run "explique a estrutura deste repo" --agent plan`.
- **Direita — terminal "doctor"** (mono, `line-height:1.9`): saída simulada de `forge init` com
  linhas ✓/○ (env vars, uv, git, ledger) e sugestões de comandos, cursor piscando ao fim.
- **Interações a fiar**: os blocos de comando devem ter botão "copiar" (copiar para clipboard). Os
  campos de env var podem virar inputs (mascarados) num app real; no mínimo, indicar estado
  detectado. Botão implícito "concluir setup" → navega para `sessao`.

#### `sessao` — Sessão de código (a tela central)
- **Layout**: `flex` horizontal — coluna principal (transcript) + rail direito (`width:210px`).
- **Transcript** (mono 13px, `line-height:1.65`, `overflow:auto`): linha de cabeçalho da sessão
  (modelo/agente/providers/cache/sessão), turno do usuário (`você ▸`, prefixo `var(--py)` 600),
  aviso de lint (`var(--wire)`), resposta do agente (`forge ▸`, prefixo `var(--amber)`), chamadas de
  ferramenta (`⚒`/`✓`/`✗`), **bloco de diff** (`background:#0a0d12; border:1px solid var(--line)`;
  linhas `+` `var(--ok)`, `-` `var(--red)`, contexto `var(--faint)`), e cursor piscando.
- **Barra de status** (`border-top`, mono 11.5px): "⋯ concluído em 6 passos · 8 mensagens
  persistidas · ledger íntegro: 12 entradas ✓ · cache hit 41%".
- **Input** (`border-top`, `display:flex`): prompt `›` (`var(--amber)`), placeholder da mensagem,
  chip de atalhos "Enter envia · Esc sai · Tab modelo".
- **Rail direito**: seções "FERRAMENTAS" (read/grep = allow, edit/bash/webfetch = ask),
  "CONTEXTO" (época 2 · compaction 1×, janela 14k/200k com barra de progresso 7%), "ATALHOS"
  (↑↓ histórico, ^C cancelar, /compact, /prompt).
- **Interações a fiar**: enviar mensagem (Enter no input → adiciona turno do usuário ao transcript,
  dispara stream simulado do agente; ver §7 stub `streamAgent`); Tab abre `modelo`; clique numa
  ferramenta do rail abre a política (`skills`); toggle allow/ask por ferramenta.

#### `permissao` — Permissão interativa (modal)
- Transcript ao fundo **desfocado** (`filter:blur(1.5px); opacity:.4`).
- **Overlay** (`position:absolute; inset:0; background:#05070aa8`) + **modal** centralizado
  (`width:520px; border:1px solid var(--wire); border-radius:14px`): cabeçalho "Permissão
  solicitada" (bolinha `var(--wire)` com glow) + "forge-core · não contornável"; corpo com o pedido
  (`⚒ bash` + escopo `$ python -m pytest tests/test_users.py`), metadados (diretório/rede/ledger),
  e **3 botões**: `[ s ] Permitir` (`background:var(--ok)`), `[ n ] Negar`, `[ a ] Sempre p/ bash`.
- **Interações a fiar**: `s`/`n`/`a` (também via teclado) resolvem a promessa de permissão, fecham o
  modal e registram uma entrada no ledger (stub). `a` grava a regra "allow" para aquela ferramenta.

#### `modelo` — Modelo, agente & autonomia
- **Grid 2 colunas** (`1fr 1fr`).
- **Esquerda — ModelTier** (3 cards): `small` (haiku·deepseek-chat, "step-discipline"),
  `medium` (gpt-4o·sonnet), `large` (claude-sonnet-5·opus, **selecionado**, borda `var(--rust)`).
- **Direita**: *Perfil de agente* (2 cards: `build` ativo borda `var(--ok)`, `plan` somente leitura)
  + *Nível de autonomia* (3 linhas: Interativo/Automático (em dev)/Somente leitura) + rodapé
  (janela 200k · cache on · compaction ~75% tier-gated).
- **Interações a fiar**: selecionar tier (radio, atualiza estado + header da sessão), selecionar
  agente (build/plan → muda matriz de permissões e capacidade de edição), selecionar autonomia.

#### `prompts` — Biblioteca de prompts (`/prompt`)
- **Layout**: coluna principal (saída de comandos) + aside direito (`width:340px`, preview do prompt
  renderizado).
- **Principal**: `> /prompt list` → lista de geradores (borda esquerda `var(--teal)`): code-review,
  test-gen, refactor, commit-msg, adr-draft (nome·[categoria]·campos). `> /prompt library` → prompts
  salvos (borda `var(--wire)`): `#id nome ★ [gerador] tags`.
- **Aside**: `> /prompt use 2 ★` + bloco renderizado (code-review) + chips de ação
  (save / fav ★ / use / rm).
- **Interações a fiar**: clicar num gerador → renderiza no aside; salvar → adiciona à biblioteca;
  fav → alterna ★; rm → remove; use → mostra no aside; copiar prompt.

#### `squad` — Squad ao vivo (Fase 4)
- Linha de comando: `> forge squad "migre o módulo de pagamentos…"`.
- **Grid** (`1.4fr 1fr`). **Esquerda — 5 agentes** (cards com bolinha de status, nome, estado,
  `conf X.XX`, tarefa): Architect (concluído), Developer (executando), Auditor (aguardando),
  Designer (ocioso), Ops (aguardando). **Direita** (stack): *Consenso ponderado* (0.82, barra,
  "decisão: developer · divergência: auditor 0.19"); *Gate HITL* (borda `var(--amber)`, ação crítica
  + botões Aprovar/Rejeitar); *Fallback progressivo* (squad → agente-único → safe-mode; "sidecar
  saudável").
- **Interações a fiar**: Aprovar/Rejeitar no HITL (resolve o gate, atualiza estado do agente e
  ledger); clicar num agente mostra detalhe; barras animam ao montar.

#### `designer` — Squad Designer (canvas editável — conceito Fase 4+)
Ver §7.2 (é a tela mais interativa). Toolbar (modos + reset + salvar) · paleta (blocos) · **canvas**
(nós arrastáveis + arestas SVG) · painel de propriedades.

#### `sugestoes` — Sugestões de interação
- **Card destaque** (borda `var(--py)`): "Squad Designer — desenhe o workflow, o código segue" +
  botão "Abrir conceito →" (navega para `designer`).
- **Grid 3 colunas** de 6 propostas (ícone, título, tag, descrição, âncora no código):
  Revisor de diff, Replay de sessão, Aprovação em lote, Modo watch, A/B de prompts, Mapa de memória.
- **Interações a fiar**: "Abrir conceito" → `designer`. Cards podem linkar para a tela relacionada.

### 6.2 ADMIN (dashboard web local)

#### `telemetria` — `127.0.0.1:7878`
- 4 **stat cards** (eventos totais 1.284, cache hit 41.2%, chamadas llm 312, execuções 706).
- **Grid** (`1fr 1.6fr`): *eventos por tipo* (barras: tool.result 706, llm.call 312, cache.hit 129,
  cache.miss 183, compaction 14) + *eventos recentes* (tabela ts/nome/sessão/props).
- Rodapé: "offline-first · escuta só em 127.0.0.1 · atualiza a cada 5s".
- **Interações a fiar**: auto-refresh a cada 5s (poll do endpoint `/api/summary` e `/api/events` —
  ver §7 stub); ordenar/filtrar a tabela. As rotas reais existem em `forge-server/src/lib.rs`.

#### `ledger` — Ledger append-only / Auditoria
- Banner de integridade (`border:1px solid var(--ok)`): "Cadeia de hash íntegra — 247 entradas
  verificadas…" + "política Nada Fake".
- **Tabela** (grid `52px 140px 96px 1fr 130px 92px`): seq, ts, ator (cor por ator: build=ok,
  humano=wire, auditor=py), ação, hash (`prev→curr`), flags (badge `override` quando aplicável).
- **Interações a fiar**: filtrar por ator/tipo; verificar integridade (botão → recomputa hash-chain,
  stub); clicar numa entrada → detalhe. Nunca há edição/exclusão (append-only).

#### `verify` — Verificação & review por valor
- **Grid 2 colunas**. Esquerda: *Pipeline /verify* (6 passos com ✓ e detalhe: cargo test, clippy,
  rustfmt, pytest, paridade de hash, gitleaks) + banner self-hosting. Direita: *Review por valor*
  (gauge value_score 0.86 · gate > 0.70 · badge CERTIFICADO; 4 reviewers com barras: qualidade .90,
  segurança .84, valor .88, manutenção .82; quality gates ✓).
- **Interações a fiar**: rodar /verify (botão → estados running→pass/fail por passo, stub); expandir
  evidência JSON; ver detalhe de reviewer.

#### `providers` — Providers & rate limits
- **Grid 2 colunas**. Esquerda: *Gateway LLM · ordem de fallback* (Anthropic ativo, DeepSeek
  standby, OpenAI standby) + aviso "🔑 keys só no Rust". Direita: *Rate limiting tier-gated* (small
  12/120, medium 34/60, large 18/30, com barras) + nota "hit de cache nunca consome vaga".
- Faixa inferior: cache hit 41.2% · JCS RFC 8785+sha256 · paridade 50/50 · SSE.
- **Interações a fiar**: reordenar fallback (drag), ativar/desativar provider (toggle), editar
  limites por tier (stub, persiste em estado).

#### `skills` — Skills, MCP & permissões
- **Grid** (`1.1fr 1fr`). Esquerda: *Skill-vetter* (sql-explain aprovado, docker-scan aprovado,
  net-crawler **bloqueado**, k6-load em análise) + *Servidores MCP* (filesystem/git ok, postgres
  amarelo). Direita: *Política de permissões* (tabela ferramenta × build × plan, com cores
  allow/ask/deny) + *Saúde do sidecar* (forge-squadd saudável, gRPC/UDS, fallback squad).
- **Interações a fiar**: aprovar/bloquear skill (muda status + ledger); alternar célula da matriz de
  permissões (allow↔ask↔deny) por agente; reconectar MCP.

---

## 7. Interações & eventos — **fiar tudo, mesmo sem backend**

Regra geral: **todo botão, link, chip, toggle, drag e campo deve ter handler**. Onde não há backend,
use um **adaptador de dados** (`api/*`) que hoje retorna mocks/Promises resolvidas e amanhã aponta
para os endpoints reais. Nada pode ser um botão morto.

### 7.1 Estado global (mínimo)
- `persona`: `'user' | 'admin'` — controla sidebar e qual tela abre por padrão
  (`user`→`sessao`, `admin`→`telemetria`).
- `screen`: id da tela ativa. Ao trocar de persona, se a tela atual não pertence à persona, cai na
  primeira da lista.
- `theme`: `'default'|'veneziana'|'ultramarino'|'marmore'|'afresco'` (persistido).
- `accent`: hex ou `null` (persistido).

### 7.2 Squad Designer — modelo de interação (o mais complexo)
Estado: `nodes[]`, `edges[]`, `mode:'select'|'connect'`, `selectedNode`, `pendingConnect`,
`dragId`, `grabDX/grabDY`, `addCount`, `wfSaved`.

- **Arrastar nó** (modo `select`): `mousedown` no nó grava `dragId` + offset relativo ao board
  (`getBoundingClientRect` do `#sqd-board`). Listeners `mousemove`/`mouseup` em `window`
  (adicionados no mount, removidos no unmount) atualizam `x/y` do nó com **clamp** aos limites
  (board 720×470; card 104×62, pill 60×30). Qualquer edição zera `wfSaved`.
- **Criar conexão** (modo `connect`): 1º clique = `pendingConnect` (origem, anel âmbar); 2º clique =
  cria aresta `{from,to}` (se não existir); clicar no mesmo cancela. Cursor `crosshair` no modo.
- **Arestas dinâmicas**: calculadas a cada render por interseção reta↔borda do retângulo
  (`computeEdges`), então **acompanham os nós** ao arrastar. Arestas de/para `hitl` saem em **âmbar
  tracejado**; demais em cinza. Rótulos opcionais no ponto médio.
- **Adicionar nó**: clicar num item da paleta (`Architect/Developer/Auditor/Designer/Ops/Consenso/
  Gate HITL`) cria um nó com os pesos-modelo daquele tipo (ver `templates()`), id único, posição
  escalonada, e o seleciona.
- **Remover nó**: no painel de propriedades, "✕ remover nó & conexões" (a entrada `task` é
  protegida) remove o nó e todas as arestas conectadas.
- **Propriedades**: refletem o nó selecionado (nome, sub, parâmetros/pesos). Pesos vêm de
  `consensus.py` (ver §9) — mantenha os valores fiéis.
- **Reset**: restaura grafo inicial. **Salvar & aplicar**: marca `wfSaved=true` e mostra o pipeline
  simulado `squad.workflow.v1 → schema → ledger seq 248 → orquestrador aplica`. No app real, POST do
  grafo serializado (ver §7.4 `saveWorkflow`).

### 7.3 Mapa de eventos por tela (resumo)
| Tela | Elemento | Evento → efeito (stub se sem backend) |
|---|---|---|
| Global | toggle perfil | troca `persona`, reseta `screen` |
| Global | item da sidebar | troca `screen` |
| Global | chip de tema | `setTheme` → aplica vars + persiste |
| Global | amostra de destaque | `setAccent` → sobrepõe `--rust` + persiste |
| onboarding | botões "copiar"/comandos | copiar p/ clipboard; concluir → `sessao` |
| sessao | input Enter | append turno + `streamAgent()` (stub) |
| sessao | Tab / ferramenta rail | abre `modelo` / política em `skills` |
| permissao | s / n / a | resolve permissão, fecha modal, grava ledger |
| modelo | tier / agente / autonomia | atualiza estado + header da sessão |
| prompts | gerador / save / fav / use / rm | render/CRUD na biblioteca (stub) |
| squad | Aprovar / Rejeitar (HITL) | resolve gate, atualiza agente + ledger |
| designer | (ver §7.2) | drag / connect / add / remove / save / reset |
| sugestoes | "Abrir conceito" / cards | navega p/ `designer`/tela relacionada |
| telemetria | auto-refresh 5s / filtros | poll `getSummary()`/`getEvents()` (stub) |
| ledger | filtros / verificar | filtrar; `verifyChain()` (stub) |
| verify | rodar /verify | estados por passo via `runVerify()` (stub) |
| providers | toggle / reordenar / limites | atualiza estado (stub persist) |
| skills | aprovar/bloquear / matriz / MCP | muda status + ledger (stub) |

### 7.4 Camada de dados (stubs → backend real)
Crie um módulo `api/` com funções assíncronas retornando mocks agora. Endpoints reais conhecidos
(do repo) para quando o backend atender:
- `GET /api/summary` → cards de telemetria (**existe** em `forge-server`).
- `GET /api/events?limit=N` → eventos recentes (**existe**).
- Ledger, verify, squad, providers, skills, workflow: **ainda não expostos** (Fases 4–6). Defina
  contratos otimistas: `getLedger()`, `verifyChain()`, `runVerify()`, `runSquad(task)`,
  `resolveHITL(id, ok)`, `getProviders()`, `setRateLimit(tier, cap)`, `listSkills()`,
  `vetSkill(id, decision)`, `saveWorkflow(graph)`, `renderPrompt(gen, fields)`, `savePrompt(...)`.
  Todas resolvem Promises com mock e marcam `// TODO: backend Fase N`.

**Estados de UI obrigatórios** para cada ação assíncrona: `idle → loading → success | error`
(spinners/skeletons + toast de erro). Nenhuma ação deve parecer travada.

---

## 8. Design tokens

### 8.1 Tipografia
- **Display**: `Space Grotesk` (400–700) — títulos, marca, números grandes.
- **Sans**: `IBM Plex Sans` (400–600) — corpo/labels.
- **Mono**: `IBM Plex Mono` (400–600) — terminal, código, metadados, tabelas técnicas.
- Escalas usadas: título de tela 27px/600; H2 seção 15px/600; corpo 12.5–14px; mono 10.5–13px;
  números de destaque 30–34px/700. `letter-spacing` negativo (−.01 a −.03em) em displays;
  positivo (.08–.16em) + uppercase em kickers/labels mono.

### 8.2 Raios, sombras, bordas
- Raios: chips 5–7px; botões 8–9px; cards 10–12px; janela 12px; pílulas 16px; círculos 50%.
- Sombra da janela: `0 24px 60px -20px #000a`. Glow de nó selecionado:
  `0 0 0 3px #4d9fff22, 0 8px 24px -8px #000c` (pendente: `#f0a13c33`).
- Bordas: `1px solid var(--line)` (padrão), `var(--line2)` (destaque).

### 8.3 Cores por tema (variáveis CSS `--nome`)
Aplicadas no `#forge-root`. Papéis semânticos: `bg` fundo raiz · `bg2` sidebar/topbar/chrome ·
`surf` superfície de janela (browser) · `term` superfície de terminal · `panel`/`panel2` cards ·
`line`/`line2` bordas · `ink`/`muted`/`faint` texto · `rust` acento primário · `amber/teal/py/wire/
ok/red` acentos semânticos.

**default (Forge, escuro)**
`bg #07090d · bg2 #0b0e13 · surf #0f1115 · term #07090d · panel #12151c · panel2 #171b24 · line #242b37 · line2 #2e3644 · ink #e8ecf3 · muted #8b95a7 · faint #5b6474 · rust #f2683c · amber #f0a13c · teal #2fb8a0 · py #4d9fff · wire #a78bfa · ok #43c463 · red #f2544f`

**veneziana (escuro)**
`bg #140b0a · bg2 #1a0e0b · surf #1c110e · term #170b09 · panel #231512 · panel2 #2b1a15 · line #3a241d · line2 #4a2f25 · ink #f3e7dc · muted #c0a189 · faint #8a6b57 · rust #c8452f · amber #d99a2b · teal #4f7a52 · py #7f88ad · wire #9b6bb0 · ok #6b8f4e · red #c0392b`

**ultramarino (escuro)**
`bg #080d1c · bg2 #0a1122 · surf #0d1428 · term #060a18 · panel #111a33 · panel2 #16203f · line #22304f · line2 #2d3d60 · ink #e6ecf7 · muted #93a3c4 · faint #5d6d90 · rust #3f6fd6 · amber #d8a63a · teal #3aa0a6 · py #5b8def · wire #8f7fd6 · ok #4fae7a · red #d65a52`

**marmore (claro)**
`bg #e9e3d5 · bg2 #e0d8c6 · surf #f4efe4 · term #efe9dd · panel #f0eadd · panel2 #e7e0cf · line #d6ccb8 · line2 #c7bca2 · ink #2b251c · muted #6f6553 · faint #9a8f78 · rust #b0532f · amber #b3852f · teal #4c7550 · py #3c6ac9 · wire #7c5896 · ok #5c7a44 · red #b0442c`

**afresco (claro)**
`bg #efe4cf · bg2 #e8dcc2 · surf #f6ecd9 · term #f3ecdb · panel #f2e9d6 · panel2 #ebe0c9 · line #ddceae · line2 #cdbc98 · ink #31271b · muted #786a52 · faint #a5967a · rust #bd5329 · amber #c2922b · teal #57794a · py #48699f · wire #875f92 · ok #657f45 · red #b0402a`

**Cores de destaque (sobrepõem `--rust`)**: `#3f6fd6, #24408f, #b1372f, #c8972f, #3f6b4f, #d8cdb4`
(+ `null` = usa o `--rust` do tema).

**Literais intencionais** (não trocam com tema — são "código/terminal embutido"): blocos de código
`#0a0d12`; semáforos da janela `#ff5f56 / #ffbd2e / #27c93f`; overlay do modal `#05070aa8`.

### 8.4 Animações (keyframes)
- `fblink` — cursor piscando (1.1s infinite).
- `fpulse` — bolinha de saúde/status (2.4s infinite, opacidade .4↔1).
- `fbar` — barra de consenso cresce ao montar (1s ease).
- Transições: hovers/toggles `.12–.15s`.

---

## 9. Fidelidade aos dados do código (não inventar)
Os valores nas telas espelham o repo `danzeroum/mix_btv_code` — mantenha-os:
- **Pesos do consenso** (`python/.../consensus.py` → `DEFAULT_AGENT_WEIGHTS`): architect
  {architecture .9, security .7}; developer {architecture .6, implementation .95, testing .8};
  auditor {security .95, quality .85}; designer {ui .95, ux .9}; ops {infrastructure .9,
  deployment .9}. Limiar HITL de escalonamento = **0.70**.
- **Autonomia** (`hitl.py`): níveis 0–3 (full_human_control → full_autonomy) por trust score;
  aprovação via `PermissionClient`; rejeição reduz trust em −0.10.
- **Comandos/flags** (`forge-cli/src/main.rs`): `run`, `chat`, `tui`, `verify`, `squad`,
  `dashboard`; flags `--model --agent --yes --no-cache --session --context-window`.
- **Providers/keys**: keys só no processo Rust; Python nunca chama LLM (gRPC `CoreService.Generate`).
- **Telemetria**: rotas `/`, `/api/summary`, `/api/events` já implementadas em
  `forge-server/src/lib.rs` (offline-first, só 127.0.0.1).

---

## 10. Assets
Nenhum binário. Ícones são **glifos Unicode** (▸ ⚿ ◑ ❯ ⧉ ⬒ ✧ ▦ ⛓ ✓ ⇄ ⬡ ◈ ⚒ ✎ ⛭ ◇ ⚑ …) — no app
real, substituir por um icon set consistente (ex.: Lucide/Phosphor) mantendo o significado. Fontes
via Google Fonts (Space Grotesk, IBM Plex Sans, IBM Plex Mono). A marca "F" é um quadrado com
gradiente + letra (recriável em CSS/SVG).

---

## 12. Telas do Grupo A (Fase 6) — novas superfícies

> Estas telas cobrem o **Grupo A** de `docs/LEVANTAMENTO-UI-DESIGNER.md` (funcionalidades da Fase 6
> com backend, sem tela). Todos os dados abaixo são **aterrados no código real** do repo — mantenha
> os valores. No frontend real, cada uma segue o padrão `AsyncStatus` sobre `web/src/api/<x>.ts`.
> Registrar tela = tocar `nav.ts` + `screenMeta.ts` + `screenComponents.tsx` + `types/domain.ts`.

### A1. Console MCP (admin) — `mcp`
Substitui o "pontinho de saúde" mock da tela Skills. Duas colunas:
- **Servidores** (`.forge/mcp.toml`, loader `forge-cli/src/skills.rs::load_mcp_servers`): id · transporte
  (stdio/sse) · status (conectado/degradado/desconectado) · nº de tools. Conexão é **por chamada**
  (connect→call→encerra, `mcp.rs`). Ação: (re)conectar. Não há "aprovar servidor" — confiança é a
  declaração no `.toml` (ADR 0012).
- **Tools expostas**: nome **namespaced** `mcp__<server>__<tool>` (o que o agente vê), `input_schema`
  resumido, e **política por chamada** (allow/ask/deny). Cada chamada MCP passa pelo gate do core Rust
  (scope `mcp:<server>/<tool> <preview>`, `mcp.rs:52-56`) — nunca auto-aprovada.
- **Backend a expor:** `GET /api/mcp` (servidores + `McpToolMeta{name,description,input_schema}` + status).

### A2. Experimentos A/B (admin) — `experimentos`
Relatório `forge experiment` (`forge-schemas/src/experiment.rs`): `ExperimentReport{experiment, metric,
variants[], verdict, winner?, p_value}`; cada `VariantStats{variant, n, successes, rate}`.
- Duas barras (A×B) com rate/n; **veredito honesto** em 3 estados: `Significant` (com vencedor),
  `Inconclusive` (**sem** vencedor, mesma dignidade), `InsufficientData` (< 20/variante). `p_value` vs α=0.05.
- **⚠ Caveat (no design):** nenhum código de produção ainda escreve a telemetria de atribuição
  (`props.experiment/variant/success`) — só testes e `examples/seed_telemetry.rs`. A tela mostra **dados
  semeados** até a instrumentação existir. O banner de aviso comunica isso.
- **Backend a expor:** `GET /api/experiment/<nome>` (hoje CLI-only, `main.rs:178`).

### A3. Mapa de memória do squad / RAG (usuário) — `memoria`
`python/packages/forge-squad/.../memory.py`. Duas colunas:
- **Memória por agente**: registros `{timestamp, agent, decision, confidence}` agrupados por agente,
  com **esquecimento inteligente** (decay). Persistido em `.forge/squad-memory/agent_memories.jsonl`.
- **Busca (recall) FUNCIONAL:** input + botão ↵. Estados **idle → loading → done(results|empty)**.
  Retorna lista ranqueada por `score` (0–1) mostrado como número. É recuperação **léxica TF-IDF**
  (ADR 0013), não semântica — o texto do rodapé é honesto sobre "por termos". No protótipo o recall é
  simulado client-side (`runRecall()`); no real: `CoreService.Recall` via gRPC lendo `memory.py:110-131`.
- **Backend a expor:** rota (via gRPC/servidor) para ler o corpus + rodar recall.

### A4. Rate limits por tier (admin) — `ratelimit`
`forge-llm/src/rate_limit.rs`. Três cards (Small 60 / Medium 30 / Large 15, **janela de 600s**) com
barra used/cap. Salvaguarda de **custo** (não defesa multiusuário): small generoso, large conservador.
- **⚠ Caveat (banner):** o `RateLimiter` **não expõe getter de uso** (`poll` é privado) — o consumo é
  ilustrativo até a engenharia expor a superfície. Os tetos são reais. Hit de cache não consome vaga.

### A5. Uso por modelo (admin) — `modelos`
Extensão da Telemetria. Tabela por modelo: volume de chamadas (barra) + cache-hit %, agrupado de
`props.model` (já presente em cada evento `llm.call`/`cache.*`). Falta só um `summary` que agrupe por
modelo (hoje agrupa por nome do evento, `telemetry.rs:83-109`).

### A7. Language servers / LSP (admin) — `lsp`
`forge-tools/src/lsp.rs`. Servidores declarados em `.forge/lsp.toml` (rust-analyzer/pyright/tsserver):
status (indexado/indexando/preguiçoso), nº de diagnósticos. **Sessão preguiçosa e reusada** (≠ MCP):
sobe no 1º uso, reaproveita o processo indexado, morto no `Drop`. Consultas expostas como tools:
`lsp__<id>__{definition,references,diagnostics}` (posições 0-indexed), sob o mesmo motor de permissões
(scope `lsp:<id>/<query> <file>`).

### A6. Sandbox & skills de terceiro (admin) — `sandbox`
`forge-tools/src/sandbox.rs` (bollard). Duas colunas:
- **Perfil de confinamento** (default `Sandbox::new`): `python:3.11-slim`, rede **none**, mem **512 MB**,
  cpu **0.5**, timeout **30s**, rootfs **read-only** (único mount `/work` gravável), `cap-drop ALL` +
  `no-new-privileges`. **Fail-closed:** sem daemon Docker → erro claro, nunca um "rodou" silencioso.
- **Ciclo de vida de skills de terceiro:** instalar (`.forge/skills/`) → vetar (skill-vetter bloqueante,
  ADR 0009/0011) → habilitar/remover. Estados: habilitada / em análise / bloqueada (ex.: pede rede → nega).
  Skill de terceiro roda no sandbox; built-in confiável segue o caminho não-containerizado.

> **Ordem no menu admin:** Telemetria · Uso por modelo · Experimentos A/B · Rate limits · Console MCP ·
> Language servers · Sandbox & skills · Ledger · Verificação · Providers · Skills. (Usuário: +Mapa de memória.)

---

## 11. Arquivos deste pacote
- `README.md` — este documento (auto-suficiente).
- `design/Forge Telas.dc.html` — protótipo de referência (todas as telas + lógica).
- `design/support.js` — runtime do protótipo (apenas para abrir no navegador).

> Leia a classe `Component` no fim de `Forge Telas.dc.html` como a especificação viva de estado e
> handlers: `THEMES`, `templates()`, `initialNodes/initialEdges`, `computeEdges`, `onNodeDown`,
> `connectClick`, `addNode`, `removeNode`, `applyTheme`, `setTheme/setAccent`, e o `renderVals()`
> que expõe todos os dados e callbacks de cada tela.
