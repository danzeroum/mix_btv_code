# ADR 0022 — `MemoryService`: ponte Rust↔Python para memória do squad

- Status: aceita
- Data: 2026-07-06

## Contexto

O mapa de memória do squad (Grupo A, A3) precisa expor ao navegador o que cada
agente já decidiu (`AgentMemorySystem`, `forge_squad/memory.py`) e a busca por
similaridade sobre esse corpus (`recall.py`, TF-IDF local, ADR 0013). O dado
sempre morou 100% em processo Python, acessado só como método direto pelo
`UnifiedOrchestrator` — nenhuma superfície externa existia.

Havia um RPC já declarado que parecia servir: `CoreService.Recall`/`Remember`
(`schemas/proto/core.proto`). Mas `CoreService` é **servido pelo Rust,
chamado pelo Python** (o padrão usado hoje por `Generate`/`RequestPermission`)
— a direção oposta do que memória precisa (o dado mora no Python; é o Rust
quem precisa perguntar). Confirmado no código: `core_server.rs`'s
`recall`/`remember` são `Status::unimplemented("... memória é local ao
Python no orquestrador atual")` — um stub abandonado desde a Fase 4, nunca
chamado por ninguém. O próprio protótipo de design deste grupo de telas erra
isso (cita "CoreService.Recall" na cópia de carregamento) — o plano desta
onda corrige, não repete o engano.

## Decisão

Um serviço gRPC **novo**, na direção correta: `MemoryService`
(`schemas/proto/memory.proto`), **servido pelo Python** (`forge_squad.
memory_server`, novo módulo dentro do pacote `forge_squad` já existente —
não um pacote novo, já que `AgentMemorySystem` mora ali), **chamado pelo
Rust** (`crates/forge-sidecar/src/memory_client.rs`, mirror de
`SidecarClient`/`SquadClient`). Três RPCs: `Health`, `Recall(query,k)`,
`List(agent?,limit)`. **Sem `Remember`** — quem grava memória é só o
orquestrador via chamada direta em processo (`remember_decision`), nunca
pela rede; expor gravação por gRPC criaria um segundo caminho de escrita
sem necessidade real.

**Por que não reviver `CoreService.Recall/Remember`:** stub abandonado,
direção errada (Rust servindo, Python chamando) — o oposto do que este caso
precisa.

**Por que não estender `SquadService`:** quebraria o precedente de
um-proto-por-concern já estabelecido no repo (`squad.proto` é escopo de
execução de tarefa; `promptforge.proto` é escopo de geração de prompt) e
acoplaria a disponibilidade de uma leitura de memória ao `SquadPool` (Fase 7
Onda 3) — uma consulta de mapa de memória concorreria por slot com uma
execução de squad real à toa.

**Supervisão singleton (`MemoryService`, mirror de `SidecarService`/ADR
0019), não pool.** `Recall`/`List` são leituras stateless sobre o corpus
episódico — o mesmo raciocínio que já justifica `SidecarService` (PromptForge)
ser singleton: serializar uma consulta de memória por vez é aceitável, o
volume esperado não justifica um pool de processos.

**Resolução do diretório do corpus: `memory_dir: None` em produção.**
`forge_squad.server`'s `SquadServicer` (quem de fato ESCREVE memória hoje)
nunca recebe `--memory-dir` — cai no default de `AgentMemorySystem()`
(`.forge/squad-memory` relativo ao `current_dir` do processo, que é o
`python_workspace_dir`). Para `MemoryService` ler o MESMO corpus que o squad
real escreve, ele precisa da mesma resolução relativa — por isso
`MemorySupervisor::spawn` aceita `memory_dir: Option<&Path>` mas a
produção passa `None` (só testes usam `Some`, para um corpus isolado).

## Não-escopo explícito (achado desta onda)

`forge_squad/forgetting.py` (`IntelligentForgetting.adaptive_forget`,
`MemoryStore`) é **código morto** — só o próprio teste unitário dele chama;
nunca `memory.py`/`orchestrator.py`/`server.py`/`memory_server.py` (confirmado
por grep no pacote inteiro). O mapa de memória **não** mostra tendência de
esquecimento — nenhum campo do tipo (`decay`, "esquecendo", "reforçando")
existe na resposta; só `agent`, contagem real, decisão mais recente real e a
de maior confiança real, todos derivados de `list_memories`/`_load_corpus()`.

## Compromisso de honestidade carregado à UI

Rótulo/nav dizem "RAG"/"busca"; a tela e a rota HTTP mantêm a mesma tensão
honesta do resto da plataforma: a recuperação é **léxica** (TF-IDF sobre
termos distintivos, `recall.py`, ADR 0013), não semântica (embedding neural)
— não finge ser o que não é.

## O que foi provado, não só declarado

- `forge_squad/memory_server.py`: testes gRPC reais (`grpc.aio` sobre UDS
  efêmero, mesmo padrão de `forge_promptforge`'s `test_server.py`) provam
  `Recall` com ground truth de vocabulário disjunto (recupera exatamente o
  tópico certo) e `List` agrupando por agente com contagem/decisão/confiança
  reais, sem filtro e filtrado por agente.
- `MemoryService` (Rust): mesmo contrato de `SidecarService` — PID estável
  entre chamadas, restart-on-crash após `kill_current` — agora com um
  sidecar de memória REAL, e `Recall`/`List` batendo com um corpus escrito
  diretamente no formato real (`AgentMemorySystem.remember_decision`).
- `GET /api/memory`/`POST /api/memory/recall`: teste HTTP contra um sidecar
  real (subprocesso `uv run python -m forge_squad.memory_server`) prova a
  fronteira de ponta a ponta; sidecar inatingível devolve `503` explícito
  (mesmo padrão de degradação do PromptForge).
- Playwright: a tela reflete um corpus semeado por fora do browser (mapa
  agrupado + busca léxica) contra o `forge dashboard` real.

## Consequências

- Python continua dono do dado de memória — nenhuma migração de storage.
- `forge-server` continua sem depender de `forge-sidecar`/`forge-tools`/
  `forge-core` — a rota mora no router mesclado de `forge-cli`, mesma regra
  de posicionamento já usada por `prompt_render`/`squad_agent`/`mcp_console`.
- Uma quarta classe de sidecar Python existe agora (`forge_promptforge`,
  `forge_squad.server`, `forge_squad.memory_server`) — o padrão de
  singleton-vs-pool (ADR 0019) escala para o caso novo sem introduzir um
  terceiro desenho.
