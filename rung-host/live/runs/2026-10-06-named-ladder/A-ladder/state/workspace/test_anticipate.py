# Test the anticipate module with current context
from anticipate import AnticipateModel, ContextHeader, CalendarEvent, Prediction
from datetime import datetime, timezone

def test_predict():
    model = AnticipateModel()
    
    # Simulate current context from turn 4 header
    # time: 2026-10-05T17:21:53Z, model: nvidia/nemotron-3-super-120b-a12b:free, rung: 4
    current_header = ContextHeader(
        time="2026-10-05T17:21:53Z",
        model="nvidia/nemotron-3-super-120b-a12b:free",
        rung=4
    )
    model.update_context(current_header)
    
    # Load calendar from calendar.json
    import json
    with open('calendar.json', 'r') as f:
        cal_data = json.load(f)
    calendar_events = [CalendarEvent(name=item['name'], time=item['time']) for item in cal_data]
    model.update_calendar(calendar_events)
    
    # Make prediction
    prediction = model.predict_next()
    print(f"Prediction: {prediction}")
    
    # Also test with a previous context to see model change detection
    print("\n--- Testing model change detection ---")
    model2 = AnticipateModel()
    prev_header = ContextHeader(
        time="2026-10-05T17:20:00Z",
        model="google/gemma-4-31b-it:free",
        rung=4
    )
    model2.update_context(prev_header)
    model2.update_context(current_header)  # model changed
    model2.update_calendar(calendar_events)
    pred2 = model2.predict_next()
    print(f"Prediction after model change: {pred2}")

if __name__ == "__main__":
    test_predict()