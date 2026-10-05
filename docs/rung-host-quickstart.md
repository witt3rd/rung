# rung-host quickstart

Run the continuous host on the scripted mock engine in five minutes. No
network, no key. The mock answers deterministically, so this shows the
plumbing (inbox, record, stop) rather than model quality. For the design,
see [rung-host.md](rung-host.md).

## 1. Build

```bash
cargo build -p rung-host
```

## 2. A config and a sandbox

`examples/rung-host-mock.yaml` is the whole config:

```yaml
state: demo/state          # the record lives in demo/state/record
workspace: demo/sandbox    # the agent's working directory
inbox: demo/inbox          # drop *.msg files here
stop_file: demo/STOP       # touch this file to stop
engine:
  kind: mock
memory: false
```

Conventions:

- Relative paths resolve from the directory you launch in. Pick a scratch
  directory; everything the host writes lands under `demo/`.
- `workspace` is the only place the agent's file tools work. Keep it a
  throwaway directory.
- A real engine is `kind: agent` with `base_url` and `api_key_env`. The key
  is named by env var and never written in the file.

```bash
mkdir -p scratch/demo/inbox scratch/demo/sandbox
cp examples/rung-host-mock.yaml scratch/
cd scratch
```

## 3. Inject an owner message

A message is a `*.msg` file holding one JSON object. Write it into the inbox,
before or while the host runs:

```bash
echo '{"role":"owner","text":"Hello, what are you working on?"}' \
  > demo/inbox/hello.msg
```

`role` is `owner`, `peer` or `observer`. The file's stem is the item id. The
host takes the file in at the next boundary and removes it.

## 4. Run

```bash
../target/debug/rung-host run --config rung-host-mock.yaml --turns 3
```

`--turns N` bounds the run; without it the host runs until stopped.

## 5. Read the record

The record is one JSON object per line, in `demo/state/record/seg-*.ndjson`.
It is the truth: every register is a projection of it. Useful views:

```bash
jq -c '{seq, kind}' demo/state/record/*.ndjson | head -40
jq -c 'select(.kind=="stimulus.accepted") | .item' demo/state/record/*.ndjson
jq -r 'select(.kind=="outbox.queued") | .text' demo/state/record/*.ndjson
```

`stimulus.accepted` means your message is durable; `stimulus.disposed` with
`answered` means a turn handled it; `outbox.queued` is what the agent said
back. Line kinds are tabulated in [rung-host.md](rung-host.md#the-record).

Run it again with the same config and the host replays the record, then
continues in a new epoch.

## 6. Stop

- Foreground: Ctrl-C or SIGTERM. The host halts at the next boundary.
- From another shell: `touch demo/STOP`. The host writes a `halted` line and
  exits 0. Remove the file before starting again.

This example is run in CI (`rung-host/tests/quickstart.rs`), so it stays
true.
