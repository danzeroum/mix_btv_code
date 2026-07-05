"""Squad multi-agente da plataforma Forge (sidecar Python).

Migração do protótipo BuildToValue (`src/` do repositório): consenso
ponderado, planejamento, roteamento, memória, HITL e fallback progressivo.
Regra de ouro: este pacote NUNCA chama provedores LLM diretamente — toda
geração passa pelo gateway Rust via gRPC (`CoreService.Generate`).
"""

from forge_squad.chains import ChainStep, ResilientPromptChain
from forge_squad.consensus import ConsensusResult, WeightedConsensusEngine
from forge_squad.forgetting import IntelligentForgetting, MemoryStore
from forge_squad.memory import AgentMemorySystem
from forge_squad.routing import LearningRouter
from forge_squad.sandbox import DockerSandbox, SecureToolSandbox, SecurityError
from forge_squad.security import SecurityConfig

__all__ = [
    "AgentMemorySystem",
    "ChainStep",
    "ConsensusResult",
    "DockerSandbox",
    "IntelligentForgetting",
    "LearningRouter",
    "MemoryStore",
    "ResilientPromptChain",
    "SecureToolSandbox",
    "SecurityConfig",
    "SecurityError",
    "WeightedConsensusEngine",
]
