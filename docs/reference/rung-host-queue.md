# rung-host API — the queue, intent, stop and release

Part of the host API (slice codes are kept; see `rung-host-doors.md` for the map).

## S1 — the queue: list and add, and the gateway's write path

A message to the agent is a **stimulus**. The queue door lists the stimuli that
wait and the one being answered; the same door adds one. Code: `rung_host::queue`
(the fold, no HTTP), `rung_host::acp::QueueDoor` (the host's door),
`rung_gateway` (the write path to a native instance). The slices after this one
add `intent` `steer` and `free` (S2, below) and the stop and release doors; move and
cancel are S3, below. This section is S1 as it was, with S2's changes marked.

### The item

Every list answer, and the `item` of an add, uses one shape, computed from the
record's `stimulus.accepted`, `admitted`, `requeued` and `disposed` lines and the
desk's latest `decision.admit`:

```json
{"id": "q-77", "text": "also include the appendix",
 "role": "owner", "name": null, "channel": "owner", "kind": "peer",
 "intent": "next", "order": 1569, "position": 1,
 "accepted_at": 1791749955653, "state": "waiting",
 "turn": null, "disposition": null, "held": null}
```

| field | meaning |
|---|---|
| `id` | the item's id (an idempotency key, when the sender gave one) |
| `text` | the message, **whole**, through the shared redactor (the record keeps the original) |
| `role`, `name`, `channel` | who sent it. `role` is `owner`, `peer` or `host` (calendar, expectations and other host sources); `name` is the peer's name, the host source's name, or null for the owner; `channel` is the channel as recorded |
| `kind` | the item kind as recorded (`peer` for a message from a channel) |
| `intent` | `next`, `free` or `steer` (S2). The item's own `intent`; a record without one reads `next` |
| `order` | the number of the item's `stimulus.accepted` line: its place by acceptance. Stable |
| `position` | the 1-based place among the items that wait, in list order (owner items first); null for any other state |
| `accepted_at` | when the host accepted it, ms since the epoch (the time sent) |
| `state` | `waiting` (accepted, no boundary has read it), `answering` (in the turn that is answering it), `answered` (disposed by a turn, whatever its `disposition`: `answered`, `digested`, …), `withdrawn` (withdrawn before any turn read it) |
| `turn` | the turn that is answering it, or that disposed it; else null |
| `disposition` | the recorded disposition once over, else null |
| `held` | words saying why the host holds a waiting item (the decision desk holds it until the agent's next break, or as one line in the next turn header; or the model is unavailable and the owner was told), else null |

### `GET /v1/queue`

Computed from the record, so a stopped instance answers the same as a running
one (the gateway reads the state directory; the host's own door reads its own).
The list is ordered for a page: the item being answered; then what waits, **the
owner's items first, then every other sender's, each group in the order the
items were accepted, as S3's moves have changed it** (a peer's item is never
listed above an owner's); then what
is over, the most recently settled first.

Query (all optional; a name it does not know, a repeated name, or a value that is
not as below is 400 `bad_request`):

| parameter | meaning |
|---|---|
| `state` | a comma list of `waiting`, `answering`, `answered`, `withdrawn`, `open` (waiting and answering: **the default**) or `all` |
| `offset`, `limit` | the page, by the paging rule above: `limit` has no maximum (default 500) |

```json
{"items": ["...the items above..."], "total": 3, "offset": 0, "limit": 500,
 "next": null,
 "counts": {"waiting": 2, "answering": 1, "answered": 9, "withdrawn": 0},
 "record_lines": 412, "damaged": null, "redaction": {"env_unset": []}}
```

`total` is the number of items in the asked states, exact; `next` is the next
offset or null on the last page; `counts` is over all four states whatever was
asked; `record_lines` is the last record line read; `damaged` is as in H3 (the
record is read as far as it is intact). `redaction.env_unset` is as for the
other gateway answers (H3). Who may read: the owner role, and the read-only
role (through the gateway); on the host's own door, an owner token only.

### `POST /v1/queue`

```json
{"text": "also include the appendix", "key": "q-77", "intent": "next"}
```

`Content-Type: application/json` (415 otherwise). `text` is required, a string,
and not blank; it is stored whole, **with no limit on its size**. A request is
refused on its headers alone, **before any of its body is read** (see "The
host's own door"). `key` is optional: when
given it **becomes the item id**, so a repeat adds once. A key is 1 to 200 bytes
of letters, digits, `.`, `_`, `:` and `-`, starting with a letter or digit (it is
an id in a path and a record, not content). `intent` is optional: `next` (the default), `free` or `steer` (S2: below); any other value
is refused in words (400), with nothing written. Any other field is refused by name.

The door answers **only after the item is written to the record and fsynced**
(the same acceptance as the ACP `_rung/stimulus` from an owner channel: one code
path), so a message the page shows survives a crash. Answers:

| status | `error` | when |
|---|---|---|
| 201 | | added: `{"id", "repeat": false, "item"}` |
| 200 | | the key was already accepted with this same text: nothing written, `{"id", "repeat": true, "item"}` (the item as it stands now) |
| 400 | `bad_request` | the body, a field or the key is not as above (the message says which) |
| 401 | `unauthorized` | no token, or one that is not the host's |
| 403 | `forbidden` | a token of the host that is not the owner's (host door) |
| 405 | `method_not_allowed` | not GET, HEAD or POST |
| 409 | `key_reused` | the key already names a **different** message; nothing written |
| 409 | `not_running` | the host is stopping, or (gateway) no host is running: **"start it first"**; nothing written. The gateway has no start door |
| 415 | `unsupported_media_type` | not JSON |

A message is never added twice by a retry: send it again with the same key and
the answer is the first one's item with `repeat: true`. Any client that may retry
(the page does) sends a key: the gateway mints none, and a keyless message gets
a new id each time. Item ids are one namespace: the host's own items use the
prefixes `cal-`, `exp-` and `acp-`, and a peer's file names are theirs, so a key
that begins with one of those can collide; `kind` is `peer` for any message from
a channel, so a page marks the owner by `role`.

### The gateway's write path to a native instance

`GET /api/i/{id}/v1/queue` is answered from the instance's state directory (a
registry instance; the same redactor and `env_unset` as `summary`), running or
not. `POST /api/i/{id}/v1/queue` is **forwarded to the running host's own
listener** and its answer is returned (status and body), after the gateway's own
checks, in this order:

1. the gateway's roles and checks, first: an unknown token is 401; the
   read-only token is 403 `read_only` (it may `GET` only); an owner-role request
   passes the Host check and, for a write, the Origin check (H9, 403
   `bad_host` / `bad_origin`). Nothing reaches a host before them.
2. the state word: a registry instance whose state lock is not held (`stopped`,
   `down`) is **409 `not_running`**, with the words "start it first", and nothing
   is sent.
3. where and with what key. **The key is the gateway's own**: the one its own
   config names for this id (`instances[].key_env`, read from this process's
   environment). A variable name an instance supplies (its config file's
   `acp.tokens`, its registry entry) is **never read**, so nothing an instance
   says can make the gateway send it an environment variable. An id the gateway
   config does not name has no write key: 503 `no_owner_key` ("name it in the
   gateway's own config (instances[].key_env)"), nothing sent. Where it goes: the
   running host's registered `address` (it may have asked for a port of its own
   choosing) if that is a **private** address (loopback, a private range, link-local,
   the tailnet's 100.64.0.0/10, or an IPv6 unique-local address; a name is not one
   and is not resolved), else 503 `no_address` and nothing is sent; with no
   registered address, the config's `url`.
   **This is the prototype's trust boundary, stated plainly: the gateway sends
   its write key to any private registry address.** An instance's registry entry
   decides where the key for that id goes, so anyone who can edit the registry
   folder can point the key at any private address (another process on the same
   machine, another box on the tailnet). The registry is the operator's own
   files; this is accepted, and the checks above only keep the key from public
   addresses and from variables an instance names.
4. the forward: the body is **streamed** to the host as it arrives (the gateway
   holds none of it, whatever its size; a refusal above is answered without
   reading it), and the body, `Content-Type` and query go on; the client's
   `Authorization`, its session-state header and `Host` do not, and the host's key is added as the
   bearer. The key value is held in memory only: it is in no response, log, error,
   record or file the gateway writes, and the host's answer, even one that echoes
   its bearer, is passed through the redactor (the key, every config-named value
   and every known key shape) before it is returned.

Failures are plain and never a hang (one async task per request; no thread is
held while the host is slow): a host that cannot be connected to is **502
`instance_unreachable`** ("... could not be reached ...: nothing was queued"); a
connection that broke after the message was sent is 502 saying the message may be
queued; a host that does not answer within the gateway's wait (30 seconds, a
bound on a wait and not on any content: it counts from the last byte moved in
either direction, so a client still sending a long message is not cut off) is **503 `instance_not_answering`**
("the message may or may not be queued; look at the queue, or send it again with
the same key (a repeat adds once)"). Other methods on `/v1/queue` are 405 for a
native instance.

### The host's own door

`GET` and `POST /v1/queue` are on the host's HTTP listener beside `/acp` and
`/v1/events`, owner token only (the host's `acp.tokens`: an unknown token is 401,
another role's token is 403). The list reads the host's own record; every text
and every error is passed through the host's redactor.

**Refused before the body is read.** The listener asks the door, from the request's
headers alone, whether to take a request at all (`RouteGate`): no token or an
unknown one is 401, another role's token 403, a method the door does not have 405,
a body that is not `application/json` 415, a host that is stopping 409
`not_running`. Each is answered at once with `Connection: close` and the body is
never read, so a client sending a body of any size to a request that is refused
costs the host nothing (a 1 GiB generated body to each refusal left the host's
peak memory where it was). Only a request the gate *admits* has its body read, as it
arrives and with no limit, and passed whole to the route (`RouteRequest.body`); a
request to any path the gate does not own (an unserved path, or a door with no
body to take) has none read, and the connection is closed after its answer. An item an
owner sends is stored whole; storing it takes a bounded number of copies of it
(the body, the parsed item, the inbox's, the line written, the answer: about nine
measured), because an item is one record line that every reader reads whole and
the inbox holds it until a turn reads it. Making that flat needs the item's text
kept outside its record line, a change to the record's format, and is not part of
this slice.

Each list and each add reads the record from its start (a fold; linear in the
record's size, about a tenth of a second at 24 MB). That is fine now and is a
cost to watch as records grow; nothing else is read or kept.

### Not here

`stop` and `release` doors, and the page. (Moving and cancelling an item is S3; `intent`
is S2.) Peers' items are listed and ordered here but only the owner adds through this
door.

### Proof

`rung-host/tests/queue_door.rs`, `rung-host/tests/queue_door_memory.rs`, `rung-gateway/tests/queue.rs` and `rung-gateway/tests/queue_memory.rs`, and
the fold's own tests in `rung-host/src/queue.rs`, written first against code with
neither door and kept red until it made them green: an item in the record before
the answer; one key twice is one item, a different message under the key is
refused; a stopped or down instance 409 "start it first" with the record byte for
byte unchanged and no request sent upstream; the read-only token cannot write
and can read; a planted owner key (and a shaped secret, and a host that echoes
its bearer) in no gateway answer; an unreachable host 502 and a silent one 503
while the gateway keeps answering; no key held 503; a bad Host or Origin refused
before any host; a stopped instance's list from its record; a 200,000-character
task queued and listed whole, through the gateway and on the host; peer items
never above owner items; the desk's reason in words; states from the record's
lines; paging with an exact total.

The harm review of the first version added, each written first and each killed
by a mutation of the code it guards: refusals answered before the body is read
and the host's memory unchanged by a 1 GiB generated body (the gate removed:
red); an item past 16 MiB stored whole; 20 concurrent posts with one key make one
201, nineteen 200s and one record line (the atomic check-and-write replaced by
check, wait, write: red); a retry after the host is killed and restarted is one item; a stopping host refuses 409 (the check removed: red);
every form of `Authorization` on the list and the add (a bare, empty or foreign
header read as no token: red); a variable an instance's config names never read
or sent (the old read: red); a registered public address never sent a key; a
256 MiB body streamed through the gateway with its peak memory unchanged, and
refused ones not read (the gateway collecting the body: red); an upload slower
than the wait not cut.

## S3 — the queue: reorder and cancel

The owner can change what waits: put an item in front of another, or take it
back. Two doors on the host's listener beside S1's, and one new record line.
Code: `rung_host::inbox` (`Order`, the fold; `InboxState::plan_move`, the
judgement), `rung_host::presence::Host` (`move_item`, `cancel`),
`rung_host::acp::QueueDoor` (the doors), `rung_gateway` (the write path).

### The record line `queue.moved`

```json
{"seq": 1571, "kind": "queue.moved", "id": "q-80", "before": "q-77", "key": "mv-1"}
```

`id` is the item moved; `before` is the id it was put in front of, or **null for
the end of its group**; `key` is present when the sender gave one. One line is
written for a move that changes the order, durable (fsynced) before the door
answers. Nothing else about an item is changed by it.

**The waiting order is a fold of acceptance and these lines**, the same fold in
the host (what a turn reads) and in every reader of the record (the list):

1. An item takes the end of the order when it is accepted. An item being
   answered keeps its place, so one requeued after a failed or interrupted turn
   waits where it did; an item that is disposed has no place.
2. A `queue.moved` line takes `id` out and puts it in front of `before`
   (after the last waiting item of its group, when `before` is null).
3. A line is applied only when `id` waits, `before` (if any) waits, they are not
   the same item, and both are in **one group**: the owner's `next` and `steer`
   items, the owner's `free` items (S2), a peer's, or the host's own (calendar,
   expectations). The group is `inbox::read_rank(role, intent)`, the same key the
   turn reads by, so an item is never moved to where it would not be read. A line
   that does not meet that is read as no move. (The doors never write one; this keeps a record that has one
   deterministic.)
4. **The later line wins**: two moves are two lines in the order the host took
   its lock, and the fold applies them in that order.
5. The list and the order a turn reads are the groups in the order owner `next`
   and `steer`, owner `free`, peers, host, and inside a group that order. So an
   **owner item stays above a peer's** (a peer's item can never be put above one,
   nor an owner's put below a peer's), and a `free` item stays below the owner's
   `next` and `steer` items and above a peer's.

**A move changes order only.** When an item is shown is still the desk's: its
interruption limits, a firm item's deadline, "at the agent's next break", a
digest line. A moved item that the desk holds is still held, with the same reason
in words in the list (`held`). What a move does change for real is the order a
turn reads the items it admits, and the order the desk's decider is asked about
the non-owner ones: both follow the folded order. **One order, one path:** the
`stimulus.admitted` ids and the rows of the turn header are in the order the list
shows (the fold above, grouped by `read_rank`), so what the owner moved is what the
agent reads (the header was once sorted by acceptance, which undid a move). The record keeps each
item's own `order` (its acceptance) unchanged in the list.

### `POST /v1/queue/{id}/move`

```json
{"before": "q-77", "key": "mv-1"}
```

`before` is **required**: the id of the item to put this one in front of, or
`null` for the end of its group (a page's "down" on the last-but-one row, or
"move to the bottom"). It is an id, not a position: a position goes stale as the
list changes under it, an id names what the owner was looking at. `key` is
optional (the S1 key rule: 1 to 200 bytes of letters, digits, `.`, `_`, `:`, `-`,
starting with a letter or digit). Any other field is refused by name.
`Content-Type: application/json` (415 otherwise). `{id}` is percent-decoded (an
id from a peer's file name may hold a space). Refused on the headers alone, before
any body is read, as S1's.

The door answers **only after the line is in the record and fsynced**. Answers
(`item` is the item in S1's shape, after the move):

| status | `error` | when |
|---|---|---|
| 200 | | moved: `{"id", "moved": true, "repeat": false, "item"}`; the item's `position` is where it is now |
| 200 | | **already in that place** (it is already in front of `before`, or already last): `moved: false`, a `message` saying so in words, **nothing written**; never `moved: true` |
| 200 | | the key already named this same move: `{"moved": false, "repeat": true, "item"}`, nothing written |
| 400 | `bad_request` | the body or a field is not as above (the message says which) |
| 401, 403 | `unauthorized`, `forbidden` | no token or an unknown one; a token of the host that is not the owner's |
| 404 | `not_found` | `id` or `before` names no item (or an owner control: stop and release are no items) |
| 405 | `method_not_allowed` | not POST on this path |
| 409 | `not_waiting` | `id` or `before` is **being answered right now** (it says the turn), or **was answered or withdrawn**: said in words |
| 409 | `group` | `id` and `before` are in different groups (the owner's `next` and `steer`, the owner's `free`, a peer's, the host's own): "a peer's item cannot be put above the owner's items: they stay on top", "the owner's free item cannot be put above a next item" (and the like). A move across them used to be impossible to tell from a move; it is refused, nothing written |
| 409 | `boundary` | the move would jump over an item that waits for a different boundary (below) |
| 409 | `key_reused` | the key already names a **different** move |
| 409 | `not_running` | the host is stopping, or (gateway) no host is running: **"start it first"** |
| 415 | `unsupported_media_type` | not JSON |

Every refusal writes **no line**; the message says "nothing was written".

**The same boundary.** Each waiting item waits for one boundary: the owner's
items are always shown at the next boundary; any other item waits for the one the
desk's latest `decision.admit` gave it (the next boundary, the agent's next break,
or a digest line). A move puts an item in front of another, so it jumps over
every waiting item of its group between where it is and where it goes; each of
those must wait for the same boundary as the item moved, else 409 `boundary`
naming both. (The page's up and down buttons move one place, so a row beside one
with another boundary just has no button.) An owner's `free` item is a group of its
own (S2), so it is never jumped over by, nor jumps, a `next` item.

**Idempotent.** A key names a line: the same key with the same move again is
`repeat: true` and one line; the key is in the line, so it is remembered across a
restart. A move that changes nothing writes nothing and so remembers no key: a
retry of it later moves the item then, if it is still where the retry puts it
the answer is `moved: false`. A client that may retry sends a key.

**Concurrent moves.** The check and the write are one step under the host's lock.
Twenty moves of the same pair at once are one line and nineteen 200s with
`moved: false`; twenty opposing moves are as many lines as changed the order, and
the later line wins.

### `DELETE /v1/queue/{id}`

Cancel: **withdraw a waiting item**. It is the path ACP's `session/cancel` of a
prompt uses (`Host::withdraw`, one `stimulus.disposed` line, disposition
`withdrawn`), so an item's end is the same line however it was cancelled, the item
never reaches the agent, and a client holding it open over ACP is answered
`cancelled`. No body; no key (it is idempotent by itself). Any item the list shows
as waiting may be cancelled, whoever sent it.

| status | `error` | when |
|---|---|---|
| 200 | | withdrawn: `{"id", "withdrawn": true, "repeat": false, "item"}` (state `withdrawn`) |
| 200 | | **already withdrawn**: `repeat: true`, nothing written (a cancel is a no-op the second time, not a refusal: a retry is safe) |
| 401, 403, 405, 409 `not_running` | | as the move's |
| 404 | `not_found` | no such item |
| 409 | `not_waiting` | the item is **being answered right now** (it would cut a running turn; the message says the turn) or was **answered** |

An item a turn took between the look and the withdrawal is the same 409: the
withdrawal itself is checked and written under the lock.

### Roles, a stopped instance, the gateway

Owner token only on the host's door (another role's token 403, none or unknown
401). A stopping host refuses 409 `not_running` ("start it first") from the
moment the stop is raised.

Through the gateway: `POST /api/i/{id}/v1/queue/{item}/move` and
`DELETE /api/i/{id}/v1/queue/{item}` are forwarded to the running host's own
listener by S1's write path and under S1's rules, unchanged: the gateway's roles
and Host/Origin checks first (the read-only token is 403 `read_only`; it never
writes); a stopped instance 409 `not_running` "start it first" with nothing sent;
the gateway's own key, by name, to a private address; the body streamed; the
host's answer redacted again. The item id is sent as the client wrote it
(percent-encoded). A failure says whether anything may have changed: an
unreachable host, "nothing was changed"; a connection that broke or a host that
did not answer, "the change may have been made; look at the queue, then ask again"
(a cancel of an item already withdrawn, and a move to the place the item is in,
change nothing). `GET` of these paths and other methods are 405; a path with more
segments 404. `GET /api/i/{id}/v1/queue` shows the folded order for a stopped
instance too, from its record.

### The order survives a restart

The order is the fold of the record, so a host that is killed and comes back,
and a stopped instance's list, show the order the moves left. A move is on disk
before it is answered.

### Redaction and size

Every text in an answer, and every error (which may name the id it was given),
goes through the redactor. An item is stored and listed whole whatever its size:
a move or a cancel writes a small line that names the item by id and does not
copy or touch its text (a 200,000-character item is the same before and after).

### Not here

`intent` (S2); stop and release are S7. A move does not override the desk,
and a cancel does not cut a running turn (that is what the owner's cancel over
ACP, or a steer, does).

### Proof

`rung-host/tests/queue_move.rs` over real HTTP against the `rung-host`
binary, and `rung-gateway/tests/queue.rs` (the S3 block, 9), and the fold's own
tests in `rung-host/src/inbox.rs` and `queue.rs`, each written first and shown
red against code with no door: an owner item moved up, down and to the end with
the line whole in the record before the answer; a move to its own place writes
nothing; a peer item never passes an owner item and an owner item is never put
below a peer (both ways, all refused in words with no line); an item being
answered, answered, withdrawn or unknown is refused (409 / 404, no line); a
cancelled item is withdrawn once, a repeat is a no-op, it never reaches the
agent after the host comes back with a model; the item being answered cannot be
cancelled; the order after a kill and a restart, and the order a turn reads;
20 equal moves at once make one line, 20 opposing moves leave every line whole
and the order the later line's; a key twice is one line and a reused key is
refused; the read-only and peer tokens never write; a stopping host and a
stopped instance say "start it first"; a 200,000-character item is the same
after a move and a cancel; an error that names the id it was given is redacted.

## S2 — intent on the queue item: `next`, `steer`, `free`

An owner's message says what it is for. `POST /v1/queue` takes `intent`; the item
carries it; the host's desk reads it. Code: `rung_host::inbox::Intent` (the type and
the one question "is this read at the next boundary": `Item::pushes`,
`InboxState::owner_pushes`), `rung_host::desk::admit` (the guard), `rung_host::queue`
(the list), `rung_host::acp::QueueDoor` (the door). The rules the desk keeps are in
`docs/reference/rung-host.md` ("Owner intent").

| `intent` | meaning | cuts a long tool call |
|---|---|---|
| `next` | a task. Read at the next turn boundary, ahead of anything the owner has not yet sent. **The default** | yes |
| `steer` | a message about what the agent is doing now. Read at the next boundary like `next`, and shown as a steer, not a task | yes: the host's own `OwnerWatch`, as for any owner item |
| `free` | wait until the agent is in free time or at a natural break; **never interrupts committed work** | no |

### The door

```json
{"text": "use the other table", "key": "q-78", "intent": "steer"}
```

`intent` is a string, one of the three words, any other value (a different word,
another case, an empty string, a number, a list) is **400 `bad_request`** in words that
name the three, with 0 lines written. A missing or null `intent` is `next`. Everything
else is S1's: the answer comes only after the item is on disk, a key is the item id,
and the answers are S1's table. **A key names one message, and the intent is part of
it:** the same key with the same text and the same intent is a repeat (200, nothing
written); the same key and text under another intent (an omitted one is `next`) is 409
`key_reused`, nothing written, since the page would otherwise show the first intent
for a message the owner meant otherwise. The gateway adds no rule of its own: it
forwards the body as it came, so the intents go through its write path as through the
host's own door, and a host's refusal is passed back with its status.

### The record

`stimulus.accepted`'s `item` carries `"intent": "free"` or `"intent": "steer"`. A
`next` is not written (absent is `next`): every record from before S2 reads as `next`
and its bytes are unchanged. The list and the answer of an add show the intent as one
of the three words; an item whose recorded intent is not one of them reads `next`, and
only an owner's item is ever anything else. The inbox is a fold of the record, so the
intent survives a restart.

### What the host does with it

What the desk does with each intent (when it is read, what it cuts, what the
limits and firm deadlines still win) is stated once, in
[`rung-host.md`](rung-host.md#owner-intent-next-steer-free), and not here. What
a client of the door can see of it:

- **`steer`** is marked in the turn header's row, `[owner/id @time · steer]`.
- **`free`** is held while the agent is committed (see "Held" below), and is read
  after 30 minutes at the longest, committed or not.

### Order

There is one read order, defined in `rung-host.md` (owner items first: `next` and
`steer` together, then `free`; then peers, then the host's own items), and the
queue list follows it. The list's `position` is that order as the owner's moves (S3)
have changed it. **A move stays inside one of those groups** (the group is the read
rank, so a `free` item is never moved among the `next` ones: that move is refused in
words, nothing written). The turn header's admitted rows, the `stimulus.admitted`
ids and the list are in that one order.

### Held

A `free` item the desk's latest decision holds (`at_break`) says so in `held`: "you
asked it to wait: it is read when the agent is in free time or at a natural break, not
while the agent is committed to a project". It is not said before a boundary has looked at
it, and not once it is read.

### The turn view

The record carries what the page's turn view needs: the item's `intent` and `at` (the
time it was sent) on `stimulus.accepted`, and the turn that read it (`stimulus.admitted`,
and `turn` on the item in the list). A steer that cut a long call shows the moment the
agent saw it in the turn's `tool.refused` line (`why: owner_waiting`). The page's own
fold of these is the page's (`ui/src/record/folds.ts`).

### Not here

Moving and cancelling an item (S3), stop and release (S7), a `intent` on ACP's
`_rung/stimulus` and on a directory source's files (an owner's `*.msg` is `next`).

### Proof

`rung-host/tests/intent_desk.rs` (the desk's guard: each intent, a firm deadline,
the longest wait, the interrupt limit), `rung-host/tests/queue_move.rs` (the S2 and S3 block: the header in the order the owner moved items to, a move across `free` and `next` refused with no line, `steer`, `next` and `free` after moves and a restart), `rung-host/tests/intent_run.rs` (whole
simulated runs on a simulated clock: a steer read at the first boundary after it arrives
and marked, a long steer whole, a free item held through committed turns and read once
the agent is free, the header order, a paced wait a free item does not end, a steer that
cuts a long call and is read next boundary while a free one cuts nothing, the intent
across a restart), `toolbox::tests` (the real `OwnerWatch` on a simulated clock: a
steer cuts a call at the moment it arrives, a free item does not), `queue::tests` (an old
record reads `next`, the order, the words for a held free item), `inbox::tests`,
`gates::tests`, `rung-host/tests/queue_door.rs` (each intent accepted, recorded and
listed; an unknown one refused in words with 0 lines; idempotency with an intent; a
restart), `rung-gateway/tests/queue.rs` (the intents through the gateway's write path)
and the page's contract test of the door (`ui/test/contract-queue.test.ts`, an intent the host does not know). Written first: the desk tests were red against a desk that
forced every owner item to `now`, the run tests against a host with no intent, and
every safeguard was mutated (the exception in the guard, the long-call watch, the
wake, the gates, the ordering, the key's intent, the header mark, the header order)
and killed by a test.

## S7 — stop and release

Two controls of the owner, on the host's listener beside the queue's. Code:
`rung_host::acp::QueueDoor` (the doors, `ControlTarget`), `rung_gateway` (the
write path). **There is no start door**: the unit manager starts a host; a
stopped instance is answered in words.

Both are `POST`, owner token only (no token or an unknown one 401, another role's
token 403, another method 405), refused on the headers alone. The gateway's
read-only token is 403 `read_only` and nothing is sent. Each is the same
acceptance as ACP's `_rung/stop` and `_rung/release` from an owner channel: an
owner control is a `stimulus.accepted` line (with `control`) and a
`stimulus.disposed` line (`control`), fsynced before the door answers; it is
never listed in the queue. A stop is then raised the way the stop file raises
one, and a release goes through the kernel's own owner release.

The check ("is the host already stopping", "is anything held") and the write are
**one step** under one lock, `Host::owner_stop` and `Host::owner_release_control`,
and ACP's `_rung/stop` and `_rung/release` go through the same two methods, so the
two roads record and answer alike: any number of concurrent stops write exactly
one stop line (the rest answer `repeat: true`), and a release writes its lines
only when it releases a commitment (ACP answers `{"released", "repeat"}` too).

### `POST /v1/stop`

No body at all: a declared body (a `Content-Length` other than 0, or a
chunked one) is refused 400 `bad_request` from the headers, before any of it is
read. (Stop takes no content, so this bounds nothing the owner writes.)

| status | when |
|---|---|
| 200 | `{"stopping": true, "repeat": false}`: the stop is in the record and raised; the host halts at its next check and exits |
| 200 | `{"stopping": true, "repeat": true}`: the host is already stopping; **nothing is written** |

### `POST /v1/release`

Body optional: `{"reason": "..."}` (a string; any other field is 400). The
reason is recorded whole, as written (no bound on its length). **A release is
not named to a commitment: it lets go of whatever is held at the moment it
arrives.** If an answer is lost and the agent has since committed to something
else, asking again releases that. Look at the instance before asking again.

| status | when |
|---|---|
| 200 | `{"released": true, "repeat": false}`: the commitment is released (the kernel's `kernel.release` line, `released_by` `owner`) |
| 200 | `{"released": false, "repeat": true}`: nothing is held; **nothing is written** |
| 409 | `not_running`: the host is stopping: "start it first" |

### Through the gateway

`POST /api/i/{id}/v1/stop` and `/v1/release` are forwarded to the running host
by the queue's write path under its rules (the gateway's roles and Host/Origin
checks first, its own key by name, a private address, the answer redacted). A
stopped instance answers 409 `not_running` in words, with nothing sent: for a
stop, that it is already stopped; for a release, "start it first". A failure
says whether anything may have changed (unreachable: nothing; a broken
connection or no answer: look at the instance, then ask again; a stop asked
again changes nothing more, but a release asked again releases whatever is held
then). Other methods are 405.
