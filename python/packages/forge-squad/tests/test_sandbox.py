import pytest

from forge_squad.sandbox import SecureToolSandbox, SecurityError


def test_ferramenta_de_alto_risco_levanta_security_error():
    sandbox = SecureToolSandbox()
    with pytest.raises(SecurityError):
        sandbox.execute_tool_sandboxed("delete_resource", {})


def test_padrao_proibido_nos_parametros_levanta_security_error():
    # SecurityConfig.validate_tool_call já varre str(params) contra
    # FORBIDDEN_PATTERNS antes de _validate_params_safety rodar — como as
    # duas checagens usam a mesma lista, o ValueError de
    # _validate_params_safety é código morto na fonte original (nunca
    # roda primeiro). Fiel ao comportamento real, não ao que a origem
    # sugeria por engano.
    sandbox = SecureToolSandbox()
    with pytest.raises(SecurityError):
        sandbox.execute_tool_sandboxed("write_code", {"cmd": "rm -rf /"})


def test_sem_docker_devolve_resposta_simulada():
    sandbox = SecureToolSandbox()
    result = sandbox.execute_tool_sandboxed("write_code", {"content": "print('oi')"})
    assert result["sandboxed"] is False
    assert result["tool"] == "write_code"
