import copy

from forge_review.certification import certify, evidence_hash
from forge_review.gates import evaluate
from forge_review.score import ReviewScores

_EVIDENCE = {
    "run_id": "run-1",
    "git_sha": "deadbeef",
    "steps": [
        {"name": "test", "tool": "cargo test", "exit_code": 0, "duration_ms": 10, "findings": []},
        {"name": "lint", "tool": "clippy", "exit_code": 0, "duration_ms": 5, "findings": []},
    ],
    "verdict": "pass",
    "produced_at": "2026-01-01T00:00:00Z",
}


def test_hash_e_deterministico_e_reproduzivel_por_dois_caminhos_independentes():
    # Duas chamadas independentes sobre o MESMO conteúdo (cópias distintas
    # em memória, não o mesmo objeto) devem produzir o hash idêntico —
    # prova que é hash de conteúdo canônico, não de identidade de objeto.
    a = evidence_hash(copy.deepcopy(_EVIDENCE))
    b = evidence_hash(copy.deepcopy(_EVIDENCE))
    assert a == b
    assert len(a) == 64  # sha256 hex


def test_hash_e_insensivel_a_ordem_de_chaves():
    # JSON canônico ordena chaves — um dict com chaves em ordem diferente
    # mas mesmo conteúdo deve produzir o mesmo hash.
    reordenado = {
        "verdict": _EVIDENCE["verdict"],
        "produced_at": _EVIDENCE["produced_at"],
        "git_sha": _EVIDENCE["git_sha"],
        "run_id": _EVIDENCE["run_id"],
        "steps": _EVIDENCE["steps"],
    }
    assert evidence_hash(_EVIDENCE) == evidence_hash(reordenado)


def test_hash_muda_se_a_evidencia_muda():
    outra = copy.deepcopy(_EVIDENCE)
    outra["verdict"] = "fail"
    assert evidence_hash(_EVIDENCE) != evidence_hash(outra)


def test_certify_monta_resumo_dos_passos_e_carrega_o_veredito():
    scores = ReviewScores(technical=0.9, performance=0.9, security=0.9, value=0.9)
    verdict = evaluate(scores, evidence=_EVIDENCE)

    cert = certify("run-1", "deadbeef", verdict, _EVIDENCE, "2026-01-01T00:00:01Z")

    assert cert.evidence_hash == evidence_hash(_EVIDENCE)
    assert cert.steps_summary == ["test: ok", "lint: ok"]
    assert cert.verdict.approved is True
    assert cert.run_id == "run-1"


def test_certify_resumo_marca_passo_que_falhou():
    evidencia_com_falha = copy.deepcopy(_EVIDENCE)
    evidencia_com_falha["steps"][1]["exit_code"] = 1
    scores = ReviewScores(technical=0.9, performance=0.9, security=0.9, value=0.9)
    verdict = evaluate(scores, evidence=evidencia_com_falha)

    cert = certify("run-2", "cafebabe", verdict, evidencia_com_falha, "2026-01-01T00:00:02Z")

    assert cert.steps_summary == ["test: ok", "lint: fail"]
