"""Servidor gRPC do `SquadService` (`schemas/proto/squad.proto`) — Onda 4c.

`ExecuteTask(SquadTask) → stream SquadEvent`: roda o `UnifiedOrchestrator`
e streama os eventos ao vivo (proposta/consenso/handoff/hitl/step) que o
orquestrador emite pelo `event_sink`. Os eventos são traduzidos de dicts
pydantic-friendly para `SquadEvent` do proto — mapeamento campo a campo,
explícito, para não cair no default-zero do proto3 (em particular
`Consensus.requires_human`, que é uma `@property` do lado pydantic e um
campo do lado proto — precisa ser setado à mão nos dois sentidos).

Bidirecional: durante `ExecuteTask` o Python é servidor, mas os agentes
precisam do LLM e da decisão de permissão — que vêm de volta do Rust via
`CoreService` (`--core-socket`). É a promessa do ADR 0005 fechando o laço:
`GrpcGatewayClient`/`GrpcPermissionClient` substituem os `Scripted*` sem
os agentes mudarem.

Uso: python -m forge_squad.server --socket <squad.sock> --core-socket <core.sock>
"""

from __future__ import annotations

import argparse
import asyncio
import json
import logging
import os
from datetime import datetime, timezone
from pathlib import Path
from typing import Any, Optional

import grpc

from forge_proto import squad_pb2, squad_pb2_grpc

from forge_squad.grpc_clients import GrpcGatewayClient, GrpcPermissionClient, GrpcToolClient
from forge_squad.memory import AgentMemorySystem
from forge_squad.orchestrator import UnifiedOrchestrator

logger = logging.getLogger(__name__)

VERSION = "0.1.0"

_PHASE = {
    "start": squad_pb2.Handoff.Phase.START,
    "ack": squad_pb2.Handoff.Phase.ACK,
    "complete": squad_pb2.Handoff.Phase.COMPLETE,
    "error": squad_pb2.Handoff.Phase.ERROR,
}


def _to_squad_event(task_id: str, event: dict[str, Any]) -> squad_pb2.SquadEvent:
    ev = squad_pb2.SquadEvent(task_id=task_id, ts=datetime.now(timezone.utc).isoformat())
    kind = event["kind"]
    if kind == "proposal":
        ev.proposal.CopyFrom(
            squad_pb2.Proposal(
                agent=event["agent"],
                confidence=float(event["confidence"]),
                content_json=json.dumps(event["content"], ensure_ascii=False),
            )
        )
    elif kind == "consensus":
        ev.consensus.CopyFrom(
            squad_pb2.Consensus(
                decision_maker=event["decision_maker"] or "",
                strength=float(event["strength"]),
                decision_json=json.dumps(event["decision"], ensure_ascii=False),
                requires_human=bool(event["requires_human"]),  # proto3: setar à mão ou vira false
            )
        )
    elif kind == "hitl":
        ev.hitl.CopyFrom(
            squad_pb2.HitlEscalation(reason=event["reason"], confidence=float(event["confidence"]))
        )
    elif kind == "handoff":
        ev.handoff.CopyFrom(
            squad_pb2.Handoff(
                phase=_PHASE[event["phase"]],
                from_agent=event["from_agent"],
                to_agent=event["to_agent"],
            )
        )
    elif kind == "step":
        ev.step.CopyFrom(
            squad_pb2.StepResult(
                step_id=event["step_id"], success=bool(event["success"]), summary=event["summary"]
            )
        )
    elif kind == "chat":
        ev.chat.CopyFrom(
            squad_pb2.ChatMessage(
                author=event["author"],
                author_role=event["author_role"],
                text=event["text"],
                in_reply_to=event.get("in_reply_to", ""),
            )
        )
    elif kind == "error":
        ev.error = event["message"]
    else:  # pragma: no cover - guarda defensiva
        ev.error = f"evento desconhecido: {kind}"
    return ev


def _parse_verification_evidence(raw: str) -> tuple[Optional[dict[str, Any]], bool]:
    """Parseia `SquadTask.verification_evidence_json` (Fase 5 Onda 3, ADR
    0008). Retorna `(evidencia, ausente_ou_invalida)`.

    proto3 zera campo string ausente para `""` — sem tratamento explícito,
    isso viraria silenciosamente "sem evidência, tudo bem" no orquestrador,
    o mesmo tipo de default otimista que a régua "Nada Fake" proíbe. Por
    isso o segundo valor de retorno é explícito: `True` sempre que a
    evidência não pôde ser recuperada (ausente, JSON inválido, ou não é um
    objeto), para o orquestrador tratar como fail-closed."""

    if not raw:
        return None, True
    try:
        parsed = json.loads(raw)
    except json.JSONDecodeError:
        logger.warning("verification_evidence_json inválido — tratando como ausente (fail-closed)")
        return None, True
    if not isinstance(parsed, dict):
        logger.warning("verification_evidence_json não é um objeto JSON — tratando como ausente (fail-closed)")
        return None, True
    return parsed, False


class SquadServicer(squad_pb2_grpc.SquadServiceServicer):
    """Roda o orquestrador e streama seus eventos como `SquadEvent`."""

    def __init__(self, core_socket: str, model: str = "claude-sonnet-5", memory_dir: Optional[Path] = None) -> None:
        self.core_socket = core_socket
        self.model = model
        self.memory_dir = memory_dir

    async def Health(self, request, context):  # noqa: N802
        return squad_pb2.HealthResponse(ready=True, version=VERSION)

    async def ExecuteTask(self, request, context):  # noqa: N802
        evidence, evidence_missing = _parse_verification_evidence(request.verification_evidence_json)
        task = {
            "task_id": request.task_id,
            "description": request.description,
            "decision_type": request.decision_type or "architecture",
            "verification_evidence": evidence,
            "verification_evidence_missing": evidence_missing,
        }
        queue: asyncio.Queue = asyncio.Queue()

        async def sink(event: dict[str, Any]) -> None:
            await queue.put(event)

        # `grpc.default_authority` explícito: sobre UDS, o grpc-python
        # deriva um `:authority` do path do socket que o servidor tonic (h2)
        # rejeita como PROTOCOL_ERROR (RST_STREAM). Fixar um authority
        # válido resolve a interop Python-cliente → Rust-servidor.
        channel = grpc.aio.insecure_channel(
            f"unix://{self.core_socket}", options=[("grpc.default_authority", "localhost")]
        )
        gateway = GrpcGatewayClient(channel)
        permission = GrpcPermissionClient(channel)
        tool_client = GrpcToolClient(channel)
        memory = AgentMemorySystem(storage_dir=self.memory_dir) if self.memory_dir else AgentMemorySystem()
        orchestrator = UnifiedOrchestrator(
            gateway,
            permission_client=permission,
            model=self.model,
            memory=memory,
            tool_client=tool_client,
        )

        async def run() -> None:
            try:
                await orchestrator.execute_complex_task(task, event_sink=sink)
            except Exception as exc:  # noqa: BLE001 - o erro vira um SquadEvent
                logger.exception("execução do squad falhou")
                await queue.put({"kind": "error", "message": str(exc)})
            finally:
                await queue.put(None)  # sentinela de fim

        runner = asyncio.create_task(run())
        try:
            while True:
                event = await queue.get()
                if event is None:
                    break
                yield _to_squad_event(request.task_id, event)
        finally:
            await runner
            await channel.close()


async def serve(socket_path: str, core_socket: str, model: str = "claude-sonnet-5") -> None:
    if os.path.exists(socket_path):
        os.remove(socket_path)
    server = grpc.aio.server()
    squad_pb2_grpc.add_SquadServiceServicer_to_server(
        SquadServicer(core_socket=core_socket, model=model), server
    )
    server.add_insecure_port(f"unix://{socket_path}")
    await server.start()
    logger.info("forge_squad sidecar ouvindo em %s (core em %s)", socket_path, core_socket)
    await server.wait_for_termination()


def main() -> None:
    parser = argparse.ArgumentParser(description="Sidecar gRPC do Squad")
    parser.add_argument("--socket", required=True, help="caminho do UDS do SquadService")
    parser.add_argument("--core-socket", required=True, help="caminho do UDS do CoreService (Rust)")
    parser.add_argument("--model", default="claude-sonnet-5")
    args = parser.parse_args()
    logging.basicConfig(level=logging.INFO)
    asyncio.run(serve(args.socket, args.core_socket, args.model))


if __name__ == "__main__":
    main()
