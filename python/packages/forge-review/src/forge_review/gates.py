"""Quality gates reais: regras duras que SOBREPÕEM a média ponderada
(`forge_review.score.value_score`) — Fase 5 Onda 4.

Antes desta onda, a aprovação era só `value_score > 0.7`; a média pode
"salvar" um código com falha crítica de segurança (uma dimensão ruim
diluída pelas outras três boas). Um gate de verdade impede exatamente
isso: qualquer um dos gates abaixo reprova independente da média.
"""

from __future__ import annotations

from typing import Any, Optional

from pydantic import BaseModel

from forge_review.score import APPROVAL_THRESHOLD, ReviewScores, value_score

#: Piso duro da dimensão `security`, verificado ANTES da média. Não é um
#: número mágico solto: é a mesma semântica de "abaixo da metade da escala
#: não é aceitável" que `APPROVAL_THRESHOLD` aplica à média das quatro
#: dimensões — aqui aplicada a UMA dimensão isolada, deliberadamente mais
#: baixo que 0.7 porque é um piso de não-desqualificação, não um alvo de
#: aprovação.
SECURITY_FLOOR = 0.5


class ReviewVerdict(BaseModel):
    approved: bool
    value_score: float
    reason: str
    #: Qual gate duro reprovou (`None` se a decisão veio da média).
    gate_triggered: Optional[str] = None


def _has_critical_finding(evidence: Optional[dict[str, Any]]) -> bool:
    if not evidence:
        return False
    return any(
        finding.get("severity") == "critical"
        for step in evidence.get("steps", [])
        for finding in step.get("findings", [])
    )


def evaluate(scores: ReviewScores, evidence: Optional[dict[str, Any]] = None) -> ReviewVerdict:
    """Aplica os gates duros, nesta ordem, ANTES de deixar a média decidir:

    1. finding de severidade `critical` na evidência → reprovado.
    2. `evidence["verdict"] == "fail"` → reprovado.
    3. `scores.security < SECURITY_FLOOR` → reprovado.
    4. só se nenhum gate disparou: a média (`value_score > 0.7`) decide.
    """

    vs = value_score(scores)

    if _has_critical_finding(evidence):
        return ReviewVerdict(
            approved=False,
            value_score=vs,
            reason="finding de severidade crítica na evidência de verificação",
            gate_triggered="critical_finding",
        )

    if evidence is not None and evidence.get("verdict") == "fail":
        return ReviewVerdict(
            approved=False,
            value_score=vs,
            reason="veredito fail na evidência de verificação",
            gate_triggered="verify_fail",
        )

    if scores.security < SECURITY_FLOOR:
        return ReviewVerdict(
            approved=False,
            value_score=vs,
            reason=f"segurança abaixo do piso ({scores.security} < {SECURITY_FLOOR})",
            gate_triggered="security_floor",
        )

    approved = vs > APPROVAL_THRESHOLD
    reason = "aprovado por média ponderada" if approved else "reprovado por média ponderada"
    return ReviewVerdict(approved=approved, value_score=vs, reason=reason, gate_triggered=None)
