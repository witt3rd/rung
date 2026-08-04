//! Integration test: the MetricOptimization ladder compiles and the types exist.

use rung::ladder;

ladder!(MetricOptimization {
    carry {
        metric_name: String,
        correlation_key: u64,
    }

    Spec(MetricOptimizationSpec)
      => Active(ActiveLoop)
      => {
          Converged
          | Stalled => Active
          | BudgetExhausted
      }

    recover {
        stalled: Stalled => Active
    }
});

// Verify the generated types exist
#[test]
fn test_module_exists() {
    let carry = metricoptimization::Carry {
        metric_name: "test".into(),
        correlation_key: 42,
    };
    // The Carry type exists and can be constructed.
    let _ = carry;
}

#[test]
fn test_carry_accessor_exists() {
    // Type-level proof: if `Spec::carry()` did not exist with this exact
    // signature, the coercion below would not compile.
    //
    // This test used to declare an *uninstantiated* generic helper —
    // `fn _check<T: Fn(&Spec) -> &Carry>(_: T) {}` — and never call it. Nothing
    // ever selected `T`, so the bound was never solved and renaming the emitted
    // accessor left the test green. It was a refusal test that could not fail
    // (rung-props.md a-refusal-test-that-cannot-fail), found by mutating the
    // macro and watching nothing happen. Naming the method is what makes the
    // compiler resolve it.
    //
    // What this proves: the accessor exists with the right signature.
    // What it doesn't prove: that it works at runtime — but `carry` is a
    // private field, so direct mutation is a compile error, and the `&Carry`
    // return makes mutation through the reference impossible.
    let _accessor: fn(&metricoptimization::Spec) -> &metricoptimization::Carry =
        metricoptimization::Spec::carry;
}

#[test]
fn test_rungs_are_not_send_or_sync() {
    // Proof of the linear-token contract (rung-props.md G3): rung tokens must not
    // cross thread boundaries, or two threads could drive a transition on the same
    // logical token via a shared `Arc`/`&`. The `PhantomData<*const ()>` marker in
    // each generated rung makes it `!Send + !Sync`.
    //
    // Autoref specialization: `IsSend<T>` has an inherent `.check()` returning true
    // (selected only when `T: Send`); the blanket `&IsSend<T>` fallback returns
    // false. `(&IsSend::<T>(..)).check()` resolves to the inherent method iff T is
    // Send, else the fallback. If the marker were ever dropped, these asserts flip.
    use core::marker::PhantomData;
    struct IsSend<T>(PhantomData<T>);
    impl<T: Send> IsSend<T> {
        #[allow(dead_code)]
        fn check(&self) -> bool {
            true
        }
    }
    trait Fallback {
        fn check(&self) -> bool {
            false
        }
    }
    impl<T> Fallback for &IsSend<T> {}

    assert!(
        !(&IsSend::<metricoptimization::Spec>(PhantomData)).check(),
        "Spec must be !Send"
    );
    assert!(
        !(&IsSend::<metricoptimization::Active>(PhantomData)).check(),
        "Active must be !Send"
    );
    // Verdicts are held to the same seal (rung-props.md G3, verdict seal):
    // terminal (`Converged`) and recoverable (`Stalled`, which carries a source
    // rung) are both `!Send`.
    assert!(
        !(&IsSend::<metricoptimization::Converged>(PhantomData)).check(),
        "Converged verdict must be !Send"
    );
    assert!(
        !(&IsSend::<metricoptimization::Stalled>(PhantomData)).check(),
        "Stalled verdict must be !Send"
    );
}

#[test]
fn test_failed_type() {
    // Failed<Prev> is generic over the previous rung type
    type _FailedActive = metricoptimization::Failed<metricoptimization::Active>;
}

#[test]
fn test_verdict_enum() {
    // StepOutcome has the verdict variants
    // We can't construct them directly (sealed), but the type exists
    let _outcome: Option<metricoptimization::StepOutcome> = None;
}

// Minimal payload types so the macro expansion works
struct MetricOptimizationSpec;
struct ActiveLoop;

// ── a type-only declaration emits no transitions (type-only-marker-is-inert) ──
//
// `MetricOptimization` above omits the `impl` block, so rules 9–10 have nothing
// to apply to (body-rules-need-an-impl-block) and no transition functions are
// emitted (emitted-functions). A gate marker on such a declaration therefore
// has nothing to constrain: the marker's only effect is on the *signature* of
// an emitted `fn`, and there is none.
//
// `NotARole` deliberately does **not** implement `rung::Role`. If the marker
// were doing anything at all — emitting a prologue, a guard, or a `Qualified`
// parameter — this declaration would not compile, because `Qualified<R>`
// requires `R: Role`. That it compiles is the inertness, stated as a fact the
// compiler checks rather than as prose.

// `dead_code` is itself part of the evidence: nothing the macro emitted refers
// to `NotARole`, which is what "inert" means.
#[allow(dead_code)]
struct NotARole;
struct DraftPayload;
struct RuledPayload;

ladder!(TypeOnly {
    Draft(DraftPayload)
        => #[judgmental(NotARole)] Ruled(RuledPayload)
        => { Done }
});

#[test]
fn a_marker_on_a_type_only_declaration_is_inert() {
    // The types exist; no transition function does, so there is nothing the
    // marker could have gated.
    let _: Option<typeonly::Draft> = None;
    let _: Option<typeonly::Ruled> = None;
    let _: Option<typeonly::StepOutcome> = None;
}
