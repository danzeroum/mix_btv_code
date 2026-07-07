"""Agente desenvolvedor (migrado de BuildToValue
`src/agents/developer_agent.py`).

Na origem, o "loop ReAct" (`think`/`decide_action`/`execute_action`) era
uma máquina de estados 100% roteirizada — cada "pensamento" e
"observação" era uma string canned escolhida por keyword matching, sem
nenhuma chamada real. Por um bom tempo, enquanto `CoreService.RunTool`
era só um stub `Unimplemented`, este agente trocou a máquina de estados
por uma única chamada ao gateway — honesto sobre não poder executar
nada de verdade. Com `RunTool` ativado (Onda 1 — "tool execution
architecture"), `_implement_with_tools` é o loop ReAct real: o modelo
decide entre `tool_call` (executado de verdade via `ToolClient`, sob o
motor de permissões do lado Rust) e `final_answer`, iterando até um dos
dois ou até estourar o teto de passos/tempo. O caminho de chamada única
(`implement_task` com `use_tools=False`) continua existindo para
proposta/avaliação, onde nenhuma ferramenta deveria rodar.

`review_system` é injetado como dependência opcional (ADR 0005, decisão
4) — sem ele, `generate_code` devolve o código gerado sem revisão; o
wiring real acontece quando `forge_review` existir (Fase 5).
"""

from __future__ import annotations

import asyncio
import json
import logging
import re
from typing import Any, Optional, Protocol

from forge_squad.agents.base import BaseAgent
from forge_squad.gateway import LlmRequest
from forge_squad.tool_client import ToolCallRequest, ToolClient

logger = logging.getLogger(__name__)

_SYSTEM_PROMPT = """Você é um desenvolvedor full-stack sênior. Dada uma tarefa de implementação, responda SOMENTE com um objeto JSON (sem markdown, sem texto fora do JSON):
{
  "final_output": "string — o código ou artefato implementado para ESTA tarefa",
  "status": "completed ou incomplete",
  "confidence": 0.0,
  "notes": "string — observações relevantes (testes sugeridos, riscos, limitações) para ESTA tarefa"
}
Todos os campos devem refletir a tarefa específica recebida — nunca um placeholder genérico."""

_REACT_SYSTEM_PROMPT = """Você é um desenvolvedor full-stack sênior com acesso a ferramentas reais (read, grep, edit, bash), executadas sob um motor de permissões do lado do núcleo — edit/bash podem pedir aprovação humana; se uma ação for negada, mude de estratégia, não repita a mesma ação. 'edit' só funciona em um arquivo que já existe; para CRIAR um arquivo novo, use 'bash' (ex.: heredoc/redirecionamento).

A cada passo, responda SOMENTE com um objeto JSON (sem markdown, sem texto fora do JSON), em uma das duas formas:
{"action": "tool_call", "tool": "read|grep|edit|bash", "args": {...}, "reasoning": "string — por que esta ação"}
ou, só depois de ter executado o necessário via tool_call (nunca alegue ter criado ou salvo algo sem ter pedido a execução real):
{"action": "final_answer", "final_output": "string — resumo do que foi feito para ESTA tarefa", "status": "completed ou incomplete", "confidence": 0.0, "notes": "string"}

Depois de criar ou editar um arquivo, rode um comando de verificação (ex.: sha256sum ou cat no arquivo) antes do final_answer — essa saída é a evidência que o auditor vai ver; sem ela, uma alegação de sucesso não tem lastro."""

_MAX_REACT_STEPS = 12
_REACT_TIMEOUT_SECONDS = 600

_JSON_BLOCK = re.compile(r"\{.*\}", re.DOTALL)


class ReviewSystem(Protocol):
    """Contrato mínimo do review system (Fase 5, `forge_review`) — só o
    suficiente pro `DeveloperAgent` chamar quando ele existir."""

    async def review_code(self, code: str, metadata: dict[str, Any]) -> dict[str, Any]: ...


class DeveloperAgent(BaseAgent):
    """Desenvolvedor full-stack que implementa tarefas via gateway LLM real."""

    def __init__(self, model: str = "claude-sonnet-5", review_system: Optional[ReviewSystem] = None) -> None:
        super().__init__("developer")
        self.model = model
        self.review_system = review_system
        self.history: list[dict[str, Any]] = []
        self.tools = ["write_code", "generate_tests", "refactor", "debug", "analyze_requirements"]
        self.tool_client: Optional[ToolClient] = None

    def attach_tool_client(self, tool_client: ToolClient) -> None:
        self.tool_client = tool_client

    async def execute(self, task: dict[str, Any]) -> dict[str, Any]:
        if not self.validate_input(task):
            raise ValueError("Invalid development task payload")

        description = task.get("description", "")
        # Sinal de ativação do loop ReAct: todo passo real do plano carrega
        # "action" (`_select_agent_for_step`/`step_task`,
        # `orchestrator.py`); chamadas de proposta/avaliação nunca setam
        # essa chave — então isto separa "trabalho real do plano" de "só
        # avaliar/propor" sem hardcodar um vocabulário de ações.
        use_tools = bool(task.get("action")) and self.tool_client is not None
        result = await self.implement_task(description, use_tools=use_tools)
        decision = {
            "task": task,
            "result": result,
            "confidence": result.get("confidence", 0.0),
        }
        self.log_decision(decision)
        return {"success": True, "agent": self.agent_type, **result}

    async def create_code(self, task: dict[str, Any]) -> str:
        """Implementa a tarefa via gateway real e devolve o código gerado."""

        description = task.get("description") or task.get("task_description") or ""
        result = await self.implement_task(description)
        return result.get("final_output", "")

    async def generate_code(self, task: dict[str, Any]) -> str:
        """Gera código e, se um `review_system` estiver anexado, roda a
        revisão automática (Fase 5); sem ele, devolve o código sem revisão."""

        code = await self.create_code(task)
        if self.review_system is None:
            return code

        metadata = {
            "task_id": task.get("id") or task.get("task_id"),
            "task_description": task.get("description") or task.get("task_description"),
            "estimated_value": task.get("business_value") or task.get("estimated_value"),
            "priority": task.get("priority", "medium"),
            "filename": task.get("filename", "generated.py"),
        }
        review = await self.review_system.review_code(code=code, metadata=metadata)
        if review.get("approved") and review.get("code"):
            return str(review["code"])
        return await self.auto_fix_issues(code, review.get("reviews", {}))

    async def auto_fix_issues(self, code: str, reviews: dict[str, Any]) -> str:
        """Aplica correções determinísticas para achados comuns de review
        (transformação mecânica sobre um veredito real do review system —
        não é decisão do agente, é bookkeeping sobre a saída dele)."""

        security_review = reviews.get("security", {})
        for vuln in security_review.get("vulnerabilities", []):
            if vuln in code:
                code = code.replace(vuln, f"# Removed insecure usage: {vuln}")

        performance_review = reviews.get("performance", {})
        if performance_review.get("impact") == "Degraded":
            code = "# Optimized placeholder\n" + code

        technical_review = reviews.get("technical", {})
        if technical_review.get("coverage", 0) < 30:
            code += "\n\n# TODO: add tests to increase coverage"

        return code

    async def implement_task(self, task: str, use_tools: bool = False) -> dict[str, Any]:
        """Chamada real ao gateway que implementa a tarefa. `use_tools=True`
        (e `self.tool_client` anexado) usa o loop ReAct real
        (`_implement_with_tools`); caso contrário, uma única chamada —
        usado por proposta/avaliação, onde nenhuma ferramenta deve rodar."""

        if self.gateway is None:
            raise RuntimeError(
                "DeveloperAgent sem gateway anexado — chame attach_gateway() antes de execute()"
            )

        if use_tools and self.tool_client is not None:
            result = await self._implement_with_tools(task)
            self.history.append({"task": task, "result": result})
            return result

        request = LlmRequest(
            model=self.model,
            messages=[
                {"role": "system", "content": _SYSTEM_PROMPT},
                {"role": "user", "content": task.strip() or "Tarefa não especificada"},
            ],
            requester=self.agent_type,
        )
        raw = await self.gateway.generate(request)
        result = self._parse_result(raw.text)
        self.history.append({"task": task, "result": result})
        return result

    async def _implement_with_tools(self, task: str) -> dict[str, Any]:
        """Loop ReAct real: o modelo alterna entre `tool_call` (executado
        de verdade via `self.tool_client`, sob o motor de permissões do
        lado Rust) e `final_answer`, até um dos dois ou até estourar
        `_MAX_REACT_STEPS`/`_REACT_TIMEOUT_SECONDS` — nesse caso, devolve
        honestamente `status: "incomplete"`, nunca fabrica sucesso."""

        async def _run_loop() -> dict[str, Any]:
            messages: list[dict[str, str]] = [
                {"role": "system", "content": _REACT_SYSTEM_PROMPT},
                {"role": "user", "content": task.strip() or "Tarefa não especificada"},
            ]
            tool_calls: list[dict[str, Any]] = []
            for _ in range(_MAX_REACT_STEPS):
                request = LlmRequest(model=self.model, messages=messages, requester=self.agent_type)
                raw = await self.gateway.generate(request)
                action = self._parse_react_action(raw.text)

                if action["action"] == "final_answer":
                    return {
                        "final_output": action.get("final_output", ""),
                        "status": action.get("status", "incomplete"),
                        "confidence": float(action.get("confidence", 0.0)),
                        "notes": action.get("notes", ""),
                        "tool_calls": tool_calls,
                    }

                if action["action"] == "tool_call":
                    messages.append({"role": "assistant", "content": raw.text})
                    result = await self.tool_client.run_tool(
                        ToolCallRequest(tool=action["tool"], args_json=json.dumps(action.get("args", {})))
                    )
                    tool_calls.append(
                        {
                            "tool": action["tool"],
                            "args": action.get("args", {}),
                            "exit_code": result.exit_code,
                            "content": result.content,
                        }
                    )
                    observation = {
                        "tool": action["tool"],
                        "content": result.content,
                        "truncated": result.truncated,
                        "exit_code": result.exit_code,
                    }
                    messages.append({"role": "user", "content": json.dumps(observation, ensure_ascii=False)})
                    continue

                # parse_error ou discriminador desconhecido — pede pro
                # modelo tentar de novo em vez de quebrar o loop.
                messages.append({"role": "assistant", "content": raw.text})
                messages.append(
                    {
                        "role": "user",
                        "content": (
                            'erro: responda SOMENTE com o JSON {"action": "tool_call", ...} '
                            'ou {"action": "final_answer", ...}'
                        ),
                    }
                )

            logger.warning("Loop ReAct do developer esgotou %d passos sem final_answer", _MAX_REACT_STEPS)
            return {
                "final_output": "",
                "status": "incomplete",
                "confidence": 0.0,
                "notes": f"loop de ferramentas esgotou {_MAX_REACT_STEPS} passos sem final_answer",
                "tool_calls": tool_calls,
            }

        try:
            return await asyncio.wait_for(_run_loop(), timeout=_REACT_TIMEOUT_SECONDS)
        except asyncio.TimeoutError:
            return {
                "final_output": "",
                "status": "incomplete",
                "confidence": 0.0,
                "notes": f"loop de ferramentas excedeu {_REACT_TIMEOUT_SECONDS}s",
                "tool_calls": [],
            }

    def _parse_react_action(self, raw_text: str) -> dict[str, Any]:
        match = _JSON_BLOCK.search(raw_text)
        if not match:
            logger.warning("Resposta do modelo (ReAct) não contém um bloco JSON: %r", raw_text[:200])
            return {"action": "parse_error"}
        try:
            candidate = json.loads(match.group(0))
        except json.JSONDecodeError:
            logger.warning("Resposta do modelo (ReAct) não é JSON válido: %r", raw_text[:200])
            return {"action": "parse_error"}
        if not isinstance(candidate, dict) or candidate.get("action") not in {"tool_call", "final_answer"}:
            return {"action": "parse_error"}
        return candidate

    def _parse_result(self, raw_text: str) -> dict[str, Any]:
        parsed: dict[str, Any] = {}
        match = _JSON_BLOCK.search(raw_text)
        if match:
            try:
                candidate = json.loads(match.group(0))
                if isinstance(candidate, dict):
                    parsed = candidate
            except json.JSONDecodeError:
                logger.warning("Resposta do modelo não é JSON válido: %r", raw_text[:200])
        else:
            logger.warning("Resposta do modelo não contém um bloco JSON: %r", raw_text[:200])

        return {
            "final_output": parsed.get("final_output", ""),
            "status": parsed.get("status", "incomplete"),
            "confidence": float(parsed.get("confidence", 0.0)),
            "notes": parsed.get("notes", ""),
        }
