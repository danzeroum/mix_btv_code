from forge_squad.security import SecurityConfig


def test_ferramenta_de_alto_risco_e_bloqueada():
    ok, reason = SecurityConfig.validate_tool_call("delete_resource", {})
    assert not ok
    assert "requires human approval" in reason


def test_padrao_proibido_nos_parametros_e_bloqueado():
    ok, reason = SecurityConfig.validate_tool_call("write_code", {"cmd": "rm -rf /"})
    assert not ok
    assert "Forbidden pattern" in reason


def test_chamada_segura_passa():
    ok, reason = SecurityConfig.validate_tool_call("write_code", {"content": "print('oi')"})
    assert ok
    assert reason == "OK"
