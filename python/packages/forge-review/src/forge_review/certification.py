"""Certificação: artefato do que foi verificado, por quais passos, veredito
e o hash da evidência que o produziu — registrável no ledger (Rust,
`forge-store::LedgerStore`) como entrada de 1ª classe (`kind:
"certification"`). O ledger é Rust-only (não há API de ledger em Python);
este módulo PRODUZ a certificação, o lado Rust é quem a REGISTRA.
"""

from __future__ import annotations

from typing import Any

from forge_promptforge.hashing import canonical_json, sha256_hex
from pydantic import BaseModel

from forge_review.gates import ReviewVerdict


class Certification(BaseModel):
    run_id: str
    git_sha: str
    verdict: ReviewVerdict
    #: sha256 do JSON canônico da evidência — o link imutável para o que
    #: foi de fato verificado.
    evidence_hash: str
    steps_summary: list[str]
    produced_at: str


def evidence_hash(evidence: dict[str, Any]) -> str:
    """sha256 do JSON canônico da evidência (`verification-evidence.v1`).

    Reusa o MESMO esquema de hash canônico do `prompt-cache-key.v1`
    (`forge_promptforge.hashing`: chaves ordenadas em todos os níveis,
    separadores compactos, sha256 hex) — não um segundo algoritmo de hash
    inventado só para certificação."""

    return sha256_hex(canonical_json(evidence))


def certify(run_id: str, git_sha: str, verdict: ReviewVerdict, evidence: dict[str, Any], produced_at: str) -> Certification:
    steps_summary = [
        f"{step.get('name', '?')}: {'ok' if step.get('exit_code') == 0 else 'fail'}"
        for step in evidence.get("steps", [])
    ]
    return Certification(
        run_id=run_id,
        git_sha=git_sha,
        verdict=verdict,
        evidence_hash=evidence_hash(evidence),
        steps_summary=steps_summary,
        produced_at=produced_at,
    )
