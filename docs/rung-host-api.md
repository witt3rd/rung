# rung-host: the doors

The shapes a client of a running `rung-host` can rely on. Each section is
added by the slice that builds the door and stays stable once merged.

## State lock (H1)

A state directory has one live host.

- **The file.** `<state>/host.lock`, taken with an exclusive advisory lock
  (`flock`, non-blocking) before the host writes anything else into the state
  (the record, the memory store), and held for the host's whole life. The
  file holds the holder's pid as one decimal line. It is never removed.
- **A crash frees it.** The kernel drops the lock when the holder dies, by
  any means, so a killed host never wedges the directory. A leftover file
  whose pid is dead is not a lock: only a live holder refuses a start.
  The descriptor is close-on-exec, so a child process cannot keep the
  directory locked after the host is gone.
- **The refusal.** A second host started on a held state (`rung-host sim` or
  `rung-host run`) exits with the bad-start code, **2**, and prints one line
  to stderr:

  ```text
  rung-host: state directory <dir> is in use by a live host (pid <pid>); one host per state
  ```

  (`pid unknown` replaces `pid <pid>` only if the holder had locked but not
  yet written its pid for more than a second.) The refused start writes
  nothing into the record or memory.
- **In the library.** `rung_host::statelock::StateLock::acquire(dir)` returns
  the hold or `LockError::Held { dir, pid }`; `Host::open` takes it when the
  builder has none, so any embedding gets the same guarantee. The lock is
  held by the `Host` and released when it is dropped.
