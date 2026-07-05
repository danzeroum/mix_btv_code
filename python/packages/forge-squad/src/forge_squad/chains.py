"""Cadeia de prompts resiliente com retry simples (migrado de
BuildToValue `src/chains/resilient_chain.py`).
"""

from __future__ import annotations

import asyncio
from dataclasses import dataclass
from typing import Any, Callable, Iterable


@dataclass
class ChainStep:
    name: str
    execute: Callable[[Any], Any]


@dataclass
class ResilientPromptChain:
    steps: Iterable[ChainStep]
    max_retries: int = 3

    async def execute_with_checkpoints(self, initial_input: Any) -> Any:
        current = initial_input
        for step in self.steps:
            for attempt in range(1, self.max_retries + 1):
                try:
                    result = step.execute(current)
                    if asyncio.iscoroutine(result):
                        result = await result
                    current = result
                    break
                except Exception as exc:
                    if attempt == self.max_retries:
                        raise RuntimeError(f"Falha na etapa {step.name}: {exc}") from exc
                    await asyncio.sleep(0)
        return current
