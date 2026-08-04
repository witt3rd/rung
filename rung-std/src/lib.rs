//! rung-std — the canonical building blocks.
//!
//! A block is admitted here when it recurs across independent projects and
//! embeds no caller-specific knowledge. There are five:
//!
//! | block | surface | what recurs |
//! |---|---|---|
//! | [`llm`] | `ladder!` | one blocking LLM call, with retry |
//! | [`agent`] | `ladder!` | one agentic turn — drive the LLM, dispatch tools, iterate |
//! | [`questions`] | `theory!` + `ladder!` | questions posed, ruled on by an outside panel, folded back through a lifecycle |
//! | [`principals`] | `theory!` | who may be dispatched to — the law of the pool both gates draw from |
//! | [`driver`] | neither | hold suspended runs; release the ones evidence answers |
//!
//! The first four exercise the two halves of the DSL: `ladder!` declares
//! **arrows**, `theory!` declares **sentences**, and both live in `rung`.
//!
//! [`driver`] declares neither, and that is the point of it. A suspended run
//! has to be held by *something* between the dispatch that could not be settled
//! and the evidence that settles it, and that something must not decide
//! anything — no order, no depth cap, no timeout, no re-entry bound, all of
//! which are worth judgments Het declines to make (`het-declares-no-worth-law`).
//! It is the smallest mechanism that lets a composition run, and it is written
//! to stay that way.
//!
//! The fourth is the odd one, and deliberately: `rung` declares the *interface*
//! a supplier of `𝒫` must expose and refuses to say what a principal is made of
//! (`nothing-further-required`). [`principals`] is a **supplier** — it names the
//! kinds, the identity fields, the cost tiers and the shape of a population,
//! which is exactly the content Het declines to legislate.
//!
//! ---
//!
//! Canonical `LlmCall` rung ladder for reuse across any rung-based project.
//!
//! ## What this is
//!
//! A two-rung ladder (Pending → verdict) that wraps a single blocking HTTP
//! request to any OpenAI-compatible endpoint, including Anthropic's native
//! `/v1/messages` API.  Retryable failures surface as `Err(Failed)` → the
//! `retry` recover edge applies exponential backoff and decrements the
//! attempt counter.  Terminal failures exit as `Ok(LlmError(LlmFailure))`.
//! Success exits as `Ok(Success(LlmResponse))`.
//!
//! ## Configuration — full parity
//!
//! [`LlmConfig`] carries every parameter that may affect request behaviour:
//!
//! | Field | Wire |
//! |---|---|
//! | `base_url` | selects provider path + Anthropic vs. OpenAI format |
//! | `api_key` | `x-api-key` (Anthropic) or `Authorization: Bearer` (OpenAI) |
//! | `model` | `model` field in request body |
//! | `timeout_secs` | HTTP client timeout |
//! | `max_tokens` | `max_tokens` in request body |
//! | `temperature` | `temperature` in request body (both providers) |
//! | `reasoning_level` | `thinking.budget_tokens` (Anthropic) / `reasoning_effort` (OpenAI o-series) |
//!
//! ## How this ladder uses rung's guarantees
//!
//! A reader new to rung should see the distinctive features in play here.
//!
//! - **G2 (sealed construction).** Only the entry rung `Pending::new` is public.
//!   The `Success` / `LlmError` verdicts are built by `step` *inside* the module
//!   — no caller can fabricate a terminal outcome. The sealed constructor is not
//!   merely a fabrication guard; it is the free-category axiom: a verb (the HTTP
//!   call) lives on the arrow (`step`'s body), never in object-position
//!   (constructing a verdict from outside) — see `docs/rung-ct-props.md`,
//!   `the-law`.
//!
//! - **G7/G9 (recover pairing — error-path semantics).** The single recover edge
//!   `retry: Failed(Pending) => Pending` is an **error-path** recovery (rung-props.md G9):
//!   it re-enters with the *unconsumed token* handed back in `Failed`, and it is
//!   deliberately **unguarded** — a retry after a transient network failure may
//!   legitimately re-send the *identical* request. This is the mirror of Lesson 2's
//!   verdict recover (`Stalled => Active`), which *is* guarded (G8) because a stall
//!   loop must make progress. The two recover forms exist for different intents.
//!
//! - **G5 (carry immutability).** `call_id` is carried on every rung as a private
//!   field, readable only through `&Carry`. The recover edge threads it forward
//!   unchanged — witness data is structurally shared, never mutated in flight.
//!
//! - **G4 (no silent drop).** Every generated token — `Pending`, `Success`,
//!   `LlmError`, `Failed`, `StepOutcome` — is `#[must_use]`. Dropping any of them
//!   without consuming or recovering it is a warning, and an error under
//!   `#![deny(unused_must_use)]`. The non-token `LlmResponse` below carries the
//!   same attribute as a style exemplar.
//!
//! - **G11 (terminal payloads).** `Success(LlmResponse)` carries the structured
//!   result out through the verdict; the caller reads it via `.payload()`.
//!
//! - **Streaming is a side-channel, not a rung.** The `StreamListener` receives
//!   incremental SSE events as a read-only notification. The ladder still blocks
//!   until the stream ends and then resolves to a verdict — streaming does not
//!   introduce a new transition. The verb (HTTP I/O) remains on the arrow.
//!

pub mod agent;
pub mod driver;
pub mod llm;
pub mod principals;
pub mod questions;
pub mod tools;
