"""Memória persistente de agentes (migrado de BuildToValue
`src/memory/agent_memory.py`). Curto/longo prazo + episódica em disco.

`chromadb` é opcional — sem ele, degrada graciosamente para um fallback
em memória (mesmo princípio de degradação graciosa do sidecar Rust).
Diretório de armazenamento segue a convenção `.forge/` do resto da
plataforma (era `.buildtoflip/ledger` na origem).
"""

from __future__ import annotations

import json
from datetime import datetime, timezone
from pathlib import Path
from typing import Any, Optional

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

    def recall_similar(self, query: str, k: int = 5) -> dict[str, Any]:
        """Recupera memórias similares via banco vetorial (ou fallback)."""

        return self.collection.query(query_texts=[query], n_results=k)
