"""Configuração de segurança para execução de ferramentas (migrado de
BuildToValue `src/safety/security_config.py`).

Camada de defesa em profundidade no lado Python — a autoridade final
continua sendo o Rust (`forge-core::permission`, ADR 0001): esta
validação nunca é o único portão, é uma checagem adicional antes de
qualquer chamada chegar ao `SecureToolSandbox`.
"""

from __future__ import annotations

import re


class SecurityConfig:
    """Regras centralizadas de segurança para chamadas de ferramenta."""

    MAX_EXECUTION_TIME_SECONDS = 30
    MAX_MEMORY_PER_TOOL_MB = 512
    MAX_CONCURRENT_TOOLS = 5

    FORBIDDEN_PATTERNS = [
        r"rm\s+-rf\s+/",
        r":\(\)\{ :\|:& \};:",
        r"dd\s+if=/dev/zero",
        r"DROP\s+DATABASE",
        r"<script",
        r"eval\(",
        r"__import__",
        r"os\.system",
        r"subprocess\.",
        r"socket\.",
    ]

    ALLOWED_DOMAINS = {"api.buildtoflip.com", "localhost", "127.0.0.1"}
    HIGH_RISK_TOOLS = {
        "database_write",
        "send_email",
        "make_payment",
        "delete_resource",
        "modify_production_config",
    }

    @classmethod
    def validate_tool_call(cls, tool_name: str, params: dict[str, object]) -> tuple[bool, str]:
        if tool_name in cls.HIGH_RISK_TOOLS:
            return False, f"Tool {tool_name} requires human approval"
        payload = str(params)
        for pattern in cls.FORBIDDEN_PATTERNS:
            if re.search(pattern, payload, re.IGNORECASE):
                return False, f"Forbidden pattern detected: {pattern}"
        return True, "OK"
