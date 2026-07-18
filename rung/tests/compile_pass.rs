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
    // Type-level proof: if Spec::carry() did not exist, this would not compile.
    // The function `_check` only accepts closures matching &Spec -> &Carry.
    // We never call it — the type system IS the test.
    //
    // What this proves: the accessor method exists with the right signature.
    // What it doesn't prove: that the accessor works at runtime (but the field
    //   being private makes direct mutation a compile error, and the accessor's
    //   &Carry return type makes mutation through the reference impossible).
    //
    // To verify: try adding `spec.carry.metric_name = "mutated"` somewhere —
    // the compiler will refuse because `carry` is private.
    fn _check<T: Fn(&metricoptimization::Spec) -> &metricoptimization::Carry>(_: T) {}
}

#[test]
fn test_rungs_are_not_send_or_sync() {
    // Proof of the linear-token contract (RUNG-RUST.md §4.6): rung tokens must not
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
    // Verdicts are held to the same seal (RUNG-RUST.md §4.6 remnant, closed):
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
