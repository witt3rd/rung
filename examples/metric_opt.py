"""MetricOptimization ladder — the PoC test case.

Spec → Active → Converged | Stalled(=> Active) | BudgetExhausted
With failure injection on step(), Stalled recovery, and provenance trace.
"""

import sys
sys.path.insert(0, "/home/dt/src/witt3rd/rung")

from rung import (
    Ladder, Rung, Transition, Verdict, RecoverEdge, RecoverFn,
    CarryField, Type, Param, Token, Failed, Result, LinearError,
    check_or_raise, check,
)
from rung.interpreter import Interpreter


# ── 1. build the AST ─────────────────────────────────────────────────────────

metric_opt_ladder = Ladder(
    name="MetricOptimization",
    carry=[
        CarryField("metric_name", Type("String")),
        CarryField("correlation_key", Type("UUID")),
    ],
    rungs=[
        Rung("Spec", Type("MetricOptimizationSpec")),
        Rung("Active", Type("ActiveLoop")),
    ],
    transitions=[
        Transition(
            name="design",
            from_rung="Spec",     # not declared — produces Spec
            to_rung="Spec",
            params=[Param("spec", Type("MetricOptimizationSpec"))],
        ),
        Transition(
            name="activate",
            from_rung="Spec",
            to_rung="Active",
        ),
        Transition(
            name="step",
            from_rung="Active",
            to_rung=None,          # branching
            verdicts=[
                Verdict("Converged", is_terminal=True),
                Verdict("Stalled", is_terminal=False, recover_target="Active"),
                Verdict("BudgetExhausted", is_terminal=True),
            ],
        ),
    ],
    recover_edges=[
        RecoverEdge("stalled_to_active", from_verdict="Stalled", to_rung="Active"),
    ],
    recover_fns=[
        RecoverFn("stalled_to_active", param_type=Type("Stalled"), return_rung="Active"),
    ],
)


# ── 2. check it ──────────────────────────────────────────────────────────────

print("── checking ──")
errors = check(metric_opt_ladder)
if errors:
    for e in errors:
        print(f"  ✗ {e}")
    sys.exit(1)
print("  ✓ ladder is well-formed")
print()


# ── 3. transition implementations ────────────────────────────────────────────

def design_impl(spec_dict: dict, carry: dict) -> Token:
    """design: creates the Spec token from raw inputs."""
    return Token(rung_name="Spec", payload={
        "spec": spec_dict,
        "metric_name": carry["metric_name"],
    }, trace_id=0)


def activate_impl(token: Token, carry: dict) -> Result:
    """activate: Spec → Active. Always succeeds in this PoC."""
    return Result.Ok({
        "verdict": "Active",
        "params": token.payload.get("params", {}),
        "iteration": 0,
        "best_metric": None,
        "metric_name": carry["metric_name"],
        "_inject_failure": token.payload.get("_inject_failure", False),
    })


def step_impl(token: Token, carry: dict) -> Result:
    """step: Active → Converged | Stalled | BudgetExhausted | Failed.

    Simulates a metric optimization step with configurable outcomes.
    """
    payload = token.payload
    it = payload["iteration"] + 1

    # ── failure injection: iteration 2 fails ──
    if it == 2 and payload.get("_inject_failure"):
        return Result.Err(token, "transient network error")

    # ── simulate metric improvement ──
    metric = 1.0 - (it * 0.15) + (it * it * 0.005)
    prev_best = payload.get("best_metric")
    improved = prev_best is None or metric > prev_best

    verdict_data = {
        "iteration": it,
        "metric": metric,
        "best_metric": max(metric, prev_best) if prev_best is not None else metric,
        "converged": it >= 8,
        "budget_remaining": 1000 - (it * 100),
    }

    if verdict_data["converged"]:
        return Result.Ok({"verdict": "Converged", **verdict_data})
    elif it >= 3 and not improved:
        return Result.Ok({"verdict": "Stalled", "params": payload["params"],
            "_inject_failure": payload.get("_inject_failure", False),
            **verdict_data, "stagnation": it - 2})
    elif verdict_data["budget_remaining"] <= 0:
        return Result.Ok({"verdict": "BudgetExhausted", **verdict_data})
    else:
        return Result.Ok({
            "verdict": "Active",
            "params": payload["params"],
            "iteration": it,
            "best_metric": verdict_data["best_metric"],
            "metric_name": carry["metric_name"],
            "_inject_failure": payload.get("_inject_failure", False),
        })


def stalled_recover_impl(token: Token, carry: dict) -> Token:
    """stalled_to_active: Stalled → Active (recovery edge)."""
    stalled_data = token.payload
    return Token(rung_name="Active", payload={
        "params": stalled_data.get("params", {}),
        "iteration": stalled_data["iteration"],
        "best_metric": stalled_data["best_metric"],
        "metric_name": carry["metric_name"],
        "_recovered_from_stall": True,
        "_inject_failure": stalled_data.get("_inject_failure", False),
    }, trace_id=0)


# ── 4. run the loop ──────────────────────────────────────────────────────────

def run_optimization(inject_failure: bool = False):
    carry = {"metric_name": "convergence_score", "correlation_key": "550e8400-e29b"}
    interp = Interpreter(carry)

    spec_payload = {
        "params": {"lr": 0.01, "epochs": 20},
        "_inject_failure": inject_failure,
    }

    # design
    spec_token = interp._new_token("", spec_payload)
    spec_token = Token(rung_name="Spec", payload=spec_payload, trace_id=1)
    interp._record("design", "→", f"Spec (trace_id={spec_token.trace_id})")

    # activate
    result = interp.run_transition("activate", spec_token, activate_impl)
    if result.is_err():
        print("✗ activate failed")
        interp.print_trace()
        return None
    ok = result.ok
    token = Token(
        rung_name="Active",
        payload={k: v for k, v in ok.items() if k != "verdict"},
        trace_id=spec_token.trace_id,
    )

    max_iter = 20
    for _ in range(max_iter):
        result = interp.run_transition("step", token, step_impl)

        if result.is_err():
            # step failed — recover
            failed_token = result.err.token
            failed_payload = failed_token.payload
            failed_token = Token(
                rung_name="Failed",
                payload={"error": result.err.error, "original": failed_payload},
                trace_id=failed_token.trace_id,
            )
            # simple retry: create fresh Active from the failed token
            new_payload = failed_payload.copy()
            new_payload["_recovered_from_failure"] = True
            new_payload.pop("_inject_failure", None)  # one-shot — don't re-fire
            new_token = Token(rung_name="Active", payload=new_payload, trace_id=failed_token.trace_id)
            interp._record("recover:step_failed", "→", f"Active (trace_id={new_token.trace_id})")
            token = new_token
            continue

        ok = result.ok
        verdict = ok.get("verdict", "Active")

        if verdict == "Converged":
            print(f"\n  ✓ Converged after {ok['iteration']} iterations (metric={ok['metric']:.4f})")
            break
        elif verdict == "Stalled":
            print(f"  ⚡ Stalled at iteration {ok['iteration']} — recovering")
            stalled_token = Token(rung_name="Stalled", payload=ok, trace_id=token.trace_id)
            token = interp.run_recover("stalled_to_active", stalled_token, stalled_recover_impl)
            continue
        elif verdict == "BudgetExhausted":
            print(f"\n  ✗ Budget exhausted at iteration {ok['iteration']}")
            break
        else:
            token = Token(rung_name="Active", payload=ok)

    interp.print_trace()
    return interp


# ── 5. run both cases ────────────────────────────────────────────────────────

if __name__ == "__main__":
    print("══════════════════════════════════════════════════")
    print("  rung — MetricOptimization ladder (happy path)")
    print("══════════════════════════════════════════════════")
    run_optimization(inject_failure=False)

    print()
    print("══════════════════════════════════════════════════")
    print("  rung — MetricOptimization ladder (with failure)")
    print("══════════════════════════════════════════════════")
    run_optimization(inject_failure=True)