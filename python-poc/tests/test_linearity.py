"""Tests for the rung linear type checker.

Verifies the compiler's "cryptographic signature" property:
invalid ladders are rejected at check time, not discovered at runtime.
"""

import sys
sys.path.insert(0, "/home/dt/src/witt3rd/rung")

import pytest  # type: ignore
from rung import (
    Ladder, Rung, Transition, Verdict, RecoverEdge, RecoverFn,
    CarryField, Type, check,
)


def test_valid_ladder():
    """The MetricOptimization ladder passes all checks."""
    ladder = Ladder(
        name="Test",
        carry=[CarryField("task_id", Type("String"))],
        rungs=[
            Rung("Spec", Type("Spec")),
            Rung("Active", Type("Active")),
        ],
        transitions=[
            Transition("activate", from_rung="Spec", to_rung="Active"),
            Transition("step", from_rung="Active", to_rung=None, verdicts=[
                Verdict("Converged", is_terminal=True),
                Verdict("Stalled", is_terminal=False, recover_target="Active"),
            ]),
        ],
        recover_edges=[
            RecoverEdge("stalled_to_active", from_verdict="Stalled", to_rung="Active"),
        ],
        recover_fns=[
            RecoverFn("stalled_to_active", param_type=Type("Stalled"), return_rung="Active"),
        ],
    )
    assert check(ladder) == []


def test_transition_from_undeclared_rung():
    """Transition references a rung that was never declared."""
    ladder = Ladder(
        name="Bad",
        rungs=[Rung("Spec", Type("Spec"))],
        transitions=[
            Transition("step", from_rung="Missing", to_rung="Spec"),
        ],
    )
    errors = check(ladder)
    assert len(errors) == 1
    assert "from_rung 'Missing' not declared" in str(errors[0])


def test_transition_to_undeclared_rung():
    """Transition's to_rung references an undeclared rung."""
    ladder = Ladder(
        name="Bad",
        rungs=[Rung("Spec", Type("Spec"))],
        transitions=[
            Transition("activate", from_rung="Spec", to_rung="Missing"),
        ],
    )
    errors = check(ladder)
    assert len(errors) == 1
    assert "to_rung 'Missing' not declared" in str(errors[0])


def test_recoverable_verdict_missing_recover_edge():
    """Recoverable verdict has no matching RecoverEdge."""
    ladder = Ladder(
        name="Bad",
        rungs=[Rung("Active", Type("Active"))],
        transitions=[
            Transition("step", from_rung="Active", to_rung=None, verdicts=[
                Verdict("Stalled", is_terminal=False, recover_target="Active"),
            ]),
        ],
    )
    errors = check(ladder)
    assert len(errors) == 1
    assert "no matching RecoverEdge" in str(errors[0])


def test_recoverable_verdict_missing_recover_target():
    """Recoverable verdict lacks a recover_target."""
    ladder = Ladder(
        name="Bad",
        rungs=[Rung("Active", Type("Active"))],
        transitions=[
            Transition("step", from_rung="Active", to_rung=None, verdicts=[
                Verdict("Stalled", is_terminal=False, recover_target=None),
            ]),
        ],
    )
    errors = check(ladder)
    assert len(errors) == 2  # both "must have a recover_target" AND "no matching RecoverEdge"
    assert any("must have a recover_target" in str(e) for e in errors)


def test_recover_edge_missing_fn():
    """RecoverEdge has no matching RecoverFn."""
    ladder = Ladder(
        name="Bad",
        rungs=[Rung("Active", Type("Active"))],
        transitions=[
            Transition("step", from_rung="Active", to_rung=None, verdicts=[
                Verdict("Stalled", is_terminal=False, recover_target="Active"),
            ]),
        ],
        recover_edges=[
            RecoverEdge("missing_fn", from_verdict="Stalled", to_rung="Active"),
        ],
    )
    errors = check(ladder)
    assert len(errors) == 1
    assert "no matching RecoverFn" in str(errors[0])


def test_terminal_verdict_must_not_have_recover_edge():
    """A terminal verdict should not have any RecoverEdge pointing at it."""
    ladder = Ladder(
        name="Bad",
        rungs=[Rung("Active", Type("Active"))],
        transitions=[
            Transition("step", from_rung="Active", to_rung=None, verdicts=[
                Verdict("Converged", is_terminal=True),
            ]),
        ],
        recover_edges=[
            RecoverEdge("bad_recover", from_verdict="Converged", to_rung="Active"),
        ],
        recover_fns=[
            RecoverFn("bad_recover", param_type=Type("Converged"), return_rung="Active"),
        ],
    )
    errors = check(ladder)
    assert len(errors) == 1
    assert "is terminal but has RecoverEdge" in str(errors[0])


def test_recover_edge_references_nonexistent_verdict():
    """RecoverEdge's from_verdict doesn't match any declared verdict."""
    ladder = Ladder(
        name="Bad",
        rungs=[Rung("Active", Type("Active"))],
        transitions=[
            Transition("step", from_rung="Active", to_rung=None, verdicts=[
                Verdict("Converged", is_terminal=True),
            ]),
        ],
        recover_edges=[
            RecoverEdge("phantom", from_verdict="Stalled", to_rung="Active"),
        ],
        recover_fns=[
            RecoverFn("phantom", param_type=Type("Stalled"), return_rung="Active"),
        ],
    )
    errors = check(ladder)
    assert len(errors) == 1  # "from_verdict 'Stalled' not declared on any transition"
    assert any("from_verdict 'Stalled' not declared" in str(e) for e in errors)


def test_recover_fn_return_rung_undeclared():
    """RecoverFn references a return_rung that doesn't exist."""
    ladder = Ladder(
        name="Bad",
        rungs=[Rung("Active", Type("Active"))],
        transitions=[
            Transition("step", from_rung="Active", to_rung=None, verdicts=[
                Verdict("Stalled", is_terminal=False, recover_target="Active"),
            ]),
        ],
        recover_edges=[
            RecoverEdge("s2a", from_verdict="Stalled", to_rung="Active"),
        ],
        recover_fns=[
            RecoverFn("s2a", param_type=Type("Stalled"), return_rung="Missing"),
        ],
    )
    errors = check(ladder)
    assert len(errors) == 1
    assert "return_rung 'Missing' not declared" in str(errors[0])


def test_recover_edge_to_undeclared_rung():
    """RecoverEdge's to_rung references a rung that doesn't exist."""
    ladder = Ladder(
        name="Bad",
        rungs=[Rung("Active", Type("Active"))],
        transitions=[
            Transition("step", from_rung="Active", to_rung=None, verdicts=[
                Verdict("Stalled", is_terminal=False, recover_target="Active"),
            ]),
        ],
        recover_edges=[
            RecoverEdge("s2a", from_verdict="Stalled", to_rung="Missing"),
        ],
        recover_fns=[
            RecoverFn("s2a", param_type=Type("Stalled"), return_rung="Missing"),
        ],
    )
    errors = check(ladder)
    assert len(errors) == 2  # both RecoverEdge.to_rung AND RecoverFn.return_rung reference "Missing"
    assert any("to_rung 'Missing' not declared" in str(e) for e in errors)


def test_duplicate_carry_field():
    """Duplicate carry field names are rejected."""
    ladder = Ladder(
        name="Bad",
        carry=[
            CarryField("task_id", Type("String")),
            CarryField("task_id", Type("Int")),
        ],
    )
    errors = check(ladder)
    assert len(errors) == 1
    assert "Duplicate carry field" in str(errors[0])