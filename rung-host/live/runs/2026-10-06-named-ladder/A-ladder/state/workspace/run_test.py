from anticipate import AnticipateModel, ContextHeader, CalendarEvent
import json
from datetime import datetime

def run_test():
    model = AnticipateModel()
    
    # Current context from turn 5 header (time: 2026-10-05T17:22:49Z)
    current_header = ContextHeader(
        time="2026-10-05T17:22:49Z",
        model="nvidia/nemotron-3-super-120b-a12b:free",
        rung=4
    )
    model.update_context(current_header)
    
    # Load calendar
    with open('calendar.json', 'r') as f:
        cal_data = json.load(f)
    calendar_events = [CalendarEvent(name=item['name'], time=item['time']) for item in cal_data]
    model.update_calendar(calendar_events)
    
    prediction = model.predict_next()
    print(f"Current prediction: {prediction}")
    
    # Let's also check what the imminent event is
    imminent = model._get_imminent_events(30)
    print(f"Imminent events (next 30 min): {[e.name for e in imminent]}")
    if imminent:
        imminent.sort(key=lambda e: model._parse_iso_time(e.time))
        next_event = imminent[0]
        event_time = model._parse_iso_time(next_event.time)
        minutes_until = (event_time - datetime.utcnow()).total_seconds() / 60
        print(f"Next event: {next_event.name} in {minutes_until:.1f} minutes")

if __name__ == "__main__":
    run_test()