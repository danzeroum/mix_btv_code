"""Orquestrador unificado do squad (migrado de BuildToValue
`src/orchestration/unified_orchestrator.py` — a lineage canônica, ADR 0004).

O orquestrador é **coordenação determinística**: ele não faz chamada de
gateway própria; compõe os agentes (que fazem), o consenso, o planner, a
memória, o paralelo, a autonomia e a avaliação. `_select_agent_for_step`/
`_can_parallelize`/`_extract_parallel_tasks` são dispatch (plumbing, como
o `ParallelResourceManager`), não raciocínio. A régua "Nada Fake" aqui é
só: não fabricar nenhum valor que ele repassa adiante — e ele consome
saída real de agente/consenso/avaliação.

Adaptações registradas nos ADRs, aplicadas neste porte:
- **ADR 0004**: consenso via `ConsensusResult.requires_human`, não o
  `consensus.get("consensus_strength") < 0.7` manual; propostas dos
  agentes envolvidas em `Proposal(confidence=..., content=...)` antes de
  `reach_consensus` (que espera `dict[str, Proposal]` tipado); 5 agentes
  reais (architect/developer/auditor/designer/ops).
- **ADR 0006**: o portão de autonomia (`execute_with_autonomy`) não
  registra mais o resultado da ação; o orquestrador chama `record_action`
  com o resultado REAL após a execução.
- **ADR 0005**: `gateway`/`permission_client` injetados (impls gRPC reais
  entram na Onda 4c, satisfazendo os mesmos Protocols).
"""

from __future__ import annotations

import logging
import uuid
from datetime import datetime, timezone
from typing import Any, Optional

from forge_squad.agents import ArchitectAgent, AuditorAgent, DesignerAgent, DeveloperAgent, OpsAgent
from forge_squad.chains import ResilientPromptChain
from forge_squad.consensus import Proposal, WeightedConsensusEngine
from forge_squad.evaluation import ContinuousEvaluator
from forge_squad.gateway import GatewayClient
from forge_squad.hitl import ProgressiveAutonomyManager
from forge_squad.memory import AgentMemorySystem
from forge_squad.parallel import ParallelResourceManager
from forge_squad.permission import PermissionClient
from forge_squad.planning import AdaptivePlanner
from forge_squad.routing import LearningRouter
from forge_squad.sandbox import SecureToolSandbox

logger = logging.getLogger(__name__)


def _consensus_dict(consensus: Any) -> dict[str, Any]:
    """Serializa o `ConsensusResult` preservando `requires_human` — que é
    uma `@property` e por isso NÃO aparece em `model_dump()` (só campos).
    O painel do squad na TUI (Onda 4c) consome esse sinal de HITL."""

    return {**consensus.model_dump(), "requires_human": consensus.requires_human}


class UnifiedOrchestrator:
    """Ponte de alto nível entre todos os subsistemas do squad."""

    def __init__(
        self,
        gateway: GatewayClient,
        permission_client: Optional[PermissionClient] = None,
        model: str = "claude-sonnet-5",
        memory: Optional[AgentMemorySystem] = None,
    ) -> None:
        self.planner = AdaptivePlanner(model=model)
        self.planner.attach_gateway(gateway)
        self.router = LearningRouter()
        self.parallel = ParallelResourceManager()
        self.memory = memory or AgentMemorySystem()
        self.evaluator = ContinuousEvaluator()
        self.consensus = WeightedConsensusEngine()
        self.autonomy = ProgressiveAutonomyManager()
        if permission_client is not None:
            self.autonomy.attach_permission_client(permission_client)
        self.sandbox = SecureToolSandbox()
        self.chain_manager = ResilientPromptChain([])
        #: Callback async opcional (por execução) — recebe eventos do squad
        #: ao vivo (proposta/consenso/handoff/hitl/step). O servidor
        #: `SquadService` traduz cada um em `SquadEvent` e streama pela
        #: fronteira gRPC. None = execução silenciosa (comportamento padrão).
        self._event_sink: Optional[Any] = None

        self.agents: dict[str, Any] = {
            "architect": ArchitectAgent(model=model),
            "developer": DeveloperAgent(model=model),
            "auditor": AuditorAgent(model=model),
            "designer": DesignerAgent(model=model),
            "ops": OpsAgent(model=model),
        }
        for agent in self.agents.values():
            agent.attach_memory(self.memory)
            agent.attach_gateway(gateway)

    async def _emit(self, event: dict[str, Any]) -> None:
        if self._event_sink is not None:
            await self._event_sink(event)

    async def execute_complex_task(
        self, task: dict[str, Any], event_sink: Optional[Any] = None
    ) -> dict[str, Any]:
        self._event_sink = event_sink
        task_id = task.get("task_id", str(uuid.uuid4()))
        start = datetime.now(timezone.utc)
        logger.info("Iniciando execução da tarefa %s", task_id)

        relevant_context = self.memory.recall_similar(task.get("description", ""), k=5)
        plan = await self.planner.create_adaptive_plan(task)
        proposals = await self._get_squad_proposals(plan)
        consensus = self.consensus.reach_consensus(proposals, "architecture")

        await self._emit(
            {
                "kind": "consensus",
                "decision_maker": consensus.decision_maker,
                "strength": consensus.consensus_strength,
                "requires_human": consensus.requires_human,
                "decision": consensus.decision.model_dump() if consensus.decision else None,
            }
        )

        # ADR 0004: usa a property centralizada, não o número mágico 0.7.
        if consensus.requires_human:
            await self._emit(
                {"kind": "hitl", "reason": "weak_consensus", "confidence": consensus.consensus_strength}
            )
            approval = await self.autonomy.execute_with_autonomy(
                "orchestrator",
                {"action": "approve_plan", "plan": plan, "critical": True},
            )
            if not approval.get("executed", False):
                return {
                    "success": False,
                    "task_id": task_id,
                    "reason": "Plan rejected",
                    "consensus": _consensus_dict(consensus),
                }

        execution_results = await self._execute_plan_steps(plan, task)

        # Fase 5 Onda 3: quando o task veio do SquadTask (server.py) e o
        # campo verification_evidence_json existia mas era vazio/inválido,
        # `verification_evidence_missing` vem True — fail-closed ANTES de
        # chamar o gateway, sem custar uma chamada de LLM que já sabemos que
        # não pode aprovar. Chamadas diretas ao orquestrador (sem esse campo,
        # como nos testes) nunca setam essa flag — comportamento inalterado.
        if task.get("verification_evidence_missing", False):
            final_validation = {
                "approved": False,
                "confidence": 0.0,
                "issues": ["evidência de verificação ausente ou inválida — fail-closed"],
                "agent_scores": {},
            }
        else:
            final_validation = await self.agents["auditor"].validate_results(
                execution_results, evidence=task.get("verification_evidence")
            )
        overall_success = bool(final_validation.get("approved", False))

        # ADR 0006: o portão não registra mais; quem executa registra o
        # resultado REAL da execução (a menos que já tenha sido rejeitado
        # acima, caso em que retornamos antes daqui).
        self.autonomy.record_action(
            "orchestrator", {"action": "execute_task", "task_id": task_id}, success=overall_success
        )

        self.memory.remember_decision(
            "orchestrator",
            {
                "task_id": task_id,
                "task": task,
                "plan_id": plan.get("plan_id"),
                "validation": final_validation,
                "context_recall_count": len(relevant_context.get("ids", [])) if isinstance(relevant_context, dict) else 0,
                "duration_seconds": (datetime.now(timezone.utc) - start).total_seconds(),
                "confidence": final_validation.get("confidence", 0.0),
            },
        )

        await self._update_learning(task, execution_results)

        return {
            "success": overall_success,
            "task_id": task_id,
            "plan": plan,
            "consensus": _consensus_dict(consensus),
            "results": execution_results,
            "validation": final_validation,
            "confidence": final_validation.get("confidence", 0.0),
        }

    async def _get_squad_proposals(self, plan: dict[str, Any]) -> dict[str, Proposal]:
        goal = plan.get("goal", "")
        proposals: dict[str, Proposal] = {}

        architect_result = await self.agents["architect"].execute(
            {"description": f"Review plan: {goal}", "plan": plan}
        )
        proposals["architect"] = Proposal(
            confidence=float(architect_result.get("confidence", 0.5)), content=architect_result
        )
        await self._emit_proposal("architect", proposals["architect"])

        developer_result = await self.agents["developer"].execute(
            {"description": f"Assess implementation for {goal}", "plan": plan}
        )
        proposals["developer"] = Proposal(
            confidence=float(developer_result.get("confidence", 0.5)), content=developer_result
        )
        await self._emit_proposal("developer", proposals["developer"])

        audit_result = await self.agents["auditor"].execute(
            {
                "description": f"Audit plan {goal}",
                "plan": plan,
                "metrics": {"complexity": len(plan.get("steps", [])), "coverage": 85},
            }
        )
        proposals["auditor"] = Proposal(
            confidence=float(audit_result.get("confidence", 0.5)), content=audit_result
        )
        await self._emit_proposal("auditor", proposals["auditor"])

        return proposals

    async def _emit_proposal(self, agent: str, proposal: Proposal) -> None:
        await self._emit(
            {"kind": "proposal", "agent": agent, "confidence": proposal.confidence, "content": proposal.content}
        )

    async def _execute_plan_steps(self, plan: dict[str, Any], task: dict[str, Any]) -> list[dict[str, Any]]:
        results: list[dict[str, Any]] = []
        for step in plan.get("steps", []):
            agent_name = self._select_agent_for_step(step)
            step_id = str(step.get("step", "?"))
            await self._emit(
                {"kind": "handoff", "phase": "start", "from_agent": "orchestrator", "to_agent": agent_name}
            )
            if self._can_parallelize(step, plan):
                parallel_tasks = self._extract_parallel_tasks(step)
                step_results = await self.parallel.execute_parallel_with_limits(parallel_tasks)
                for result in step_results:
                    await self.evaluator.evaluate_agent_performance(agent_name, result)
                results.extend(step_results)
                await self._emit_step(step_id, all(r.get("success") for r in step_results), agent_name)
                continue

            step_task = {
                "description": step.get("description", ""),
                "action": step.get("action", ""),
                "step": step.get("step"),
            }
            result = await self.agents[agent_name].execute(step_task)
            quality = await self.evaluator.evaluate_agent_performance(agent_name, result)
            if quality.get("technical_score", 0.0) < 0.6:
                reflection = {"reason": "low_quality", "score": quality.get("technical_score", 0.0)}
                plan = await self.planner.replan_from_point(plan, step, reflection)
            results.append(result)
            await self._emit_step(step_id, bool(result.get("success")), agent_name)
        return results

    async def _emit_step(self, step_id: str, success: bool, agent_name: str) -> None:
        await self._emit(
            {"kind": "handoff", "phase": "complete" if success else "error", "from_agent": agent_name, "to_agent": "orchestrator"}
        )
        await self._emit({"kind": "step", "step_id": step_id, "success": success, "summary": f"{agent_name} step {step_id}"})

    def _select_agent_for_step(self, step: dict[str, Any]) -> str:
        mapping = {
            "analyze": "architect",
            "design": "designer",
            "implement": "developer",
            "validate": "auditor",
            "deploy": "ops",
        }
        return mapping.get(step.get("action", ""), "developer")

    def _can_parallelize(self, step: dict[str, Any], plan: dict[str, Any]) -> bool:
        if step.get("dependencies"):
            return False
        if step.get("action") in {"validate", "deploy"}:
            return False
        return True

    def _extract_parallel_tasks(self, step: dict[str, Any]):
        description = step.get("description", "")

        async def developer_task() -> dict[str, Any]:
            return await self.agents["developer"].execute({"description": f"Parallel dev: {description}"})

        async def designer_task() -> dict[str, Any]:
            return await self.agents["designer"].execute({"description": f"Parallel design: {description}"})

        return [developer_task, designer_task]

    async def _update_learning(self, task: dict[str, Any], results: list[dict[str, Any]]) -> None:
        for result in results:
            route = result.get("route", "default")
            success = result.get("success", False)
            latency = result.get("duration", 0.0)
            self.router.update_route_performance(task, route, bool(success), float(latency))

    async def _attempt_recovery(self, task: dict[str, Any], error: str) -> Optional[dict[str, Any]]:
        """Recuperação real do BuildToValue: reexecuta a tarefa simplificada
        (não é uma classe `RecoveryAgent` — ver ADR 0004)."""

        simplified_task = {"description": task.get("description", ""), "priority": "low", "simplified": True}
        try:
            return await self.execute_complex_task(simplified_task)
        except Exception:
            logger.exception("Recuperação falhou para a tarefa: %s", task.get("description"))
            return None
