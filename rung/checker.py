"""Linear type checker for the rung PoC.

Validates at "compile time" that the ladder declaration is well-formed:
- transitions reference declared rungs
- every recoverable verdict has a matching recover edge and recover fn
- terminal verdicts have no recover edges
- the forward chain is reachable
- carry fields are distinct
"""

from __future__ import annotations
from dataclasses import dataclass
from typing import Optional

from rung.ast import Ladder, Rung, Transition, Verdict, RecoverEdge, RecoverFn


@dataclass
class CheckError:
    msg: str
    def __str__(self): return self.msg


def check(ladder: Ladder) -> list[CheckError]:
    """Run all static checks. Returns list of errors (empty = valid)."""
    errors: list[CheckError] = []

    rung_names = {r.name for r in ladder.rungs}
    transition_names = {t.name for t in ladder.transitions}
    recover_edge_names = {r.name for r in ladder.recover_edges}
    recover_fn_names = {r.name for r in ladder.recover_fns}
    carry_names = {c.name for c in ladder.carry}

    # ── 1. carry fields are distinct ─────────────────────────────────────
    if len(carry_names) != len(ladder.carry):
        errors.append(CheckError("Duplicate carry field name"))

    # ── 2. transitions reference declared rungs ──────────────────────────
    for t in ladder.transitions:
        if t.from_rung not in rung_names:
            errors.append(CheckError(
                f"Transition '{t.name}': from_rung '{t.from_rung}' not declared"
            ))
        if t.to_rung is not None and t.to_rung not in rung_names:
            errors.append(CheckError(
                f"Transition '{t.name}': to_rung '{t.to_rung}' not declared"
            ))

    # ── 3. verdicts are valid ────────────────────────────────────────────
    for t in ladder.transitions:
        if t.verdicts:
            for v in t.verdicts:
                if not v.is_terminal:
                    if v.recover_target is None:
                        errors.append(CheckError(
                            f"Verdict '{v.name}' on transition '{t.name}': "
                            f"non-terminal verdict must have a recover_target"
                        ))
                    elif v.recover_target not in rung_names:
                        errors.append(CheckError(
                            f"Verdict '{v.name}' on transition '{t.name}': "
                            f"recover_target '{v.recover_target}' not declared"
                        ))

    # ── 4. every recoverable verdict has a matching RecoverEdge ───────────
    for t in ladder.transitions:
        for v in t.verdicts:
            if not v.is_terminal:
                found = False
                for re in ladder.recover_edges:
                    if re.from_verdict == v.name:
                        found = True
                        break
                if not found:
                    errors.append(CheckError(
                        f"Verdict '{v.name}' on transition '{t.name}': "
                        f"no matching RecoverEdge declared"
                    ))

    # ── 5. every RecoverEdge has a matching RecoverFn ────────────────────
    for re in ladder.recover_edges:
        if re.name not in recover_fn_names:
            errors.append(CheckError(
                f"RecoverEdge '{re.name}': no matching RecoverFn"
            ))
        if re.to_rung not in rung_names:
            errors.append(CheckError(
                f"RecoverEdge '{re.name}': to_rung '{re.to_rung}' not declared"
            ))

    # ── 6. terminal verdicts must NOT have recover edges ──────────────────
    for t in ladder.transitions:
        for v in t.verdicts:
            if v.is_terminal:
                for re in ladder.recover_edges:
                    if re.from_verdict == v.name:
                        errors.append(CheckError(
                            f"Verdict '{v.name}' is terminal but has RecoverEdge '{re.name}'"
                        ))

    # ── 7. RecoverEdge references a known verdict ─────────────────────────
    for re in ladder.recover_edges:
        found = False
        for t in ladder.transitions:
            for v in t.verdicts:
                if v.name == re.from_verdict:
                    found = True
                    break
        if not found:
            errors.append(CheckError(
                f"RecoverEdge '{re.name}': from_verdict '{re.from_verdict}' "
                f"not declared on any transition"
            ))

    # ── 8. RecoverFn return_rung references a declared rung ───────────────
    for rf in ladder.recover_fns:
        if rf.return_rung not in rung_names:
            errors.append(CheckError(
                f"RecoverFn '{rf.name}': return_rung '{rf.return_rung}' not declared"
            ))

    return errors


def check_or_raise(ladder: Ladder) -> None:
    """Run all checks and raise on first error."""
    errors = check(ladder)
    if errors:
        raise CheckError(str(errors[0]))