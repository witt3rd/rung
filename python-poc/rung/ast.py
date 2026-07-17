"""AST nodes for the rung PoC — first-class type ladders with linear state tokens."""

from __future__ import annotations
from dataclasses import dataclass, field
from typing import Optional


# ── types ────────────────────────────────────────────────────────────────────

@dataclass(frozen=True)
class Type:
    """A type reference. Simple for PoC; richer later."""
    name: str


@dataclass(frozen=True)
class Param:
    name: str
    ty: Type


# ── verdicts ─────────────────────────────────────────────────────────────────

@dataclass(frozen=True)
class Verdict:
    """A branch on the final rung.

    is_terminal=True  → no outgoing edges (Converged, BudgetExhausted)
    is_terminal=False → has a declared re-entry edge (Stalled => Active)
    """
    name: str
    is_terminal: bool
    recover_target: Optional[str] = None   # rung name, for recoverable verdicts


# ── ladder declaration ───────────────────────────────────────────────────────

@dataclass(frozen=True)
class CarryField:
    name: str
    ty: Type


@dataclass(frozen=True)
class Rung:
    """A named state in the ladder. e.g. Designed(DesignedWork)"""
    name: str
    payload_type: Type


# ── transitions ──────────────────────────────────────────────────────────────

@dataclass(frozen=True)
class Transition:
    """A forward transition consuming one rung and producing the next.

    If to_rung is None, this is a branching transition that produces
    a Result type with multiple verdicts.
    """
    name: str
    from_rung: str
    to_rung: Optional[str]        # None if branching
    verdicts: list[Verdict] = field(default_factory=list)
    params: list[Param] = field(default_factory=list)
    # body: Expr — deferred to interpreter; transitions are externally defined


# ── recovery ─────────────────────────────────────────────────────────────────

@dataclass(frozen=True)
class RecoverEdge:
    """A declared re-entry edge from a verdict or failure to a rung."""
    name: str
    from_verdict: str              # or "Failed" for failure recovery
    to_rung: str


@dataclass(frozen=True)
class RecoverFn:
    """An implementation of a recover edge."""
    name: str                      # matches RecoverEdge.name
    param_type: Type
    return_rung: str
    # body: Expr — deferred to interpreter


# ── top-level ladder ─────────────────────────────────────────────────────────

@dataclass(frozen=True)
class Ladder:
    """A complete ladder declaration."""
    name: str
    carry: list[CarryField] = field(default_factory=list)
    rungs: list[Rung] = field(default_factory=list)
    transitions: list[Transition] = field(default_factory=list)
    recover_edges: list[RecoverEdge] = field(default_factory=list)
    recover_fns: list[RecoverFn] = field(default_factory=list)


# ── runtime state tokens ─────────────────────────────────────────────────────

class LinearError(Exception):
    """Raised when a linear token is misused."""
    pass


@dataclass
class Token:
    """A runtime state token. Mutable (consumed flag is set at runtime)."""
    rung_name: str
    payload: dict
    consumed: bool = False
    trace_id: int = 0

    def consume(self) -> None:
        if self.consumed:
            raise LinearError(f"Token {self.trace_id} ({self.rung_name}) already consumed")
        self.consumed = True

    def ensure_alive(self) -> None:
        if self.consumed:
            raise LinearError(
                f"Token {self.trace_id} ({self.rung_name}) already consumed"
            )


@dataclass
class Failed:
    """Payload for a failed transition — carries the unconsumed token."""
    token: Token
    error: str


@dataclass
class Result:
    """Ok(value) | Err(Failed(...))."""
    ok: Optional[any] = None
    err: Optional[Failed] = None

    @staticmethod
    def Ok(value):
        return Result(ok=value)

    @staticmethod
    def Err(token: Token, error: str):
        return Result(err=Failed(token, error))

    def is_ok(self) -> bool:
        return self.ok is not None

    def is_err(self) -> bool:
        return self.err is not None