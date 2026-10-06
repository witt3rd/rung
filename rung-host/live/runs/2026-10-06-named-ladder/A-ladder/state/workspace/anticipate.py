# Anticipate module skeleton
# Model of what the host will do next based on context

import json
from dataclasses import dataclass, asdict
from typing import List, Optional, Dict, Any
from datetime import datetime, timedelta
import os

@dataclass
class ContextHeader:
    time: str
    model: str
    rung: int
    # additional fields as needed

@dataclass
class CalendarEvent:
    name: str
    time: str  # ISO format

@dataclass
class Prediction:
    action: str
    probability: float
    reason: str

class AnticipateModel:
    def __init__(self, log_file: str = "predictions.jsonl"):
        self.context_headers: List[ContextHeader] = []
        self.calendar_events: List[CalendarEvent] = []
        self.recent_predictions: List[Prediction] = []
        self.log_file = log_file
        # Ensure log file exists
        if not os.path.exists(self.log_file):
            open(self.log_file, 'w').close()
    
    def update_context(self, header: ContextHeader):
        self.context_headers.append(header)
        # keep only recent, e.g., last 10
        if len(self.context_headers) > 10:
            self.context_headers = self.context_headers[-10:]
    
    def update_calendar(self, events: List[CalendarEvent]):
        self.calendar_events = events
    
    def _parse_iso_time(self, time_str: str) -> datetime:
        # Parse ISO format string to datetime
        return datetime.fromisoformat(time_str.replace('Z', '+00:00'))
    
    def _get_imminent_events(self, minutes_ahead: int = 30) -> List[CalendarEvent]:
        """Get calendar events within the next N minutes."""
        now = datetime.utcnow()
        imminent = []
        for event in self.calendar_events:
            event_time = self._parse_iso_time(event.time)
            if 0 <= (event_time - now).total_seconds() <= minutes_ahead * 60:
                imminent.append(event)
        return imminent
    
    def log_prediction(self, prediction: Prediction):
        """Log prediction to jsonl file."""
        self.recent_predictions.append(prediction)
        # keep only recent predictions
        if len(self.recent_predictions) > 10:
            self.recent_predictions = self.recent_predictions[-10:]
        with open(self.log_file, 'a') as f:
            json.dump(asdict(prediction), f)
            f.write('\n')
    
    def predict_next(self) -> Optional[Prediction]:
        """Predict the host's next action based on context and calendar."""
        if not self.context_headers:
            return None
        
        # Check for imminent calendar events
        imminent = self._get_imminent_events(30)  # next 30 minutes
        if imminent:
            # Sort by time
            imminent.sort(key=lambda e: self._parse_iso_time(e.time))
            next_event = imminent[0]
            event_time = self._parse_iso_time(next_event.time)
            minutes_until = (event_time - datetime.utcnow()).total_seconds() / 60
            
            if minutes_until < 5:
                action = f"prepare_for_{next_event.name}"
                probability = 0.9
                reason = f"Calendar event '{next_event.name}' starts in {minutes_until:.0f} minutes"
            else:
                action = f"anticipate_{next_event.name}"
                probability = 0.7
                reason = f"Calendar event '{next_event.name}' in {minutes_until:.0f} minutes"
            
            pred = Prediction(action=action, probability=probability, reason=reason)
            self.log_prediction(pred)
            return pred
        
        # No imminent events, look at context patterns
        last = self.context_headers[-1]
        
        # Simple heuristic: if model just changed, expect continued work on current task
        # In a real system, we'd look at history of model changes
        action = "continue_current_task"
        probability = 0.6
        reason = "No imminent events; likely to continue current focus"
        
        # If we have multiple context headers, check for patterns
        if len(self.context_headers) >= 2:
            # Check if model changed recently
            if self.context_headers[-1].model != self.context_headers[-2].model:
                action = "adjust_to_new_model"
                probability = 0.7
                reason = f"Model changed from {self.context_headers[-2].model} to {self.context_headers[-1].model}; likely adjusting to new capabilities"
            # Check if rung changed
            elif self.context_headers[-1].rung != self.context_headers[-2].rung:
                action = "adjust_to_new_rung"
                probability = 0.65
                reason = f"Rung changed from {self.context_headers[-2].rung} to {self.context_headers[-1].rung}; likely adjusting to new complexity level"
        
        pred = Prediction(action=action, probability=probability, reason=reason)
        self.log_prediction(pred)
        return pred