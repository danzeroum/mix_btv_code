# ADR 0021 — escopo da autonomia progressiva na Fase 7 (`max_autonomy_level`)

- Status: aceita
- Data: 2026-07-07

## Contexto

O plano-mestre da Fase 7 (Onda 13, "Modelo & Onboarding") previa duas rotas
possíveis para o campo `max_autonomy_level` (`schemas/proto/squad.proto`,
`SquadTask`): (a) só ligar o valor real escolhido na UI até o campo HTTP,
deixando o orquestrador continuar ignorando-o, com a tela declarando isso
explicitamente; ou (b) também mudar o orquestrador Python para respeitar um
teto de autonomia por tarefa, produzindo comportamento observável diferente
por nível — a fronteira que o próprio plano exige caso "autonomia entre":
"dois `SquadTask` com `max_autonomy_level` diferentes produzem comportamento
diferente (aprovação pedida num caso, não no outro) — não só 'o campo
viajou'".

Investigação desta onda confirmou que o campo é **ignorado ponta-a-ponta**
hoje, nos dois pontos de entrada:

- `crates/forge-cli/src/squad.rs` (CLI, `forge squad`) e
  `crates/forge-cli/src/squad_agent.rs` (`POST /api/squad/run`, web) — ambos
  hardcodam `max_autonomy_level: 3` ao montar `SquadTask`; nenhum dos dois lê
  de uma flag de CLI ou de um campo do corpo HTTP (`RunSquadBody` só tem
  `task: String`).
- `python/packages/forge-squad/src/forge_squad/server.py`'s `ExecuteTask`
  nunca lê `request.max_autonomy_level` ao montar o `task` interno repassado
  ao orquestrador — confirmado por grep no pacote inteiro (a única outra
  ocorrência do nome é o `FileDescriptorProto` serializado dentro do stub
  gerado, `squad_pb2.py`).
- A autonomia que de fato roda vem de um mecanismo **completamente
  desconectado** desse campo: `ProgressiveAutonomyManager`
  (`forge_squad/hitl.py`), que decide por **agente**, via
  `agent_trust_scores` (score inicial 0.5 → nível 2, ajustado ±0.02/-0.1 por
  sucesso/falha em `record_action`) — nunca por um teto vindo de fora na
  tarefa.

## Decisão

**Piso, não degrau — opção (a).** A Onda 13 liga `model`/`agent` (que já
tinham campo em `SendMessageBody` desde a Onda 1, nunca populados pelo
frontend) até a sessão de chat real, porque esses dois genuinamente mudam
comportamento observável (perfil de agente seleciona overlay de permissão
real via `load_rule_overrides`). `max_autonomy_level` **não** é wireado até
a UI. A tela (`Modelo.tsx`) mantém os 3 níveis como um bloco informativo,
não um seletor com efeito: cada um descreve o que acontece hoje (`interativo`:
toda ferramenta "ask" pede confirmação, sem teto por tarefa; `automático`:
não implementado; `somente leitura`: use o perfil de agente "plan", que já
nega edits por padrão) com uma nota explícita de que nada disso é aplicado
pelo orquestrador ainda.

**Por que não a opção (b):** mudar o orquestrador para respeitar um teto por
tarefa é uma mudança arquitetural real no mecanismo de HITL do squad — trocar
ou aumentar `ProgressiveAutonomyManager`'s decisão-por-agente por um teto
externo por tarefa, cross-linguagem (Rust decide o valor, Python decide o
efeito). Isso está fora do escopo de "Modelo & Onboarding" (uma onda de
telas), e o próprio plano-mestre trata essa mudança como uma decisão
separada, não uma consequência tácita de expor o campo na UI.

**Por que não wireá-lo mesmo assim, "só para viajar":** faria exatamente o
que o texto do plano nomeia como insuficiente — "o campo viajou" sem
nenhum efeito, uma segunda camada de teatro sobre um campo que já era
teatro (hardcoded `3`, nunca lido). Pior que não wireado: pareceria
funcional (o valor chega ao `SquadTask` de verdade) sem jamais mudar uma
decisão de aprovação.

## Não-escopo explícito

- Mudança no orquestrador Python (`ProgressiveAutonomyManager`/`hitl.py`)
  para aceitar e respeitar um teto de autonomia por tarefa: não implementada
  nesta fase. Fica re-declarada como pendência real, mesmo padrão da
  pendência de consenso→ledger da Fase 6 e do próprio `max_autonomy_level`
  já citado como descope na Fase 7 original.
- `RunSquadBody`/flag de CLI para autonomia: não adicionados — não faria
  sentido adicionar o transporte sem o consumidor real do outro lado.
- Os dois hardcodes (`squad.rs`, `squad_agent.rs`) permanecem `3` — agora
  com comentário explícito apontando esta ADR, não silenciosos.

## Consequências

- `Modelo.tsx` não tem mais um controle de autonomia com efeito fabricado —
  o nível de detalhe da tela cai (não há mais um "●" de seleção), mas nada
  nela finge um efeito que não existe.
- Se um futuro trabalho decidir implementar (b), o ponto de entrada já
  documentado é: `SquadTask.max_autonomy_level` chega intacto ao Python
  (proto já carrega o valor); falta só `ExecuteTask` lê-lo e
  `ProgressiveAutonomyManager` (ou um mecanismo novo) passar a respeitá-lo,
  mais os dois hardcodes virarem parâmetros reais.
- Nenhum ADR anterior precisa mudar — `ProgressiveAutonomyManager`
  (ADR 0006/0007) continua sendo o mecanismo de autonomia real do squad.
