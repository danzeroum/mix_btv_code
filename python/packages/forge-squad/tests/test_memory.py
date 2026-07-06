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


def test_recall_similar_recupera_memoria_lembrada(tmp_path):
    """Onda 6 — o caminho que antes era no-op (devolvia vazio sempre) agora
    RECUPERA: uma memória lembrada volta na consulta sobre o mesmo assunto."""
    memory = AgentMemorySystem(storage_dir=tmp_path)
    memory.remember_decision(
        "architect", {"confidence": 0.9, "summary": "plano de arquitetura do gateway aprovado"}
    )

    result = memory.recall_similar("plano de arquitetura", k=3)
    # Antes: ids == [] sempre. Agora recupera a memória de verdade.
    assert result["ids"], "recall não é mais no-op: deve recuperar a memória lembrada"
    assert result["metadatas"][0]["agent"] == "architect"
    assert result["scores"][0] > 0.0


def test_recall_similar_corpus_vazio_devolve_vazio(tmp_path):
    """Sem memórias, o recall é honestamente vazio (não inventa relevância)."""
    memory = AgentMemorySystem(storage_dir=tmp_path)
    result = memory.recall_similar("qualquer coisa", k=5)
    assert result["ids"] == []


def test_recall_similar_ground_truth_recupera_exatamente_os_relevantes(tmp_path):
    """Onda 6 — a fronteira: com ground truth (memórias de 2 tópicos distintos),
    a consulta de um tópico recupera **exatamente** as memórias daquele tópico —
    igualdade de conjunto, não "retornou algo". Prova que discrimina relevância,
    não que casa qualquer coisa."""
    memory = AgentMemorySystem(storage_dir=tmp_path)
    # Tópico A — autenticação.
    memory.remember_decision("dev", {"summary": "corrigir login e senha do usuário no fluxo de autenticação"})
    memory.remember_decision("dev", {"summary": "expirar o token de sessão após logout do usuário"})
    memory.remember_decision("architect", {"summary": "política de senha e autenticação multifator"})
    # Tópico B — sandbox (vocabulário disjunto).
    memory.remember_decision("dev", {"summary": "isolar o contêiner docker sem acesso à rede externa"})
    memory.remember_decision("dev", {"summary": "limitar memória e cpu do sandbox de terceiro"})
    memory.remember_decision("architect", {"summary": "montar o filesystem do contêiner como somente-leitura"})

    result = memory.recall_similar("problema de login e senha do usuário", k=3)
    docs = result["documents"]
    assert len(docs) == 3, f"esperava exatamente os 3 de autenticação; veio {len(docs)}"
    # Todos os recuperados são do tópico de autenticação; nenhum de sandbox.
    joined = " ".join(docs).casefold()
    assert "sandbox" not in joined and "docker" not in joined and "contêiner" not in joined, (
        f"recuperou memória do tópico errado: {docs}"
    )
    for termo in ("login", "senha", "autenticação"):
        assert termo in joined, f"esperava o vocabulário de autenticação ({termo}); veio {docs}"
