"""Agente auditor (migrado de BuildToValue `src/agents/auditor_agent.py`).

Na origem, o veredito final (`passed`/`confidence`/`approved`) vinha de
fórmulas de pontuação hardcoded (`score_agent_result`: +0.2 se sucesso,
+0.1 se confiança > 0.7, +0.1 se sem erro, +0.1 se testado) — um carimbo
automático disfarçado de auditoria, o oposto da tese do projeto ("o LLM
orquestra; ferramentas determinísticas verificam").

Esta versão mantém `check_security`/`check_quality` como checagens
determinísticas de verdade (busca de padrão / limiares — legítimas, no
mesmo espírito de um linter) e as usa como **evidência de entrada** para
uma chamada real ao gateway, que produz o veredito. O modelo nunca
aprova automaticamente por ausência de achados críticos — é instruído a
considerar o contexto da tarefa, não só a lista de achados.

Fase 5 Onda 3: `validate_results` passa a receber a evidência real do
`/verify` (Rust, `forge-verify`) — a `verification-evidence.v1` que o
`forge squad` roda sobre o workspace e anexa ao `SquadTask` antes de
disparar a tarefa. `check_security`/`check_quality` continuam (baratos,
complementares); a evidência do `/verify` é entrada adicional para o
gateway, nunca decisão automática — o veredito ainda vem do modelo.
"""

from __future__ import annotations

import json
import logging
import re
from typing import Any

from forge_squad.agents.base import BaseAgent
from forge_squad.gateway import LlmRequest

logger = logging.getLogger(__name__)

_AUDIT_SYSTEM_PROMPT = """Você é um auditor de segurança e qualidade sênior. Você recebe uma tarefa e achados determinísticos (já verificados por ferramentas de padrão/métricas) e deve produzir um julgamento real.
Responda SOMENTE com um objeto JSON:
{
  "passed": true ou false,
  "confidence": 0.0,
  "notes": "string — sua avaliação dos achados e do contexto para ESTA tarefa",
  "additional_checks": ["lista de strings — verificações adicionais recomendadas, se houver"]
}
Não aprove automaticamente só porque não há achados críticos na lista — considere a criticidade e o contexto da tarefa. Não reprove automaticamente por qualquer achado menor — pondere severidade."""

_VALIDATE_RESULTS_SYSTEM_PROMPT = """Você é um auditor revisando os resultados de uma equipe de agentes especialistas antes de aprovar a conclusão de uma tarefa.
Responda SOMENTE com um objeto JSON:
{
  "approved": true ou false,
  "confidence": 0.0,
  "issues": ["lista de strings — problemas encontrados nos resultados, se houver"],
  "agent_scores": {"nome_do_agente": 0.0}
}
Avalie cada resultado pelo conteúdo real reportado (sucesso, confiança declarada, presença de erros) — não aprove por padrão. Quando houver evidência determinística de verificação (`verification_evidence` — typecheck/test/lint/SAST reais), pese o veredito e os achados dela — um veredito "fail" ou achados de severidade alta pesam contra a aprovação, mas a decisão final ainda é sua, considerando o contexto da tarefa."""

_JSON_BLOCK = re.compile(r"\{.*\}", re.DOTALL)

_DANGEROUS_PATTERNS = [
    ("eval(", "Code injection risk"),
    ("exec(", "Code execution risk"),
    ("__import__", "Dynamic import risk"),
    ("os.system", "System command execution"),
    ("subprocess", "Process spawning risk"),
]


class AuditorAgent(BaseAgent):
    """Especialista em segurança e qualidade com veredito real via gateway."""

    def __init__(self, model: str = "claude-sonnet-5") -> None:
        super().__init__("auditor")
        self.model = model
        self.validation_history: list[dict[str, Any]] = []
        self.tools = ["security_scan", "quality_check"]

    async def execute(self, task: dict[str, Any]) -> dict[str, Any]:
        if not self.validate_input(task):
            raise ValueError("Invalid audit task payload")

        assessment = await self.audit(task)
        self.validation_history.append(assessment)
        self.log_decision({"task": task, "assessment": assessment, "confidence": assessment.get("confidence", 0.0)})
        return {
            "success": True,
            "agent": self.agent_type,
            "assessment": assessment,
            "confidence": assessment.get("confidence", 0.0),
        }

    async def audit(self, task: dict[str, Any]) -> dict[str, Any]:
        """Roda as checagens determinísticas e pede ao gateway um veredito
        real informado por elas."""

        if self.gateway is None:
            raise RuntimeError(
                "AuditorAgent sem gateway anexado — chame attach_gateway() antes de execute()"
            )

        issues = self.check_security(task.get("code", "")) if task.get("code") else []
        warnings = self.check_quality(task.get("metrics", {})) if task.get("metrics") else []

        request = LlmRequest(
            model=self.model,
            messages=[
                {"role": "system", "content": _AUDIT_SYSTEM_PROMPT},
                {
                    "role": "user",
                    "content": json.dumps(
                        {
                            "task_description": task.get("description", ""),
                            "security_issues": issues,
                            "quality_warnings": warnings,
                        },
                        ensure_ascii=False,
                    ),
                },
            ],
            requester=self.agent_type,
        )
        raw = await self.gateway.generate(request)
        judgment = self._parse_judgment(raw.text)
        return {"issues": issues, "warnings": warnings, **judgment}

    async def validate_results(
        self, results: list[dict[str, Any]], evidence: dict[str, Any] | None = None
    ) -> dict[str, Any]:
        """Pede ao gateway um veredito real sobre os resultados agregados
        de outros agentes (usado pelo orquestrador ao final de um plano).

        `evidence` é a `verification-evidence.v1` real do `/verify` (Fase 5
        Onda 3), quando disponível — entra no payload como contexto
        adicional para o gateway pesar; o veredito continua vindo do
        modelo, nunca é derivado automaticamente da evidência sozinha (ver
        `test_validade_pass_nao_forca_aprovacao_sozinha`)."""

        if self.gateway is None:
            raise RuntimeError(
                "AuditorAgent sem gateway anexado — chame attach_gateway() antes de execute()"
            )

        payload: dict[str, Any] = {"results": results}
        if evidence is not None:
            payload["verification_evidence"] = evidence

        request = LlmRequest(
            model=self.model,
            messages=[
                {"role": "system", "content": _VALIDATE_RESULTS_SYSTEM_PROMPT},
                {"role": "user", "content": json.dumps(payload, ensure_ascii=False)},
            ],
            requester=self.agent_type,
        )
        raw = await self.gateway.generate(request)
        return self._parse_validation(raw.text)

    def check_security(self, code: str) -> list[dict[str, Any]]:
        """Busca de padrões perigosos conhecidos — checagem determinística
        (evidência de entrada para o julgamento real, não o julgamento)."""

        issues: list[dict[str, Any]] = []
        for pattern, description in _DANGEROUS_PATTERNS:
            if pattern in code:
                issues.append(
                    {"type": "security", "severity": "critical", "pattern": pattern, "description": description}
                )
        return issues

    def check_quality(self, metrics: dict[str, Any]) -> list[dict[str, Any]]:
        """Checagem determinística de métricas contra limiares fixos."""

        warnings: list[dict[str, Any]] = []
        complexity = metrics.get("complexity")
        if complexity is not None and complexity > 10:
            warnings.append(
                {"type": "quality", "severity": "warning", "metric": "complexity", "value": complexity, "threshold": 10}
            )
        coverage = metrics.get("coverage")
        if coverage is not None and coverage < 80:
            warnings.append(
                {"type": "quality", "severity": "warning", "metric": "coverage", "value": coverage, "threshold": 80}
            )
        return warnings

    def _parse_judgment(self, raw_text: str) -> dict[str, Any]:
        parsed = self._extract_json(raw_text)
        return {
            "passed": bool(parsed.get("passed", False)),
            "confidence": float(parsed.get("confidence", 0.0)),
            "notes": parsed.get("notes", ""),
            "additional_checks": parsed.get("additional_checks", []),
        }

    def _parse_validation(self, raw_text: str) -> dict[str, Any]:
        parsed = self._extract_json(raw_text)
        return {
            "approved": bool(parsed.get("approved", False)),
            "confidence": float(parsed.get("confidence", 0.0)),
            "issues": parsed.get("issues", []),
            "agent_scores": parsed.get("agent_scores", {}),
        }

    def _extract_json(self, raw_text: str) -> dict[str, Any]:
        match = _JSON_BLOCK.search(raw_text)
        if not match:
            logger.warning("Resposta do modelo não contém um bloco JSON: %r", raw_text[:200])
            return {}
        try:
            candidate = json.loads(match.group(0))
        except json.JSONDecodeError:
            logger.warning("Resposta do modelo não é JSON válido: %r", raw_text[:200])
            return {}
        return candidate if isinstance(candidate, dict) else {}
