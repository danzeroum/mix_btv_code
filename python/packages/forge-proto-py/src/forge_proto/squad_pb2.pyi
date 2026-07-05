from google.protobuf.internal import enum_type_wrapper as _enum_type_wrapper
from google.protobuf import descriptor as _descriptor
from google.protobuf import message as _message
from collections.abc import Mapping as _Mapping
from typing import ClassVar as _ClassVar, Optional as _Optional, Union as _Union

DESCRIPTOR: _descriptor.FileDescriptor

class SquadTask(_message.Message):
    __slots__ = ("task_id", "description", "decision_type", "max_autonomy_level", "verification_evidence_json")
    TASK_ID_FIELD_NUMBER: _ClassVar[int]
    DESCRIPTION_FIELD_NUMBER: _ClassVar[int]
    DECISION_TYPE_FIELD_NUMBER: _ClassVar[int]
    MAX_AUTONOMY_LEVEL_FIELD_NUMBER: _ClassVar[int]
    VERIFICATION_EVIDENCE_JSON_FIELD_NUMBER: _ClassVar[int]
    task_id: str
    description: str
    decision_type: str
    max_autonomy_level: int
    verification_evidence_json: str
    def __init__(self, task_id: _Optional[str] = ..., description: _Optional[str] = ..., decision_type: _Optional[str] = ..., max_autonomy_level: _Optional[int] = ..., verification_evidence_json: _Optional[str] = ...) -> None: ...

class SquadEvent(_message.Message):
    __slots__ = ("task_id", "ts", "proposal", "consensus", "handoff", "hitl", "step", "error")
    TASK_ID_FIELD_NUMBER: _ClassVar[int]
    TS_FIELD_NUMBER: _ClassVar[int]
    PROPOSAL_FIELD_NUMBER: _ClassVar[int]
    CONSENSUS_FIELD_NUMBER: _ClassVar[int]
    HANDOFF_FIELD_NUMBER: _ClassVar[int]
    HITL_FIELD_NUMBER: _ClassVar[int]
    STEP_FIELD_NUMBER: _ClassVar[int]
    ERROR_FIELD_NUMBER: _ClassVar[int]
    task_id: str
    ts: str
    proposal: Proposal
    consensus: Consensus
    handoff: Handoff
    hitl: HitlEscalation
    step: StepResult
    error: str
    def __init__(self, task_id: _Optional[str] = ..., ts: _Optional[str] = ..., proposal: _Optional[_Union[Proposal, _Mapping]] = ..., consensus: _Optional[_Union[Consensus, _Mapping]] = ..., handoff: _Optional[_Union[Handoff, _Mapping]] = ..., hitl: _Optional[_Union[HitlEscalation, _Mapping]] = ..., step: _Optional[_Union[StepResult, _Mapping]] = ..., error: _Optional[str] = ...) -> None: ...

class Proposal(_message.Message):
    __slots__ = ("agent", "confidence", "content_json")
    AGENT_FIELD_NUMBER: _ClassVar[int]
    CONFIDENCE_FIELD_NUMBER: _ClassVar[int]
    CONTENT_JSON_FIELD_NUMBER: _ClassVar[int]
    agent: str
    confidence: float
    content_json: str
    def __init__(self, agent: _Optional[str] = ..., confidence: _Optional[float] = ..., content_json: _Optional[str] = ...) -> None: ...

class Consensus(_message.Message):
    __slots__ = ("decision_maker", "strength", "decision_json", "requires_human")
    DECISION_MAKER_FIELD_NUMBER: _ClassVar[int]
    STRENGTH_FIELD_NUMBER: _ClassVar[int]
    DECISION_JSON_FIELD_NUMBER: _ClassVar[int]
    REQUIRES_HUMAN_FIELD_NUMBER: _ClassVar[int]
    decision_maker: str
    strength: float
    decision_json: str
    requires_human: bool
    def __init__(self, decision_maker: _Optional[str] = ..., strength: _Optional[float] = ..., decision_json: _Optional[str] = ..., requires_human: _Optional[bool] = ...) -> None: ...

class Handoff(_message.Message):
    __slots__ = ("phase", "from_agent", "to_agent", "contract", "payload_digest")
    class Phase(int, metaclass=_enum_type_wrapper.EnumTypeWrapper):
        __slots__ = ()
        PHASE_UNSPECIFIED: _ClassVar[Handoff.Phase]
        START: _ClassVar[Handoff.Phase]
        ACK: _ClassVar[Handoff.Phase]
        COMPLETE: _ClassVar[Handoff.Phase]
        ERROR: _ClassVar[Handoff.Phase]
    PHASE_UNSPECIFIED: Handoff.Phase
    START: Handoff.Phase
    ACK: Handoff.Phase
    COMPLETE: Handoff.Phase
    ERROR: Handoff.Phase
    PHASE_FIELD_NUMBER: _ClassVar[int]
    FROM_AGENT_FIELD_NUMBER: _ClassVar[int]
    TO_AGENT_FIELD_NUMBER: _ClassVar[int]
    CONTRACT_FIELD_NUMBER: _ClassVar[int]
    PAYLOAD_DIGEST_FIELD_NUMBER: _ClassVar[int]
    phase: Handoff.Phase
    from_agent: str
    to_agent: str
    contract: str
    payload_digest: str
    def __init__(self, phase: _Optional[_Union[Handoff.Phase, str]] = ..., from_agent: _Optional[str] = ..., to_agent: _Optional[str] = ..., contract: _Optional[str] = ..., payload_digest: _Optional[str] = ...) -> None: ...

class HitlEscalation(_message.Message):
    __slots__ = ("reason", "confidence")
    REASON_FIELD_NUMBER: _ClassVar[int]
    CONFIDENCE_FIELD_NUMBER: _ClassVar[int]
    reason: str
    confidence: float
    def __init__(self, reason: _Optional[str] = ..., confidence: _Optional[float] = ...) -> None: ...

class StepResult(_message.Message):
    __slots__ = ("step_id", "success", "summary")
    STEP_ID_FIELD_NUMBER: _ClassVar[int]
    SUCCESS_FIELD_NUMBER: _ClassVar[int]
    SUMMARY_FIELD_NUMBER: _ClassVar[int]
    step_id: str
    success: bool
    summary: str
    def __init__(self, step_id: _Optional[str] = ..., success: _Optional[bool] = ..., summary: _Optional[str] = ...) -> None: ...

class HealthRequest(_message.Message):
    __slots__ = ()
    def __init__(self) -> None: ...

class HealthResponse(_message.Message):
    __slots__ = ("ready", "version")
    READY_FIELD_NUMBER: _ClassVar[int]
    VERSION_FIELD_NUMBER: _ClassVar[int]
    ready: bool
    version: str
    def __init__(self, ready: _Optional[bool] = ..., version: _Optional[str] = ...) -> None: ...
