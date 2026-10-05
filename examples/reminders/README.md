A reminder agent: an owner calendar entry in `reminders.yaml` fires into the host, and the agent answers it. `examples/reminders/run.sh` runs on the host's mock engine (no key); for live, set `engine.kind: agent` with `base_url` and `api_key_env`.
Run in CI, so it cannot rot.
