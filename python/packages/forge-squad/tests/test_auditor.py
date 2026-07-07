import asyncio
import json

from forge_squad.agents.auditor import (
    _AUDIT_SYSTEM_PROMPT,
    _VALIDATE_RESULTS_SYSTEM_PROMPT,
    AuditorAgent,
)
from forge_squad.gateway import LlmResponse, ScriptedGatewayClient


def test_execute_deriva_veredito_real_do_gateway():
    payload = {
        "passed": True,
        "confidence": 0.91,
        "notes": "Sem achados críticos; complexidade dentro do esperado.",
        "additional_checks": ["Revisar tratamento de timeout externo"],
    }
    agent = AuditorAgent()
    agent.attach_gateway(ScriptedGatewayClient([LlmResponse(text=json.dumps(payload))]))

    result = asyncio.run(agent.execute({"description": "revisar endpoint de pagamento"}))

    assert result["success"] is True
    assert result["assessment"]["passed"] is True
    assert result["assessment"]["confidence"] == 0.91
    assert result["assessment"]["notes"] == "Sem achados críticos; complexidade dentro do esperado."
    assert result["assessment"]["additional_checks"] == ["Revisar tratamento de timeout externo"]


def test_ausencia_de_achados_criticos_nao_forca_aprovacao():
    # O modelo pode reprovar mesmo sem nenhum achado determinístico crítico
    # — prova que o veredito não é derivado de "issues vazio => aprovado",
    # que seria o carimbo automático disfarçado.
    payload = {
        "passed": False,
        "confidence": 0.4,
        "notes": "Sem achados de padrão, mas o contexto da tarefa (dados financeiros sem auditoria de acesso) exige revisão humana.",
        "additional_checks": ["Revisão manual de controle de acesso"],
    }
    agent = AuditorAgent()
    agent.attach_gateway(ScriptedGatewayClient([LlmResponse(text=json.dumps(payload))]))

    result = asyncio.run(agent.execute({"description": "exportar dados financeiros sem métricas ou código"}))

    assert result["assessment"]["passed"] is False
    assert result["assessment"]["issues"] == []  # nenhum achado de padrão
    assert result["assessment"]["confidence"] == 0.4


def test_check_security_encontra_padrao_perigoso_deterministicamente():
    agent = AuditorAgent()
    issues = agent.check_security("resultado = eval(entrada_do_usuario)")
    assert len(issues) == 1
    assert issues[0]["pattern"] == "eval("


def test_check_quality_sinaliza_complexidade_e_cobertura_fora_do_limiar():
    agent = AuditorAgent()
    warnings = agent.check_quality({"complexity": 15, "coverage": 40})
    metrics = {w["metric"] for w in warnings}
    assert metrics == {"complexity", "coverage"}


def test_achados_deterministicos_sao_repassados_como_evidencia_ao_gateway():
    payload = {"passed": False, "confidence": 0.3, "notes": "eval() é bloqueante", "additional_checks": []}
    gateway = ScriptedGatewayClient([LlmResponse(text=json.dumps(payload))])
    agent = AuditorAgent()
    agent.attach_gateway(gateway)

    result = asyncio.run(agent.execute({"description": "revisar script", "code": "eval(x)"}))

    assert result["assessment"]["issues"][0]["pattern"] == "eval("
    # A evidência determinística de fato chegou na mensagem enviada ao gateway.
    sent_content = gateway.requests[0].messages[1]["content"]
    assert "eval(" in sent_content


def test_validate_results_deriva_veredito_agregado_do_gateway():
    payload = {
        "approved": False,
        "confidence": 0.5,
        "issues": ["Baixa confiança do developer"],
        "agent_scores": {"architect": 0.9, "developer": 0.4},
    }
    agent = AuditorAgent()
    agent.attach_gateway(ScriptedGatewayClient([LlmResponse(text=json.dumps(payload))]))

    validation = asyncio.run(
        agent.validate_results([{"agent": "architect", "success": True}, {"agent": "developer", "success": False}])
    )

    assert validation["approved"] is False
    assert validation["agent_scores"] == {"architect": 0.9, "developer": 0.4}


def test_validate_results_repassa_evidencia_real_ao_gateway():
    # A evidência determinística do /verify (Fase 5 Onda 3) precisa
    # efetivamente chegar na mensagem enviada ao gateway — mesmo padrão do
    # "achados determinísticos são repassados como evidência" (execute()).
    payload = {"approved": True, "confidence": 0.8, "issues": [], "agent_scores": {}}
    gateway = ScriptedGatewayClient([LlmResponse(text=json.dumps(payload))])
    agent = AuditorAgent()
    agent.attach_gateway(gateway)

    evidence = {
        "run_id": "run-1",
        "git_sha": "deadbeef",
        "steps": [{"name": "test", "tool": "cargo test", "exit_code": 1, "duration_ms": 10, "findings": []}],
        "verdict": "fail",
        "produced_at": "2026-01-01T00:00:00Z",
    }
    asyncio.run(agent.validate_results([{"agent": "developer", "success": True}], evidence=evidence))

    sent_content = gateway.requests[0].messages[1]["content"]
    assert "verification_evidence" in sent_content
    assert "deadbeef" in sent_content
    assert '"verdict": "fail"' in sent_content or '"verdict":"fail"' in sent_content


def test_validate_pass_nao_forca_aprovacao_sozinha():
    # Evidência com verdict "pass" não deve forçar approved=True — o
    # veredito ainda vem do modelo, senão a evidência boa vira carimbo
    # automático (inverteria a régua "Nada Fake" para o sentido oposto).
    payload = {"approved": False, "confidence": 0.3, "issues": ["regressão funcional não coberta pelo /verify"], "agent_scores": {}}
    gateway = ScriptedGatewayClient([LlmResponse(text=json.dumps(payload))])
    agent = AuditorAgent()
    agent.attach_gateway(gateway)

    evidence = {
        "run_id": "run-2",
        "git_sha": "cafebabe",
        "steps": [{"name": "test", "tool": "cargo test", "exit_code": 0, "duration_ms": 10, "findings": []}],
        "verdict": "pass",
        "produced_at": "2026-01-01T00:00:00Z",
    }
    validation = asyncio.run(agent.validate_results([{"agent": "developer", "success": True}], evidence=evidence))

    assert validation["approved"] is False


def test_execute_sem_gateway_levanta_erro_claro():
    agent = AuditorAgent()
    try:
        asyncio.run(agent.execute({"description": "tarefa qualquer"}))
        assert False, "deveria ter levantado RuntimeError"
    except RuntimeError as exc:
        assert "attach_gateway" in str(exc)


def test_prompts_de_auditoria_proibem_alegar_persistencia_de_arquivo():
    # Nenhum caminho do auditor tem evidência de filesystem hoje (RunTool
    # ainda não existe, ver core_server.rs) — os dois prompts têm que
    # proibir a alegação explicitamente, não só "não mencionar" por acaso.
    assert "NUNCA afirme" in _AUDIT_SYSTEM_PROMPT
    assert "NUNCA afirme" in _VALIDATE_RESULTS_SYSTEM_PROMPT


def test_audit_repassa_resultados_anteriores_reais_como_evidencia_ao_gateway():
    # Quando audit() é a ação "validate" de um passo do plano (não a
    # proposta inicial), orchestrator.py anexa os resultados reais dos
    # passos anteriores em `prior_results` — antes desta correção, o
    # auditor per-step não recebia nada além de description/action/step.
    payload = {"passed": True, "confidence": 0.8, "notes": "ok", "additional_checks": []}
    gateway = ScriptedGatewayClient([LlmResponse(text=json.dumps(payload))])
    agent = AuditorAgent()
    agent.attach_gateway(gateway)

    prior = [{"agent": "developer", "final_output": "<html>calculadora</html>", "success": True}]
    asyncio.run(agent.audit({"description": "validar arquivo gerado", "prior_results": prior}))

    sent_content = gateway.requests[0].messages[1]["content"]
    assert "prior_agent_results" in sent_content
    assert "calculadora" in sent_content


def test_audit_sem_prior_results_nao_inclui_a_chave_no_payload():
    # A proposta inicial (`_get_squad_proposals`) chama audit() sem
    # `prior_results` — o payload não deve ganhar uma chave vazia/fabricada.
    payload = {"passed": True, "confidence": 0.8, "notes": "ok", "additional_checks": []}
    gateway = ScriptedGatewayClient([LlmResponse(text=json.dumps(payload))])
    agent = AuditorAgent()
    agent.attach_gateway(gateway)

    asyncio.run(agent.audit({"description": "revisar plano"}))

    sent_content = gateway.requests[0].messages[1]["content"]
    assert "prior_agent_results" not in sent_content


def test_resposta_sem_json_cai_no_fallback_honesto_nao_aprovado():
    agent = AuditorAgent()
    agent.attach_gateway(ScriptedGatewayClient([LlmResponse(text="não consigo avaliar isso.")]))

    result = asyncio.run(agent.execute({"description": "tarefa"}))

    # Fallback honesto: não aprovado, confiança zero — nunca aprova por
    # engano quando o modelo não devolve um julgamento parseável.
    assert result["assessment"]["passed"] is False
    assert result["assessment"]["confidence"] == 0.0
