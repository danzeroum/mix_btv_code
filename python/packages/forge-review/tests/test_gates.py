from forge_review.gates import SECURITY_FLOOR, evaluate
from forge_review.score import ReviewScores


def _alta_media() -> ReviewScores:
    # value_score ~0.9 — bem acima de APPROVAL_THRESHOLD (0.7).
    return ReviewScores(technical=0.9, performance=0.9, security=0.9, value=0.9)


def test_finding_critico_reprova_mesmo_com_media_alta():
    # O teste que carrega a onda: se isto aprovar, o gate não fez seu
    # trabalho — a média sozinha "salvaria" o finding crítico.
    evidence = {
        "steps": [{"name": "sast", "exit_code": 1, "findings": [{"tool": "x", "severity": "critical", "message": "y"}]}],
        "verdict": "fail",
    }
    verdict = evaluate(_alta_media(), evidence=evidence)
    assert verdict.approved is False
    assert verdict.gate_triggered == "critical_finding"
    assert verdict.value_score > 0.7  # a média em si continua alta — só não decide


def test_verdict_fail_reprova_mesmo_com_media_alta():
    evidence = {"steps": [{"name": "test", "exit_code": 1, "findings": []}], "verdict": "fail"}
    verdict = evaluate(_alta_media(), evidence=evidence)
    assert verdict.approved is False
    assert verdict.gate_triggered == "verify_fail"


def test_seguranca_abaixo_do_piso_reprova_mesmo_com_media_alta():
    scores = ReviewScores(technical=0.9, performance=0.9, security=SECURITY_FLOOR - 0.1, value=0.9)
    verdict = evaluate(scores, evidence=None)
    assert verdict.approved is False
    assert verdict.gate_triggered == "security_floor"


def test_sem_gate_disparado_media_decide_aprovando():
    verdict = evaluate(_alta_media(), evidence={"steps": [], "verdict": "pass"})
    assert verdict.approved is True
    assert verdict.gate_triggered is None


def test_sem_gate_disparado_media_decide_reprovando():
    baixa_media = ReviewScores(technical=0.3, performance=0.3, security=0.6, value=0.3)
    verdict = evaluate(baixa_media, evidence=None)
    assert verdict.approved is False
    assert verdict.gate_triggered is None
    assert verdict.reason == "reprovado por média ponderada"


def test_sem_evidencia_nenhuma_apenas_o_piso_de_seguranca_pode_disparar():
    # evidence=None não deve disparar critical_finding/verify_fail (não há
    # o que checar) — só o piso de segurança, que é sobre o score, continua ativo.
    verdict = evaluate(_alta_media(), evidence=None)
    assert verdict.approved is True
    assert verdict.gate_triggered is None
