"""Servidor gRPC do `MemoryService` (`schemas/proto/memory.proto`) — Fase 7
Onda 8, ADR 0022. Mirror de `forge_promptforge.server`: sidecar stateless,
supervisão singleton (não pool — leitura de memória é barata, não deveria
disputar recurso com uma execução de squad real).

Direção OPOSTA de `SquadServicer`/`PromptForgeServicer` quanto a quem é dono
do dado: aqui o Python já era o dono (o corpus episódico JSONL de
`AgentMemorySystem` sempre viveu aqui) — o serviço só expõe leitura sobre o
que já existe, nunca grava (sem `Remember`: quem grava é só o orquestrador,
em processo, via `remember_decision`).

Uso: python -m forge_squad.memory_server --socket <memory.sock> [--memory-dir <dir>]
"""

from __future__ import annotations

import argparse
import asyncio
import json
import logging
import os
from pathlib import Path
from typing import Any, Optional

import grpc

from forge_proto import memory_pb2, memory_pb2_grpc

from forge_squad.memory import AgentMemorySystem

logger = logging.getLogger(__name__)

VERSION = "0.1.0"


def _summarize_by_agent(records: list[dict[str, Any]]) -> list[memory_pb2.MemorySummary]:
    """Agrupa registros (já filtrados/ordenados por `list_memories`, mais
    recente primeiro) por agente — contagem real, decisão mais recente real
    (primeira ocorrência, já que a lista chega nessa ordem) e a de maior
    confiança real. Nenhuma tendência de esquecimento: não há cálculo disso
    no código (`forgetting.py` é código morto, não consultado aqui)."""
    by_agent: dict[str, list[dict[str, Any]]] = {}
    for rec in records:
        by_agent.setdefault(rec.get("agent") or "?", []).append(rec)

    summaries = []
    for agent, recs in by_agent.items():
        latest = recs[0]
        top = max(recs, key=lambda r: float(r.get("confidence", 0.0)))
        summaries.append(
            memory_pb2.MemorySummary(
                agent=agent,
                count=len(recs),
                latest_decision_json=json.dumps(latest.get("decision", {}), ensure_ascii=False),
                latest_timestamp=str(latest.get("timestamp") or ""),
                top_confidence=float(top.get("confidence", 0.0)),
            )
        )
    return summaries


class MemoryServicer(memory_pb2_grpc.MemoryServiceServicer):
    """Expõe `AgentMemorySystem` (recall léxico + mapa por agente) sobre gRPC."""

    def __init__(self, memory_dir: Optional[Path] = None) -> None:
        self.memory = AgentMemorySystem(storage_dir=memory_dir) if memory_dir else AgentMemorySystem()

    async def Health(self, request, context):  # noqa: N802
        return memory_pb2.HealthResponse(ready=True, version=VERSION)

    async def Recall(self, request, context):  # noqa: N802
        result = self.memory.recall_similar(request.query, request.k or 5)
        matches = [
            memory_pb2.MemoryMatch(
                id=id_,
                agent=meta.get("agent") or "",
                decision_json=doc,
                timestamp=str(meta.get("timestamp") or ""),
                score=float(score),
            )
            for id_, doc, meta, score in zip(
                result["ids"], result["documents"], result["metadatas"], result["scores"]
            )
        ]
        return memory_pb2.RecallResponse(matches=matches)

    async def List(self, request, context):  # noqa: N802
        agent = request.agent if request.HasField("agent") else None
        limit = request.limit or 50
        records = self.memory.list_memories(agent, limit)
        return memory_pb2.ListResponse(agents=_summarize_by_agent(records))


async def serve(socket_path: str, memory_dir: Optional[str] = None) -> None:
    if os.path.exists(socket_path):
        os.remove(socket_path)
    server = grpc.aio.server()
    memory_pb2_grpc.add_MemoryServiceServicer_to_server(
        MemoryServicer(Path(memory_dir) if memory_dir else None), server
    )
    server.add_insecure_port(f"unix://{socket_path}")
    await server.start()
    logger.info("forge_squad memory sidecar ouvindo em %s", socket_path)
    await server.wait_for_termination()


def main() -> None:
    parser = argparse.ArgumentParser(description="Sidecar gRPC do MemoryService")
    parser.add_argument("--socket", required=True, help="caminho do Unix Domain Socket")
    parser.add_argument(
        "--memory-dir", default=None, help="diretório de armazenamento (default: .forge/squad-memory)"
    )
    args = parser.parse_args()
    logging.basicConfig(level=logging.INFO)
    asyncio.run(serve(args.socket, args.memory_dir))


if __name__ == "__main__":
    main()
