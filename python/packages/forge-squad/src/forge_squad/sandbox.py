"""Sandbox de execução de ferramentas (migrado de BuildToValue
`src/tools/sandbox/{secure_executor,docker_sandbox}.py`).

`DockerSandbox` fica como stub nesta onda — contêineres reais são
escopo da Fase 6 (sandbox Docker no roadmap). Sem Docker disponível,
`SecureToolSandbox` devolve uma resposta simulada em vez de falhar,
mesmo princípio de degradação graciosa usado no resto da plataforma.
"""

from __future__ import annotations

import json
import logging
import re
from dataclasses import dataclass
from typing import Any, Optional

from forge_squad.security import SecurityConfig

try:
    import docker
except Exception:  # pragma: no cover - dependência opcional
    docker = None

logger = logging.getLogger(__name__)


class SecurityError(RuntimeError):
    """Levantado quando uma chamada de ferramenta viola os guardrails."""


@dataclass
class DockerSandbox:
    """Wrapper mínimo para rodar comandos em contêineres isolados."""

    image: str = "python:3.11-slim"
    network_disabled: bool = True

    def run(self, command: list[str], environment: Optional[dict[str, Any]] = None, timeout: int = 30) -> str:
        if docker is None:
            raise RuntimeError("Docker SDK não disponível no ambiente atual")

        client = docker.from_env()
        container = client.containers.run(
            self.image,
            command,
            detach=True,
            auto_remove=True,
            environment=environment or {},
            network_disabled=self.network_disabled,
        )
        result = container.wait(timeout=timeout)
        if result.get("StatusCode", 1) != 0:
            logs = container.logs()
            raise RuntimeError(f"Sandboxed process falhou: {logs}")
        logs = container.logs()
        return logs.decode("utf-8") if isinstance(logs, bytes) else str(logs)


@dataclass
class SecureToolSandbox:
    """Validação e execução (opcionalmente containerizada) de ferramentas."""

    docker_sandbox: Optional[DockerSandbox] = None
    execution_timeout: int = 30
    memory_limit_mb: int = 512
    cpu_quota: float = 0.5

    def execute_tool_sandboxed(self, tool_name: str, params: dict[str, Any]) -> dict[str, Any]:
        """Executa a ferramenta num ambiente isolado quando possível."""

        self._validate_security(tool_name, params)

        if self.docker_sandbox is None:
            logger.warning("Docker sandbox indisponível; devolvendo resposta simulada")
            return {
                "tool": tool_name,
                "params": params,
                "sandboxed": False,
                "message": "Docker indisponível, execução simulada",
            }

        command = ["python", "-m", tool_name]
        output = self.docker_sandbox.run(
            command,
            environment={"PARAMS": json.dumps(params)},
            timeout=self.execution_timeout,
        )
        return {"tool": tool_name, "output": output, "sandboxed": True}

    def _validate_security(self, tool_name: str, params: dict[str, Any]) -> None:
        is_safe, reason = SecurityConfig.validate_tool_call(tool_name, params)
        if not is_safe:
            raise SecurityError(f"Tool execution blocked: {reason}")
        if not self._validate_params_safety(params):
            raise ValueError(f"Dangerous parameters detected for {tool_name}")

    def _validate_params_safety(self, params: dict[str, Any]) -> bool:
        param_str = json.dumps(params, ensure_ascii=False)
        for pattern in SecurityConfig.FORBIDDEN_PATTERNS:
            if re.search(pattern, param_str, re.IGNORECASE):
                logger.error("Padrão proibido detectado na validação do sandbox: %s", pattern)
                return False
        return True
