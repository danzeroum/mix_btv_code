"""Review orientado a valor (origem: BuildToValue `.buildtovalue/review/`).

Quatro reviewers (technical, performance, security, value/ROI) produzem um
`value_score` ponderado; mudanças com score > 0.7 são aprovadas por
padrão — mas a Fase 5 Onda 4 acrescenta `gates.evaluate`, que SOBREPÕE essa
média com regras duras (finding crítico, veredito fail do /verify, piso de
segurança), e `certification.certify`, que produz o artefato registrável
no ledger (Rust) com o hash da evidência que o embasou.
"""

from forge_review.certification import Certification, certify, evidence_hash
from forge_review.gates import SECURITY_FLOOR, ReviewVerdict, evaluate
from forge_review.score import APPROVAL_THRESHOLD, ReviewScores, value_score

__all__ = [
    "APPROVAL_THRESHOLD",
    "ReviewScores",
    "value_score",
    "SECURITY_FLOOR",
    "ReviewVerdict",
    "evaluate",
    "Certification",
    "certify",
    "evidence_hash",
]
