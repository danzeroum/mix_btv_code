"""Implementações gRPC reais dos Protocols `GatewayClient`/`PermissionClient`
(ADR 0005), falando `CoreService` (`schemas/proto/core.proto`) do lado Rust.

A promessa do ADR 0005 sendo cobrada: os agentes/planner/autonomia
consomem os Protocols e não mudam uma linha — só a implementação injetada
troca de `Scripted*` para `Grpc*`.

Cuidado com o default-zero do proto3 (a defesa é mapear campo a campo,
explicitamente, e testar): um campo ausente no wire vira 0/""/false sem
erro. Aqui, em particular, `PermissionDecision.decision` ausente vira
`DECISION_UNSPECIFIED` (0) → `approved=False` — que é o default
**fail-closed** correto (nunca aprova por omissão).
"""

from __future__ import annotations

import json

from forge_proto import core_pb2, llm_pb2
from forge_proto import core_pb2_grpc

from forge_squad.gateway import LlmRequest, LlmResponse
from forge_squad.permission import PermissionDecision, PermissionRequest
from forge_squad.tool_client import ToolCallRequest, ToolCallResult


class GrpcGatewayClient:
    """`GatewayClient` sobre `CoreService.Generate` (stream de `LlmChunk`)."""

    def __init__(self, channel) -> None:
        self._stub = core_pb2_grpc.CoreServiceStub(channel)

    async def generate(self, request: LlmRequest) -> LlmResponse:
        proto_req = llm_pb2.LlmRequest(
            model=request.model,
            messages_json=json.dumps(request.messages),
            requester=request.requester,
        )
        if request.temperature is not None:
            proto_req.temperature = request.temperature
        if request.max_tokens is not None:
            proto_req.max_tokens = request.max_tokens

        text_parts: list[str] = []
        input_tokens = 0
        output_tokens = 0
        cache_hit = False
        provider = ""

        async for chunk in self._stub.Generate(proto_req):
            which = chunk.WhichOneof("payload")
            if which == "text_delta":
                text_parts.append(chunk.text_delta)
            elif which == "usage":
                input_tokens = chunk.usage.input_tokens
                output_tokens = chunk.usage.output_tokens
                cache_hit = chunk.usage.cache_hit
                provider = chunk.usage.provider
            elif which == "error":
                raise RuntimeError(f"gateway error: {chunk.error}")

        return LlmResponse(
            text="".join(text_parts),
            input_tokens=input_tokens,
            output_tokens=output_tokens,
            cache_hit=cache_hit,
            provider=provider,
        )


class GrpcPermissionClient:
    """`PermissionClient` sobre `CoreService.RequestPermission`."""

    def __init__(self, channel) -> None:
        self._stub = core_pb2_grpc.CoreServiceStub(channel)

    async def request_permission(self, request: PermissionRequest) -> PermissionDecision:
        proto_req = core_pb2.PermissionRequest(
            tool=request.tool,
            scope=request.scope,
            reason=request.reason,
            confidence=request.confidence,
        )
        decision = await self._stub.RequestPermission(proto_req)
        approved = decision.decision == core_pb2.PermissionDecision.ALLOW
        note = decision.operator_note if decision.HasField("operator_note") else None
        return PermissionDecision(approved=approved, operator_note=note)


class GrpcToolClient:
    """`ToolClient` sobre `CoreService.RunTool` — execução real de
    ferramenta do lado Rust ("tool execution architecture", Onda 1/2)."""

    def __init__(self, channel) -> None:
        self._stub = core_pb2_grpc.CoreServiceStub(channel)

    async def run_tool(self, request: ToolCallRequest) -> ToolCallResult:
        proto_req = core_pb2.ToolCall(
            tool=request.tool,
            args_json=request.args_json,
            scope=request.scope,
        )
        result = await self._stub.RunTool(proto_req)
        return ToolCallResult(
            content=result.content,
            truncated=result.truncated,
            exit_code=result.exit_code,
        )
