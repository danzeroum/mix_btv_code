# Handoff de Desenvolvimento — BuildToValue (evolução do `mix_btv_code`)

> **Objetivo deste documento:** dar a um desenvolvedor tudo que ele precisa para implementar,
> **sem abrir nenhum outro repositório**. Todo caminho de arquivo, contrato, assinatura, trecho
> de código de origem e passo de verificação está inlinado aqui. Onde citamos um repo de origem
> (btvChatCorp, squadIA, etc.), o **padrão relevante já está copiado no texto** — o repo é só a
> proveniência, não uma dependência de leitura.
>
> **Base de análise:** leitura profunda de 11 repositórios irmãos (btvChatCorp, squadIA,
> SquadIAds, buildtovalue-governance, silent-decisions-proof, BuildToValue, BuildToValueV7,
> BuildToValueIDE, BuildToValuePython, buildtovalue-factory) cruzada com o estado atual do
> `mix_btv_code`. A síntese executiva está em `docs/handoff/desenvolvimento/` (este arquivo);
> o roadmap resumido também.
>
> **Branch de trabalho:** `claude/buildtovalue-repo-analysis-ek8eje`.
> **Convenções do repo:** código/comentários em português, identificadores em inglês; testes
> junto do módulo (Rust `#[cfg(test)]`, Python `tests/`); contrato tem fonte única em `schemas/`.

---

## 0. Contexto e norte do produto

O `mix_btv_code` (codinome interno "Forge") **passa a se chamar BuildToValue** — o nome oficial
que ele herda do produto Python original. O produto migra de *agente de código para
desenvolvedores* para uma **plataforma onde profissionais de qualquer área montam squads de
agentes de IA** que rodam uma **esteira** (linha de produção): **briefing → produzir → revisar →
exportar** entregas reais (DOCX, XLSX, MusicXML, MIDI, PDF, SVG…).

Cinco princípios de experiência (dos documentos de design anexados ao pedido):
1. **A esteira é a interface.** Linha de produção horizontal; nada de logs/tokens/grafos na visão
   padrão.
2. **Papéis humanos, não agentes.** Pauteiro, Redator, Revisor, Copista — a equipe espelha
   equipes reais da profissão.
3. **O humano é um gate, não um espectador** — e, com este handoff, também um **membro** da squad.
4. **A entrega é o produto.** Artefatos exportáveis com versão e procedência.
5. **Complexidade tem endereço próprio.** Telemetria/ledger/providers/permissões vivem no perfil
   Admin.

A **filosofia herdada** (tem que sobreviver ao rename): *"Squad sobre Solo → Sinergia →
Velocidade → **Valor**"*, *"Valor sobre Complexidade"*. A esteira **é** essa cadeia de valor.

**O pedido que originou este handoff:** além de incorporar as melhores ideias dos repos, colocar
um **chat junto ao squad onde o usuário é um membro ativo** — "não apenas o aprovador, mas o outro
item da squad". Essa é a **Fase 1** e o foco principal.

---

## 1. Arquitetura atual do `mix_btv_code` (o que já existe)

Você vai **estender**, não reconstruir. Entenda este encadeamento primeiro.

### 1.1 Fronteira Rust × Python (ADR 0001)
- **Rust** (`crates/`): tudo que toca disco/rede/processo/segredo ou roda a cada keystroke — CLI/
  TUI, sessões, gateway LLM (API keys **só** aqui), ferramentas, permissões, `/verify`, storage,
  ledger, servidor web (`forge-server`, axum).
- **Python** (`python/packages/`): tudo que decide o próximo passo por raciocínio de agente —
  `forge-squad` (orquestrador multi-agente), `forge-promptforge`, `forge-review`, `forge-eval`.
  **Python nunca chama provedor LLM direto** — sempre via `CoreService.Generate` (gRPC).
- **Integração:** gRPC bidirecional sobre Unix Domain Socket (`tonic`/`prost` × `grpcio`).

### 1.2 O caminho do squad ao vivo (é AQUI que a Fase 1 mexe)
Fluxo ponta-a-ponta de uma execução de squad pelo navegador, com os arquivos reais:

```
Navegador (web/)                Rust (crates/forge-cli/src/squad_agent.rs)         Python (forge-squad)
─────────────────               ───────────────────────────────────────────       ────────────────────
POST /api/squad/run   ────────► run_squad_handler
  {task}                          → SquadHub::new_task() → "sq1"
                                  → tokio::spawn(run_squad_task(...))
                                       │ gRPC ExecuteTask(SquadTask) ───────────►  server.py::ExecuteTask
◄──────────── 202 {task_id}          │                                              → UnifiedOrchestrator
                                     │                                                 .execute_complex_task(
GET /api/squad/{id}/events           │                                                    task, event_sink=sink)
  (EventSource / SSE)  ────────► squad_sse_handler                                    → emite dict events:
◄════ stream SquadEvent ═══════   ← SquadHub::publish(task_id, ev) ◄══ SquadEvent ══   {"kind":"proposal"|...}
                                     │        (_to_squad_event mapeia dict→proto)
                                     │
                    (quando consenso fraco / permissão)                              orchestrator emite
POST /api/squad/{id}/hitl  ───►  resolve_hitl_handler                                {"kind":"hitl", ...} e
  {allow}                          → SquadHub::resolve_hitl(id, allow)               o backend Rust bloqueia
                                     (destrava o oneshot pendente)                   em request_permission →
                                                                                     SquadHub::request_hitl
```

**Arquivos-chave (leia antes de codar):**
- `crates/forge-cli/src/squad_agent.rs` — `SquadHub`, handlers HTTP, `router()`, `run_squad_task`,
  `WebSquadCoreBackend`. É o coração da Fase 1.
- `schemas/proto/squad.proto` — contrato `SquadService.ExecuteTask(SquadTask) → stream SquadEvent`.
- `schemas/proto/core.proto` — `CoreService` (o back-channel Python→Rust): `Generate`, `RunTool`,
  `AppendLedger`, `Recall`/`Remember`, `RequestPermission`.
- `python/packages/forge-squad/src/forge_squad/server.py` — implementa `ExecuteTask`; mapeia os
  dicts do orquestrador para `SquadEvent` (`_to_squad_event`).
- `python/packages/forge-squad/src/forge_squad/orchestrator.py` — `UnifiedOrchestrator.
  execute_complex_task(task, event_sink)`; é onde os agentes propõem e o consenso decide.
- `web/src/api/squad.ts` — cliente do frontend (`runSquad`, `resolveHitl`, `connectSquadEvents`).
- `web/src/components/screens/user/Squad.tsx` — a tela do squad ao vivo.

### 1.3 Contratos `SquadEvent` e `SquadTask` (estado atual — `schemas/proto/squad.proto`)
```proto
syntax = "proto3";
package forge.squad.v1;

service SquadService {
  rpc ExecuteTask(SquadTask) returns (stream SquadEvent);
  rpc Health(HealthRequest) returns (HealthResponse);
}

message SquadTask {
  string task_id = 1;
  string description = 2;
  string decision_type = 3;              // ex.: "architecture"
  uint32 max_autonomy_level = 4;         // IGNORADO ponta-a-ponta hoje (ADR 0021)
  string verification_evidence_json = 5; // verification-evidence.v1; fail-closed se ausente
}

message SquadEvent {
  string task_id = 1;
  string ts = 2;
  oneof payload {
    Proposal proposal = 3;
    Consensus consensus = 4;
    Handoff handoff = 5;
    HitlEscalation hitl = 6;
    StepResult step = 7;
    string error = 8;
  }
}

message Proposal   { string agent = 1; double confidence = 2; string content_json = 3; }
message Consensus  { string decision_maker = 1; double strength = 2; string decision_json = 3; bool requires_human = 4; }
message Handoff    { enum Phase { PHASE_UNSPECIFIED=0; START=1; ACK=2; COMPLETE=3; ERROR=4; } Phase phase=1; string from_agent=2; string to_agent=3; string contract=4; string payload_digest=5; }
message HitlEscalation { string reason = 1; double confidence = 2; }
message StepResult { string step_id = 1; bool success = 2; string summary = 3; }
```
Regras de contrato (obrigatórias): **protos evoluem só aditivamente**; mudança breaking = novo
arquivo `.v2` + ADR novo. Novos campos usam **os próximos números de tag livres** (no `oneof
payload`, o próximo é **9**).

### 1.4 O `SquadHub` (Rust) — mecanismo que você vai reusar como molde
Assinaturas reais atuais em `crates/forge-cli/src/squad_agent.rs`:
```rust
pub struct SquadHub { /* tasks: Arc<Mutex<HashMap<String, SquadTaskState>>>, hitl_timeout, next_task_seq */ }

impl SquadHub {
    pub fn new(hitl_timeout: Duration) -> Self;
    fn new_task(&self) -> String;                                  // gera "sq{seq}", cria estado
    pub fn publish(&self, task_id: &str, event: SquadEvent);       // append no log + broadcast
    fn finish_task(&self, task_id: &str);                          // dropa o Sender → SSE fecha
    fn subscribe(&self, task_id: &str) -> (Vec<SquadEvent>, Option<broadcast::Receiver<SquadEvent>>);
    async fn request_hitl(&self, task_id: &str) -> bool;           // BLOQUEIA no oneshot até resposta/timeout (fail-closed: nega)
    fn resolve_hitl(&self, task_id: &str, allow: bool) -> Result<(), ()>; // destrava o oneshot pendente
}
```
`SquadTaskState { log: Vec<SquadEvent>, tx: Option<broadcast::Sender<SquadEvent>>, pending:
Option<PendingHitl> }`. O `PendingHitl { responder: oneshot::Sender<bool> }` é o padrão exato de
"back-channel bloqueante do Rust esperando uma ação HTTP do usuário" — a Fase 1 replica ISSO para
mensagens de chat.

### 1.5 Como o orquestrador Python emite eventos hoje (`orchestrator.py`)
```python
async def execute_complex_task(self, task, event_sink=None):
    self._event_sink = event_sink
    plan = await self.planner.create_adaptive_plan(task)
    proposals = await self._get_squad_proposals(plan)          # architect/developer/auditor/... → emite "proposal"
    consensus = self.consensus.reach_consensus(proposals, "architecture")
    await self._emit({"kind":"consensus", "decision_maker":..., "strength":..., "requires_human":...})
    if consensus.requires_human:
        await self._emit({"kind":"hitl", "reason":"weak_consensus", "confidence":...})
        approval = await self.autonomy.execute_with_autonomy(...)   # chama RequestPermission (gRPC) → bloqueia no Rust
        if not approval.get("executed", False): return {...}        # reprovado → aborta
    execution_results = await self._execute_plan_steps(plan, task)  # emite "step"
    ...

async def _emit(self, event): 
    if self._event_sink is not None: await self._event_sink(event)
```
E `server.py::_to_squad_event` mapeia `{"kind": ...}` → `SquadEvent`. **Ponto crítico:** o
orquestrador é um **pipeline determinístico** (plano → propostas → consenso → passos). Não há hoje
um ponto de "esperar o turno do usuário". Fazer o usuário virar membro **de verdade** = adicionar
pontos de consulta a esse pipeline (Fase 1, §3).

---

## 2. Visão geral do que incorporar (mapa dos repos → alvo)

| Origem | O que porta | Onde entra | Prioridade |
|---|---|---|---|
| **btvChatCorp** | Transporte chat SSE-sobre-`fetch` (auth), enum tipado de payload, montador de system-prompt empilhado, `messages.sources/feedback` on-row, fila de curadoria = ledger de review, worker `SKIP LOCKED`, robustez de RAG (degrada-nunca-falha), webhooks/api-keys/white-label/audit | Fase 1 (chat) + Fase 2 (export/worker) + Fase 3 (admin) | **Máxima** |
| **BuildToValue/V7** | Filosofia da cadeia de valor; três gates (Spec-First/Plan-First/Harden); **biblioteca de personas como conteúdo** (`persona.v1`); **DSL de plano** (`plan.v1`); roteamento por problema; placar "what-matters" | Fase 2 (persona/plano) + Fase 4 (método) | **Alta** |
| **BuildToValueIDE** | **Editor Monaco de entrega** (save gateado + ledger-on-edit); **Squad Composer (ReactFlow)**; arquitetura WS de run ao vivo; **validador Prompt Integrity**; X-Trace-ID; CLI headless | Fase 2 (editor) + Fase 3 (prompt integrity) | **Alta** |
| **squadIA** | Roteamento por **confiança de 4 fatores**; espinha operacional (dep-graph, health, logging, validação); **Decisão→ADR**; histerese; governança-por-decorator | Fase 4 (endurecimento) | **Média** |
| **SquadIAds** | Subsistema de **export** (BaseExporter + manifesto); modelo de dados plano/artefato/validação; validação graduada; pipeline de hardening. **NÃO** portar o loop L1-L5 | Fase 2 (export/validação) | **Média** |
| **buildtovalue-governance** | Scoring de **impacto regulatório**; resultado de **4 estados** + piso-crítico-irredutível; **kill-switch** Prioridade-Zero; relatório de compliance; base-global+overlay-por-profissão | Fase 3 (governança) | **Média** |
| **silent-decisions-proof** | **Produzir≠Revisar≠Aprovar** (capacidades disjuntas); **HMAC por entrada** no ledger; versionamento+expiração de template; tokens lineares de evidência (Rust real, P1/P2) | Fase 3 (confiança) | **Média** |

### O que NÃO portar (descopes explícitos — não deixe vazar)
- **Loop de auto-promoção/rebaixamento L1-L5** do SquadIAds e os bridges de integração — não
  governam nada nem no próprio repo (a esteira nunca lê o nível; bridges são stubs de dados
  fixos; métricas placeholder). Isso **reforça** o ADR 0021. Mantenha autonomia como rótulo
  descritivo consultável, não como gate que se auto-ajusta.
- **Roteamento vencedor-leva-tudo** do squadIA (substituiria o consenso ponderado). Use a
  matemática de confiança **dentro** do consenso (pesar votos), não no lugar dele.
- **Classificador keyword/regex** e os números "100% / €1.28B / 0.21ms" do buildtovalue-governance
  (constantes de marketing, eval circular). Porte o mecanismo de scoring, nunca os números.
- **Core executável** do BuildToValue original (recuperação de artefato por regex só-Ollama,
  ledgers vazios, scripts faltando). Herde método e conteúdo, não código.
- **Circuitos ZK/Noir** do silent-decisions (PoC meio-stub) — item de roadmap distante.

---

## 3. FASE 1 — Usuário como membro ativo da squad (feature de destaque)

**Meta:** um chat ao vivo ao lado da esteira onde (a) as mensagens agente-a-agente aparecem como
conversa, e (b) o usuário posta mensagens que a squad **consome como contribuição de um membro**,
não só aprova/reprova. Mantemos os gates HITL (decisão "membro E gate").

Entregamos em duas sub-fases: **1a (MVP não-bloqueante)** e **1b (membro pleno com pontos de
consulta)**. Ambas usam a mesma fundação de contrato.

### 3.1 Contrato — adicionar mensagens de chat ao `SquadEvent` (aditivo)
Em `schemas/proto/squad.proto`, adicione ao `oneof payload` (próxima tag livre = **9**) e uma
mensagem nova:
```proto
message SquadEvent {
  string task_id = 1;
  string ts = 2;
  oneof payload {
    Proposal proposal = 3;
    Consensus consensus = 4;
    Handoff handoff = 5;
    HitlEscalation hitl = 6;
    StepResult step = 7;
    string error = 8;
    ChatMessage chat = 9;      // NOVO — mensagem de conversa (agente OU usuário)
  }
}

// Mensagem de conversa renderizada no chat ao vivo. `author_role` distingue
// um membro-agente do membro-humano; `author` é o nome de exibição (papel).
message ChatMessage {
  string author = 1;          // ex.: "Arquiteto", "Você", "Revisor"
  AuthorRole author_role = 2; // AGENT | HUMAN | SYSTEM
  string text = 3;            // conteúdo em linguagem natural
  string in_reply_to = 4;     // opcional: id de mensagem/etapa a que responde
}

enum AuthorRole {
  AUTHOR_ROLE_UNSPECIFIED = 0;
  AGENT = 1;
  HUMAN = 2;
  SYSTEM = 3;
}
```
Depois de editar o proto: rode a geração de stubs (Rust via `forge-proto/build.rs` no build normal;
Python via `scripts/gen_proto_py.py`). Comandos em §7.

### 3.2 Rust — canal de entrada de mensagem do usuário (`squad_agent.rs`)
Replique o padrão do HITL. Passos concretos:

**(a) Estado pendente de turno do usuário.** Ao lado de `PendingHitl`, o `SquadTaskState` ganha uma
fila/uma espera de mensagem do usuário. Para o MVP 1a, basta uma fila; para 1b (consulta
bloqueante), use o mesmo molde de `oneshot` do HITL:
```rust
struct SquadTaskState {
    log: Vec<SquadEvent>,
    tx: Option<broadcast::Sender<SquadEvent>>,
    pending: Option<PendingHitl>,
    // NOVO: mensagens do usuário ainda não consumidas por um ponto de consulta.
    inbox: std::collections::VecDeque<String>,
    // NOVO (1b): quem está esperando o próximo turno do usuário (consulta bloqueante).
    awaiting_user: Option<tokio::sync::oneshot::Sender<String>>,
}
```

**(b) Métodos no `SquadHub`:**
```rust
impl SquadHub {
    /// Registra uma mensagem do usuário. Se houver um ponto de consulta esperando
    /// (1b), entrega direto; senão enfileira (1a) para o próximo `await_user_turn`.
    pub fn push_user_message(&self, task_id: &str, text: String) -> Result<(), ()> {
        let mut tasks = self.tasks.lock().expect("squad hub mutex poisoned");
        let Some(state) = tasks.get_mut(task_id) else { return Err(()); };
        if let Some(waiter) = state.awaiting_user.take() {
            let _ = waiter.send(text);
        } else {
            state.inbox.push_back(text);
        }
        Ok(())
    }

    /// Chamado pelo CoreBackend quando o orquestrador Python pede o turno do
    /// usuário (1b). Se já há mensagem na inbox, retorna na hora; senão espera
    /// até `timeout` (ou devolve None p/ "usuário não contribuiu, siga").
    async fn await_user_turn(&self, task_id: &str, timeout: Duration) -> Option<String> {
        let rx = {
            let mut tasks = self.tasks.lock().expect("squad hub mutex poisoned");
            let state = tasks.get_mut(task_id)?;
            if let Some(msg) = state.inbox.pop_front() { return Some(msg); }
            let (tx, rx) = tokio::sync::oneshot::channel();
            state.awaiting_user = Some(tx);
            rx
        };
        match tokio::time::timeout(timeout, rx).await { Ok(Ok(m)) => Some(m), _ => None }
    }
}
```

**(c) Endpoint HTTP** — em `router()` adicione:
```rust
.route("/api/squad/{task_id}/message", post(post_message_handler))
```
```rust
#[derive(Deserialize)]
struct PostMessageBody { text: String }

async fn post_message_handler(
    State(state): State<SquadAgentState>,
    Path(task_id): Path<String>,
    Json(body): Json<PostMessageBody>,
) -> Response {
    // 1) Ecoa a mensagem do usuário no próprio stream, para todos os assinantes
    //    verem a fala do humano na conversa (mesma UX de qualquer membro).
    state.hub.publish(&task_id, /* SquadEvent com ChatMessage{author:"Você",
        author_role: HUMAN, text: body.text.clone()} */);
    // 2) Entrega/enfileira para o orquestrador consumir.
    match state.hub.push_user_message(&task_id, body.text) {
        Ok(()) => StatusCode::ACCEPTED.into_response(),
        Err(()) => (StatusCode::NOT_FOUND,
            Json(ErrorBody::new("task_not_found", "tarefa de squad inexistente"))).into_response(),
    }
}
```
> **Cuidado de contrato (bug real já visto neste repo):** `202 Accepted` tem corpo vazio. No
> frontend, **não** chame `.json()` no corpo de um 202 (a Onda 15 corrigiu exatamente esse bug em
> `fetchJson`). Retorne 202 sem corpo e trate no cliente sem parsear.

**(d) Back-channel Python→Rust (1b).** O orquestrador Python precisa **puxar** o turno do usuário.
Adicione um RPC ao `CoreService` em `schemas/proto/core.proto` (aditivo):
```proto
service CoreService {
  // ... Generate, RunTool, AppendLedger, Recall, Remember, RequestPermission ...
  rpc AwaitUserTurn(UserTurnRequest) returns (UserTurnResponse);  // NOVO
}
message UserTurnRequest  { string task_id = 1; uint32 timeout_ms = 2; }
message UserTurnResponse { bool has_message = 1; string text = 2; } // has_message=false → timeout/skip
```
No `WebSquadCoreBackend` (impl Rust do CoreService que o server web injeta), implemente
`AwaitUserTurn` chamando `hub.await_user_turn(task_id, timeout)`. É o **espelho exato** de como
`RequestPermission` já é implementado ali (resolve o gate via HTTP em vez de stdin).

### 3.3 Python — usuário como papel de primeira classe (`forge-squad`)
**(1a MVP)** Sem tocar no fluxo determinístico: apenas repasse as mensagens de chat que o
orquestrador já produz (se produzir) e garanta que falas do usuário aparecem. Como o usuário
posta via Rust (§3.2c) e o Rust já ecoa no stream, o MVP 1a **não exige mudança no Python** — o
chat funciona como "canal lateral" visível, e a contribuição entra como contexto do próximo passo
se você repassá-la (opcional) no próximo prompt.

**(1b membro pleno)** Adicione **pontos de consulta** no `execute_complex_task`:
```python
# forge_squad/agents/user.py  (NOVO)
class UserAgent:
    """Representa o humano como membro. Não chama LLM: sua 'proposta' é a
    mensagem que o usuário digitou no chat, puxada via CoreService.AwaitUserTurn."""
    def __init__(self, core):  # core = cliente gRPC do CoreService (já injetado no orquestrador)
        self.core = core
    async def contribute(self, task_id, prompt_hint, timeout_ms=60_000):
        turn = await self.core.await_user_turn(task_id, timeout_ms)   # bloqueia no Rust
        return turn.text if turn.has_message else None
```
No `_get_squad_proposals`, **depois** de coletar as propostas dos agentes e **antes** do consenso,
consulte o usuário e, se ele contribuir, dobre a fala como uma `Proposal` de alto peso (o humano é
o membro sênior):
```python
proposals = { ... }  # architect/developer/auditor
user_text = await self.user_agent.contribute(task["task_id"], prompt_hint="revise/oriente a proposta")
if user_text:
    proposals["voce"] = Proposal(confidence=1.0, content=user_text)   # peso máximo: é o dono
    await self._emit({"kind":"chat","author":"Você","author_role":"HUMAN","text":user_text})
consensus = self.consensus.reach_consensus(proposals, "architecture")
```
E acrescente o mapeamento `"chat"` em `server.py::_to_squad_event`:
```python
elif kind == "chat":
    ev.chat.CopyFrom(squad_pb2.ChatMessage(
        author=event["author"],
        author_role=squad_pb2.AuthorRole.Value(event["author_role"]),
        text=event["text"],
        in_reply_to=event.get("in_reply_to", ""),
    ))
```
> **Padrão de fundamentação (portado do btvChatCorp — "montador de system-prompt empilhado").** No
> btvChatCorp, cada turno monta o system prompt empilhando **RAG + instruções-do-projeto + anexos +
> histórico** (arquivo `api/src/routes/chats.rs:528-566`). Aplique o **mesmo empilhamento** ao
> montar o prompt de cada agente do squad: injete as **personas dos outros membros** (inclusive a
> fala do usuário) como contexto, para que os agentes "conversem" cientes uns dos outros. Não é
> preciso abrir o btvChatCorp: o padrão é `system = "\n\n".join([rag_ctx, personas_ctx,
> anexos_ctx, historico])`.

### 3.4 Frontend — transporte SSE com auth + UI de chat
**(a) Transporte SSE-sobre-`fetch` (portado do btvChatCorp).** O `connectSquadEvents` atual usa
`EventSource` nativo (`web/src/api/squad.ts`). `EventSource` **não envia header `Authorization`** —
quando a plataforma exigir auth no stream (multi-tenant), troque por `fetch`+reader. Padrão
completo (auto-suficiente — é o do `chat-stream.service.ts` do btvChatCorp, adaptado):
```ts
export function connectSquadEvents(taskId: string, handlers: SquadEventHandlers): () => void {
  const ctrl = new AbortController()
  ;(async () => {
    const res = await fetch(`/api/squad/${encodeURIComponent(taskId)}/events`, {
      headers: { /* 'Authorization': `Bearer ${token}` quando houver */ },
      signal: ctrl.signal,
    })
    const reader = res.body!.getReader()
    const dec = new TextDecoder()
    let buf = ''
    for (;;) {
      const { done, value } = await reader.read()
      if (done) break
      buf += dec.decode(value, { stream: true })
      let nl
      while ((nl = buf.indexOf('\n')) >= 0) {         // SSE separa eventos por \n
        const line = buf.slice(0, nl); buf = buf.slice(nl + 1)
        if (!line.startsWith('data: ')) continue
        try { handlers.onEvent(JSON.parse(line.slice(6)) as SquadEventEnvelope) } catch { /* keep-alive */ }
      }
    }
  })().catch(() => handlers.onConnectionError?.())
  return () => ctrl.abort()
}
```
> Mantenha o `EventSource` atual se a plataforma não precisar de auth no stream ainda; a troca só é
> obrigatória quando o stream ficar autenticado. **Adicione reconnect/backoff** (o hook do IDE não
> tinha) — mas lembre que uma tarefa de squad é **finita**: pare de reconectar no fim do stream.

**(b) `postSquadMessage` no cliente** (`web/src/api/squad.ts`):
```ts
export async function postSquadMessage(taskId: string, text: string): Promise<void> {
  const r = await fetch(`/api/squad/${encodeURIComponent(taskId)}/message`, {
    method: 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify({ text }),
  })
  if (!r.ok) throw new ApiError('falha ao enviar mensagem', `http_${r.status}`)
  // 202 sem corpo — NÃO chamar r.json().
}
```
Adicione o tipo do novo payload em `SquadEventPayload`:
```ts
export interface SquadChatMessage { author: string; author_role: 'AGENT'|'HUMAN'|'SYSTEM'; text: string; in_reply_to?: string }
export type SquadEventPayload =
  | { Proposal: SquadProposal } | { Consensus: SquadConsensus } | { Handoff: SquadHandoff }
  | { Hitl: SquadHitl } | { Step: SquadStep } | { Error: string }
  | { Chat: SquadChatMessage }   // NOVO
```

**(c) UI de chat** em `web/src/components/screens/user/Squad.tsx`: derive uma lista `chat` dos
eventos (como já faz para `proposals`/`consensus`), renderize um painel de conversa ao lado da
esteira, e um `<textarea>` + botão "enviar" chamando `postSquadMessage(taskId, text)`. Bolhas
`author_role==='HUMAN'` alinhadas à direita; agentes à esquerda com o nome do papel. Mantenha o
card de gate HITL existente (decisão "membro E gate").

### 3.5 Verificação da Fase 1 (fronteira executável)
- **Rust** (`crates/forge-cli/src/squad_agent.rs`, `#[cfg(test)]`): teste de ida-e-volta —
  `new_task` → `push_user_message` → assinar SSE (`subscribe`) → observar um `SquadEvent` com
  `ChatMessage{author_role: HUMAN}`. Espelhe o teste e2e existente
  `run_squad_via_http_com_gate_hitl_real_e_ledger`. Para 1b, teste que `await_user_turn` desbloqueia
  quando `push_user_message` chega, e retorna `None` no timeout.
- **Python** (`forge-squad/tests/`): com o `ScriptedSquadCoreBackend`, teste que uma contribuição
  do usuário entra em `proposals["voce"]` e um evento `{"kind":"chat"}` é emitido.
- **Web** (Playwright, `web/tests/`, contra `forge dashboard` real com `FORGE_SCRIPTED=1`): abrir a
  tela do squad, enviar uma mensagem, ver a bolha do usuário e a reação da squad.
- Rode: `cargo test -p forge-cli`, `cd python && uv run pytest packages/forge-squad`, `pnpm -C web test`.

---

## 4. FASE 2 — As entregas são o produto (esteira → export)

**Meta:** squads viram **conteúdo** (galeria de templates), o plano vira um **manifesto de
entregas**, e cada etapa emite um **artefato durável exportável** (DOCX/XLSX/PDF/MusicXML).

### 4.1 Novo contrato `persona.v1` (a "biblioteca de personas como conteúdo")
Crie `schemas/json/persona.v1.schema.json` + fixture `schemas/fixtures/persona.v1.json` (siga o
padrão dos schemas existentes em `schemas/json/`). Estrutura (destilada do `SQUAD-PERSONAS.md` do
BuildToValue original — 2215 linhas; o essencial está aqui, não precisa abrir o repo):
```json
{
  "id": "revisor-de-estilo",
  "display_name": "Revisor de estilo",
  "domain": "editorial",
  "mental_models": [
    { "reference": "The Elements of Style — Strunk & White", "apply_when": "clareza e concisão" }
  ],
  "core_principles": [
    { "id": "voz-ativa", "description": "prefira voz ativa", "validation": "manual", "severity": "medium" }
  ],
  "autonomy": { "level": "L3", "can_decide_alone": ["ajustes de estilo"], "requires_approval": ["mudança de sentido"], "can_veto": false },
  "activation_triggers": { "semantic_patterns": ["revisar", "clareza"], "context_keywords": ["texto", "artigo"], "confidence_threshold": 0.6 },
  "communication": { "receives_from": ["redator"], "delivers_to": ["fact-checker"], "handoff_contract": "artigo.md + notas de revisão" },
  "delivery_formats": ["DOCX", "Markdown", "HTML"]
}
```
> **Autonomia é DESCRITIVA** (rótulo consultável), não um loop que se auto-ajusta — respeita o
> ADR 0021. É metadado de UI/permissão, não um gate automático.

Escreva **teste de paridade Rust×Python** para o novo schema (mesmo padrão do `prompt-cache-key.v1`:
`crates/forge-schemas/` valida a fixture; um teste Python valida a mesma fixture). Uma **galeria
semeada** (5–8 personas por profissão da Onda 1: Editorial/SEO, Pesquisa, BI/Dados, Operações/SOP,
Sales) vira dados, não código — o Admin publica e aparece na galeria.

### 4.2 Novo contrato `plan.v1` (DSL de plano / manifesto de entrega)
Crie `schemas/json/plan.v1.schema.json`. Estrutura (destilada de `templates/plans/backend-
service.json` do BuildToValue original):
```json
{
  "prerequisites": { "contracts": [], "approvals": [], "dependencies": [] },
  "execution_sequence": [
    { "order": 1, "primary_role": "pauteiro", "support_roles": [], "deliverables": ["pauta.md"],
      "approval_required": true, "estimated_confidence": 0.8, "quality_gates": ["revisao:pass"] }
  ],
  "success_criteria": { "functional": [], "non_functional": [] },
  "budget": { "estimated_cost": 0.0, "max_llm_calls": 20 },
  "rollback_strategy": { "kill_switch": true }
}
```
As `deliverables` são exatamente os artefatos exportáveis. Esse contrato é a ponte entre
`squad.workflow.v1` (a fiação, que já existe) e a exportação.

### 4.3 Subsistema de export + worker de produção
- **Modelo de export (portado do SquadIAds).** Um `BaseExporter` com `_generate_filename(prefix,
  ext)` (timestamp), `_save_json(indent=2, ensure_ascii=False, default=str)`, e exporters por tipo
  (DOCX/XLSX/PDF/MusicXML). Cada etapa da esteira emite um artefato + acumula um **manifesto
  `exported_files`**. Implemente em Rust no `forge-tools` (é quem toca disco) ou como job no
  sandbox.
- **Worker de jobs (portado do btvChatCorp — padrão `SKIP LOCKED`).** Para renderizar entregas sem
  broker: uma tabela de jobs com `processing_status` + `retry_count`; workers pegam trabalho com
  `SELECT ... FOR UPDATE SKIP LOCKED`, com concorrência limitada por semáforo, retry com backoff, e
  status `failed` após `max_retries`. Já existe o **sandbox Docker real** (`forge-tools::sandbox`,
  bollard) — rode os exportadores de terceiros ali.
- **Editor Monaco de entrega (portado do BuildToValueIDE).** Porte o padrão do
  `EditableMonacoEditor.tsx`: validar-ao-digitar (debounce 400ms) → `monaco.editor.setModelMarkers`,
  **botão salvar desabilitado enquanto houver erro** (`!result.valid`), e **ledger-on-edit** (cada
  save faz POST de um evento auditado `file_edited` com tamanhos antes/depois). Isso faz a edição do
  humano ser logada como a de um agente — reforça "usuário é membro".

### 4.4 Rigor de review
- **Validação graduada (SquadIAds `spec_first`).** `ValidationLevel` BASIC(1)/STANDARD(2)/STRICT(3);
  o nível gateia quais checagens rodam. Deixe o humano regular o rigor por execução.
- **Pipeline de hardening (SquadIAds).** Estágios FORMAT/LINT/TYPE/TEST/SECURITY/INVARIANTS via
  subprocess, gateados por disponibilidade de ferramenta, retornando `StageResult{status, tool,
  metrics, execution_time}`. O `mix_btv_code` já tem `forge-verify` — estenda-o em vez de duplicar.

---

## 5. FASE 3 — Confiança e governança (perfil Admin)

### 5.1 Ledger — HMAC por entrada sobre a hash-chain
O `crates/forge-store/src/ledger.rs` já é append-only com hash-chain (ordem/append-only). Adicione
**HMAC-SHA256 por entrada** (autenticidade: prova que a entrada foi escrita pelo motor, não
fabricada por quem tem acesso de append). Chave via env/KMS. Chain + HMAC juntos > cada um sozinho.
`subtle::ConstantTimeEq` para comparar. (Padrão portado do `validate_ledger.py` do
buildtovalue-governance, mas **melhor**: eles não tinham chaining; você tem os dois.)

### 5.2 `gates.evaluate` — 4 estados + piso-crítico-irredutível
Estenda o veredito de review de booleano para **APPROVED / CONDITIONAL / ESCALATE / BLOCKED**
(thresholds por score). **ESCALATE** roteia para review humano (encaixa no gate HITL). Regra dura
(portada do `enforcement.py`): mitigações reduzem risco comum multiplicativamente, **exceto** para
uma lista `CRITICAL_SUB_THREATS` — essas têm piso irredutível (nenhuma média alta salva um finding
crítico). Anexe **impacto regulatório** ao bloqueio: `{regulation, article, exposure, executive_
summary}` a partir de uma tabela YAML por profissão (mecanismo, não os números fixos).

### 5.3 Kill-switch de squad (Prioridade-Zero)
Adicione `operational_status` a uma execução de squad. `POST /api/squad/{id}/emergency-stop`
(admin-only) seta `EMERGENCY_STOP`. **Antes de cada passo** da esteira (plano/produzir/revisar/
exportar), cheque o status **primeiro**, antes de qualquer scoring — um squad parado não pode passar
requisição alguma. Registre o evento no ledger (operador, motivo, timestamp, status anterior).
**Exija aprovação para retomar** (o buildtovalue-governance dizia "não retome sem aprovação" mas não
impunha — imponha).

### 5.4 Produzir ≠ Revisar ≠ Aprovar (separação estrutural)
Portado do silent-decisions P5. Imponha, via o motor de permissões já existente, que o agente que
**produz** um artefato é um principal estruturalmente diferente do que **revisa**, diferente do que
**aprova/exporta**:
- produtor: capacidade de *anexar* rascunho ao ledger, **sem** capacidade de aprovar;
- revisor: lê + anexa veredito, **não** edita o artefato (append-only);
- gate de export: **consome um recibo assinado do revisor** — o caminho de export é restrito a
  passar por review. (Molde Rust real: `DeliveryToken::seal(verdict, receipt)` — o export não
  compila/roda sem o recibo. Stretch opcional: tokens lineares de evidência não-`Clone`/`#[must_
  use]` do P1/P2, com testes `trybuild` compile-fail provando o invariante.)
- **Ressalva honesta:** em single-tenant os três "poderes" podem ser agentes no mesmo processo, então
  "impossível" vira "imposto pelo motor de permissões". Ainda é propriedade de confiança real — não
  superestime como impossibilidade matemática.

### 5.5 Versionamento e expiração de template/política (P6)
Versione cada template/política; carimbe o **hash da versão** em cada entrada do ledger; julgue
artefatos passados contra a versão vigente na produção (reprodutibilidade de auditoria). Promoção de
template a "certificado" exige assinatura **multi-papel**. Certificações **expiram** e, ao expirar,
**param** em vez de silenciosamente entregar sob regra velha.

### 5.6 Prompt Integrity (complementar ao cache-key)
Porte o validador do BuildToValueIDE: responde "esse contrato é **seguro e completo** para rodar?"
(campos obrigatórios, regras de ética `no_pii`/`no_bias`, piso de qualidade, regex de padrão
perigoso: `rm -rf`, `DROP TABLE`, `eval(`, `exec(`, `os.system`) com **severidade por modo**
(vitrine=warning / enterprise=error = tier de tenant) + log no ledger. Rode no **cache-miss** do
`prompt-cache-key.v1` e guarde o veredito ao lado da entrada de cache; recuse executar contratos que
falham. É ortogonal ao hashing (que responde "é o mesmo prompt?").

---

## 6. FASE 4 — Endurecimento do orquestrador + fidelidade ao método

- **Espinha operacional (squadIA):** init com grafo de dependências (topo-sort + detecção de ciclo),
  health checks (registráveis, agregam healthy/degraded/critical), logging estruturado JSON com
  contexto por `mission_id`, validação de estrutura (probe de import antes do uso), config-validate-
  na-construção (falha antes de qualquer trabalho).
- **Consenso ponderado por confiança:** pese o voto de cada agente pelo modelo de 4 fatores do
  squadIA — Histórico(40%) + Match de modelo mental(35%) + Autonomia(15%) + Performance recente(10%),
  com penalidade de workload. **Dentro** do consenso, não no lugar dele.
- **Decisão→ADR (squadIA):** cada gate emite um ADR markdown com o breakdown de confiança e as
  **alternativas rejeitadas** (com seus scores). "Decisão → artefato durável".
- **Histerese** para qualquer threshold adaptativo: promoções exigem N avaliações estáveis
  consecutivas; rebaixamentos imediatos; avaliação com rate-limit. (Reutilizável mesmo sem níveis.)
- **Método (BuildToValue original):** gate de **entrada Spec-First** (um contrato tem que existir
  antes de rotear um job); **placar "what-matters"** sobre o ledger (razão de retrabalho, taxa de
  override, custo-por-sucesso, acurácia de confiança) — torna o "Valor" mensurável; **roteamento por
  problema** (problem_type → squad recomendado + sequência + confiança); **modo `$0` local** (allowlist
  por-persona num modelo Ollama local) como alavanca de adoção.

---

## 7. Comandos, verificação e disciplina de entrega

```sh
# Rust
cargo test --workspace                 # inclui testes SSE de web_agent/squad_agent e paridade de hash
cargo clippy --workspace -- -D warnings
cargo fmt --all --check

# Python
cd python && uv sync && uv run pytest  # forge-squad, forge-promptforge, etc.

# Web
pnpm -C web install && pnpm -C web test        # Playwright contra `forge dashboard` real

# Protos (após editar schemas/proto/*.proto)
#  - Rust: recompilado por forge-proto/build.rs no `cargo build` normal (protoc vendorizado)
#  - Python: python scripts/gen_proto_py.py

# Verify self-hosting (o CI roda isso sobre o próprio workspace)
cargo run -p forge-cli -- verify

# Atalhos
just test | just lint | just verify
```

**Regras de contrato (não-negociáveis):**
- Fonte única de contrato em `schemas/` (protos + `*.v1.schema.json` + fixtures). Protos evoluem
  **só aditivamente**; breaking = `.v2` + ADR novo.
- Ledger é **append-only com hash-chain** — nunca UPDATE/DELETE; overrides são entradas novas
  marcadas.
- O hash `prompt-cache-key.v1` tem implementação dupla Rust×Python — qualquer mudança regenera
  `schemas/fixtures/` (`scripts/gen_fixtures.py`) e os testes de paridade dos DOIS lados devem passar.
- Novos schemas (`persona.v1`, `plan.v1`) seguem esse mesmo padrão: schema + fixture + teste de
  paridade.

**ADRs a escrever** (conforme você implementa):
- Fase 1 → ADR "Usuário como membro do squad: canal de chat + `AwaitUserTurn`".
- Fase 2 → ADRs "`persona.v1` e galeria de conteúdo", "`plan.v1` e manifesto de export".
- Fase 3 → ADRs "HMAC no ledger", "gates 4-estados + kill-switch", "separação produzir/revisar/
  aprovar", "versionamento/expiração de template".
- Se **reabrir** autonomia progressiva (dial Manual/Assistido/Autônomo), é um ADR **novo e
  explícito** que supersede o ADR 0021 — não faça em silêncio.

**Ordem recomendada de execução:** Fase 1 (chat/membro) → Fase 2 (persona/plano/export) → Fase 3
(governança) → Fase 4 (endurecimento). Cada capacidade aterrissa atrás da sua fronteira de teste
antes de seguir. A Fase 1 é a de maior valor percebido e a menos acoplada — comece por ela.

---

## 8. Apêndice — mapa rápido de arquivos que você vai tocar

| Fase | Arquivo | Ação |
|---|---|---|
| 1 | `schemas/proto/squad.proto` | add `ChatMessage`, `AuthorRole`, tag 9 no `oneof` |
| 1 | `schemas/proto/core.proto` | add `AwaitUserTurn` + msgs (1b) |
| 1 | `crates/forge-cli/src/squad_agent.rs` | `inbox`/`awaiting_user`, `push_user_message`, `await_user_turn`, `post_message_handler`, rota, impl `AwaitUserTurn` no `WebSquadCoreBackend` |
| 1 | `python/packages/forge-squad/src/forge_squad/agents/user.py` | novo `UserAgent` |
| 1 | `python/packages/forge-squad/src/forge_squad/orchestrator.py` | ponto de consulta em `_get_squad_proposals`; emitir `{"kind":"chat"}` |
| 1 | `python/packages/forge-squad/src/forge_squad/server.py` | mapear `"chat"` em `_to_squad_event` |
| 1 | `web/src/api/squad.ts` | `postSquadMessage`, `SquadChatMessage`, (opcional) SSE-sobre-`fetch` |
| 1 | `web/src/components/screens/user/Squad.tsx` | painel de chat + textarea |
| 2 | `schemas/json/persona.v1.schema.json` + fixture + testes de paridade | novo contrato + galeria semeada |
| 2 | `schemas/json/plan.v1.schema.json` + fixture | novo contrato |
| 2 | `crates/forge-tools/` (export + worker) ; `web/` (editor Monaco) | export/edição de entrega |
| 3 | `crates/forge-store/src/ledger.rs` | HMAC por entrada |
| 3 | `crates/forge-verify/` ou camada de gates | 4-estados, piso-crítico, impacto regulatório |
| 3 | `crates/forge-cli/src/squad_agent.rs` + `forge-store` | kill-switch + status |
| 4 | `python/packages/forge-squad/` | espinha operacional, consenso ponderado, ADR, histerese |

Fim do handoff.
