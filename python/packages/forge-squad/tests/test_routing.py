import asyncio

from forge_squad.routing import LearningRouter


def test_smart_route_respeita_preferencia():
    router = LearningRouter()
    route = asyncio.run(router.smart_route({"preferred_route": "fast-path"}))
    assert route == "fast-path"


def test_smart_route_sem_preferencia_usa_default():
    router = LearningRouter()
    route = asyncio.run(router.smart_route({}))
    assert route == "default"


def test_update_route_performance_acumula_estatisticas():
    router = LearningRouter()
    request = {"task": "x"}
    router.update_route_performance(request, "default", True, 100.0)
    router.update_route_performance(request, "default", False, 300.0)

    key = f"default_{router._hash_request(request)}"
    stats = router.route_performance[key]
    assert stats["attempts"] == 2
    assert stats["successes"] == 1
    assert stats["success_rate"] == 0.5
    assert stats["avg_latency"] == 200.0
