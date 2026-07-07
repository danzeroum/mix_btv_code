# ADR 0019 — Sidecar como serviço de longa duração: singleton vs. pool, restart-on-crash

- Status: aceita
- Data: 2026-07-06

## Contexto

`SidecarSupervisor`/`SquadSupervisor` (crate `forge-sidecar`) spawnam `uv run
python -m forge_{promptforge,squad}.server` com `kill_on_drop(true)` — ciclo de
vida por-**invocação-CLI**: `forge run`/`forge chat`/`forge squad` sobem o
processo, usam, e ele morre quando o comando termina. O processo de longa duração
do `forge dashboard` (Fase 7) atende **muitas** requisições HTTP ao longo de sua
vida — recriar o sidecar a cada requisição seria caro (segundos de startup do
`uv run`) e destruiria qualquer estado de handshake do processo Python. Precisamos
de uma camada de serviço que mantenha o processo vivo entre requisições, com
detecção de queda e restart.

## Decisão — dois desenhos distintos, por perfil de carga

Camada nova (`forge-sidecar::service`), ao lado da camada CLI existente (que
continua intocada — o CLI de invocação única não precisa de nada disto):

- **`SidecarService` (PromptForge): instância ÚNICA compartilhada.** O sidecar é
  stateless — `lint`/`render`/`list_generators` não têm estado entre chamadas.
  Serializar o acesso com um `tokio::sync::Mutex` (mantido através do `.await` do
  health-check + eventual respawn, deliberadamente — é a política declarada, não
  um detalhe de implementação) é aceitável: um `render` por vez não é um gargalo
  real para o volume de uso esperado.
- **`SquadPool`: pool pequeno com limite (`Semaphore` + free-list).** Squad é
  execução longa — múltiplos agentes, múltiplas chamadas de LLM por execução. Uma
  instância única serializaria squads concorrentes à toa (uma segunda tarefa
  esperaria a primeira terminar inteira); um pool de N processos independentes dá
  paralelismo real até o teto E isolamento (um squad travado/morto não afeta os
  outros slots).

Ambos detectam queda da MESMA forma: antes de devolver um cliente, fazem o
health-check real (o RPC `Health`, não uma sonda separada); se falhar, descartam o
estado e sobem um processo novo (PID diferente) antes de responder — transparente
para quem chama. Uma requisição que corre exatamente contra o momento da queda
pode falhar (aceito — não há retry-no-meio-do-voo); a PRÓXIMA requisição sempre
encontra um processo saudável.

**Achado colateral corrigido:** ao construir o teste de queda/restart para o
`SidecarService`, ficou claro que `SidecarSupervisor::spawn` não tinha o mesmo
`process_group(0)` + kill-de-grupo que `SquadSupervisor` já tinha (fix anterior,
Onda 4d) — matar só o processo `uv` órfãos deixaria o `python` órfão rodando.
Corrigido para espelhar exatamente o padrão do squad.

## O que foi provado, não só declarado

- `SidecarService`: 3 chamadas sequenciais reusam o MESMO processo (PID estável,
  provado por igualdade); `kill_current()` (mesmo padrão de `squad_e2e.rs`) mata
  o processo; a chamada seguinte sobe um processo NOVO (PID diferente, provado por
  desigualdade) e responde ao health-check — sem que o processo Rust em si
  reinicie.
- `SquadPool` capacidade 2: duas aquisições concorrentes usam 2 slots e 2 PIDs
  distintos (nunca serializa até o teto). Capacidade 1 (isola o mecanismo de
  qualquer ambiguidade de qual slot é reusado): adquirir, devolver, matar o
  processo do slot, adquirir de novo sobe um processo novo no MESMO slot.

## Consequências

- `SquadPool::acquire` é `self: &Arc<Self>` (não `&self`) — o lease devolvido é
  `'static` (owned, via `Arc`+`OwnedSemaphorePermit`), pensado para ser movido
  para dentro de uma task spawnada (a duração de uma execução de squad inteira),
  não só usado inline.
- Nenhuma rota HTTP nova nesta onda — é puramente a camada de supervisão de
  processo. O wiring para `/api/squad/run` (Onda 4) e `/prompt render` via HTTP
  (Onda 5) consomem `SquadPool`/`SidecarService` respectivamente, construídos uma
  vez no startup do `forge dashboard`.
