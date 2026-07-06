"""Testes do retriever TF-IDF local (Fase 6 Onda 6).

Provam recuperação REAL por relevância — não vacuidade: ground truth com
conjuntos disjuntos, discriminação por termo distintivo (IDF), e os cantos
honestos (corpus vazio, consulta sem casamento, limite k)."""

from forge_squad.recall import rank


# Ground truth: dois tópicos com um termo distintivo COMPARTILHADO por todos os
# docs do tópico ("autenticação" / "sandbox") e vocabulário disjunto entre
# tópicos — assim a relevância é determinável lexicalmente (o teste justo para um
# retriever léxico; embeddings neurais fariam a ponte sinônimo, mas isso é onda
# futura, ver recall.py).
def _docs():
    return [
        "corrigir login e senha no fluxo de autenticação do usuário",  # 0 auth
        "expirar token de sessão e refazer autenticação após logout",  # 1 auth
        "política de autenticação multifator com senha forte",         # 2 auth
        "isolar o sandbox docker sem acesso à rede externa",           # 3 sandbox
        "limitar memória e cpu do sandbox de terceiro",                # 4 sandbox
        "montar o filesystem do sandbox como somente-leitura",         # 5 sandbox
    ]


def test_ground_truth_recupera_exatamente_o_topico():
    """A fronteira: consulta de autenticação → exatamente os índices {0,1,2}."""
    ranked = rank("problema de autenticação com login e senha", _docs(), k=3)
    idxs = {i for i, _ in ranked}
    assert idxs == {0, 1, 2}, f"esperava os 3 de auth; veio {idxs}"


def test_topico_oposto_recupera_o_outro_conjunto():
    """Simétrico: consulta de sandbox → exatamente {3,4,5}. Prova que não é o
    conjunto A que sempre vence — a relevância manda."""
    ranked = rank("isolar o sandbox da rede externa", _docs(), k=3)
    idxs = {i for i, _ in ranked}
    assert idxs == {3, 4, 5}, f"esperava os 3 de sandbox; veio {idxs}"


def test_ordena_por_relevancia_decrescente():
    ranked = rank("login e senha", _docs(), k=6)
    scores = [s for _, s in ranked]
    assert scores == sorted(scores, reverse=True)
    assert all(s > 0.0 for s in scores)


def test_exclui_score_zero_nao_preenche():
    """k grande, mas só 3 docs casam o tópico → devolve 3, não preenche com
    irrelevantes (score 0 é excluído)."""
    ranked = rank("autenticação login senha token sessão", _docs(), k=6)
    assert len(ranked) == 3
    assert {i for i, _ in ranked} == {0, 1, 2}


def test_consulta_sem_casamento_devolve_vazio():
    """Termos ausentes do corpus → vazio (honesto: não inventa relevância)."""
    assert rank("kubernetes helm terraform", _docs(), k=5) == []


def test_corpus_vazio_e_k_invalido():
    assert rank("qualquer", [], k=5) == []
    assert rank("login", _docs(), k=0) == []


def test_memoria_unica_e_recuperavel():
    """Corpus de 1 doc: o IDF suavizado (≥1) mantém a memória recuperável — não
    regride ao no-op para N=1."""
    ranked = rank("login do usuário", ["corrigir login e senha do usuário"], k=3)
    assert len(ranked) == 1 and ranked[0][0] == 0


def test_termo_ubiquo_nao_domina_a_relevancia():
    """'usuário' aparece em vários docs; a consulta que o contém ainda
    discrimina pelo termo distintivo ('senha'), não empata tudo."""
    docs = [
        "o usuário alterou a senha",       # 0 — distintivo: senha
        "o usuário abriu o painel",        # 1 — só o ubíquo 'usuário'
        "o usuário fechou a sessão",       # 2 — só o ubíquo 'usuário'
    ]
    ranked = rank("senha do usuário", docs, k=1)
    assert ranked[0][0] == 0, f"o doc com 'senha' deveria vencer; veio {ranked}"
