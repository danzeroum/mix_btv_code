import asyncio

import pytest

from forge_squad.chains import ChainStep, ResilientPromptChain


def test_execute_with_checkpoints_caminho_feliz():
    steps = [
        ChainStep(name="dobra", execute=lambda x: x * 2),
        ChainStep(name="soma_um", execute=lambda x: x + 1),
    ]
    chain = ResilientPromptChain(steps=steps)
    result = asyncio.run(chain.execute_with_checkpoints(3))
    assert result == 7  # (3*2)+1


def test_retry_ate_suceder():
    calls = {"n": 0}

    def flaky(x):
        calls["n"] += 1
        if calls["n"] < 2:
            raise RuntimeError("falha transitória")
        return x

    chain = ResilientPromptChain(steps=[ChainStep(name="instavel", execute=flaky)], max_retries=3)
    result = asyncio.run(chain.execute_with_checkpoints("ok"))
    assert result == "ok"
    assert calls["n"] == 2


def test_esgota_retries_e_levanta_erro():
    def sempre_falha(x):
        raise RuntimeError("nunca funciona")

    chain = ResilientPromptChain(steps=[ChainStep(name="quebrado", execute=sempre_falha)], max_retries=2)
    with pytest.raises(RuntimeError, match="Falha na etapa quebrado"):
        asyncio.run(chain.execute_with_checkpoints("x"))
