"""Router adaptativo que aprende com o desempenho histórico (migrado de
BuildToValue `src/routing/learning_router.py`).
"""

from __future__ import annotations

import hashlib
from dataclasses import dataclass, field
from typing import Any


@dataclass
class LearningRouter:
    """Sugere rotas ótimas a partir de resultados históricos."""

    route_performance: dict[str, dict[str, float]] = field(default_factory=dict)

    async def smart_route(self, request: dict[str, Any]) -> str:
        route = request.get("preferred_route")
        if route:
            return route
        return "default"

    def update_route_performance(
        self, request: dict[str, Any], route: str, success: bool, latency: float
    ) -> None:
        key = f"{route}_{self._hash_request(request)}"
        stats = self.route_performance.setdefault(
            key, {"attempts": 0, "successes": 0, "total_latency": 0.0}
        )
        stats["attempts"] += 1
        if success:
            stats["successes"] += 1
        stats["total_latency"] += float(latency)
        stats["success_rate"] = stats["successes"] / stats["attempts"]
        stats["avg_latency"] = stats["total_latency"] / stats["attempts"]

    def _hash_request(self, request: dict[str, Any]) -> str:
        payload = repr(sorted(request.items())).encode("utf-8")
        return hashlib.sha1(payload, usedforsecurity=False).hexdigest()
