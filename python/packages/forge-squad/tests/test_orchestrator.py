import asyncio
import json

import pytest

from forge_squad.gateway import LlmRequest, LlmResponse
from forge_squad.memory import AgentMemorySystem
from forge_squad.orchestrator import UnifiedOrchestrator
from forge_squad.permission import PermissionDecision, ScriptedPermissionClient


class RoutingGatewayClient:
    """Gateway falso que roteia por `requester` — cada agente/planner tem
    sua própria resposta, tornando o fluxo multi-agente do orquestrador
    determinístico sem depender da ordem exata das chamadas."""

    def __init__(self, by_requester: dict[str, LlmResponse]) -> None:
        self.by_requester = by_requester
        self.calls: list[LlmRequest] = []

    async def generate(self, request: LlmRequest) -> LlmResponse:
        self.calls.append(request)
        resp = self.by_requester.get(request.requester)
        if resp is None:
            raise AssertionError(f"sem resposta roteirizada para requester={request.requester}")
        return resp


def _gateway(arch_conf: float, dev_conf: float, aud_conf: float, approved: bool) -> RoutingGatewayClient:
    return RoutingGatewayClient(
        {
            # plano de um passo "deploy" → não paralelizável → ops (conf alta, sem replan).
            "planner": LlmResponse(
                text=json.dumps(
                    {
                        "steps": [
                            {"step": 1, "action": "deploy", "description": "publicar serviço", "estimated_time": 10, "dependencies": [], "can_fail": True}
                        ],
                        "estimated_duration": 10,
                        "confidence": 0.8,
                    }
                )
            ),
            "architect": LlmResponse(
                text=json.dumps(
                    {"problem_analysis": "x", "recommendation": "microservices", "architecture": "microservices", "components": ["api"], "confidence": arch_conf}
                )
            ),
            "developer": LlmResponse(
                text=json.dumps({"final_output": "codigo", "status": "completed", "confidence": dev_conf})
            ),
            "auditor": LlmResponse(
                text=json.dumps(
                    {"passed": approved, "approved": approved, "confidence": aud_conf, "notes": "ok", "issues": [], "agent_scores": {}, "additional_checks": []}
                )
            ),
            "designer": LlmResponse(text=json.dumps({"pattern": "material", "components": ["ui"], "confidence": 0.8})),
            "ops": LlmResponse(
                text=json.dumps({"strategy": "blue-green", "stages": ["build"], "confidence": 0.9})
            ),
        }
    )


def test_instancia_exatamente_os_5_agentes_reais(tmp_path):
    orch = UnifiedOrchestrator(_gateway(0.9, 0.2, 0.2, True), memory=AgentMemorySystem(storage_dir=tmp_path))
    assert set(orch.agents) == {"architect", "developer", "auditor", "designer", "ops"}


def test_consenso_forte_dispensa_hitl_e_completa(tmp_path):
    # arch 0.9/dev 0.2/aud 0.2 → strength ~0.79 ≥ 0.7 → requires_human False.
    orch = UnifiedOrchestrator(_gateway(0.9, 0.2, 0.2, approved=True), memory=AgentMemorySystem(storage_dir=tmp_path))
    result = asyncio.run(orch.execute_complex_task({"description": "publicar serviço de pagamentos"}))

    assert result["success"] is True  # auditor.validate_results approved
    assert result["consensus"]["requires_human"] is False
    assert result["consensus"]["decision_maker"] == "architect"


def test_record_action_registra_o_resultado_real_apos_execucao(tmp_path):
    orch = UnifiedOrchestrator(_gateway(0.9, 0.2, 0.2, approved=True), memory=AgentMemorySystem(storage_dir=tmp_path))
    asyncio.run(orch.execute_complex_task({"description": "tarefa"}))

    # ADR 0006: o portão não registra; o orquestrador registra o resultado real.
    assert len(orch.autonomy.action_history) == 1
    assert orch.autonomy.action_history[-1]["success"] is True
    assert orch.autonomy.agent_trust_scores["orchestrator"] == pytest.approx(0.52)  # 0.5 + 0.02


def test_consenso_fraco_dispara_hitl_e_aprovacao_deixa_seguir(tmp_path):
    # arch 0.6/dev 0.6/aud 0.9 → strength 0.4 < 0.7 → requires_human True.
    orch = UnifiedOrchestrator(
        _gateway(0.6, 0.6, 0.9, approved=True),
        permission_client=ScriptedPermissionClient([PermissionDecision(approved=True)]),
        memory=AgentMemorySystem(storage_dir=tmp_path),
    )
    result = asyncio.run(orch.execute_complex_task({"description": "tarefa crítica"}))

    assert result["consensus"]["requires_human"] is True
    assert result["success"] is True  # humano aprovou e a execução seguiu


def test_consenso_fraco_com_hitl_negado_aborta(tmp_path):
    orch = UnifiedOrchestrator(
        _gateway(0.6, 0.6, 0.9, approved=True),
        permission_client=ScriptedPermissionClient([PermissionDecision(approved=False, operator_note="risco alto")]),
        memory=AgentMemorySystem(storage_dir=tmp_path),
    )
    result = asyncio.run(orch.execute_complex_task({"description": "tarefa crítica"}))

    assert result["success"] is False
    assert result["reason"] == "Plan rejected"
    # A negação humana foi registrada como falha para o orquestrador.
    assert orch.autonomy.action_history[-1]["success"] is False


def test_event_sink_emite_eventos_ao_vivo_na_ordem(tmp_path):
    orch = UnifiedOrchestrator(_gateway(0.9, 0.2, 0.2, approved=True), memory=AgentMemorySystem(storage_dir=tmp_path))
    events: list[dict] = []

    async def sink(event: dict) -> None:
        events.append(event)

    asyncio.run(orch.execute_complex_task({"description": "tarefa"}, event_sink=sink))

    kinds = [e["kind"] for e in events]
    # 3 propostas antes do consenso; consenso antes dos handoffs/steps.
    assert kinds[:3] == ["proposal", "proposal", "proposal"]
    assert "consensus" in kinds
    assert kinds.index("consensus") < kinds.index("handoff")
    assert kinds.index("handoff") < kinds.index("step")
    consensus_ev = next(e for e in events if e["kind"] == "consensus")
    assert consensus_ev["requires_human"] is False


def test_verification_evidence_missing_e_fail_closed_sem_chamar_o_gateway(tmp_path):
    # Fase 5 Onda 3 / ADR 0008: quando o task veio do SquadTask com a
    # evidência ausente/inválida, verification_evidence_missing=True deve
    # reprovar ANTES de chamar validate_results — não é "sem evidência = ok".
    gateway = _gateway(0.9, 0.2, 0.2, approved=True)  # aprovaria se chamado
    orch = UnifiedOrchestrator(gateway, memory=AgentMemorySystem(storage_dir=tmp_path))

    result = asyncio.run(
        orch.execute_complex_task({"description": "tarefa", "verification_evidence_missing": True})
    )

    assert result["success"] is False
    assert "fail-closed" in result["validation"]["issues"][0]
    # o gateway nunca foi chamado com requester="auditor" para validate_results —
    # só as 3 chamadas de proposta (architect/developer/auditor) do plano.
    auditor_calls = [c for c in gateway.calls if c.requester == "auditor"]
    assert len(auditor_calls) == 1  # só a proposta em _get_squad_proposals, não validate_results


def test_verification_evidence_presente_flui_para_validate_results(tmp_path):
    # Sem a flag de ausência, a evidência (se houver) deve chegar ao
    # auditor via validate_results — comportamento normal preservado.
    gateway = _gateway(0.9, 0.2, 0.2, approved=True)
    orch = UnifiedOrchestrator(gateway, memory=AgentMemorySystem(storage_dir=tmp_path))
    evidence = {"run_id": "r1", "git_sha": "sha", "steps": [], "verdict": "pass", "produced_at": "2026-01-01T00:00:00Z"}

    result = asyncio.run(
        orch.execute_complex_task({"description": "tarefa", "verification_evidence": evidence})
    )

    assert result["success"] is True
    auditor_calls = [c for c in gateway.calls if c.requester == "auditor"]
    assert len(auditor_calls) == 2  # proposta + validate_results (chamou o gateway de verdade)
    # a evidência de fato foi enviada na segunda chamada (validate_results).
    assert "verification_evidence" in auditor_calls[1].messages[1]["content"]


def test_step_task_de_validate_recebe_resultados_reais_dos_passos_anteriores(tmp_path):
    # Antes desta correção, o step_task de uma ação "validate" só carregava
    # description/action/step do plano — o auditor per-step era cego até ao
    # texto que o developer disse ter produzido, não só ao filesystem
    # (RunTool ainda não existe). Plano de 2 passos sequenciais (o 1º tem
    # `dependencies` não-vazio só pra escapar do caminho paralelo de
    # `_can_parallelize` e provar a passagem via step_task simples).
    gateway = RoutingGatewayClient(
        {
            "planner": LlmResponse(
                text=json.dumps(
                    {
                        "steps": [
                            {
                                "step": 1,
                                "action": "implement",
                                "description": "criar arquivo",
                                "estimated_time": 5,
                                "dependencies": ["seed"],
                                "can_fail": True,
                            },
                            {
                                "step": 2,
                                "action": "validate",
                                "description": "validar arquivo",
                                "estimated_time": 5,
                                "dependencies": [1],
                                "can_fail": True,
                            },
                        ],
                        "estimated_duration": 10,
                        "confidence": 0.8,
                    }
                )
            ),
            "architect": LlmResponse(
                text=json.dumps({"problem_analysis": "x", "recommendation": "y", "architecture": "z", "components": [], "confidence": 0.9})
            ),
            "developer": LlmResponse(
                text=json.dumps({"final_output": "conteudo-real-do-developer", "status": "completed", "confidence": 0.9})
            ),
            "auditor": LlmResponse(
                text=json.dumps({"passed": True, "approved": True, "confidence": 0.9, "notes": "ok", "issues": [], "agent_scores": {}, "additional_checks": []})
            ),
            "designer": LlmResponse(text=json.dumps({"pattern": "x", "components": [], "confidence": 0.8})),
            "ops": LlmResponse(text=json.dumps({"strategy": "x", "stages": [], "confidence": 0.8})),
        }
    )
    # Conteúdos de proposta heterogêneos (arch/dev/aud não concordam em
    # forma) podem disparar HITL por consenso fraco — irrelevante pro que
    # este teste prova, então aprova automaticamente pra chegar no step.
    orch = UnifiedOrchestrator(
        gateway,
        permission_client=ScriptedPermissionClient([PermissionDecision(approved=True)]),
        memory=AgentMemorySystem(storage_dir=tmp_path),
    )
    asyncio.run(orch.execute_complex_task({"description": "criar e validar arquivo"}))

    # Das chamadas do auditor (proposta + step "validate" + validate_results
    # final), só a do step carrega prior_agent_results — e o final_output
    # real do developer está lá dentro, não só metadados do plano.
    audit_step_calls = [
        c for c in gateway.calls if c.requester == "auditor" and "prior_agent_results" in c.messages[1]["content"]
    ]
    assert len(audit_step_calls) == 1
    assert "conteudo-real-do-developer" in audit_step_calls[0].messages[1]["content"]


def test_propostas_sao_envolvidas_em_proposal_e_consenso_computa(tmp_path):
    # Se o wrapping Proposal(...) falhasse, reach_consensus levantaria; aqui
    # provamos que o consenso computou um decision_maker real.
    orch = UnifiedOrchestrator(_gateway(0.9, 0.2, 0.2, approved=True), memory=AgentMemorySystem(storage_dir=tmp_path))
    result = asyncio.run(orch.execute_complex_task({"description": "tarefa"}))
    assert result["consensus"]["decision_maker"] in {"architect", "developer", "auditor"}
    assert 0.0 <= result["consensus"]["consensus_strength"] <= 1.0
