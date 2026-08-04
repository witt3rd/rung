//! The `ladder!` proc macro — a compiler for type-state ladders.
//!
//! Parses the ladder syntax, runs 10 static checks, and emits a sealed Rust
//! module (rung/verdict structs, the `StepOutcome` enum, and — with an inline
//! `impl { .. }` block — the transition/recover functions). The borrow checker
//! enforces linear consumption; the macro enforces structural correctness.
//! See `SPEC.md` for the normative rules.

use proc_macro::TokenStream;
use quote::{ToTokens, format_ident, quote};
use syn::{
    Ident, Token, Type, braced,
    parse::{Parse, ParseStream},
    parse_macro_input,
    punctuated::Punctuated,
    spanned::Spanned,
};

// ── AST (mirrors the Python rung/ast.py) ────────────────────────────────────

struct Ladder {
    name: Ident,
    carry_fields: Vec<CarryField>,
    rungs: Vec<Rung>,
    transitions: Vec<Transition>,
    recover_edges: Vec<RecoverEdge>,
    recover_fns: Vec<RecoverFn>,
    /// Inline transition/recover bodies from a trailing `impl { .. }` block.
    /// Empty ⇒ type-only declaration (structs, enum, guards — no logic).
    bodies: Vec<TransitionBody>,
}

/// One `name = |arg| { .. }` entry in the trailing `impl { .. }` block.
struct TransitionBody {
    name: Ident,
    closure: syn::ExprClosure,
}

struct CarryField {
    name: Ident,
    ty: Type,
}

struct Rung {
    name: Ident,
    payload_type: Type,
}

struct Transition {
    name: Ident,
    from_rung: Ident,
    // None if branching (has verdicts instead)
    to_rung: Option<Ident>,
    verdicts: Vec<Verdict>,
    /// The gate marker on this transition's *target*, if any (SPEC.md §1).
    ///
    /// `Some(role)` ⇒ `#[judgmental(role)]`: the transition takes a second
    /// parameter, `::rung::Qualified<role>`, and so cannot be called without a
    /// token minted by the pool. `None` ⇒ unmarked, which reads as *decidable*
    /// and emits byte-for-byte what it emitted before markers existed.
    ///
    /// The other two gates never reach here — `#[authorial]` and
    /// `#[conditional(..)]` are refused at parse time.
    gate_role: Option<Type>,
}

/// Parse the optional gate marker in front of a transition target.
///
/// The marker annotates the **target** rung, because a forward transition is
/// named after its target (SPEC.md §1, "Transition naming"):
///
/// ```text
/// Spec(SpecData)
///   => #[judgmental(Reviewer)] Active(LoopState)
///   => #[judgmental(Judge)] { Converged(Report) | Stalled => Active }
/// ```
///
/// Returns the declared competence role, or `None` when unmarked.
fn parse_gate_marker(input: ParseStream) -> syn::Result<Option<Type>> {
    if !input.peek(Token![#]) {
        return Ok(None);
    }
    let attrs = input.call(syn::Attribute::parse_outer)?;
    if attrs.len() > 1 {
        return Err(syn::Error::new(
            attrs[1].span(),
            "a transition carries at most one gate marker: Het's four gates are \
             alternatives, not a set (see rung-het-propositions.md#four-gates)",
        ));
    }
    let attr = &attrs[0];
    let Some(gate) = attr.path().get_ident().map(|i| i.to_string()) else {
        return Err(syn::Error::new(
            attr.span(),
            "unknown gate marker; the only marker `ladder!` implements is \
             `#[judgmental(Role)]`",
        ));
    };

    match gate.as_str() {
        "judgmental" => match &attr.meta {
            syn::Meta::List(list) => Ok(Some(list.parse_args::<Type>()?)),
            _ => Err(syn::Error::new(
                attr.span(),
                "`#[judgmental]` must name the competence role it requires — write \
                 `#[judgmental(Role)]`. A judgmental operation declares the role \
                 needed to discharge it (rung-het-propositions.md#judgmental-declares-role); \
                 an unnamed role cannot resolve a judge, so there is no signature \
                 to emit.",
            )),
        },
        "authorial" => Err(syn::Error::new(
            attr.span(),
            "`#[authorial]` is not yet supported. The authorial gate needs a third \
             signature — an `Authorized` pen over a declared standing predicate \
             (rung-het-propositions.md#authorial-declares-standing) — and standing is \
             conditional-gated, so it is settled per call rather than per declaration \
             (rung-het-propositions.md#standing-conditional-gated). Only \
             `#[judgmental(Role)]` is implemented. See \
             docs/questions/open/q11-gate-faithfulness.md.",
        )),
        "conditional" => Err(syn::Error::new(
            attr.span(),
            "`#[conditional(..)]` is not yet supported. Het classifies a conditional \
             gate per model, one level up (rung-het-propositions.md#classifier-one-level-up), \
             while `ladder!`'s checks run at expansion time against a single \
             declaration; the two do not yet meet. This is the open half of Q11 — \
             see docs/questions/open/q11-gate-faithfulness.md.",
        )),
        other => Err(syn::Error::new(
            attr.span(),
            format!(
                "unknown gate marker `{other}`; the only marker `ladder!` implements \
                 is `#[judgmental(Role)]`. An unmarked transition reads as \
                 `decidable` and takes no qualifying token."
            ),
        )),
    }
}

struct Verdict {
    name: Ident,
    is_terminal: bool,
    recover_target: Option<Ident>,
    /// Optional result payload, e.g. `Converged(Report)`. Terminal verdicts only —
    /// a recoverable verdict carries its source rung instead (checked).
    payload_type: Option<Type>,
    /// `Some(rung)` for a *continue* arm, `Name -> Rung`: `step` produces the next
    /// rung directly (no recover fn, no guard). The `StepOutcome` variant carries
    /// that rung as a live token rather than a verdict marker.
    continue_target: Option<Ident>,
}

/// Parse one verdict inside a `{ .. }` branching block:
/// `Name` / `Name(Payload)` (terminal), `Name => Rung` (recoverable, guarded),
/// or `Name -> Rung` (continue: step produces the rung directly).
fn parse_verdict(block: ParseStream) -> syn::Result<Verdict> {
    let name: Ident = block.parse()?;
    let payload_type = if block.peek(syn::token::Paren) {
        let inner;
        syn::parenthesized!(inner in block);
        Some(inner.parse::<Type>()?)
    } else {
        None
    };
    let mut is_terminal = true;
    let mut recover_target = None;
    let mut continue_target = None;
    if block.peek(Token![=>]) {
        block.parse::<Token![=>]>()?;
        is_terminal = false;
        recover_target = Some(block.parse()?);
    } else if block.peek(Token![->]) {
        block.parse::<Token![->]>()?;
        is_terminal = false;
        continue_target = Some(block.parse()?);
    }
    Ok(Verdict {
        name,
        is_terminal,
        recover_target,
        payload_type,
        continue_target,
    })
}

/// Parse a verdict branching block `{ verdict ( "|" verdict )* }`.
fn parse_verdict_block(content: ParseStream) -> syn::Result<Vec<Verdict>> {
    let block;
    braced!(block in content);
    let mut verdicts = Vec::new();
    let mut first = true;
    while !block.is_empty() {
        if !first {
            block.parse::<Token![|]>()?;
        }
        first = false;
        verdicts.push(parse_verdict(&block)?);
    }
    Ok(verdicts)
}

struct RecoverEdge {
    name: Ident,
    from_verdict: Ident,
    to_rung: Ident,
}

struct RecoverFn {
    name: Ident,
    param_type: Type,
    return_rung: Ident,
    /// `Some(rung)` when this recovers from the error path (`Failed(rung) => ..`),
    /// rather than from a verdict. Failed-recovery has no verdict edge and no
    /// auto-injected progress guard (a retry may legitimately reuse the token).
    from_failed: Option<Ident>,
}

// ── parser ──────────────────────────────────────────────────────────────────

impl Parse for Ladder {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let name: Ident = input.parse()?;
        let content;
        braced!(content in input);

        let mut carry_fields = Vec::new();
        let mut rungs = Vec::new();
        let mut transitions = Vec::new();
        let mut recover_edges = Vec::new();
        let mut recover_fns = Vec::new();

        while !content.is_empty() {
            // Peek: if the next token is an ident and it's "carry" or "recover",
            // handle it specially. Otherwise, parse as a rung.
            let is_carry_or_recover = content
                .fork()
                .parse::<Ident>()
                .map(|kw: Ident| kw == "carry" || kw == "recover")
                .unwrap_or(false);

            if is_carry_or_recover {
                let kw: Ident = content.parse()?;
                if kw == "carry" {
                    // carry { field: Type, ... }
                    let block;
                    braced!(block in content);
                    let fields: Punctuated<CarryField, Token![,]> =
                        block.parse_terminated(CarryField::parse, Token![,])?;
                    carry_fields = fields.into_iter().collect();
                    let _ = content.parse::<Token![;]>();
                } else if kw == "recover" {
                    // recover { name: Type => Rung(Type), ... }
                    let block;
                    braced!(block in content);
                    while !block.is_empty() {
                        let edge_name: Ident = block.parse()?;
                        block.parse::<Token![:]>()?;

                        // Detect the error-path form: `name: Failed(Rung) => Rung`.
                        let fork = block.fork();
                        let is_failed = fork
                            .parse::<Ident>()
                            .map(|i| i == "Failed")
                            .unwrap_or(false)
                            && fork.peek(syn::token::Paren);

                        if is_failed {
                            let _failed_kw: Ident = block.parse()?;
                            let inner;
                            syn::parenthesized!(inner in block);
                            let from_rung: Ident = inner.parse()?;
                            block.parse::<Token![=>]>()?;
                            let return_rung: Ident = block.parse()?;
                            let param_type: Type = syn::parse_quote!(Failed<#from_rung>);
                            recover_fns.push(RecoverFn {
                                name: edge_name,
                                param_type,
                                return_rung,
                                from_failed: Some(from_rung),
                            });
                            let _ = block.parse::<Token![;]>();
                            continue;
                        }

                        // Verdict-recovery form: `name: Verdict => Rung[(payload)]`.
                        let param_type: Type = block.parse()?;
                        block.parse::<Token![=>]>()?;
                        let return_rung: Ident = block.parse()?;
                        let _payload: Type;
                        if block.peek(syn::token::Paren) {
                            let p;
                            syn::parenthesized!(p in block);
                            _payload = p.parse()?;
                        } else {
                            _payload = param_type.clone();
                        }
                        let fv = Ident::new(
                            &param_type.to_token_stream().to_string().replace(' ', ""),
                            param_type.span(),
                        );
                        recover_edges.push(RecoverEdge {
                            name: edge_name.clone(),
                            from_verdict: fv,
                            to_rung: return_rung.clone(),
                        });
                        recover_fns.push(RecoverFn {
                            name: edge_name,
                            param_type,
                            return_rung,
                            from_failed: None,
                        });
                        let _ = block.parse::<Token![;]>();
                    }
                } else {
                    return Err(syn::Error::new(
                        kw.span(),
                        format!("unexpected identifier `{kw}`"),
                    ));
                }
            } else {
                // Spine: `rung ( "=>" rung )* "=>" "{" verdicts "}"` (SPEC.md §1).
                // Parse the first rung, then loop over forward hops until a verdict
                // block terminates the spine. Each hop `=> Next` adds a rung and a
                // forward transition named after its target (lowercased); a `=> { .. }`
                // adds the branching `step` transition and ends the spine.
                let mut cur_name: Ident = content.parse()?;
                let payload;
                syn::parenthesized!(payload in content);
                let payload_type: Type = payload.parse()?;
                rungs.push(Rung {
                    name: cur_name.clone(),
                    payload_type,
                });

                while content.peek(Token![=>]) {
                    content.parse::<Token![=>]>()?;

                    // The gate marker sits between the `=>` and the target it
                    // annotates — which is also the transition it names.
                    let gate_role = parse_gate_marker(&content)?;

                    if content.peek(syn::token::Brace) {
                        // Verdict branching terminates the spine. It is markable
                        // too: the branching transition is the `dispose` position
                        // of Het's pass, and that is a judgmental arrow.
                        let verdicts = parse_verdict_block(&content)?;
                        transitions.push(Transition {
                            name: format_ident!("step"),
                            from_rung: cur_name.clone(),
                            to_rung: None,
                            verdicts,
                            gate_role,
                        });
                        break;
                    }

                    // Another forward hop: `=> Next(Type)`.
                    let next_name: Ident = content.parse()?;
                    let next_payload;
                    syn::parenthesized!(next_payload in content);
                    let next_type: Type = next_payload.parse()?;
                    rungs.push(Rung {
                        name: next_name.clone(),
                        payload_type: next_type,
                    });
                    transitions.push(Transition {
                        name: format_ident!("{}", next_name.to_string().to_lowercase()),
                        from_rung: cur_name.clone(),
                        to_rung: Some(next_name.clone()),
                        verdicts: vec![],
                        gate_role,
                    });
                    cur_name = next_name;
                }
                let _ = content.parse::<Token![;]>();
            }
        }

        // Optional trailing `impl { name = |arg| { body } ... }` block: inline
        // transition/recover bodies. When present, the macro expands them *inside*
        // the module (so construction stays sealed) and auto-injects the recovery
        // guard. When absent, only the types are emitted (a structural declaration).
        let mut bodies = Vec::new();
        if input.peek(Token![impl]) {
            input.parse::<Token![impl]>()?;
            let block;
            braced!(block in input);
            while !block.is_empty() {
                let name: Ident = block.parse()?;
                block.parse::<Token![=]>()?;
                let closure: syn::ExprClosure = block.parse()?;
                bodies.push(TransitionBody { name, closure });
                // entries separated by `,` or `;` (trailing optional)
                let _ = block.parse::<Token![,]>();
                let _ = block.parse::<Token![;]>();
            }
        }

        Ok(Ladder {
            name,
            carry_fields,
            rungs,
            transitions,
            recover_edges,
            recover_fns,
            bodies,
        })
    }
}

impl Parse for CarryField {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let name: Ident = input.parse()?;
        input.parse::<Token![:]>()?;
        let ty: Type = input.parse()?;
        Ok(CarryField { name, ty })
    }
}

// ── checker (10 rules: 1–8 structural, mirror rung/checker.py; 9–10 impl-block) ─

fn check(ladder: &Ladder) -> Result<(), String> {
    let rung_names: Vec<String> = ladder.rungs.iter().map(|r| r.name.to_string()).collect();
    let recover_fn_names: Vec<String> = ladder
        .recover_fns
        .iter()
        .map(|r| r.name.to_string())
        .collect();
    let carry_names: Vec<String> = ladder
        .carry_fields
        .iter()
        .map(|c| c.name.to_string())
        .collect();

    // 1. duplicate carry fields
    let mut seen = std::collections::HashSet::new();
    for name in &carry_names {
        if !seen.insert(name) {
            return Err(format!("duplicate carry field `{name}`"));
        }
    }

    // 2. transitions reference declared rungs
    for t in &ladder.transitions {
        if !rung_names.contains(&t.from_rung.to_string()) {
            return Err(format!(
                "transition `{}`: from_rung `{}` not declared",
                t.name, t.from_rung
            ));
        }
        if let Some(ref to) = t.to_rung
            && !rung_names.contains(&to.to_string())
        {
            return Err(format!(
                "transition `{}`: to_rung `{}` not declared",
                t.name, to
            ));
        }
    }

    // 3. verdicts are valid
    for t in &ladder.transitions {
        for v in &t.verdicts {
            if let Some(ref cont) = v.continue_target {
                // Continue arm `Name -> Rung`: target must be a declared rung, and
                // it carries that rung — not a payload.
                if !rung_names.contains(&cont.to_string()) {
                    return Err(format!(
                        "continue arm `{}`: target rung `{}` not declared",
                        v.name, cont
                    ));
                }
                if v.payload_type.is_some() {
                    return Err(format!(
                        "continue arm `{}` cannot declare a payload; it carries its target rung",
                        v.name
                    ));
                }
            } else if !v.is_terminal {
                // Recoverable verdict `Name => Rung`.
                if let Some(ref target) = v.recover_target
                    && !rung_names.contains(&target.to_string())
                {
                    return Err(format!(
                        "verdict `{}`: recover_target `{}` not declared",
                        v.name, target
                    ));
                }
                // A recoverable verdict carries its source rung, not a payload.
                if v.payload_type.is_some() {
                    return Err(format!(
                        "recoverable verdict `{}` cannot declare a payload; it carries its source rung (use `{}(..)` only on terminal verdicts)",
                        v.name, v.name
                    ));
                }
            }
        }
    }

    // 4. every recoverable verdict has a matching RecoverEdge (continue arms don't)
    for t in &ladder.transitions {
        for v in &t.verdicts {
            if !v.is_terminal && v.continue_target.is_none() {
                let found = ladder
                    .recover_edges
                    .iter()
                    .any(|re| re.from_verdict == v.name);
                if !found {
                    return Err(format!(
                        "recoverable verdict `{}` on transition `{}`: no matching RecoverEdge",
                        v.name, t.name
                    ));
                }
            }
        }
    }

    // 5. every RecoverEdge has a matching RecoverFn
    for re in &ladder.recover_edges {
        if !recover_fn_names.contains(&re.name.to_string()) {
            return Err(format!("RecoverEdge `{}`: no matching RecoverFn", re.name));
        }
        if !rung_names.contains(&re.to_rung.to_string()) {
            return Err(format!(
                "RecoverEdge `{}`: to_rung `{}` not declared",
                re.name, re.to_rung
            ));
        }
    }

    // 6. terminal verdicts must NOT have recover edges
    for t in &ladder.transitions {
        for v in &t.verdicts {
            if v.is_terminal {
                for re in &ladder.recover_edges {
                    if re.from_verdict == v.name {
                        return Err(format!(
                            "terminal verdict `{}` has RecoverEdge `{}`",
                            v.name, re.name
                        ));
                    }
                }
            }
        }
    }

    // 7. RecoverEdge references a known verdict
    for re in &ladder.recover_edges {
        let found = ladder
            .transitions
            .iter()
            .any(|t| t.verdicts.iter().any(|v| v.name == re.from_verdict));
        if !found {
            return Err(format!(
                "RecoverEdge `{}`: from_verdict `{}` not declared on any transition",
                re.name, re.from_verdict
            ));
        }
    }

    // 8. RecoverFn return_rung is declared; a Failed-recovery's source rung too.
    for rf in &ladder.recover_fns {
        if !rung_names.contains(&rf.return_rung.to_string()) {
            return Err(format!(
                "RecoverFn `{}`: return_rung `{}` not declared",
                rf.name, rf.return_rung
            ));
        }
        if let Some(ref from) = rf.from_failed
            && !rung_names.contains(&from.to_string())
        {
            return Err(format!(
                "recover `{}`: `Failed({})` names an undeclared rung",
                rf.name, from
            ));
        }
    }

    // 9 & 10. If an inline `impl { .. }` block is present, its bodies must
    // correspond exactly to the ladder's transition + recover functions.
    if !ladder.bodies.is_empty() {
        let expected: Vec<String> = ladder
            .transitions
            .iter()
            .filter(|t| t.to_rung.is_some() || !t.verdicts.is_empty())
            .map(|t| t.name.to_string())
            .chain(ladder.recover_fns.iter().map(|rf| rf.name.to_string()))
            .collect();

        // 9. every body names a real transition/recover fn (no phantom bodies)
        let mut seen = std::collections::HashSet::new();
        for b in &ladder.bodies {
            let n = b.name.to_string();
            if !expected.contains(&n) {
                return Err(format!(
                    "impl body `{n}` does not match any transition or recover function"
                ));
            }
            if !seen.insert(n.clone()) {
                return Err(format!("impl body `{n}` is defined more than once"));
            }
        }
        // 10. every transition/recover fn has a body (no gaps)
        for e in &expected {
            if !ladder.bodies.iter().any(|b| b.name == *e) {
                return Err(format!("impl block is missing a body for `{e}`"));
            }
        }
    }

    Ok(())
}

// ── code generator ──────────────────────────────────────────────────────────

/// The identifier a closure input binds, for use in an *expression* position.
///
/// `arg_pat` splices a closure input into a parameter list, where `spec: Spec`
/// is legal. An injected prologue needs the binding alone. Falls back to
/// `default` when the input is absent or is not a plain (possibly typed)
/// identifier — the same fallback the parameter list uses, so the two agree.
fn pat_binding(input: Option<&syn::Pat>, default: &str) -> Ident {
    fn ident_of(p: &syn::Pat) -> Option<Ident> {
        match p {
            syn::Pat::Ident(pi) => Some(pi.ident.clone()),
            syn::Pat::Type(pt) => ident_of(&pt.pat),
            _ => None,
        }
    }
    input
        .and_then(ident_of)
        .unwrap_or_else(|| Ident::new(default, proc_macro2::Span::call_site()))
}

fn emit(ladder: &Ladder) -> proc_macro2::TokenStream {
    let mod_name = format_ident!("{}", ladder.name.to_string().to_lowercase());
    let mod_vis = quote! { pub };

    // ── Carry struct ────────────────────────────────────────────────────
    let carry_fields: Vec<_> = ladder
        .carry_fields
        .iter()
        .map(|f| {
            let name = &f.name;
            let ty = &f.ty;
            quote! { pub #name: #ty }
        })
        .collect();
    let carry_struct = if carry_fields.is_empty() {
        quote! {}
    } else {
        quote! {
            #[derive(Clone, Debug)]
            pub struct Carry { #(#carry_fields),* }
        }
    };

    // ── Rung structs (sealed) + sealed constructor + carry accessor ──────
    let has_carry = !carry_fields.is_empty();
    let has_bodies = !ladder.bodies.is_empty();
    let entry_name = ladder.rungs.first().map(|r| r.name.to_string());
    let rung_structs: Vec<_> = ladder
        .rungs
        .iter()
        .map(|r| {
            let name = &r.name;
            let payload = &r.payload_type;
            let is_entry = entry_name.as_deref() == Some(&name.to_string());
            // Constructor visibility (SPEC.md G2):
            //   - With an inline `impl { .. }` block, transition bodies live INSIDE
            //     the module, so only the *entry* rung needs a public constructor
            //     (to start a run). Every downstream rung's `new` is module-private —
            //     no external code can fabricate a mid-ladder token (this is G2).
            //   - Without bodies (a type-only declaration), all constructors are
            //     `pub` so external code (e.g. a hand-written driver) can build them.
            let ctor_vis = if has_bodies && !is_entry {
                quote! {}
            } else {
                quote! { pub }
            };
            let (carry_field, carry_ctor_param, carry_ctor_init, carry_accessor) = if has_carry {
                (
                    quote! { carry: Carry, },
                    quote! { , carry: Carry },
                    quote! { carry, },
                    quote! {
                        /// Immutable witness data. Never consumed; read via shared reference.
                        pub fn carry(&self) -> &Carry { &self.carry }
                    },
                )
            } else {
                (quote! {}, quote! {}, quote! {}, quote! {})
            };
            quote! {
                // `_not_send: PhantomData<*const ()>` makes every rung `!Send + !Sync`.
                // This enforces the linear-token contract across threads: an `Arc<#name>`
                // or `&#name` cannot cross a thread boundary, so two threads can never
                // drive a transition on the same logical token. Rust's move semantics
                // enforce one-consumer for owned values; this closes the shared-reference
                // hole (SPEC.md G3). Constructed inside the module alongside `_seal`.
                //
                // `#[must_use]`: Rust types are affine (may be silently dropped), but the
                // linear-token contract is "consumed *exactly* once". Move semantics give
                // "at most once"; this attribute guards "at least once" — dropping a live
                // rung without advancing it or returning it in a `Failed` is a warning
                // (an error under `#![deny(unused_must_use)]`). Closes the no-silent-drop
                // half of linearity without waiting on language-level linear types (SPEC.md §5).
                #[must_use = "a rung token must be consumed by a transition or returned in a Failed; dropping it silently abandons the ladder run"]
                pub struct #name {
                    _seal: (),
                    _not_send: ::core::marker::PhantomData<*const ()>,
                    #carry_field
                    pub payload: #payload,
                }
                impl #name {
                    /// Sealed constructor — the only way to mint this rung.
                    #[allow(dead_code)]
                    #ctor_vis fn new(payload: #payload #carry_ctor_param) -> Self {
                        Self {
                            _seal: (),
                            _not_send: ::core::marker::PhantomData,
                            #carry_ctor_init
                            payload,
                        }
                    }
                    #carry_accessor
                }
            }
        })
        .collect();

    // ── Failed<Prev> ────────────────────────────────────────────────────
    let failed_type = quote! {
        // `#[must_use]`: a `Failed` holds the unconsumed token from a failed transition.
        // Dropping it swallows both the error and the token — the ladder run vanishes
        // with no recovery and no completion. Force the caller to handle it.
        #[must_use = "a Failed carries the unconsumed token and the error; dropping it swallows both — handle it or recover from it"]
        pub struct Failed<Prev> { pub token: Prev, pub error: String }
    };

    // ── Verdict structs (sealed, !Send, constructible) ───────────────────
    // Verdicts are outcome tokens, held to the same seal as rungs (SPEC.md G3):
    // private `_seal` + `_not_send: PhantomData<*const ()>`.
    // A *recoverable* verdict additionally carries the rung it was produced from
    // (`source`), so its recover edge has the full prior context to re-enter with —
    // without this, recovery would have to fabricate the next rung from nothing.
    let verdict_structs: Vec<_> = ladder
        .transitions
        .iter()
        .flat_map(|t| {
            let from_rung = t.from_rung.clone();
            // Verdict constructors follow the same SPEC.md G2 visibility rule as rungs:
            // module-private when transition bodies are inline (they build verdicts
            // in-module), `pub` for a type-only declaration.
            let vctor_vis = if has_bodies { quote! {} } else { quote! { pub } };
            t.verdicts.iter().map(move |v| {
                let name = &v.name;
                let vis = &vctor_vis;
                let common_must_use = "a verdict is the outcome of a step; dropping it discards the outcome (recoverable verdicts must be fed to their recover edge)";
                // Continue arms carry a live target rung, not a verdict marker — no
                // struct is emitted for them (the StepOutcome variant holds the rung).
                if v.continue_target.is_some() {
                    quote! {}
                } else if v.is_terminal {
                    // A terminal verdict may carry a result payload, e.g.
                    // `Converged(Report)` — how a run returns a value through the
                    // verdict instead of a contentless marker.
                    if let Some(payload) = &v.payload_type {
                        quote! {
                            #[must_use = #common_must_use]
                            pub struct #name {
                                _seal: (),
                                _not_send: ::core::marker::PhantomData<*const ()>,
                                payload: #payload,
                            }
                            impl #name {
                                /// Sealed constructor for a terminal verdict with a result payload.
                                #[allow(dead_code)]
                                #vis fn new(payload: #payload) -> Self {
                                    Self { _seal: (), _not_send: ::core::marker::PhantomData, payload }
                                }
                                /// Borrow the result payload.
                                #[allow(dead_code)]
                                pub fn payload(&self) -> &#payload { &self.payload }
                                /// Consume the verdict, taking the result payload.
                                #[allow(dead_code)]
                                pub fn into_payload(self) -> #payload { self.payload }
                            }
                        }
                    } else {
                        quote! {
                            #[must_use = #common_must_use]
                            pub struct #name {
                                _seal: (),
                                _not_send: ::core::marker::PhantomData<*const ()>,
                            }
                            impl #name {
                                /// Sealed constructor for a terminal verdict.
                                #[allow(dead_code)]
                                #vis fn new() -> Self {
                                    Self { _seal: (), _not_send: ::core::marker::PhantomData }
                                }
                            }
                        }
                    }
                } else {
                    // Recoverable: carries the source rung for re-entry.
                    let from = &from_rung;
                    quote! {
                        #[must_use = #common_must_use]
                        pub struct #name {
                            _seal: (),
                            _not_send: ::core::marker::PhantomData<*const ()>,
                            source: #from,
                        }
                        impl #name {
                            /// Sealed constructor. `source` is the rung this verdict
                            /// was produced from; recovery re-enters from it.
                            #[allow(dead_code)]
                            #vis fn new(source: #from) -> Self {
                                Self { _seal: (), _not_send: ::core::marker::PhantomData, source }
                            }
                            /// Borrow the rung this verdict was produced from.
                            #[allow(dead_code)]
                            pub fn source(&self) -> &#from { &self.source }
                            /// Consume the verdict, recovering the source rung.
                            #[allow(dead_code)]
                            pub fn into_source(self) -> #from { self.source }
                        }
                    }
                }
            })
        })
        .collect();

    // ── Verdict enum ────────────────────────────────────────────────────
    let verdicts: Vec<_> = ladder
        .transitions
        .iter()
        .flat_map(|t| t.verdicts.iter())
        .collect();

    let verdict_enum = if verdicts.is_empty() {
        quote! {}
    } else {
        let variants: Vec<_> = verdicts
            .iter()
            .map(|v| {
                let name = &v.name;
                // A continue arm's variant carries the live target rung directly;
                // every other variant carries its verdict struct.
                match &v.continue_target {
                    Some(rung) => quote! { #name(#rung), },
                    None => quote! { #name(#name), },
                }
            })
            .collect();
        quote! {
            // `#[must_use]`: the result of a step. Dropping it — including the
            // `Continue(Active)` variant that carries a live rung — silently abandons
            // the run. The caller must match on it and route every variant.
            #[must_use = "StepOutcome is the result of a step and may carry a live rung (Continue); match on it — dropping it abandons the run"]
            pub enum StepOutcome { #(#variants)* }
        }
    };

    // ── Transition + recover bodies (inline `impl { .. }` form) ──────────
    // When an `impl { .. }` block is present, the transition/recover bodies expand
    // as `pub fn`s INSIDE the module. Because they live inside the seal boundary,
    // they use the now-private constructors (no external fabrication, SPEC.md G2) and the
    // macro wraps each recover body with the progress guard (SPEC.md G8 enforced, not by
    // convention). There is no `Transitions` trait — one API surface.
    let body_for =
        |n: &str| -> Option<&TransitionBody> { ladder.bodies.iter().find(|b| b.name == n) };
    // Pull the (single) argument pattern out of a `|arg| { .. }` closure — it
    // becomes the generated function's parameter directly (so the closure body
    // needs no rebinding, and a `{ .. }` body isn't double-wrapped).
    let arg_pat = |c: &syn::ExprClosure| -> proc_macro2::TokenStream {
        c.inputs
            .first()
            .map(|p| quote! { #p })
            .unwrap_or(quote! { __arg })
    };
    // The closure body as a function body: use its block directly if it is one,
    // else wrap the single expression in braces. Avoids `unused_braces`.
    let fn_body = |c: &syn::ExprClosure| -> proc_macro2::TokenStream {
        match &*c.body {
            syn::Expr::Block(eb) => quote! { #eb },
            other => quote! { { #other } },
        }
    };
    // The closure body's *statements*, for splicing after an injected prologue.
    // A block body contributes its statements directly (rather than nesting a
    // block inside the fn body, which would trip `unused_braces` when the body
    // is a single expression); anything else contributes itself as the tail.
    let fn_body_stmts = |c: &syn::ExprClosure| -> proc_macro2::TokenStream {
        match &*c.body {
            syn::Expr::Block(eb) => {
                let stmts = &eb.block.stmts;
                quote! { #(#stmts)* }
            }
            other => quote! { #other },
        }
    };

    // The gate parameter, appended to a marked transition's signature.
    //
    // THE ENFORCING LINE is the `None => quote! {}` arm: an unmarked transition
    // contributes an empty token stream, so its emitted signature is identical
    // to what it was before gate markers existed. Everything downstream of this
    // function is shared by both cases.
    //
    // For `#[judgmental(R)]` the parameter is `::rung::Qualified<R>`, taken **by
    // value** — a judgment licence is spent on the arrow it was minted for. The
    // token has no constructor outside `Pool::qualify`, so a marked transition
    // cannot be called without one, and an unmarked one has no parameter that
    // could admit one. Mis-marking is then not a claim that could be false; it
    // is a signature the compiler either accepts or does not.
    //
    // The token's name comes from the body's *second* closure input when there
    // is one (`active = |spec, q| { .. }`), so a body can read the judge off it;
    // otherwise it is bound to `_q` and consumed unread.
    let gate_param = |t: &Transition, closure: &syn::ExprClosure| -> proc_macro2::TokenStream {
        match &t.gate_role {
            Some(role) => {
                let qpat = closure
                    .inputs
                    .iter()
                    .nth(1)
                    .map(|p| quote! { #p })
                    .unwrap_or(quote! { _q });
                quote! { , #qpat: ::rung::Qualified<#role> }
            }
            None => quote! {},
        }
    };

    // The **injected gate prologue** (SPEC.md G13) — the second half of Q11.
    //
    // `gate_param` above makes the *signature* honest: a judgmental transition
    // cannot be called without a token. It does not make the *arrow* admissible,
    // because the token could have been minted against some other argument.
    // Het's disjointness-against-argument requires `π(p) ∩ π(a) = ∅` for the very
    // `a` the operation is applied to, so the token has to be admitted against
    // the source rung's payload before the body runs.
    //
    // The body is the DOMAIN's, so the check cannot live there — a body that
    // simply never looked at its token would discharge nothing and nothing could
    // notice. The macro therefore injects it as a prologue, exactly as it injects
    // `must_progress` around a recover body for G8, and for the same reason: a
    // guarantee a body can skip is not a guarantee.
    //
    // This is what makes `#[judgmental(R)]` require the source rung's payload to
    // be `::rung::Provenanced` — without `π(a)` there is nothing to measure.
    let gate_prologue = |t: &Transition, closure: &syn::ExprClosure| -> proc_macro2::TokenStream {
        if t.gate_role.is_none() {
            return quote! {};
        }
        let arg = pat_binding(closure.inputs.first(), "__arg");
        let q = pat_binding(closure.inputs.iter().nth(1), "_q");
        quote! { must_be_bound_to(&#arg.payload, &#q); }
    };

    let logic = if has_bodies {
        let transition_fns: Vec<_> = ladder
            .transitions
            .iter()
            .filter_map(|t| {
                let name = &t.name;
                let from = &t.from_rung;
                let b = body_for(&name.to_string())?;
                let pat = arg_pat(&b.closure);
                let gate_param = gate_param(t, &b.closure);
                let prologue = gate_prologue(t, &b.closure);
                // Unmarked: emit byte-for-byte what was emitted before markers
                // existed (SPEC.md G12). Marked: prologue first, then the body's
                // statements — the body cannot run ahead of the check (G13).
                let body = if t.gate_role.is_none() {
                    fn_body(&b.closure)
                } else {
                    let stmts = fn_body_stmts(&b.closure);
                    quote! { { #prologue #stmts } }
                };
                if let Some(ref to) = t.to_rung {
                    Some(quote! {
                        pub fn #name(#pat: #from #gate_param) -> #to #body
                    })
                } else if !t.verdicts.is_empty() {
                    Some(quote! {
                        pub fn #name(#pat: #from #gate_param) -> Result<StepOutcome, Failed<#from>> #body
                    })
                } else {
                    None
                }
            })
            .collect();

        let recover_fns: Vec<_> = ladder
            .recover_fns
            .iter()
            .map(|rf| {
                let name = &rf.name;
                let param = &rf.param_type;
                let ret = &rf.return_rung;
                // `check()` guarantees a body exists for every recover fn.
                let b = body_for(&name.to_string()).expect("recover body checked");
                let pat = arg_pat(&b.closure);
                if rf.from_failed.is_some() {
                    // Error-path recovery (`Failed(rung) => rung`): no progress guard.
                    // A retry after a transient error may legitimately reuse the token
                    // (the `Failed`'s `.token` field), so progress is the body's call.
                    let body = fn_body(&b.closure);
                    quote! {
                        pub fn #name(#pat: #param) -> #ret #body
                    }
                } else {
                    // Verdict recovery: auto-inject the progress guard (SPEC.md G8). The body
                    // is used as the initializer of `__after`, so a `{ .. }` body isn't
                    // double-wrapped. `#pat` is the parameter; the snapshot borrows it
                    // before the body consumes it.
                    let body = &b.closure.body;
                    quote! {
                        pub fn #name(#pat: #param) -> #ret {
                            let __before = ::core::clone::Clone::clone(&#pat.source().payload);
                            let __after: #ret = #body;
                            must_progress(&__before, &__after.payload);
                            __after
                        }
                    }
                }
            })
            .collect();

        quote! { #(#transition_fns)* #(#recover_fns)* }
    } else {
        quote! {}
    };

    // ── recovery-progress guard (SPEC.md G8) ──────────────────────
    let progress_helper = quote! {
        /// Recovery-progress guard. A recover edge must make forward progress; a
        /// recover that returns a token identical to the one it received is an
        /// infinite-stall bug — a *liveness* failure that typestate (a safety
        /// discipline) cannot catch. Asserts the recovered value differs from the
        /// source; panics on no-progress.
        ///
        /// With the inline `impl { .. }` form the macro injects the call around
        /// every recover body, so it cannot be skipped. The recoverable verdict
        /// carries its `source` rung precisely so there is a `before` to compare.
        #[allow(dead_code)]
        pub fn must_progress<T: ::core::cmp::PartialEq>(before: &T, after: &T) {
            assert!(
                before != after,
                "recovery made no progress: the recovered value equals its source \
                 (SPEC.md G8 — infinite-stall guard)"
            );
        }

        /// Token-binding guard (SPEC.md G13). A `#[judgmental(R)]` transition is
        /// licensed by a token that was measured against **one** argument; this
        /// refuses it anywhere else.
        ///
        /// Het disjointness-against-argument states the non-identity condition
        /// as `π(p) ∩ π(a) = ∅` for the very `a` the operation is applied to.
        /// The seal on `Qualified` closes fabrication — nobody can write a
        /// token. It does not close *transfer*: a token earned honestly against
        /// one argument could be spent on another, which is the act the
        /// proposition forbids.
        ///
        /// The macro injects the call at the head of every marked transition, so
        /// a body cannot skip it — the same discipline as `must_progress`, and
        /// panicking for the same reason: the transition's return type is the
        /// domain's declaration, so there is no `Err` variant to route a refusal
        /// through. A P0 violation is not a recoverable step outcome.
        #[allow(dead_code)]
        pub fn must_be_bound_to<A, R>(argument: &A, licence: &::rung::Qualified<R>)
        where
            A: ::rung::Provenanced,
            R: ::rung::Role,
        {
            assert!(
                licence.is_bound_to(argument),
                "P0: this qualifying token was minted against a different argument \
                 (minted against {:?}, applied to {:?}); disjointness is measured \
                 against the argument the operation is applied to — SPEC.md G13, \
                 rung-het-propositions.md#disjointness-against-argument",
                licence.argument_provenance(),
                ::rung::Provenanced::provenance(argument),
            );
        }
    };

    // ── assemble module ─────────────────────────────────────────────────
    quote! {
        #mod_vis mod #mod_name {
            use super::*;
            #carry_struct
            #(#rung_structs)*
            #(#verdict_structs)*
            #failed_type
            #verdict_enum
            #progress_helper
            #logic
        }
    }
}

// ── entry point ─────────────────────────────────────────────────────────────

#[proc_macro]
pub fn ladder(input: TokenStream) -> TokenStream {
    let ladder = parse_macro_input!(input as Ladder);
    if let Err(e) = check(&ladder) {
        return syn::Error::new(proc_macro2::Span::call_site(), e)
            .to_compile_error()
            .into();
    }
    emit(&ladder).into()
}
