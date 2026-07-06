from forge_review.reviewers import security_score, technical_score


def test_technical_score_e_a_fracao_de_passos_que_passaram():
    evidence = {
        "steps": [
            {"exit_code": 0, "findings": []},
            {"exit_code": 1, "findings": []},
            {"exit_code": 0, "findings": []},
        ]
    }
    assert technical_score(evidence) == 2 / 3


def test_technical_score_sem_evidencia_e_neutro():
    assert technical_score(None) == 0.5
    assert technical_score({"steps": []}) == 0.5


def test_security_score_maximo_sem_findings():
    evidence = {"steps": [{"exit_code": 0, "findings": []}]}
    assert security_score(evidence) == 1.0


def test_security_score_penaliza_finding_critico_mais_que_warning():
    critico = {"steps": [{"findings": [{"severity": "critical"}]}]}
    warning = {"steps": [{"findings": [{"severity": "warning"}]}]}
    assert security_score(critico) < security_score(warning)


def test_security_score_tem_piso_zero():
    muitos_criticos = {"steps": [{"findings": [{"severity": "critical"}] * 10}]}
    assert security_score(muitos_criticos) == 0.0
