# rung-host API — the gateway

Part of the host API (slice codes are kept; see `rung-host-doors.md` for the map).

## H9 — the gateway

`rung-gateway` is one listener in front of any number of rung hosts. It
serves the UI's built app, lists the instances it fronts, and passes
`/v1` through to one instance, adding that instance's key and streaming the
answer back. The keys are held only here: no page and no response carries
one. It is a product crate, not published.

```
rung-gateway --config gateway.yaml
```

### Config

Names of variables, never secrets:

```yaml
listen: 127.0.0.1:8787            # default; tailscale serve puts it on https
app_dir: ui/dist                  # optional; relative to this file
allowed_hosts: [gw.tailnet.example]   # optional: names a request may be addressed to, besides loopback
read_only_token_envs: [RUNG_GATEWAY_VIEW_TOKEN]   # optional, each names a variable
registry_dir: /home/me/.rung/instances   # optional: the H2 registry folder of native hosts (relative to this file if not absolute)
instances:
  - id: alpha                     # letters, digits, '-', '_' (a path segment)
    name: Alpha                   # optional, defaults to the id
    url: http://127.0.0.1:7001    # the host's own listener, http only
    key_env: ALPHA_HOST_KEY       # the variable holding the owner key
```

Startup refuses, naming the variable and never its value, when a variable is
unset or empty, an id is repeated or not a plain name, a url is not `http://`,
a key has a byte a header cannot carry (line break, NUL, non-ASCII), or a field is unknown. The instances come from a `Registry` (a trait asked on
every request); the config file is one source, and the registry folder (H2)
plugs in as another. An instance named here is also the one a **native**
instance of the same id is written to with (S1): `key_env` is the only key the
gateway ever sends a host to write.

### Who may do what

The tailnet is the only boundary: there is no login. A request with no token
has the owner's role. A request that presents a read-only token has the
read-only role and may only read. The token rides `Authorization: Bearer
<token>`, or the `token` query value (an event stream cannot set headers).
It is the gateway's own: it is never forwarded, and neither are the client's
`Authorization` header or its session-state header. A token that matches no configured read-only
token is refused with 401, wherever it appears.

**An `Authorization` header that is there but is not a bearer token the
gateway can read is 401, never "no token".** Only a request with *no*
`Authorization` header (and no `token` value) is the owner's. These are all 401
`unauthorized`, on every door, reads and writes alike: a bare `Bearer`, an empty
`Bearer `, another scheme (`Basic ...`), text that is not UTF-8, more than one
`Authorization` header, a bearer that is not a configured read-only token (the
hosts' own keys included), and an empty `?token=`. The scheme name is read
without regard to case.

A write by the read-only role is refused with 403 before it reaches a host:

```json
{"error": "read_only", "message": "this token can only read"}
```

### Host and Origin

The owner role is whoever reaches the gateway, so every owner-role request
under `/api` (reads included) passes a Host check first, and every write (any
method other than GET or HEAD, whatever door it is for, now or added by a
later slice) also passes an Origin check. A request with a read-only token
skips both: the read-only role keeps its own 403 on a write.

- The `Host` must be `localhost`, `127.0.0.1`, `[::1]`, a name or bare IP in
  `allowed_hosts` (compared without case or port), or the IP the gateway is
  bound to when that is a specific address that is neither loopback nor
  unspecified (so a tailnet-bound listener is not locked out). Anything else,
  or no `Host`, is refused: this is what stops a rebound DNS name from
  reading or driving an instance. Behind the tailnet's serve command, list
  the name it gives the gateway.
- On a write only, an `Origin`, if the client sends one, must be the same
  origin as the request: the same host name as the `Host` and the same port (a
  scheme's default port counts as no port). `null`, another host, or another
  port on the same host is refused. A browser sends `Origin` on every write; a
  script usually sends none and passes.

Refusals are 403 with `{"error": "bad_host", ...}` or `{"error": "bad_origin",
...}`, before anything reaches an instance.

### Routes

| Route | What |
|---|---|
| `GET /api/health` | `{"ok": true}` |
| `GET /api/instances` | the configured instances with each one's summary, and the native instances of `registry_dir` read from their state directories (below, and H3) |
| `/api/i/{id}/v1/...` | pass-through to that instance's `/v1/...`, any method; for a native instance of `registry_dir`, `summary` and `record` are answered from its state directory (H3) |
| anything else | the app: a file under `app_dir`, or `index.html` for a path with no extension (client-side routes); `/api/...` is never the app |

Only `/v1/` of an instance passes (its ACP and other paths do not). A path
with a segment that decodes to `.`, `..`, or contains a separator is 404, for
the passed paths and the app alike. Unknown instance: 404
`no_such_instance`. Errors are always `{"error": <code>, "message": <text>}`.

`GET /api/instances`:

```json
{"instances": [
  {"id": "alpha", "name": "Alpha", "reachable": true,
   "summary": {"...": "the instance's GET /v1/summary, as it answered"},
   "error": null},
  {"id": "beta", "name": "beta", "reachable": false,
   "summary": null, "error": "client error (Connect): ..."}
]}
```

An instance that does not answer within 5 seconds, or answers badly, is
listed with `reachable: false`, never left out.

The pass-through adds `Authorization: Bearer <that instance's key>`, keeps the
method, body, query (minus `token`) and end-to-end headers (so `Last-Event-ID`
resumes a stream), and does not buffer the answer: an event stream arrives as
the host writes it. An instance that cannot be reached answers 502
`instance_unreachable`; error text never carries a key. A registry that hands over a key a header cannot carry gets the same 502 for that instance (and `reachable: false` in the overview), never a dropped connection. A failed `accept` is logged to stderr and retried after a short pause.

### The door table

`rung_gateway::doors::DOORS` names each host door and its access. The gateway
streams every answer through, so a door's being a stream changes nothing
there; the Stream column below is for the reader of the host's API only. A request not in the table is classed by method: GET and
HEAD read, anything else writes. The read-only role may use `Read` doors only.
Later slices add rows as their doors land.

| Door | Access | Stream | Slice |
|---|---|---|---|
| `GET /v1/summary`, `/v1/status`, `/v1/report` | read | | H3 |
| `GET /v1/record`, `/v1/decisions`, `/v1/pack`, `/v1/spend` | read | | H3 |
| `GET /v1/turns`, `/v1/turns/{n}` | read | | S8 |
| `GET /v1/events?after=N` | read | yes | H5 |
| `GET /v1/turns/{n}/context?call=k` | read | yes | S4 |
| `GET /v1/queue` | read | | S1 |
| `POST /v1/queue` (with `intent`) | write | | S1, S2 |
| `DELETE /v1/queue/{id}`, `POST /v1/queue/{id}/move` | write | | H6 |
| `GET /v1/config` | read | | H7 |
| `POST /v1/config/validate`, `PUT /v1/config` | write | | H7 |
| `GET /v1/console` | read | | H8 |
| `GET /v1/console/stream` | read | yes | H8 |
| `POST /v1/stop`, `POST /v1/release` | write | | S7 |

Of the H3 rows, the first slice serves `summary` and `record` for native
instances (H3, in `rung-host-doors.md`); `status`, `report`, `decisions`, `pack` and
`spend` are named here and not served yet; `turns` is specified in S8 (`rung-host-doors.md`;
not served yet). `events` (H5) is served
for native instances from their state directories, and by the host's own
listener (H5, in `rung-host-doors.md`).

Validating a config change is classed as a write: it only serves the owner who
may then apply it.

Lists on the host's doors follow one paging rule: `offset`, `limit`, `total`,
`next`; `limit` has no maximum and `total` is exact. The gateway does not
interpret it.
