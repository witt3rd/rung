#!/usr/bin/env python3
import subprocess
import sys

result = subprocess.run([sys.executable, 'calendar_parser.py', '../notes/calendar_events.log'], 
                       capture_output=True, text=True, cwd='utility')
print(result.stdout)
if result.stderr:
    print(result.stderr, file=sys.stderr)
sys.exit(result.returncode)