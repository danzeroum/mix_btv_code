# Plano: Fase 6 — Onda 1 — `skills/` built-in + runtime de skill

> Documento de execução. Ancorado no código real deste commit (main `c4aa386`,
> plano-mestre da Fase 6 mergeado). Funda o conceito executável de skill: uma skill
> vetada vira ferramenta viva no `ToolRegistry`, invocável pelo agente. Built-ins
> são confiáveis (sem Docker); o sandbox é Onda 2 e terceiros são Onda 3.

## Objetivo

1. Criar `skills/` na raiz com 1–2 skills built-in de exemplo + o padrão de autoria
   documentado (o PLANO-mestre prevê o diretório desde o início — linha 91).
2. O runtime: carregar skills de um diretório, **vetar cada uma** (`vet_skill` da
   Fase 5 Onda 5 — dogfooding do mecanismo mesmo para built-ins), e registrar as
   vetadas como `dyn Tool` no `ToolRegistry`, sob o motor de permissões existente.
3. Skill com `Block` **não** é registrada — e isso é provado por teste.

## Estado real na entrada (verificado neste commit)

- **O trait `Tool`** (`forge-tools/src/lib.rs:46-54`): `name()/description() ->
  &'static str`, `input_schema() -> Value`, `scope(&args) -> String` (avaliado pelo
  motor de permissões), `run(&args) -> Result<ToolOutput, ToolError>`.
  **Restrição encontrada:** `&'static str` impede uma skill carregada em runtime de
  implementar o trait (nome/descrição vêm do manifest, são `String` dinâmicas). Ver
  Decisão 1 — é a primeira mudança da onda.
- **`ToolRegistry`** (`registry.rs`): `default_set(root)`, `get`, `iter` — **sem
  método de registro** (a nuance (a) registrada no corpo do commit do plano-mestre).
  Construído em 2+ pontos do CLI (`main.rs:429` run, `:482` chat; conferir tui/squad).
- **O formato de skill já existe** (Onda 5): `skill.toml` com `name`, `description`,
  `entrypoint: Option<String>`, `permissions: Vec<String>` e `[[verify]]` steps
  opcionais (mesmo formato do `forge.toml`). O `vet_skill(dir, run_id, git_sha,
  produced_at) -> VettingResult` valida manifest, roda os verify steps, checa
  padrões perigosos e **coerência de permissões** (ex.: usa bash sem declarar).
- **O motor de permissões já cobre tools por escopo**: `LoopEvent::ToolDenied`
  existe, o loop pergunta por tool+scope. Uma skill registrada entra nesse fluxo
  automaticamente se o `scope()` dela for informativo.
- **`forge run` exige provider** (`prepare()`); a fronteira de "invocada de
  verdade" deve usar generator roteirizado em teste de integração, não key real
  (o padrão cassette de sempre).
- **Não existe** `skills/`, método de registro, nem qualquer `SkillTool`.

Baseline a preservar: contagens do main na entrada (medir), zero falhas, clippy/fmt
limpos, job `verify` do CI verde.

## Decisões de contrato (candidatas a ADR — provavelmente 0011)

1. **O trait `Tool` muda: `name()/description() -> &str`** (com lifetime de `&self`)
   em vez de `&'static str`. É a mudança mínima que permite implementadores
   dinâmicos; os 4 tools built-in continuam compilando (literal `&'static` coage
   para `&str`). Alternativa rejeitada: `Box::leak` das strings do manifest —
   funciona mas vaza por design; mudar o trait é limpo e o trait é interno.
2. **Skill → `dyn Tool` (o `SkillTool`)**: um wrapper que carrega o manifest + dir e
   implementa o trait. `run(args)` executa o `entrypoint` como subprocesso (cwd =
   dir da skill), args JSON via argv ou stdin (**decidir e documentar**; recomendação:
   um argumento posicional com o JSON serializado — simples e testável), stdout
   capturado com o `DEFAULT_OUTPUT_LIMIT` existente (mesma truncagem dos outros
   tools). `scope()` = o entrypoint + resumo dos args (para o permission-engine
   perguntar algo informativo, como faz com bash).
3. **`input_schema` da skill na Onda 1: genérico** (ex.: um campo `input: string`).
   O manifest da Onda 5 não tem campo de schema; estendê-lo com um schema declarado
   é aditivo e pode vir depois, quando uma skill real precisar. Não inventar agora.
4. **Registro no registry**: adicionar `register(Box<dyn Tool>)` (ou um construtor
   `with_skills(root, skills_dir)`). O fluxo de carga: descobrir subdiretórios de
   `skills/` → `vet_skill` em cada → `Vet` registra, `Block` pula **com log
   explícito** (o motivo/findings no stderr; entrada no ledger fica para a Onda 3,
   quando terceiros tornam isso obrigatório).
5. **Vetting a cada carga vs cache**: `vet_skill` roda os verify steps da skill —
   para built-ins com steps vazios/baratos, o custo é desprezível. **Recomendação:
   vetar a cada carga e medir; cache por hash de conteúdo só se doer** (a regra da
   casa: medir antes de otimizar). Registrar a decisão.

## Escopo desta onda

1. Mudança do trait (`&str`) + `register` no `ToolRegistry`.
2. `SkillTool` + o loader (descobre → veta → registra/pula).
3. `skills/` com 1–2 built-ins de exemplo (ex.: uma skill inócua tipo
   `word-count`/`todo-scan` com entrypoint shell simples) + `skills/README.md` com o
   padrão de autoria (formato do `skill.toml`, permissões, verify steps).
4. Integração no CLI: os pontos que constroem `default_set` passam a também carregar
   `skills/` (decidir: sempre, ou atrás de flag/config — recomendação: sempre para
   `skills/` do repo, que é confiável e vetado; diretório de usuário é Onda 3).
5. Testes (ver Fronteira).

## Fora de escopo (explícito)

- Sandbox/Docker → Onda 2. Built-ins rodam como subprocesso direto (são do repo,
  confiáveis, e ainda assim vetados).
- Skills de terceiros, diretório do usuário, ledger de vetting → Onda 3.
- Schema de input declarado no manifest → quando uma skill real exigir.
- MCP/LSP → Ondas 4–5.

## Fronteira verificável (o que o próximo — eu — vai rodar)

1. **O fio completo, com generator roteirizado (o teste que carrega a onda):** num
   workspace fixture com uma skill built-in, o loop do agente (scripted generator
   emitindo um tool_use para a skill) → a skill **executa de verdade** como
   subprocesso → o output real dela volta ao loop. Não é unit do wrapper: é o
   caminho registry→permissão→run→output.
2. **Block não registra:** uma skill fixture maliciosa (das que o vetter já bloqueia
   — padrão perigoso ou permissão incoerente) **não aparece** no registry
   (`get(name).is_none()`), e o log/motivo é emitido. Se ela aparecer, o vetting é
   decorativo.
3. **Fail-closed do loader:** `skill.toml` ausente/ilegível num subdiretório →
   pulado com log, nunca registrado (a lição recorrente — sem manifest não é "skill
   sem permissões", é não-skill).
4. **Permissão pedida:** a invocação da skill passa pelo permission-engine (o
   `scope()` chega ao resolver — teste com resolver que nega e prova
   `ToolDenied`/skill não executada).
5. **O trait mudado não regride:** os 4 tools existentes + suíte inteira verdes.

Além disso, o de sempre: `cargo test --workspace`, `clippy -- -D warnings`,
`fmt --check`, `uv run pytest`, e o job `verify` do CI (que agora inclui esta onda
no self-hosting).

## Riscos específicos

| Risco | Mitigação |
|---|---|
| `&'static str` resolvido com `Box::leak` (vazamento por design) | Mudar o trait para `&str` (Decisão 1); leak rejeitado explicitamente |
| Skill Block registrada mesmo assim (vetting decorativo) | Teste nº 2; o loader só registra em `Decision::Vet` |
| Entrypoint herdando env/cwd perigosos | cwd = dir da skill; env mínimo; subprocesso com timeout (reusar `exec::run_with_timeout` da Fase 5 O1, que já mata grupo de processos) |
| Skill que trava o loop | timeout do `run_with_timeout` + output truncado no limite existente |
| Registro espalhado (cada call-site do CLI carregando diferente) | Um único helper de construção (registry+skills) usado por todos os call-sites |
| Vetting a cada carga ficar lento no futuro | Medir agora; cache por hash só quando doer (decisão registrada) |

## Por que esta onda primeiro

É a fundação de todo o resto da Fase 6: a Onda 2 confina o que a 1 executa; a 3
aplica a máquina da 1+2 a código de terceiro; a 4 (MCP) reusa o `register` que a 1
cria. E ela fecha o dogfooding do vetter: desde o primeiro dia, **nenhuma skill —
nem as nossas — entra no registry sem passar pelo vetting**.

Contrato de sempre: implementa, commita, o próximo puxa e roda — com atenção ao
teste nº 2 (Block não registra) e ao nº 1 (o fio completo com execução real), que
são onde a onda prova que o runtime existe e que o vetting morde desde o built-in.
