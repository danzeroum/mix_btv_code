# `skills/` — skills built-in da Forge

Uma **skill** é uma ferramenta empacotada que o agente pode invocar como
qualquer tool (`read`, `grep`, `bash`, …). Este diretório reúne as skills
built-in que acompanham a Forge. Elas são carregadas e **vetadas** no início de
cada sessão (`forge run`/`chat`/`tui`) e expostas ao modelo no `ToolRegistry`.

Introduzido na **Fase 6 Onda 1** (runtime de skill). O confinamento em sandbox
Docker de skills de terceiro é a Onda 2/3 — os built-ins aqui são confiáveis e
rodam como subprocesso direto, **mas passam pelo vetter mesmo assim**
(dogfooding: nenhuma skill entra no registry sem vetting).

## Formato

Cada skill é um subdiretório com um `skill.toml` na raiz:

```toml
name = "word-count"                 # identidade (vira o nome da tool)
description = "Conta palavras…"     # anunciada ao modelo
entrypoint = 'printf "%s" "$1" | wc -w'   # comando shell a executar
permissions = []                    # permissões declaradas (ex.: ["bash", "webfetch"])

# Passos de verificação próprios (opcionais), no mesmo formato do forge.toml:
# [[verify]]
# name = "testa"
# program = "sh"
# args = ["-c", "test -f main.sh"]
```

## Contrato de execução (Onda 1)

- O `entrypoint` roda via `sh -c`, **no diretório da skill** (cwd).
- O valor do campo `input` da chamada chega como **`$1`**.
- stdout+stderr voltam ao agente (truncados no limite padrão de 32 KiB).
- Há um timeout; uma skill que trava não trava o loop (o grupo de processos é
  morto). Args estruturados (além de `input`) são escopo futuro.

## Vetting (fail-closed)

No carregamento, cada skill passa por `forge-verify::vetter::vet_skill`:

- manifesto ausente/inválido → **bloqueada** (não registrada);
- padrão perigoso no código (ex.: `curl … | sh`, `rm -rf /`) → **bloqueada**;
- permissão incoerente com o uso (ex.: executa comando externo sem declarar
  `bash`) → **bloqueada**;
- passos `[[verify]]` que falham → **bloqueada**.

Uma skill **bloqueada nunca é registrada** — o motivo é impresso no stderr. O
mecanismo é o mesmo que a Fase 5 (Onda 5) já usava para skills de terceiro;
a Fase 6 o coloca a serviço do runtime.

## Exemplos aqui

- `word-count/` — conta as palavras do `input`.
- `uppercase/` — passa o `input` para maiúsculas.

Ambas são puro shell, sem dependências externas, e servem de molde para novas
skills built-in.
