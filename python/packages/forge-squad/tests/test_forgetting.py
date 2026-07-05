from forge_squad.forgetting import IntelligentForgetting, MemoryStore


def test_adaptive_forget_remove_baixa_relevancia():
    store = MemoryStore(
        data={
            "a": {"relevance": 0.9},
            "b": {"relevance": 0.05},
            "c": {"relevance": 0.0},
        }
    )
    removed = IntelligentForgetting(memory=store).adaptive_forget()
    assert removed == 2
    assert set(store.get_all_memories()) == {"a"}


def test_adaptive_forget_sem_baixa_relevancia_nao_remove_nada():
    store = MemoryStore(data={"a": {"relevance": 0.9}})
    removed = IntelligentForgetting(memory=store).adaptive_forget()
    assert removed == 0
    assert set(store.get_all_memories()) == {"a"}
