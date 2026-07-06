"""Recuperação local por similaridade (Fase 6 Onda 6).

Substitui o `recall_similar` que era um **no-op na prática** (o
`_FallbackCollection.query` devolvia listas vazias sempre; chromadb nunca foi
dep declarada) por uma recuperação **real**: um índice TF-IDF esparso sobre o
corpus de memórias, ranqueado por cosseno.

Decisões de fronteira (registradas em pendencias.md, ADR na Onda 9):

- **Léxico, não neural.** É TF-IDF (frequência de termo × frequência inversa de
  documento) — recuperação real por sobreposição de termos distintivos, com
  stopwords removidas e termos ubíquos naturalmente descontados pelo IDF. Não é
  embedding neural (sinônimo/paráfrase); isso exigiria um modelo local ou o
  gateway Rust (keys só no Rust) — fica para uma onda futura. O que entrega:
  recupera **exatamente** as memórias relevantes de um tópico, não "retorna
  algo".
- **Offline, zero-dependência.** Puro Python (só stdlib) — coerente com o
  princípio offline-first do produto e sem inflar `uv.lock`. Nada sai da máquina.
- **A fonte é o corpus persistido** (o JSONL episódico); o índice é derivado e
  reconstruído a cada consulta (o corpus é pequeno — dezenas/centenas). Um índice
  materializado é otimização futura.
"""

from __future__ import annotations

import math
import re
from collections import Counter
from typing import Iterable

# Stopwords PT+EN comuns: removidas para o TF-IDF discriminar por conteúdo, não
# por conectivos ubíquos. Lista modesta e transparente (não é um recurso oculto).
_STOPWORDS = {
    # português
    "a", "o", "e", "de", "do", "da", "dos", "das", "em", "no", "na", "nos",
    "nas", "um", "uma", "uns", "umas", "para", "por", "com", "sem", "que", "se",
    "ao", "aos", "à", "às", "os", "as", "ou", "mais", "menos", "muito", "já",
    "não", "sim", "é", "foi", "ser", "está", "este", "esta", "isso", "como",
    "pelo", "pela", "seu", "sua", "the", "of", "to", "and", "in", "is", "it",
    # inglês
    "for", "on", "with", "as", "at", "by", "an", "be", "or", "this", "that",
    "was", "are", "from", "but", "not", "can", "has",
}

_TOKEN = re.compile(r"\w+", re.UNICODE)


def _tokenize(text: str) -> list[str]:
    """Casefold + tokens de palavra (unicode, aceita acentos), sem stopwords nem
    tokens de 1 caractere."""
    out = []
    for raw in _TOKEN.findall(text):
        tok = raw.casefold()
        if len(tok) < 2 or tok in _STOPWORDS:
            continue
        out.append(tok)
    return out


def _idf(docs_tokens: list[list[str]]) -> dict[str, float]:
    """IDF suavizado: log((1+N)/(1+df)) + 1 — sempre ≥ 1 (nunca zera um termo,
    então uma memória única continua recuperável), mas termos ubíquos ficam
    próximos do piso enquanto os distintivos sobem."""
    n = len(docs_tokens)
    df: Counter[str] = Counter()
    for toks in docs_tokens:
        for t in set(toks):
            df[t] += 1
    return {t: math.log((1 + n) / (1 + d)) + 1.0 for t, d in df.items()}


def _vector(tokens: Iterable[str], idf: dict[str, float]) -> dict[str, float]:
    """Vetor TF-IDF normalizado (L2) — assim o cosseno é só o produto interno."""
    tf = Counter(tokens)
    vec = {t: c * idf.get(t, 0.0) for t, c in tf.items()}
    norm = math.sqrt(sum(v * v for v in vec.values()))
    if norm == 0.0:
        return {}
    return {t: v / norm for t, v in vec.items()}


def _cosine(a: dict[str, float], b: dict[str, float]) -> float:
    """Produto interno de dois vetores já normalizados (= cosseno)."""
    if len(a) > len(b):
        a, b = b, a
    return sum(v * b.get(t, 0.0) for t, v in a.items())


def rank(query: str, docs: list[str], k: int = 5) -> list[tuple[int, float]]:
    """Ranqueia `docs` pela similaridade TF-IDF-cosseno com `query`.

    Devolve `[(índice, score)]` dos top-`k` com score **estritamente positivo**,
    em ordem decrescente. Score positivo = há termos distintivos em comum; docs
    sem sobreposição (score 0) são **excluídos** (não vira preenchimento
    irrelevante). Consulta cujos termos não aparecem no corpus → lista vazia
    (honesto: não inventa relevância).
    """
    if not docs or k <= 0:
        return []
    docs_tokens = [_tokenize(d) for d in docs]
    idf = _idf(docs_tokens)
    qvec = _vector(_tokenize(query), idf)
    if not qvec:
        return []
    scored = []
    for i, toks in enumerate(docs_tokens):
        s = _cosine(qvec, _vector(toks, idf))
        if s > 1e-9:
            scored.append((i, s))
    scored.sort(key=lambda pair: pair[1], reverse=True)
    return scored[:k]
