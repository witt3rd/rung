"""Data structures for the anticipation engine."""
from dataclasses import dataclass, field
from datetime import datetime
from typing import Any
from enum import Enum


class StimulusType(Enum):
    """Types of stimuli that can arrive at turn boundaries."""
    MESSAGE = "message"
    CALENDAR = "calendar"
    EXPECTATION_SETTLED = "expectation_settled"
    MODEL_CHANGE = "model_change"
    QUOTA_CHANGE = "quota_change"
    TOOL_GROUP_CHANGE = "tool_group_change"
    COMMITMENT_CHANGE = "commitment_change"


@dataclass
class ContextHeader:
    """A single context header from the host."""
    epoch: int
    started_at: datetime
    model: str
    rung: int
    quota_left: int
    quota_total: int
    seconds_since_external: int
    note: str | None = None
    commitment: str | None = None
    projects: list[str] = field(default_factory=list)
    open_expectations: int = 0
    todo_open: int = 0
    open_questions: int = 0
    tools_enabled: list[str] = field(default_factory=list)
    calendar_within_2h: list[str] = field(default_factory=list)

    @classmethod
    def from_text(cls, text: str) -> "ContextHeader":
        """Parse a context header from the host's text format."""
        lines = text.strip().split('\n')
        header = cls(
            epoch=0,
            started_at=datetime.now(),
            model="",
            rung=0,
            quota_left=0,
            quota_total=0,
            seconds_since_external=0,
        )
        
        for line in lines:
            line = line.strip()
            if line.startswith('[epoch'):
                # Parse: [epoch 6 · started 2026-10-05T17:23:20Z · model nvidia/nemotron-3-ultra-550b-a55b:free (rung 1)]
                parts = line[1:-1].split(' · ')
                header.epoch = int(parts[0].split()[1])
                header.started_at = datetime.fromisoformat(parts[1].replace('started ', '').replace('Z', '+00:00'))
                model_part = parts[2].replace('model ', '')
                header.model = model_part.split(' (rung')[0]
                header.rung = int(model_part.split('rung ')[1].rstrip(')'))
            elif line.startswith('quota'):
                # Parse: quota 374/400 left
                parts = line.split()
                header.quota_left = int(parts[1].split('/')[0])
                header.quota_total = int(parts[1].split('/')[1])
            elif line.startswith('## Your note'):
                if '(none yet)' not in line:
                    header.note = line.split('## Your note (as of this epoch\'s start)')[1].strip()
            elif line.startswith('commitment:'):
                header.commitment = line.split('commitment:')[1].strip()
            elif line.startswith('projects:'):
                header.projects = [p.strip() for p in line.split('projects:')[1].split(';')]
            elif line.startswith('open expectations:'):
                parts = line.split()
                header.open_expectations = int(parts[2])
                header.todo_open = int(parts[4])
                header.open_questions = int(parts[6])
            elif line.startswith('tools on:'):
                header.tools_enabled = [t.strip() for t in line.split('tools on:')[1].split(',')]
            elif 'since anything external' in line:
                # Parse: 2m23s since anything external
                time_part = line.split('since anything external')[0].strip().split()[-1]
                header.seconds_since_external = parse_duration(time_part)
            elif line.startswith('calendar within 2h:'):
                cal_str = line.split('calendar within 2h:')[1].strip()
                if cal_str:
                    header.calendar_within_2h = [c.strip() for c in cal_str.split(',')]
        
        return header


def parse_duration(text: str) -> int:
    """Parse duration like '2m23s' into seconds."""
    total = 0
    current = ""
    for char in text:
        if char.isdigit():
            current += char
        elif char in ('m', 's', 'h'):
            if current:
                val = int(current)
                if char == 'h':
                    total += val * 3600
                elif char == 'm':
                    total += val * 60
                elif char == 's':
                    total += val
                current = ""
    return total


@dataclass
class CalendarEvent:
    """A calendar event from the host."""
    name: str
    at: datetime
    raw: str

    @classmethod
    def from_text(cls, text: str) -> "CalendarEvent":
        """Parse calendar event from text like 'cal-checkin at 2026-10-05T17:26:48Z'."""
        if ' at ' in text:
            name, at_str = text.split(' at ', 1)
            return cls(
                name=name.strip(),
                at=datetime.fromisoformat(at_str.strip().replace('Z', '+00:00')),
                raw=text.strip()
            )
        return cls(name=text.strip(), at=datetime.now(), raw=text.strip())


@dataclass
class Prediction:
    """A prediction about what the host will do next."""
    stimulus_type: StimulusType
    probability: float
    description: str
    expected_at: datetime | None = None
    confidence: float = 0.5
    reasoning: str = ""


@dataclass
class AnticipationContext:
    """Full context for making predictions."""
    headers: list[ContextHeader] = field(default_factory=list)
    calendar_events: list[CalendarEvent] = field(default_factory=list)
    current_epoch: int = 0
    current_model: str = ""
    current_rung: int = 0
    quota_left: int = 0
    quota_total: int = 0
    seconds_since_external: int = 0
    active_commitment: str | None = None
    active_project: str | None = None
    tools_enabled: list[str] = field(default_factory=list)
    note: str | None = None

    def add_header(self, header: ContextHeader) -> None:
        """Add a context header and update derived state."""
        self.headers.append(header)
        self.current_epoch = header.epoch
        self.current_model = header.model
        self.current_rung = header.rung
        self.quota_left = header.quota_left
        self.quota_total = header.quota_total
        self.seconds_since_external = header.seconds_since_external
        self.active_commitment = header.commitment
        self.tools_enabled = header.tools_enabled
        self.note = header.note
        
        # Parse calendar events
        for cal_text in header.calendar_within_2h:
            self.calendar_events.append(CalendarEvent.from_text(cal_text))

    def upcoming_events(self, within_seconds: int = 7200) -> list[CalendarEvent]:
        """Get calendar events within the given seconds from now."""
        now = datetime.now()
        cutoff = now.timestamp() + within_seconds
        return [e for e in self.calendar_events if e.at.timestamp() <= cutoff]