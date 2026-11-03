# Run the gateway

Informative. The gateway is one listener in front of any number of hosts: it
serves the UI's built app, lists the instances, and passes `/v1` through to one
instance with that instance's key added. The contract (config, roles, routes,
refusals) is [rung-gateway.md](../reference/rung-gateway.md); this page is the
steps.

1. **Run a host** with its listener on, and note its url and the variable that
   will hold its owner key: [rung-host-quickstart.md](rung-host-quickstart.md).
2. **Write the config.** Names of variables, never secrets:

   ```yaml
   listen: 127.0.0.1:8787
   read_only_token_envs: [RUNG_GATEWAY_VIEW_TOKEN]   # optional
   instances:
     - id: alpha
       url: http://127.0.0.1:7001
       key_env: ALPHA_HOST_KEY
   ```

   Add `registry_dir` to list the native hosts that register themselves
   (the registry is in [rung-host-doors.md](../reference/rung-host-doors.md), H2),
   and `app_dir` for a built UI.
3. **Export the variables, then start it:**

   ```bash
   ALPHA_HOST_KEY=... RUNG_GATEWAY_VIEW_TOKEN=... rung-gateway --config gateway.yaml
   curl -s http://127.0.0.1:8787/api/health      # {"ok": true}
   ```

   Startup names a variable that is unset or empty and refuses; it never
   prints a value.
4. **Reach it from elsewhere.** The tailnet is the only boundary: a request with
   no token has the owner's role. Put the name it is served under in
   `allowed_hosts`; a read-only token (`Authorization: Bearer` or `?token=`)
   may only read.

The queue, redaction and read doors the gateway passes through are in
[rung-host-queue.md](../reference/rung-host-queue.md),
[rung-redaction.md](../reference/rung-redaction.md) and
[rung-host-doors.md](../reference/rung-host-doors.md).
