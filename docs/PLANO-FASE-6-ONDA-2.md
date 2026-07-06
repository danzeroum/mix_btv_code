# Plano: Fase 6 — Onda 2 — Sandbox Docker real (bollard, em Rust)

> Documento de execução. Ancorado no código real deste commit (main `8518dc9`,
> planos das Ondas 1 e mestre da Fase 6 mergeados; **implementação da Onda 1 ainda
> não existe** — ver Dependência). É a onda mais sensível da fase: constrói o
> confinamento para código que não é da plataforma. O risco muda de natureza —
> deixa de ser "o LLM errar" e passa a ser "código alheio rodar na máquina".

## Dependência de ordem (ler primeiro)

- **A implementação da Onda 1 ainda não está no `main`** (`8518dc9` trouxe só o
  *plano* da Onda 1; não há `skills/`, `SkillTool`, nem a mudança do trait). A Onda
  2 constrói o sandbox como capacidade **independente** do `SkillTool` — o sandbox é
  um executor confinado que a Onda 3 vai usar para rodar skills de terceiro. Pode
  ser construída antes ou em paralelo à impl da Onda 1; a **amarração** (SkillTool
  de terceiro → roda no sandbox) é escopo da **Onda 3**, não desta.
- Portanto a Onda 2 não depende do código da Onda 1; depende só do que já está no
  `main` (o trait `Tool`, `ToolOutput`/`ToolError`, o padrão de `run_with_timeout`).

## Objetivo

`forge-tools::sandbox` em **Rust via `bollard`**: rodar um comando dentro de um
contêiner Docker com limites duros — filesystem (mount restrito), rede (desabilitada
por padrão), tempo e memória. Desligamento gracioso e **fail-closed para terceiros**:
sem daemon Docker, terceiro **não roda** (built-in confiável continua pelo caminho
não-containerizado da Onda 1).

## Estado real na entrada (verificado neste commit)

- **O PLANO-mestre é inequívoco sobre linguagem e lugar:** sandbox em **Rust**,
  `forge-tools::sandbox`, via **`bollard`** (PLANO-PLATAFORMA linhas 71, 121, 183:
  "`secure_executor.py` → `forge-tools::sandbox` em Rust"). O stub Python **não** é
  o lugar da implementação.
- **O stub Python (`forge_squad/sandbox.py`) vira client/interface** e já define o
  contrato a espelhar: `DockerSandbox{image, network_disabled}` + `run(command,
  environment, timeout)`; `SecureToolSandbox{execution_timeout=30, memory_limit_mb=
  512, cpu_quota=0.5}`. **Os quatro limites já estão nomeados** — a Onda 2 os traduz
  para as opções do bollard. O comentário verbatim no arquivo diz "contêineres reais
  são escopo da Fase 6".
- **Precedente de desligamento gracioso já existe:** o `execute_tool_sandboxed`
  atual, sem docker, devolve `{sandboxed: False, message: "Docker indisponível..."}`.
  A Onda 2 mantém o princípio, mas com a régua da fase: gracioso para built-in,
  **fail-closed para terceiro** (a distinção é aplicada na Onda 3, mas o sandbox
  Rust deve expor claramente "rodou confinado" vs "não rodou" para a 3 decidir).
- **`bollard` não é dep de ninguém ainda** — adição nova (passa pelo `cargo deny` do
  CI; verificar licença/advisories).
- **Reuso:** `ToolOutput`/`ToolError` (`forge-tools/src/lib.rs:23,31`) para o
  retorno; o `DEFAULT_OUTPUT_LIMIT` para truncar; e o `bash.rs` como referência de
  como um tool executa comando hoje (o sandbox é o `bash` confinado).
- **`run_with_timeout` (forge-verify)** mata processo local com timeout, mas o
  timeout do **contêiner** é do bollard (parar/matar o container), não do processo
  host — não confundir os dois mecanismos.

**Realidade de ambiente (contrato de verificação desta onda):** o container de
verificação tem o **client** docker (`/usr/bin/docker` v29.3.1) mas o **daemon é
inalcançável** — contêineres não rodam aqui. Logo o teste de contenção **não roda no
meu ambiente**; a verificação por execução dessa fronteira **vive no CI** (runner
ubuntu tem daemon). Isto é desenho, não lacuna — ver Fronteira.

Baseline a preservar: contagens do `main` na entrada, zero falhas, clippy/fmt
limpos, job `verify` do CI verde.

## Decisões de contrato (candidatas a ADR 0012)

1. **API do sandbox Rust.** Ex.: `Sandbox { image, mount, network, mem_limit,
   cpu_quota, timeout }` + `run(cmd, args, env) -> Result<SandboxOutput, SandboxError>`.
   `SandboxOutput{ stdout, exit_code, timed_out }`; `SandboxError` distingue
   **"daemon indisponível"** de **"contenção violada/erro de execução"** — a Onda 3
   precisa dessa distinção para o fail-closed de terceiros.
2. **Padrão de contenção default (o que "confinado" significa):** rede **off**,
   filesystem só o mount de trabalho (read-only fora dele), sem privilégios
   (`--cap-drop ALL`, no-new-privileges), limites de memória/cpu/tempo. Documentar o
   perfil — é a superfície de segurança da fase inteira.
3. **Onde o sandbox conecta ao produto:** a Onda 2 entrega o executor + testes; a
   ligação "tool de terceiro roda aqui" é Onda 3. Não plugar no `bash`/registry
   nesta onda (o `bash` built-in não passa a exigir Docker — regressão a evitar).
4. **Imagem base:** o stub usa `python:3.11-slim`. Decidir a imagem default do
   sandbox (mínima, pinada por digest para reprodutibilidade). ADR.

## Escopo desta onda

1. `forge-tools::sandbox` (Rust, bollard): a struct, os limites, `run`.
2. Tradução dos quatro limites do contrato (timeout/mem/cpu/rede) para opções do
   bollard, + o perfil de contenção (cap-drop, mount ro, no-net).
3. `SandboxError` distinguindo daemon-ausente de contenção-violada.
4. Desligamento gracioso (daemon ausente → erro claro, não panic).
5. Testes de contenção (ver Fronteira) — os que rodam local e os que só rodam no CI.
6. Dep `bollard` no Cargo + passar no `cargo deny`.

## Fora de escopo (explícito)

- Amarração skill-de-terceiro → sandbox (**Onda 3**).
- Tornar `bash`/tools built-in containerizados (built-in é confiável; não regredir).
- MCP/LSP/RAG (Ondas 4–6).
- O stub Python virar client real do sandbox Rust — só se a Onda 3 precisar; nesta
  onda o stub permanece como está (interface documentada).

## Fronteira verificável (o que o próximo vai rodar — e onde)

O teste que **carrega** a onda é a **contenção que morde**: um comando que teria
sucesso fora do sandbox é **bloqueado** dentro. Quatro vetores, cada um um teste:

1. **Escrita fora do mount** — comando tenta escrever em `/` ou fora do dir de
   trabalho → falha; o mesmo comando escrevendo no mount → sucesso.
2. **Rede proibida** — comando tenta uma conexão de saída → falha (rede off); prova
   que o default nega rede.
3. **Timeout do contêiner** — comando que dorme além do limite → o container é
   morto, `timed_out=true`. (Mecanismo bollard, não o host.)
4. **Limite de memória** — comando que estoura a memória → morto pelo cgroup, erro
   claro (não trava o host).

**Onde roda — o ponto honesto desta onda:**
- **No CI (home do teste):** o job precisa de daemon Docker (runner ubuntu tem). Os
  quatro testes de contenção rodam lá. A fronteira só é considerada satisfeita com
  **evidência do job de CI de que rodou e mordeu** — nunca marcado verde por terem
  sido "pulados".
- **Localmente (meu ambiente):** o daemon é inalcançável (client v29.3.1 sem
  daemon). Os testes de contenção **pulam com log audível** (a lição do skip
  silencioso: `eprintln!` do motivo, `#[ignore]` ou guard que imprime). Eu verifico
  o que **posso** aqui — compila, clippy/fmt, os testes não-Docker (parsing de erro,
  desligamento gracioso quando daemon ausente, a distinção de `SandboxError`) — e
  **declaro explicitamente** que os quatro de contenção foram verificados pela
  evidência do CI, não pela minha execução. Verificação se desloca honestamente,
  como no #40.

Guard-rail de honestidade: um teste de contenção que **passa quando o daemon está
ausente** é um falso positivo (a lição do `kill -9` órfão e do skip da 4d). O teste
tem que **falhar ou pular audívelmente** sem daemon — nunca passar verde sem ter
containerizado nada.

Além disso, o de sempre: `cargo test --workspace` (não-Docker verde localmente),
`clippy -- -D warnings`, `fmt --check`, `cargo deny` (a dep nova), `uv run pytest`.

## Riscos específicos

| Risco | Mitigação |
|---|---|
| Teste de contenção passa sem daemon (falso positivo — o pior desta onda) | Guard que pula audívelmente sem daemon; CI (com daemon) é onde a fronteira conta; nunca verde sem containerizar |
| `bash`/built-in passar a exigir Docker (regressão) | Sandbox é executor separado; não plugar no bash nesta onda; built-in continua não-containerizado |
| Confundir timeout do host (`run_with_timeout`) com timeout do container (bollard) | São mecanismos distintos; o do container mata o container, documentar |
| Perfil de contenção fraco (escapa) | cap-drop ALL, no-new-privileges, rede off, mount ro por default; os 4 testes provam cada vetor |
| `bollard` reprovar no `cargo deny` (licença/advisory) | Verificar antes de fixar; é dep de segurança, escolher versão limpa |
| daemon-ausente vira panic em vez de erro | `SandboxError::DaemonUnavailable` explícito; teste do caminho gracioso (esse eu rodo local) |
| Imagem não-pinada (irreprodutível/supply-chain) | Pinar por digest; ADR da imagem base |

## Por que esta onda, nesta posição

A Onda 1 dá o runtime de skill; a Onda 2 dá o **confinamento**; só com as duas a
Onda 3 pode rodar código de terceiro com segurança. Sandbox **antes** de terceiros
é regra dura da fase — inverter seria rodar código alheio sem cela. Esta é a onda
onde a postura de segurança da plataforma (permissões Rust não-contornáveis + vetter
bloqueante + agora confinamento) fica completa para o passo seguinte.

Contrato de sempre, com a ressalva desta onda explícita: implementa, commita, o
próximo puxa e roda **o que o ambiente permite** — e a contenção (que exige daemon)
é verificada pela evidência do CI, declarada como tal, nunca fingida. Atenção
cirúrgica ao guard que faz o teste pular audívelmente sem daemon: é o que impede o
falso positivo que seria catastrófico justamente na onda de segurança.
