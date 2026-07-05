import json

from forge_squad.memory import AgentMemorySystem


def test_remember_decision_persiste_em_jsonl(tmp_path):
    memory = AgentMemorySystem(storage_dir=tmp_path)
    memory.remember_decision("architect", {"confidence": 0.9, "summary": "plano aprovado"})

    lines = memory.episodic_path.read_text(encoding="utf-8").splitlines()
    assert len(lines) == 1
    entry = json.loads(lines[0])
    assert entry["agent"] == "architect"
    assert entry["confidence"] == 0.9


def test_remember_decision_acumula_multiplas_entradas(tmp_path):
    memory = AgentMemorySystem(storage_dir=tmp_path)
    memory.remember_decision("developer", {"confidence": 0.5})
    memory.remember_decision("auditor", {"confidence": 0.8})

    lines = memory.episodic_path.read_text(encoding="utf-8").splitlines()
    assert len(lines) == 2


def test_recall_similar_sem_chromadb_usa_fallback(tmp_path):
    memory = AgentMemorySystem(storage_dir=tmp_path)
    memory.remember_decision("architect", {"confidence": 0.9})

    result = memory.recall_similar("plano de arquitetura", k=3)
    assert result["query"] == ["plano de arquitetura"]
    assert result["n_results"] == 3
