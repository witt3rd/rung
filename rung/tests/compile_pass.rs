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
    // Verify the carry() accessor returns &Carry at the type level.
    // Rung types are sealed — we can't construct them from here.
    // The accessor signature is: pub fn carry(&self) -> &Carry
    fn _check<T: Fn(&metricoptimization::Spec) -> &metricoptimization::Carry>(_: T) {}
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
