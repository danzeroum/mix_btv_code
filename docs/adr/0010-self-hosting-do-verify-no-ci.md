# ADR 0010 — Self-hosting do `/verify` no CI: job separado + artefato como evidência

- Status: aceita
- Data: 2026-07-06

## Contexto

A Fase 5 Onda 6 fecha a fase realizando o critério de conclusão do
PLANO-mestre: *self-hosting; PR sem evidência bloqueado*. Duas perguntas
de contrato precisavam de resposta: (1) como a evidência de verificação de
um PR é exigida/tornada visível, e (2) se o job novo substitui ou
complementa o job `rust` já existente (que roda `cargo test/clippy/fmt`).

## Decisão 1 — evidência como artefato do workflow, não arquivo commitado

Opções consideradas:
- (a) **Artefato do workflow** (`actions/upload-artifact`) — a evidência
  fica anexada ao run do CI, baixável, mas não entra no diff do PR.
- (b) Evidência **commitada** no PR, com o CI validando que bate com o run atual.
- (c) Um **status check** dedicado que só fica verde com evidência válida.

Escolhida **(a)**. Razões: o próprio exit code de `forge verify` (≠0 em
`Fail`, já construído na Onda 2 exatamente para este momento) já é o gate
— nenhum encanamento adicional de status check é necessário para
"bloquear o PR sem evidência", pois um `Fail` já reprova o job e portanto
o check do GitHub. (b) obrigaria commitar um arquivo gerado a cada push
(ruído de diff, e o risco de divergência entre o que foi commitado e o
que o run atual produziria). (c) duplicaria o que o exit code já faz.

## Decisão 2 — job `verify` separado do job `rust`, não substituindo-o

Os `default_steps()` de `forge-verify` (test/clippy/fmt --workspace) já
duplicam o que o job `rust` roda. Considerado reescrever o job `rust`
para passar inteiramente por `forge verify` (dogfooding máximo), mas
decidido manter **job separado**: não arriscar o gate que já funciona e é
o mais antigo/testado do repositório. A fusão dos dois é possível numa
onda futura, quando `forge verify` tiver mais tempo de estrada em CI real
(hoje só foi exercitado localmente e nesta onda).

Custo aceito: o job `verify` roda a suíte de testes uma segunda vez
(medido localmente nesta onda: ~32s para `cargo test --workspace` +
`clippy` + `fmt --check` via `forge verify` no container de
desenvolvimento — GitHub Actions runners têm perfil de CPU diferente, mas
a mesma ordem de grandeza é esperada). Aceito porque o objetivo desta
onda é *existir* o self-hosting, não otimizá-lo; medir antes de otimizar.

## O que foi provado, não só declarado

- `forge verify` rodado localmente sobre o próprio workspace: `verdict:
  pass`, exit 0 (~32s).
- Um teste quebrado propositalmente (`assert_eq!(1, 2, ...)` inserido
  temporariamente em `forge-schemas::canonical`, depois revertido) fez
  `forge verify` sair com `verdict: fail` e **exit code 1** — a prova de
  que o gate morde, não um job decorativo que sempre passa.

## Consequências

- `.github/workflows/ci.yml` ganha o job `verify`: `cargo run -p
  forge-cli -- verify --out verification-evidence.json` seguido de
  `actions/upload-artifact` com `if: always()` (a evidência fica anexada
  ao run mesmo quando o veredito é `Fail` — é justamente aí que ela mais
  importa para diagnóstico).
- **Fora do que este ADR/commit pode fazer:** marcar o job `verify` como
  *required status check* na proteção de branch do repositório é
  configuração de administração do GitHub (Settings → Branches), não algo
  que este commit altera — é uma mudança de política de merge com escopo
  maior que o diff de código, e fica como recomendação explícita para
  quem administra o repositório, não uma ação silenciosa desta onda.
- Se o job `verify` se mostrar lento demais em CI real (diferente do
  medido localmente), a mitigação natural é um `forge.toml` de CI com
  passos mais enxutos que os `default_steps()` — não implementado agora
  por não haver evidência real do custo em CI ainda.
