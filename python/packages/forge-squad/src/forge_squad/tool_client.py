"""Contrato do client de ferramentas consumido pelo `DeveloperAgent` —
desacoplado do transporte gRPC real (`CoreService.RunTool`,
`schemas/proto/core.proto`) pelo mesmo motivo do `GatewayClient`/
`PermissionClient` (ADR 0005). "Tool execution architecture" (Onda 2): o
loop ReAct do developer chama `run_tool`, que executa de verdade do lado
Rust sob `ToolRegistry`/`PermissionEngine` — o Python nunca toca disco.
"""

from __future__ import annotations

from typing import Protocol

from pydantic import BaseModel


class ToolCallRequest(BaseModel):
    """Espelha `forge.core.v1.ToolCall` (`schemas/proto/core.proto`)."""

    tool: str
    args_json: str
    scope: str = ""


class ToolCallResult(BaseModel):
    """Espelha `forge.core.v1.ToolResult`. `exit_code`: 0 sucesso, 1 erro de
    execução/args inválidos/ferramenta desconhecida, -1 negado pelo motor de
    permissões ou por um humano (nunca chegou a executar)."""

    content: str
    truncated: bool = False
    exit_code: int = 0


class ToolClient(Protocol):
    """Contrato consumido pelo `DeveloperAgent`. A implementação real fala
    gRPC com `CoreService.RunTool`; testes usam `ScriptedToolClient`.
    """

    async def run_tool(self, request: ToolCallRequest) -> ToolCallResult: ...


class ScriptedToolClient:
    """Client de ferramentas falso e determinístico para testes — mesmo
    princípio do `ScriptedGatewayClient`/`ScriptedPermissionClient`.
    """

    def __init__(self, results: list[ToolCallResult]) -> None:
        self._results = list(results)
        self.requests: list[ToolCallRequest] = []

    async def run_tool(self, request: ToolCallRequest) -> ToolCallResult:
        self.requests.append(request)
        if not self._results:
            raise AssertionError("ScriptedToolClient esgotou os resultados roteirizados")
        return self._results.pop(0)
