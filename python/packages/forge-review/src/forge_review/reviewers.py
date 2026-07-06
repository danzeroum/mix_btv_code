"""Reviewers determinísticos que derivam scores de sinal REAL da evidência
do `/verify` (Fase 5 Onda 1/2, `verification-evidence.v1`) — não de
heurística sobre texto de código solto.

**Nota de proveniência (honesta, não presumida):** o PLANO desta onda pedia
para "portar" os 4 reviewers de `.buildtovalue/review/orchestrator.py`.
Esse arquivo **não está disponível neste ambiente** (não foi clonado/
preservado no workspace) — não há como ler o código de origem para portar
fielmente. Em vez de reconstruir de memória (o que seria fabricar uma
"origem" não verificável — exatamente o que a régua "Nada Fake" proíbe),
este módulo é **código novo**, desenhado para este projeto:

- `technical_score`/`security_score` derivam de sinal determinístico real
  já disponível (`VerificationEvidence.steps[].exit_code`/`.findings[]`).
- `performance`/`value` (as outras duas dimensões de `ReviewScores`) não
  têm sinal determinístico equivalente nesta fase — ficam por conta do
  chamador (ex.: confiança do squad, avaliação humana), em vez de uma
  fórmula fabricada sem dado real por trás.
"""

from __future__ import annotations

from typing import Any, Optional

#: Penalidade subtraída de 1.0 por finding, conforme severidade — más
#: severidades não listadas usam o piso conservador (0.05).
_SEVERITY_PENALTY = {"critical": 0.4, "error": 0.4, "warning": 0.1}


def technical_score(evidence: Optional[dict[str, Any]]) -> float:
    """Fração de passos do `/verify` que passaram (`exit_code == 0`).

    Sem evidência (ou sem passos), devolve 0.5 — neutro, não uma opinião
    fabricada sobre código que não foi verificado."""

    if not evidence or not evidence.get("steps"):
        return 0.5
    steps = evidence["steps"]
    passed = sum(1 for s in steps if s.get("exit_code") == 0)
    return passed / len(steps)


def security_score(evidence: Optional[dict[str, Any]]) -> float:
    """1.0 menos penalidade por finding de severidade alta, piso em 0.0.

    Sem evidência, devolve 0.5 — mesmo espírito de `technical_score`."""

    if not evidence or not evidence.get("steps"):
        return 0.5
    score = 1.0
    for step in evidence["steps"]:
        for finding in step.get("findings", []):
            score -= _SEVERITY_PENALTY.get(finding.get("severity", ""), 0.05)
    return max(0.0, score)
