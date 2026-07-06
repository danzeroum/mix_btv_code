"""Memória persistente de agentes (migrado de BuildToValue
`src/memory/agent_memory.py`). Curto/longo prazo + episódica em disco.

`chromadb` é opcional — sem ele, o **corpus episódico em disco** (o JSONL) é a
fonte da verdade. A recuperação (`recall_similar`) é feita por um índice TF-IDF
local (`recall.py`, Fase 6 Onda 6), **não mais** pelo `_FallbackCollection` — que
era um no-op (devolvia listas vazias sempre). O ramo chromadb permanece como
sink alternativo (inativo enquanto a dep não for declarada), mas o recall não
depende mais dele. Diretório de armazenamento segue a convenção `.forge/` do
resto da plataforma (era `.buildtoflip/ledger` na origem).
"""

from __future__ import annotations

import json
from datetime import datetime, timezone
from pathlib import Path
from typing import Any, Optional

from . import recall

try:
    import chromadb
    from chromadb.config import Settings
except Exception:  # pragma: no cover - dependência opcional
    chromadb = None
    Settings = None


class _FallbackCollection:
    def __init__(self) -> None:
        self._items: list[dict[str, Any]] = []

    def add(self, *, documents: list[str], metadatas: list[dict[str, Any]], ids: list[str]) -> None:
        for doc, meta, id_ in zip(documents, metadatas, ids):
            self._items.append({"id": id_, "document": doc, "metadata": meta})

    def query(self, query_texts: list[str], n_results: int = 5) -> dict[str, Any]:
        return {"ids": [], "metadatas": [], "documents": [], "query": query_texts, "n_results": n_results}


class AgentMemorySystem:
    """Gerencia memórias de curto, longo prazo e episódicas dos agentes."""

    def __init__(self, storage_dir: Optional[Path] = None) -> None:
        self.short_term: dict[str, Any] = {}
        self.storage_dir = storage_dir or Path(".forge") / "squad-memory"
        self.storage_dir.mkdir(parents=True, exist_ok=True)
        self.episodic_path = self.storage_dir / "agent_memories.jsonl"

        self.collection = self._initialise_vector_store()

    def _initialise_vector_store(self):
        if chromadb is None or Settings is None:
            return _FallbackCollection()

        try:
            client = chromadb.Client(
                Settings(
                    chroma_server_host="localhost",
                    chroma_server_http_port=8000,
                    chroma_client_auth_provider="no_auth",
                )
            )
            try:
                return client.create_collection("agent_memories")
            except Exception:
                return client.get_collection("agent_memories")
        except Exception:
            return _FallbackCollection()

    def remember_decision(self, agent: str, decision: dict[str, Any]) -> None:
        """Grava uma decisão importante em disco e no armazenamento vetorial."""

        memory = {
            "timestamp": datetime.now(timezone.utc).isoformat(),
            "agent": agent,
            "decision": decision,
            "confidence": float(decision.get("confidence", 0.0)),
        }
        with self.episodic_path.open("a", encoding="utf-8") as handle:
            handle.write(json.dumps(memory, ensure_ascii=False) + "\n")

        self.collection.add(
            documents=[json.dumps(decision, ensure_ascii=False)],
            metadatas=[{"agent": agent, "timestamp": memory["timestamp"]}],
            ids=[f"{agent}_{memory['timestamp']}"],
        )

    def _load_corpus(self) -> list[dict[str, Any]]:
        """Lê o corpus episódico do disco (JSONL). É a fonte da verdade do
        recall — persiste entre sessões e já contém o que foi lembrado nesta
        (o `remember_decision` grava na hora). Linhas malformadas são puladas."""
        if not self.episodic_path.exists():
            return []
        records: list[dict[str, Any]] = []
        with self.episodic_path.open("r", encoding="utf-8") as handle:
            for line in handle:
                line = line.strip()
                if not line:
                    continue
                try:
                    rec = json.loads(line)
                except json.JSONDecodeError:
                    continue
                if isinstance(rec, dict) and "decision" in rec:
                    records.append(rec)
        return records

    def recall_similar(self, query: str, k: int = 5) -> dict[str, Any]:
        """Recupera as `k` memórias mais similares à `query` por TF-IDF-cosseno
        sobre o corpus episódico (Fase 6 Onda 6 — recuperação real, substitui o
        no-op). Devolve listas paralelas (`ids`/`documents`/`metadatas`/`scores`)
        das relevantes, em ordem decrescente de relevância; vazio se nada casa."""
        corpus = self._load_corpus()
        docs = [json.dumps(rec.get("decision", {}), ensure_ascii=False) for rec in corpus]
        ranked = recall.rank(query, docs, k)
        return {
            "ids": [
                f"{corpus[i].get('agent', '?')}_{corpus[i].get('timestamp', i)}"
                for i, _ in ranked
            ],
            "documents": [docs[i] for i, _ in ranked],
            "metadatas": [
                {"agent": corpus[i].get("agent"), "timestamp": corpus[i].get("timestamp")}
                for i, _ in ranked
            ],
            "scores": [score for _, score in ranked],
            "query": [query],
            "n_results": k,
        }
