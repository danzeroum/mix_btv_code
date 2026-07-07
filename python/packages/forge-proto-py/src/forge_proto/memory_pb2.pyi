from google.protobuf.internal import containers as _containers
from google.protobuf import descriptor as _descriptor
from google.protobuf import message as _message
from collections.abc import Iterable as _Iterable, Mapping as _Mapping
from typing import ClassVar as _ClassVar, Optional as _Optional, Union as _Union

DESCRIPTOR: _descriptor.FileDescriptor

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

class RecallRequest(_message.Message):
    __slots__ = ("query", "k")
    QUERY_FIELD_NUMBER: _ClassVar[int]
    K_FIELD_NUMBER: _ClassVar[int]
    query: str
    k: int
    def __init__(self, query: _Optional[str] = ..., k: _Optional[int] = ...) -> None: ...

class MemoryMatch(_message.Message):
    __slots__ = ("id", "agent", "decision_json", "timestamp", "score")
    ID_FIELD_NUMBER: _ClassVar[int]
    AGENT_FIELD_NUMBER: _ClassVar[int]
    DECISION_JSON_FIELD_NUMBER: _ClassVar[int]
    TIMESTAMP_FIELD_NUMBER: _ClassVar[int]
    SCORE_FIELD_NUMBER: _ClassVar[int]
    id: str
    agent: str
    decision_json: str
    timestamp: str
    score: float
    def __init__(self, id: _Optional[str] = ..., agent: _Optional[str] = ..., decision_json: _Optional[str] = ..., timestamp: _Optional[str] = ..., score: _Optional[float] = ...) -> None: ...

class RecallResponse(_message.Message):
    __slots__ = ("matches",)
    MATCHES_FIELD_NUMBER: _ClassVar[int]
    matches: _containers.RepeatedCompositeFieldContainer[MemoryMatch]
    def __init__(self, matches: _Optional[_Iterable[_Union[MemoryMatch, _Mapping]]] = ...) -> None: ...

class ListRequest(_message.Message):
    __slots__ = ("agent", "limit")
    AGENT_FIELD_NUMBER: _ClassVar[int]
    LIMIT_FIELD_NUMBER: _ClassVar[int]
    agent: str
    limit: int
    def __init__(self, agent: _Optional[str] = ..., limit: _Optional[int] = ...) -> None: ...

class MemorySummary(_message.Message):
    __slots__ = ("agent", "count", "latest_decision_json", "latest_timestamp", "top_confidence")
    AGENT_FIELD_NUMBER: _ClassVar[int]
    COUNT_FIELD_NUMBER: _ClassVar[int]
    LATEST_DECISION_JSON_FIELD_NUMBER: _ClassVar[int]
    LATEST_TIMESTAMP_FIELD_NUMBER: _ClassVar[int]
    TOP_CONFIDENCE_FIELD_NUMBER: _ClassVar[int]
    agent: str
    count: int
    latest_decision_json: str
    latest_timestamp: str
    top_confidence: float
    def __init__(self, agent: _Optional[str] = ..., count: _Optional[int] = ..., latest_decision_json: _Optional[str] = ..., latest_timestamp: _Optional[str] = ..., top_confidence: _Optional[float] = ...) -> None: ...

class ListResponse(_message.Message):
    __slots__ = ("agents",)
    AGENTS_FIELD_NUMBER: _ClassVar[int]
    agents: _containers.RepeatedCompositeFieldContainer[MemorySummary]
    def __init__(self, agents: _Optional[_Iterable[_Union[MemorySummary, _Mapping]]] = ...) -> None: ...
