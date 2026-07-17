//! The `ladder!` proc macro — a compiler for type-state ladders.
//!
//! Parses the ladder syntax, runs 8 static checks, emits sealed Rust types
//! with transition traits. The borrow checker enforces linear consumption.
//! The macro enforces structural correctness (rung existence, recover pairing,
//! terminal vs recoverable distinction).

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
}

struct Verdict {
    name: Ident,
    is_terminal: bool,
    recover_target: Option<Ident>,
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
                // Rung: Xxx(Type) => ...
                let rung_name: Ident = content.parse()?;
                let payload;
                syn::parenthesized!(payload in content);
                let payload_type: Type = payload.parse()?;
                rungs.push(Rung {
                    name: rung_name.clone(),
                    payload_type: payload_type.clone(),
                });

                if content.peek(Token![=>]) {
                    content.parse::<Token![=>]>()?;

                    if content.peek(syn::token::Brace) {
                        // Verdict branching
                        let block;
                        braced!(block in content);
                        let mut verdicts = Vec::new();
                        let mut first = true;
                        while !block.is_empty() {
                            if !first {
                                block.parse::<Token![|]>()?;
                            }
                            first = false;
                            let verdict_name: Ident = block.parse()?;
                            let mut is_terminal = true;
                            let mut recover_target = None;
                            if block.peek(Token![=>]) {
                                block.parse::<Token![=>]>()?;
                                is_terminal = false;
                                let target: Ident = block.parse()?;
                                recover_target = Some(target);
                            }
                            verdicts.push(Verdict {
                                name: verdict_name,
                                is_terminal,
                                recover_target,
                            });
                        }
                        transitions.push(Transition {
                            name: format_ident!("step"),
                            from_rung: rung_name,
                            to_rung: None,
                            verdicts,
                        });
                    } else {
                        // Simple next rung — but may chain: => Next => { verdicts }
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
                            from_rung: rung_name.clone(),
                            to_rung: Some(next_name.clone()),
                            verdicts: vec![],
                        });

                        // Chain: check for another => (verdict branching from this rung)
                        if content.peek(Token![=>]) {
                            content.parse::<Token![=>]>()?;
                            if content.peek(syn::token::Brace) {
                                let block;
                                braced!(block in content);
                                let mut verdicts = Vec::new();
                                let mut first = true;
                                while !block.is_empty() {
                                    if !first {
                                        block.parse::<Token![|]>()?;
                                    }
                                    first = false;
                                    let vn: Ident = block.parse()?;
                                    let mut it = true;
                                    let mut rt = None;
                                    if block.peek(Token![=>]) {
                                        block.parse::<Token![=>]>()?;
                                        it = false;
                                        rt = Some(block.parse()?);
                                    }
                                    verdicts.push(Verdict {
                                        name: vn,
                                        is_terminal: it,
                                        recover_target: rt,
                                    });
                                }
                                transitions.push(Transition {
                                    name: format_ident!("step"),
                                    from_rung: next_name,
                                    to_rung: None,
                                    verdicts,
                                });
                            }
                        }
                    }
                }
                let _ = content.parse::<Token![;]>();
            }
        }

        Ok(Ladder {
            name,
            carry_fields,
            rungs,
            transitions,
            recover_edges,
            recover_fns,
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

// ── checker (8 static rules, same as rung/checker.py) ────────────────────────

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
        if let Some(ref to) = t.to_rung {
            if !rung_names.contains(&to.to_string()) {
                return Err(format!(
                    "transition `{}`: to_rung `{}` not declared",
                    t.name, to
                ));
            }
        }
    }

    // 3. verdicts are valid
    for t in &ladder.transitions {
        for v in &t.verdicts {
            if !v.is_terminal {
                if v.recover_target.is_none() {
                    return Err(format!(
                        "verdict `{}` on transition `{}`: non-terminal verdict must have a recover_target",
                        v.name, t.name
                    ));
                }
                if let Some(ref target) = v.recover_target {
                    if !rung_names.contains(&target.to_string()) {
                        return Err(format!(
                            "verdict `{}`: recover_target `{}` not declared",
                            v.name, target
                        ));
                    }
                }
            }
        }
    }

    // 4. every recoverable verdict has a matching RecoverEdge
    for t in &ladder.transitions {
        for v in &t.verdicts {
            if !v.is_terminal {
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

    // 8. RecoverFn return_rung is declared
    for rf in &ladder.recover_fns {
        if !rung_names.contains(&rf.return_rung.to_string()) {
            return Err(format!(
                "RecoverFn `{}`: return_rung `{}` not declared",
                rf.name, rf.return_rung
            ));
        }
    }

    Ok(())
}

// ── code generator ──────────────────────────────────────────────────────────

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

    // ── Rung structs (sealed) ───────────────────────────────────────────
    let rung_structs: Vec<_> = ladder
        .rungs
        .iter()
        .map(|r| {
            let name = &r.name;
            let payload = &r.payload_type;
            let carry_field = if carry_fields.is_empty() {
                quote! {}
            } else {
                quote! { pub carry: Carry, }
            };
            quote! {
                pub struct #name { _seal: (), #carry_field pub payload: #payload }
            }
        })
        .collect();

    // ── Failed<Prev> ────────────────────────────────────────────────────
    let failed_type = quote! {
        pub struct Failed<Prev> { pub token: Prev, pub error: String }
    };

    // ── Verdict structs ──────────────────────────────────────────────────
    let verdict_structs: Vec<_> = ladder
        .transitions
        .iter()
        .flat_map(|t| t.verdicts.iter())
        .map(|v| {
            let name = &v.name;
            quote! { pub struct #name; }
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
                quote! { #name(#name), }
            })
            .collect();
        quote! {
            pub enum StepOutcome { #(#variants)* }
        }
    };

    // ── Transition trait ────────────────────────────────────────────────
    let _carry_param = if carry_fields.is_empty() {
        quote! {}
    } else {
        quote! { carry: &Carry, }
    };

    let transition_methods: Vec<_> = ladder
        .transitions
        .iter()
        .map(|t| {
            let name = &t.name;
            let from_rung = &t.from_rung;
            let _from_payload = ladder
                .rungs
                .iter()
                .find(|r| r.name == *from_rung)
                .map(|r| &r.payload_type)
                .unwrap();

            if let Some(ref to_rung) = t.to_rung {
                // Simple transition: from_rung -> to_rung
                let _to_payload = ladder
                    .rungs
                    .iter()
                    .find(|r| r.name == *to_rung)
                    .map(|r| &r.payload_type)
                    .unwrap();
                quote! {
                    fn #name(_token: #from_rung) -> #to_rung;
                }
            } else if !t.verdicts.is_empty() {
                // Branching transition
                let error_type = quote! { Failed<#from_rung> };
                quote! {
                    fn #name(_token: #from_rung) -> Result<StepOutcome, #error_type>;
                }
            } else {
                quote! {}
            }
        })
        .collect();

    let recover_methods: Vec<_> = ladder
        .recover_fns
        .iter()
        .map(|rf| {
            let name = &rf.name;
            let param_type = &rf.param_type;
            let return_type = &rf.return_rung;
            quote! {
                fn #name(_token: #param_type) -> #return_type;
            }
        })
        .collect();

    let trait_def = quote! {
        pub trait Transitions {
            #(#transition_methods)*
            #(#recover_methods)*
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
            #trait_def
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
