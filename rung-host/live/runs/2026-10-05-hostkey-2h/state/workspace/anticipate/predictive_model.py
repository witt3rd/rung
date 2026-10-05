#!/usr/bin/env python3
"""
Anticipate Predictive Model
===========================
A forecasting function for host behavior based on 69-turn observational data
across 3 epochs (~63 minutes).

Encodes the boundary-type taxonomy and all confirmed patterns.
"""

from dataclasses import dataclass
from enum import Enum
from typing import Optional, List, Dict, Tuple
import json


class BoundaryType(Enum):
    """The four fundamental boundary types that drive all state changes."""
    FREE_TIME = "free_time"           # No commit, no rapid sequence
    RAPID_HEARTBEAT = "rapid_heartbeat"  # Turns ≤~5s apart
    COMMIT = "commit"                 # `commit` called
    WANT_TOOLS = "want_tools"         # `want_tools` requested (granted next boundary)
    EPOCH = "epoch"                   # Epoch rollover (hard reset)


class ToolGroup(Enum):
    CORE = "core"
    MEMORY = "memory"
    READ = "read"
    WORKSPACE_WRITE = "workspace_write"


BASE_TOOLS = {ToolGroup.CORE, ToolGroup.MEMORY, ToolGroup.READ}
ALL_TOOLS = BASE_TOOLS | {ToolGroup.WORKSPACE_WRITE}


@dataclass
class HostState:
    """Current observable host state at a turn boundary."""
    tools: set[ToolGroup]
    quota: int
    admitted_stimuli: List[str]  # Simplified: just identifiers
    time_to_next_calendar: Optional[int]  # seconds, None if no pending calendar
    epoch: int
    turn: int
    model: str
    has_commitment: bool
    last_turn_interval: Optional[float]  # seconds since previous turn


@dataclass
class Prediction:
    """Predicted next state after a boundary transition."""
    next_tools: set[ToolGroup]
    quota_delta: int
    stimulus_probability: float
    stimulus_type: Optional[str]  # "calendar" | "owner" | None
    boundary_type: BoundaryType
    confidence: float


# Quota burn patterns by boundary type (empirically observed)
QUOTA_DELTA = {
    BoundaryType.FREE_TIME: (-6, -5),      # -5 to -6
    BoundaryType.RAPID_HEARTBEAT: (-1, 0),  # 0 to -1
    BoundaryType.COMMIT: (-5, -2),         # -2 to -5
    BoundaryType.WANT_TOOLS: (-6, -6),     # -6 (when combined with commit)
    BoundaryType.EPOCH: (1, 1),            # +1 regeneration
}

# Tool transition rules
TOOL_TRANSITIONS = {
    BoundaryType.FREE_TIME: BASE_TOOLS,
    BoundaryType.RAPID_HEARTBEAT: None,  # Persists current
    BoundaryType.COMMIT: ALL_TOOLS,
    BoundaryType.WANT_TOOLS: ALL_TOOLS,  # Next boundary
    BoundaryType.EPOCH: BASE_TOOLS,
}


def classify_boundary(state: HostState, action: Optional[str] = None) -> BoundaryType:
    """
    Classify the boundary type from state and action.
    
    The boundary type is the primary predictor of all state changes.
    """
    # Epoch boundary takes precedence (but we only detect it retrospectively)
    # In practice, we predict based on what we *choose* to do.
    
    if action == "commit":
        return BoundaryType.COMMIT
    elif action == "want_tools":
        return BoundaryType.WANT_TOOLS
    elif state.last_turn_interval is not None and state.last_turn_interval <= 5.0:
        return BoundaryType.RAPID_HEARTBEAT
    else:
        return BoundaryType.FREE_TIME


def predict_tools(state: HostState, boundary: BoundaryType) -> set[ToolGroup]:
    """Predict next tool set from boundary type."""
    if boundary == BoundaryType.RAPID_HEARTBEAT:
        return state.tools.copy()  # Persists
    return TOOL_TRANSITIONS[boundary].copy()


def predict_quota_delta(boundary: BoundaryType, has_want_tools_pending: bool = False) -> Tuple[int, int]:
    """Return (min_delta, max_delta) for quota change."""
    if has_want_tools_pending and boundary == BoundaryType.COMMIT:
        return QUOTA_DELTA[BoundaryType.WANT_TOOLS]
    return QUOTA_DELTA[boundary]


def predict_stimulus(state: HostState, boundary: BoundaryType) -> Tuple[float, Optional[str]]:
    """
    Predict stimulus probability and type.
    
    Calendar stimuli arrive at turn boundaries near their scheduled time.
    Owner messages arrive irregularly.
    """
    prob = 0.0
    stim_type = None
    
    # Calendar stimulus prediction
    if state.time_to_next_calendar is not None:
        if state.time_to_next_calendar <= 60:  # Within 1 minute
            prob = 0.9
            stim_type = "calendar"
        elif state.time_to_next_calendar <= 300:  # Within 5 minutes
            prob = 0.5
            stim_type = "calendar"
        elif state.time_to_next_calendar <= 600:  # Within 10 minutes
            prob = 0.2
            stim_type = "calendar"
    
    # Owner message baseline (observed ~1 per ~15 turns in active periods)
    if boundary != BoundaryType.RAPID_HEARTBEAT:
        prob = max(prob, 0.05)
        if stim_type is None:
            stim_type = "owner"
    
    # Rapid heartbeat: near-zero stimulus probability
    if boundary == BoundaryType.RAPID_HEARTBEAT:
        prob = min(prob, 0.01)
    
    return prob, stim_type


def predict_epoch_transition(state: HostState) -> Prediction:
    """Special handling for epoch boundary (hard reset)."""
    return Prediction(
        next_tools=BASE_TOOLS,
        quota_delta=1,
        stimulus_probability=0.0,  # All cleared
        stimulus_type=None,
        boundary_type=BoundaryType.EPOCH,
        confidence=0.95
    )


def forecast(state: HostState, action: Optional[str] = None) -> Prediction:
    """
    Main forecasting function.
    
    Args:
        state: Current host state at turn boundary
        action: Planned action ("commit", "want_tools", or None for free time)
    
    Returns:
        Prediction for the next turn boundary state
    """
    boundary = classify_boundary(state, action)
    
    # Check for epoch boundary (heuristic: quota near 0 or turn count high)
    # In practice, epoch boundaries are opaque until they happen.
    # We don't predict them prospectively.
    
    next_tools = predict_tools(state, boundary)
    quota_min, quota_max = predict_quota_delta(boundary)
    quota_delta = (quota_min + quota_max) // 2  # Expected value
    
    stim_prob, stim_type = predict_stimulus(state, boundary)
    
    # Confidence based on boundary type (most reliable predictor)
    confidence = {
        BoundaryType.FREE_TIME: 0.9,
        BoundaryType.RAPID_HEARTBEAT: 0.95,
        BoundaryType.COMMIT: 0.9,
        BoundaryType.WANT_TOOLS: 0.85,
        BoundaryType.EPOCH: 0.95,
    }[boundary]
    
    return Prediction(
        next_tools=next_tools,
        quota_delta=quota_delta,
        stimulus_probability=stim_prob,
        stimulus_type=stim_type,
        boundary_type=boundary,
        confidence=confidence
    )


def validate_against_log(predictions: List[Tuple[HostState, Optional[str], Prediction]], 
                          actual_outcomes: List[Dict]) -> Dict:
    """
    Validate predictions against actual observed outcomes.
    Returns accuracy metrics.
    """
    correct_tools = 0
    correct_quota_direction = 0  # Sign of delta
    correct_stimulus_presence = 0
    total = len(predictions)
    
    for (state, action, pred), actual in zip(predictions, actual_outcomes):
        # Tools
        if pred.next_tools == actual["tools"]:
            correct_tools += 1
        
        # Quota direction (positive/negative/zero)
        pred_sign = 1 if pred.quota_delta > 0 else (-1 if pred.quota_delta < 0 else 0)
        actual_sign = 1 if actual["quota_delta"] > 0 else (-1 if actual["quota_delta"] < 0 else 0)
        if pred_sign == actual_sign:
            correct_quota_direction += 1
        
        # Stimulus presence
        pred_has_stim = pred.stimulus_probability > 0.5
        actual_has_stim = actual.get("stimulus_delivered", False)
        if pred_has_stim == actual_has_stim:
            correct_stimulus_presence += 1
    
    return {
        "tool_accuracy": correct_tools / total if total > 0 else 0,
        "quota_direction_accuracy": correct_quota_direction / total if total > 0 else 0,
        "stimulus_presence_accuracy": correct_stimulus_presence / total if total > 0 else 0,
        "total_validated": total
    }


# Example usage: reconstruct turn 69 state and forecast turn 70
if __name__ == "__main__":
    # Turn 69 state (epoch 3, rapid heartbeat)
    state_69 = HostState(
        tools=ALL_TOOLS,
        quota=270,
        admitted_stimuli=["cal-hour", "o08-tidy"],
        time_to_next_calendar=None,  # All 4 delivered
        epoch=3,
        turn=69,
        model="nvidia/nemotron-3-ultra-550b-a55b:free",
        has_commitment=True,
        last_turn_interval=1.0  # ~1-2 min intervals in rapid heartbeat
    )
    
    # Forecast turn 70 (free time, no commit)
    pred_70 = forecast(state_69, action=None)
    print("Forecast turn 70 (free time):")
    print(f"  Next tools: {[t.value for t in pred_70.next_tools]}")
    print(f"  Quota delta: {pred_70.quota_delta}")
    print(f"  Stimulus prob: {pred_70.stimulus_probability}")
    print(f"  Stimulus type: {pred_70.stimulus_type}")
    print(f"  Boundary type: {pred_70.boundary_type.value}")
    print(f"  Confidence: {pred_70.confidence}")
    
    # Actual turn 70: tools=BASE_TOOLS, quota=-5, stimuli cleared
    print("\nActual turn 70:")
    print(f"  Next tools: {[t.value for t in BASE_TOOLS]}")
    print(f"  Quota delta: -5")
    print(f"  Stimuli: cleared")
    print(f"  Boundary type: free_time (epoch transition)")
    
    # Forecast turn 71 (commit)
    state_70 = HostState(
        tools=BASE_TOOLS,
        quota=265,
        admitted_stimuli=[],
        time_to_next_calendar=None,
        epoch=4,
        turn=70,
        model="nvidia/nemotron-3-ultra-550b-a55b:free",
        has_commitment=True,
        last_turn_interval=39.0  # ~39s since turn 69
    )
    
    pred_71 = forecast(state_70, action="commit")
    print("\nForecast turn 71 (commit):")
    print(f"  Next tools: {[t.value for t in pred_71.next_tools]}")
    print(f"  Quota delta: {pred_71.quota_delta}")
    print(f"  Boundary type: {pred_71.boundary_type.value}")
    print(f"  Confidence: {pred_71.confidence}")