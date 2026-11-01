# rung-agent — OAuth credentials

**Status: informative** (a crate design, `docs/README.md`). The propositions
below are numbered `O1…`; each names the test that proves it, or says it is
not proven. Where this page and the code disagree, the test is right.

`rung-agent` has so far had one kind of credential: `llm.api_key_env`, the
name of a variable that holds a key, or no credential at all (a LAN server).
Some providers have no per-token key. They issue a **login** to a person, and
the login yields a short-lived bearer on demand. This page is the second kind
of credential, with **GitHub Copilot** as its first provider, so an agent can
run on a Copilot subscription.

Everything past the seam is unchanged: the turn engine sees "an
OpenAI-compatible route plus a bearer".

## Contract

### Choosing it

**O1. A credential kind next to `api_key_env`.** `config.yaml` selects it:

```yaml
llm:
  model: gpt-4.1
  auth:
    oauth: { provider: github-copilot, profile: work }   # profile: default if omitted
oauth:                                                  # optional, per provider
  github-copilot: { headers: { Editor-Version: "vscode/1.200.0" } }
```

`RUNG_OAUTH_PROVIDER` and `RUNG_OAUTH_PROFILE` win over the file, so one
process (one instance of a host) picks its own login. The credential
replaces the key: `api_key_env` beside it is an error, a key from the
environment is dropped, an unknown provider, a malformed profile name, an
unknown key in the block, or the Anthropic route is an error at load. Loading
touches no network.
*Proof:* `config::tests::oauth_in_the_file_selects_a_credential_and_drops_any_key`,
`config::tests::the_environment_picks_the_profile_per_process`,
`config::tests::oauth_beside_a_key_a_bad_name_or_the_wrong_route_is_refused`,
`config::tests::a_provider_section_with_an_unknown_key_is_refused`.

**O2. One seam, in the kernel.** `rung_std::llm::CredentialSource` is the only
thing the call knows: `authorize()` returns a `RequestAuth` (bearer, optional
base URL, extra headers) and `rejected()` says the server refused it. How a
login is made, kept and renewed is the source's business
(`rung-agent-core::oauth`). A provider registry (`oauth::provider`) maps a
name to its login flow, its token exchange and its base-URL rule; Copilot is
the first entry. Supported on the OpenAI-compatible route only (v1).
*Proof:* `rung-agent-core/tests/oauth.rs::a_chat_call_goes_through_the_minted_credential`.

### Making a login

**O3. Device-code login.** `rung-agent auth login github-copilot [--profile
NAME] [--enterprise DOMAIN]` asks GitHub for a device code (the public
editor-client app id and `scope=read:user`), shows the code and URL, and
polls for approval. It honours `interval`; `slow_down` lengthens the wait (to
the interval GitHub names, else by five seconds); `authorization_pending`
keeps waiting; `expired_token`, `access_denied` and the code's own lifetime
end the flow; an unknown error ends it with its name. A failed login stores
nothing.
*Proof:* `rung-agent-core/tests/oauth.rs::device_flow_waits_out_pending_and_slow_down_then_stores_the_login`,
`rung-agent-core/tests/oauth.rs::device_flow_ends_on_expired_token_denied_and_a_spent_code`,
`rung-agent-core/tests/oauth.rs::an_unknown_device_flow_error_is_reported_without_a_token`.

**O4. No browser on the host.** The code and URL are printed (to stderr); the
user opens the URL on any machine. Login works over SSH. The token is never
printed, by `login`, `status`, `models` or a turn.
*Proof:* `rung-agent/tests/oauth.rs::login_status_models_and_logout_never_print_a_token`.

### Keeping it

**O5. Machine-local, mode 0600, never in a repo.** A login is
`$RUNG_HOME/oauth/<provider>--<profile>.json` (default `~/.rung/oauth/`):
directory 0700, files 0600, each replaced whole (write a sibling, `rename`).
It holds the long-lived token and the latest minted one with its expiry. It
is **not** in `~/.rung/auth.yaml`: that file is a flat provider-to-string map
that the driver parses wholesale, and one structured value would blank it for
the driver. A name is a path component (`[A-Za-z0-9._-]`, no leading `.`, no
`--`, at most 64), so it cannot leave the store.
*Proof:* `rung-agent-core/tests/oauth.rs::a_login_is_stored_at_mode_0600_in_a_private_directory`,
`rung-agent-core/tests/oauth.rs::a_profile_name_cannot_leave_the_store`,
`rung-agent-core/tests/oauth.rs::logins_list_and_remove`.

**O6. Named profiles.** A provider may have several logins (a personal and a
work one). The profile is chosen per instance (O1) and the logins never mix.
*Proof:* `rung-agent-core/tests/oauth.rs::two_profiles_of_one_provider_use_their_own_logins`.

### Using it

**O7. Minted on demand, renewed ahead of expiry.** The long-lived GitHub
token (GitHub does not rotate it) is exchanged for a short-lived Copilot
token (`GET /copilot_internal/v2/token`). The minted token is cached in memory
and in the login's file. It is reused until fewer than five minutes remain,
then replaced before it expires.
*Proof:* `rung-agent-core/tests/oauth.rs::a_good_token_is_reused_and_renewed_ahead_of_its_expiry`.

**O8. Single flight.** Callers racing for a new token cause **one** exchange:
inside a process they queue behind one lock; across processes on one machine
they queue behind the login's file lock (`flock`, released by the kernel if
its holder dies), re-read the file, and use what the winner wrote. A second
process finds the first one's token.
*Proof:* `rung-agent-core/tests/oauth.rs::parallel_callers_cause_exactly_one_exchange` (four sources
over one store, sixteen threads, one exchange on the wire),
`rung-agent-core/tests/oauth.rs::a_second_process_reuses_the_token_the_first_stored`,
`rung-agent/tests/oauth.rs::a_turn_on_an_oauth_credential_mints_once_and_sends_the_bearer`
(two real processes, one exchange).

**O9. The route comes from the token.** The Copilot token names its proxy
(`proxy-ep=proxy.<tier>.githubcopilot.com`), which maps to
`https://api.<tier>.githubcopilot.com`. A token with none falls back to
`https://copilot-api.<domain>` for an enterprise login and to the individual
endpoint otherwise. `llm.base_url`, `RUNG_BASE_URL` or
`oauth.github-copilot.base_url` wins over all of these.
*Proof:* `rung-agent-core/tests/oauth.rs::the_token_names_the_route_and_enterprise_has_its_own_rule`,
`rung-agent-core/tests/oauth.rs::the_token_route_is_used_when_nothing_overrides_it`.

**O10. The editor identity is configuration.** The service expects an editor
client's `User-Agent`, `Editor-Version`, `Editor-Plugin-Version` and
`Copilot-Integration-Id` (and sends `Openai-Intent`, `X-Initiator`). Their
versions go stale, so `oauth.github-copilot.headers` overrides each; the
values compiled in are only defaults.
*Proof:* `rung-agent-core/tests/oauth.rs::editor_identity_headers_are_configuration`,
`rung-agent-core/tests/oauth.rs::authorize_returns_a_bearer_the_route_and_the_editor_headers`.

**O11. A 401 forces one refresh and one retry.** A 401 on a minted bearer
drops that bearer (and the shared copy, if it is still that one), mints
another and sends the call once more. A second refusal is the answer, never a
loop. Several callers refused at once still cause one exchange (O8).
*Proof:* `rung-agent-core/tests/oauth.rs::a_401_forces_one_refresh_and_one_retry`,
`rung-agent-core/tests/oauth.rs::a_second_401_is_the_answer_not_a_loop`.

**O12. A revoked or missing login is a clear auth failure, not a crash
loop.** GitHub refusing the long-lived token (401), the account having no
Copilot access (403/404), or no login at all, is an **auth** failure whose
message carries the command that fixes it (`rung-agent auth login
github-copilot --profile <p>`) and no token. It is not retried, and a process
that has seen GitHub reject a stored token asks GitHub no more until that
login changes. A host reads an auth failure as `blocked`: it probes slowly,
owns one message per incident and never exits (`docs/rung-host.md`, the
governor).
*Proof:* `rung-agent-core/tests/oauth.rs::a_revoked_login_is_an_actionable_auth_failure_and_is_not_retried_hot`,
`rung-agent-core/tests/oauth.rs::a_missing_login_says_how_to_make_one`,
`rung-agent-core/tests/oauth.rs::no_account_access_is_a_revoked_style_failure_and_a_throttle_waits`,
`rung-agent/tests/oauth.rs::a_turn_with_no_login_fails_with_the_command_that_fixes_it`.

**O13. Throttling is a backoff.** A 429 from the chat endpoint is a rate
limit carrying the `Retry-After` the provider sent, so the call's retry and a
host's governor wait that long instead of retrying hot. A throttled token
exchange is a rate limit too. Neither mints another token.
*Proof:* `rung-agent-core/tests/oauth.rs::quota_and_rate_refusals_from_the_api_are_a_backoff_not_a_hot_retry`,
`rung-agent-core/tests/oauth.rs::no_account_access_is_a_revoked_style_failure_and_a_throttle_waits`.

**O14. The model list is fetched and cached.** `rung-agent auth models
github-copilot` lists what the account may use (`GET {base}/models`, with
`X-GitHub-Api-Version`), cached an hour per login. v1 serves the
chat-completions style only: models that list endpoints without it are shown
and marked unsupported.
*Proof:* `rung-agent-core/tests/oauth.rs::the_model_list_is_fetched_filtered_and_cached`.

### Not leaking it

**O15. The redactor knows the shapes.** The credential redactor (H4,
`docs/rung-host-api.md`) removes GitHub's token prefixes (`ghp_ gho_ ghu_ ghs_
ghr_ github_pat_`) and a Copilot token (`tid=…;exp=…;…`, trailing sentence
punctuation excluded), in text, in every record line, partial frame and door
frame, in time linear in the text.
*Proof:* `rung-host/tests/redactor.rs` (the sentinel list and the hostile-text
test), `rung-host/tests/events_stream.rs::an_oauth_token_in_a_record_or_a_partial_frame_never_appears`,
`rung-host/tests/events_door.rs::an_oauth_token_never_reaches_a_door_client_in_a_record_or_a_delta`.

**O16. A secret never becomes text.** `Debug` of a login, a minted token, a
request auth, a credential or a config shows no token; errors name the login
(provider, profile), never its value; a provider's own error text is cut and
redacted before it is shown; the bearer is scrubbed from a failed call's body.
A turn writes no token into a session or any other file outside the login
store.
*Proof:* `rung-agent-core/tests/oauth.rs::no_debug_or_error_text_holds_a_token`,
`rung-agent/tests/oauth.rs::a_turn_on_an_oauth_credential_mints_once_and_sends_the_bearer`.

### Open and not yet built

**O17. Usage.** Copilot is not priced per token (premium-request multipliers
against a subscription). A call's usage already records tokens and the model
and leaves `cost_usd` unset, which the host reads as zero dollars. *Not yet:*
a cap in a non-dollar unit (requests) that stops an instance, and the host
(`rung-host`) selecting a login by profile for each instance
(`engine.auth`). Both are follow-ups; this change is the agent and the
kernel seam.

**O18. Decision 10 is open.** Whether GitHub's terms let a client that is not
an editor present an editor's identity and use this API is **not decided
here**. The mechanism is built; the identity is configuration (O10), and the
app id is `oauth.github-copilot.client_id`. Until the owner records the
decision, treat the defaults as placeholders.

**O19. Not exercised live.** Every test runs against a fake server. A real
login and a real chat call have not been made from this code. Two things a
live run may show: a route that refuses the prompt-cache markers `llm.cache:
auto` places for a Claude model id (set `llm.cache: none`), and that
`X-Initiator` (default `agent`) decides how a request is billed against the
subscription.

## Notes

- **Why a sibling file, not `auth.yaml`** (O5). The driver reads `auth.yaml`
  as `BTreeMap<String, String>` and treats a parse failure as empty. A login
  is structured and rotates its minted token, so it needs atomic whole-file
  replace and a lock, which a hand-edited key list does not.
- **Why a file lock** (O8). Hosts on one machine share one login. Without a
  lock each would see an expiring token and exchange at once. With one, the
  winner's token is already on disk when the others get the lock. `flock` is
  released by the kernel if its holder is killed, so a crash cannot wedge the
  login.
- **Why `rejected` also clears the shared copy only if it is the refused
  one** (O11). Callers refused at once each ask to drop the same token. The
  first drops it and mints; the rest find a different token on disk and keep
  it.
- **Protocol facts** (public; studied from another client's source, described
  here, not copied): device code at `{github}/login/device/code`, poll at
  `{github}/login/oauth/access_token` with grant
  `urn:ietf:params:oauth:grant-type:device_code`; exchange at
  `{api}/copilot_internal/v2/token` answers `{token, expires_at}`; the
  enterprise hosts are `https://<domain>` and `https://api.<domain>`.
- **Known quirk, not handled in v1.** A Gemini-family model on this route can
  end a stream with no terminal `finish_reason`. v1 certifies no model
  family; the list in O14 is what the account may use, not what is
  certified.
