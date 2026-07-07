"""Testa o servidor gRPC do `MemoryService` (grpc.aio real, sobre um Unix
Domain Socket efêmero) — mesmo padrão de `forge_promptforge`'s
`test_server.py`, sem pytest-asyncio."""

from __future__ import annotations

import asyncio

import grpc

from forge_proto import memory_pb2, memory_pb2_grpc
from forge_squad.memory import AgentMemorySystem
from forge_squad.memory_server import VERSION, MemoryServicer


async def _with_server(socket_path: str, memory_dir, body):
    server = grpc.aio.server()
    memory_pb2_grpc.add_MemoryServiceServicer_to_server(MemoryServicer(memory_dir), server)
    server.add_insecure_port(f"unix://{socket_path}")
    await server.start()
    try:
        async with grpc.aio.insecure_channel(f"unix://{socket_path}") as channel:
            stub = memory_pb2_grpc.MemoryServiceStub(channel)
            await body(stub)
    finally:
        await server.stop(None)


def test_health_responde_pronto(tmp_path):
    async def body(stub):
        resp = await stub.Health(memory_pb2.HealthRequest())
        assert resp.ready is True
        assert resp.version == VERSION

    asyncio.run(_with_server(str(tmp_path / "s.sock"), tmp_path, body))


def test_recall_ground_truth_recupera_exatamente_os_relevantes(tmp_path):
    """Mesma fronteira de `test_recall_similar_ground_truth_...` de
    `test_memory.py`, agora atravessando o gRPC de ponta a ponta."""
    memory = AgentMemorySystem(storage_dir=tmp_path)
    memory.remember_decision("dev", {"summary": "corrigir login e senha do usuário"})
    memory.remember_decision("dev", {"summary": "isolar o contêiner docker sem rede"})

    async def body(stub):
        resp = await stub.Recall(memory_pb2.RecallRequest(query="problema de login e senha", k=3))
        assert len(resp.matches) == 1
        assert "login" in resp.matches[0].decision_json
        assert resp.matches[0].agent == "dev"
        assert resp.matches[0].score > 0.0

    asyncio.run(_with_server(str(tmp_path / "s.sock"), tmp_path, body))


def test_recall_sem_correspondencia_devolve_vazio(tmp_path):
    async def body(stub):
        resp = await stub.Recall(memory_pb2.RecallRequest(query="qualquer coisa", k=5))
        assert list(resp.matches) == []

    asyncio.run(_with_server(str(tmp_path / "s.sock"), tmp_path, body))


def test_list_sem_filtro_agrupa_por_agente_com_contagem_e_decisao_reais(tmp_path):
    memory = AgentMemorySystem(storage_dir=tmp_path)
    memory.remember_decision("architect", {"confidence": 0.4, "summary": "primeira do architect"})
    memory.remember_decision("architect", {"confidence": 0.9, "summary": "segunda do architect"})
    memory.remember_decision("developer", {"confidence": 0.6, "summary": "unica do developer"})

    async def body(stub):
        resp = await stub.List(memory_pb2.ListRequest(limit=50))
        by_agent = {s.agent: s for s in resp.agents}
        assert set(by_agent) == {"architect", "developer"}

        architect = by_agent["architect"]
        assert architect.count == 2
        # Mais recente primeiro: a "segunda" foi lembrada por último.
        assert "segunda do architect" in architect.latest_decision_json
        # Maior confiança real (0.9), não fabricada.
        assert architect.top_confidence == 0.9

        developer = by_agent["developer"]
        assert developer.count == 1
        assert "unica do developer" in developer.latest_decision_json

    asyncio.run(_with_server(str(tmp_path / "s.sock"), tmp_path, body))


def test_list_filtra_por_agente(tmp_path):
    memory = AgentMemorySystem(storage_dir=tmp_path)
    memory.remember_decision("architect", {"confidence": 0.5, "summary": "x"})
    memory.remember_decision("developer", {"confidence": 0.5, "summary": "y"})

    async def body(stub):
        resp = await stub.List(memory_pb2.ListRequest(agent="architect", limit=50))
        assert len(resp.agents) == 1
        assert resp.agents[0].agent == "architect"

    asyncio.run(_with_server(str(tmp_path / "s.sock"), tmp_path, body))


def test_list_corpus_vazio_devolve_lista_vazia_nao_erro(tmp_path):
    async def body(stub):
        resp = await stub.List(memory_pb2.ListRequest(limit=50))
        assert list(resp.agents) == []

    asyncio.run(_with_server(str(tmp_path / "s.sock"), tmp_path, body))
