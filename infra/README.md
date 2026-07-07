# infra/ — provisionamento e load-test (Fase 6 Onda 8)

> **Estado honesto:** o Forge é **local-first** — o servidor só escuta em
> `127.0.0.1` e **não há alvo de deploy hospedado** hoje (sem Dockerfile, sem
> cloud). Portanto o terraform/ansible aqui é **esqueleto marcado**, não infra de
> produção: um ponto de partida para quando (e se) houver um alvo real. Entregar o
> esqueleto honesto é melhor que terraform decorativo — decisão pré-autorizada no
> `docs/PLANO-FASE-6.md` §Onda 8 ("Nota honesta").

## Conteúdo

- **`k6/gateway_load.js`** — o único artefato **executado** aqui: load-test
  **real** do caminho do gateway. Martela o endpoint de carga (`forge-server` bin
  `loadgen`, que embrulha o `ScriptedGenerator` — **sem key real**) e valida o
  **P95** sob concorrência. Roda no CI (job `k6`) e localmente. É a régua do
  critério de conclusão nº 3 da fase ("k6 valida o P95 do gateway").
- **`terraform/`** — esqueleto de provisionamento (sem alvo real; ver o cabeçalho
  do `main.tf`).
- **`ansible/`** — esqueleto de configuração (idem).
- **`docker/`** — imagem de **TESTE / homologação** (não é deploy de produção):
  empacota o CLI `forge` + o sidecar Python para você rodar o Forge como
  ferramenta dentro de um container, na sua VPS via SSH. Continua local-first (o
  dashboard segue em `127.0.0.1`). Ver `docker/README.md` para as pegadinhas de
  container (o `FORGE_PYTHON_DIR` obrigatório, o sandbox via socket do Docker, o
  bind do dashboard). Não expõe serviço multiusuário na internet.

## Rodar o load-test localmente

```sh
# 1. sobe o endpoint de carga (sem key — usa o ScriptedGenerator)
FORGE_LOADGEN_PORT=7900 cargo run -p forge-server --bin loadgen &

# 2. martela e valida o P95 (precisa do k6 instalado: https://k6.io)
k6 run infra/k6/gateway_load.js
```

O threshold do P95 vive no próprio script (`http_req_duration: p(95)<...`); o k6
sai com código ≠0 se estourar — o gate é real, não decorativo.
