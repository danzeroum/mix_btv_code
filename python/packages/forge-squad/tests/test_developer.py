import asyncio
import json

from forge_squad.agents.developer import _MAX_REACT_STEPS, DeveloperAgent
from forge_squad.gateway import LlmResponse, ScriptedGatewayClient
from forge_squad.tool_client import ScriptedToolClient, ToolCallResult


def test_execute_deriva_saida_real_do_gateway():
    payload = {
        "final_output": "def fatura(pedido): return pedido.total * 1.1",
        "status": "completed",
        "confidence": 0.82,
        "notes": "Falta tratar desconto promocional",
    }
    agent = DeveloperAgent()
    agent.attach_gateway(ScriptedGatewayClient([LlmResponse(text=json.dumps(payload))]))

    result = asyncio.run(agent.execute({"description": "calcular fatura com imposto"}))

    assert result["success"] is True
    # Igualdade, não só presença: prova pass-through fiel.
    assert result["final_output"] == "def fatura(pedido): return pedido.total * 1.1"
    assert result["status"] == "completed"
    assert result["confidence"] == 0.82
    assert result["notes"] == "Falta tratar desconto promocional"


def test_dois_pedidos_diferentes_produzem_saidas_diferentes():
    payload_a = {"final_output": "codigo A", "status": "completed", "confidence": 0.9}
    payload_b = {"final_output": "codigo B", "status": "incomplete", "confidence": 0.4}
    agent = DeveloperAgent()
    agent.attach_gateway(
        ScriptedGatewayClient([LlmResponse(text=json.dumps(payload_a)), LlmResponse(text=json.dumps(payload_b))])
    )

    result_a = asyncio.run(agent.execute({"description": "tarefa A"}))
    result_b = asyncio.run(agent.execute({"description": "tarefa B"}))

    assert result_a["final_output"] == "codigo A"
    assert result_b["final_output"] == "codigo B"


def test_execute_sem_gateway_levanta_erro_claro():
    agent = DeveloperAgent()
    try:
        asyncio.run(agent.execute({"description": "tarefa qualquer"}))
        assert False, "deveria ter levantado RuntimeError"
    except RuntimeError as exc:
        assert "attach_gateway" in str(exc)


def test_resposta_sem_json_cai_no_fallback_honesto():
    agent = DeveloperAgent()
    agent.attach_gateway(ScriptedGatewayClient([LlmResponse(text="não consigo processar essa tarefa.")]))

    result = asyncio.run(agent.execute({"description": "tarefa"}))

    assert result["final_output"] == ""
    assert result["status"] == "incomplete"
    assert result["confidence"] == 0.0


def test_generate_code_sem_review_system_devolve_codigo_sem_revisao():
    payload = {"final_output": "print('oi')", "status": "completed", "confidence": 0.7}
    agent = DeveloperAgent()  # review_system=None por padrão
    agent.attach_gateway(ScriptedGatewayClient([LlmResponse(text=json.dumps(payload))]))

    code = asyncio.run(agent.generate_code({"description": "hello world"}))

    assert code == "print('oi')"


def test_generate_code_com_review_system_aprovado_usa_codigo_revisado():
    payload = {"final_output": "print('oi')", "status": "completed", "confidence": 0.7}

    class _ApprovingReview:
        async def review_code(self, code, metadata):
            return {"approved": True, "code": "print('oi revisado')"}

    agent = DeveloperAgent(review_system=_ApprovingReview())
    agent.attach_gateway(ScriptedGatewayClient([LlmResponse(text=json.dumps(payload))]))

    code = asyncio.run(agent.generate_code({"description": "hello world"}))

    assert code == "print('oi revisado')"


def test_execute_com_action_usa_tool_client_e_faz_tool_call_antes_do_final_answer():
    tool_call_turn = json.dumps(
        {"action": "tool_call", "tool": "bash", "args": {"command": "echo oi > out.txt"}}
    )
    final_turn = json.dumps(
        {
            "action": "final_answer",
            "final_output": "arquivo criado",
            "status": "completed",
            "confidence": 0.9,
            "notes": "verificado com cat",
        }
    )
    agent = DeveloperAgent()
    agent.attach_gateway(
        ScriptedGatewayClient([LlmResponse(text=tool_call_turn), LlmResponse(text=final_turn)])
    )
    tool_client = ScriptedToolClient([ToolCallResult(content="oi\n", exit_code=0)])
    agent.attach_tool_client(tool_client)

    result = asyncio.run(agent.execute({"description": "crie out.txt", "action": "implement"}))

    assert len(tool_client.requests) == 1
    assert tool_client.requests[0].tool == "bash"
    assert json.loads(tool_client.requests[0].args_json) == {"command": "echo oi > out.txt"}
    assert result["status"] == "completed"
    assert result["final_output"] == "arquivo criado"
    assert len(result["tool_calls"]) == 1
    assert result["tool_calls"][0]["exit_code"] == 0


def test_execute_sem_action_nao_usa_tools_mesmo_com_tool_client_anexado():
    payload = {"final_output": "código", "status": "completed", "confidence": 0.8}
    agent = DeveloperAgent()
    agent.attach_gateway(ScriptedGatewayClient([LlmResponse(text=json.dumps(payload))]))
    tool_client = ScriptedToolClient([ToolCallResult(content="não deveria ser chamado")])
    agent.attach_tool_client(tool_client)

    # Estilo proposta/avaliação (`_get_squad_proposals`) — sem "action".
    result = asyncio.run(agent.execute({"description": "avalie a tarefa X"}))

    assert tool_client.requests == []
    assert result["final_output"] == "código"
    assert "tool_calls" not in result  # caminho antigo de chamada única, formato intacto


def test_react_loop_esgota_passos_sem_final_answer_devolve_incomplete_honesto():
    tool_call_turn = json.dumps({"action": "tool_call", "tool": "bash", "args": {"command": "ls"}})
    agent = DeveloperAgent()
    agent.attach_gateway(ScriptedGatewayClient([LlmResponse(text=tool_call_turn)] * _MAX_REACT_STEPS))
    tool_client = ScriptedToolClient(
        [ToolCallResult(content="arquivo.txt", exit_code=0) for _ in range(_MAX_REACT_STEPS)]
    )
    agent.attach_tool_client(tool_client)

    result = asyncio.run(agent.execute({"description": "tarefa sem fim", "action": "implement"}))

    assert result["status"] == "incomplete"
    assert result["final_output"] == ""
    assert len(result["tool_calls"]) == _MAX_REACT_STEPS
