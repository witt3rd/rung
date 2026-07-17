"""Runtime interpreter for the rung PoC.

Enforces linear token rules at runtime:
- A consumed token cannot be used again
- Error paths must return the token
- The provenance trace is printed at each step
"""

from __future__ import annotations
from dataclasses import dataclass, field
from typing import Callable, Optional

from rung.ast import Token, Failed, Result, LinearError


@dataclass
class TraceEntry:
    step: int
    transition: str
    rung: str
    outcome: str
    payload: dict = field(default_factory=dict)


class Interpreter:
    """Runs a ladder's transitions against live tokens, enforcing linear rules."""

    def __init__(self, carry: dict):
        self.carry = carry          # immutable witness data
        self.trace: list[TraceEntry] = []
        self.step_counter = 0
        self._next_trace_id = 0

    def _new_token(self, rung_name: str, payload: dict | None = None) -> Token:
        self._next_trace_id += 1
        return Token(
            rung_name=rung_name,
            payload=payload or {},
            trace_id=self._next_trace_id,
        )

    def _record(self, transition: str, rung: str, outcome: str, payload: dict | None = None):
        self.step_counter += 1
        self.trace.append(TraceEntry(
            step=self.step_counter,
            transition=transition,
            rung=rung,
            outcome=outcome,
            payload=payload or {},
        ))

    def run_transition(
        self,
        name: str,
        token: Token,
        fn: Callable[[Token, dict], Result],
    ) -> Result:
        """Run a transition, consuming the input token on success.

        On success: the input token is consumed, a new token (or verdict) is returned.
        On failure: the input token is NOT consumed, returned inside Failed.
        """
        token.ensure_alive()
        result = fn(token, self.carry)

        if result.is_ok():
            token.consume()
            self._record(name, token.rung_name, "→ ok")
        else:
            # token unconsumed — returned inside Failed
            self._record(name, token.rung_name, "→ err", {"error": result.err.error})

        return result

    def run_recover(
        self,
        name: str,
        token: Token,
        fn: Callable[[Token, dict], Token],
    ) -> Token:
        """Run a recover function, producing a fresh token.

        The input token (Failed or recoverable verdict) is consumed.
        """
        token.ensure_alive()
        new_token = fn(token, self.carry)
        token.consume()
        self._record(f"recover:{name}", "→", f"{new_token.rung_name} (trace_id={new_token.trace_id})")
        return new_token

    def print_trace(self):
        """Print the provenance trace."""
        print("\n── provenance trace ──")
        for entry in self.trace:
            extra = ""
            if entry.payload.get("error"):
                extra = f" — {entry.payload['error']}"
            elif entry.payload.get("metric"):
                extra = f" — metric={entry.payload['metric']}"
            print(f"  [{entry.step:02d}] {entry.transition}  {entry.rung}  {entry.outcome}{extra}")
        print("── end trace ──\n")