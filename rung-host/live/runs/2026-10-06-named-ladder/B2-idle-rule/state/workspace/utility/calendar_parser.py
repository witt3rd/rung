#!/usr/bin/env python3
"""
Utility to parse calendar event logs and compute latency statistics.
Expects a log file with lines containing calendar event observations.
Example format from host_model.md:
  cal-checkin: due 00:12:44, fired 00:15:43 (+2m59s)
We'll extract due time, fired time, and latency.
"""

import re
from datetime import datetime, timedelta
import sys

def parse_time(timestr):
    """Parse HH:MM:SS string to seconds since midnight."""
    try:
        h, m, s = map(int, timestr.split(':'))
        return h * 3600 + m * 60 + s
    except ValueError:
        return None

def parse_log_line(line):
    # Match pattern: cal-<name>: due HH:MM:SS, fired HH:MM:SS (+/-?XmYs)
    match = re.search(r'cal-(\w+):\s*due\s*(\d{2}:\d{2}:\d{2}),\s*fired\s*(\d{2}:\d{2}:\d{2})\s*\(\+?(\d+)m(\d+)s\)', line)
    if match:
        name, due_str, fired_str, lat_m, lat_s = match.groups()
        due = parse_time(due_str)
        fired = parse_time(fired_str)
        latency = int(lat_m) * 60 + int(lat_s)
        # Optionally verify latency matches fired - due (could be negative if fired before due? but events are late)
        return {
            'name': name,
            'due': due,
            'fired': fired,
            'latency_sec': latency,
            'raw': line.strip()
        }
    return None

def main(logfile):
    events = []
    with open(logfile, 'r') as f:
        for line in f:
            event = parse_log_line(line)
            if event:
                events.append(event)
    if not events:
        print("No calendar events found in log.")
        return 1
    print(f"Found {len(events)} calendar events:")
    for ev in events:
        print(f"  {ev['name']}: due {ev['due']//3600:02d}:{(ev['due']%3600)//60:02d}:{ev['due']%60:02d}, "
              f"fired {ev['fired']//3600:02d}:{(ev['fired']%3600)//60:02d}:{ev['fired']%60:02d}, "
              f"latency {ev['latency_sec']//60}m{ev['latency_sec']%60}s")
    latencies = [ev['latency_sec'] for ev in events]
    print(f"\nLatency statistics (seconds):")
    print(f"  Min: {min(latencies)}s ({min(latencies)//60}m{min(latencies)%60}s)")
    print(f"  Max: {max(latencies)}s ({max(latencies)//60}m{max(latencies)%60}s)")
    print(f"  Mean: {sum(latencies)/len(latencies):.1f}s ({sum(latencies)/len(latencies)//60:.0f}m{sum(latencies)/len(latencies)%60:.0f}s)")
    return 0

if __name__ == '__main__':
    if len(sys.argv) < 2:
        print("Usage: calendar_parser.py <logfile>")
        sys.exit(1)
    sys.exit(main(sys.argv[1]))