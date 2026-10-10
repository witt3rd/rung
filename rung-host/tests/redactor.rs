//! H4: the credential redactor. Every secret here is a SENTINEL: a made-up
//! value shaped like a credential, never a real one.
//!
//! The design note's check: zero hits for a canary in every door, stream line
//! and console file; no output shorter than its input except by the replaced
//! value.

use rung_host::redact::{MARK, Redactor, redact};
use serde_json::json;

/// Sentinel values, one per shape. Built by concatenation so no literal in
/// this file is itself token-shaped to a scanner.
fn sentinels() -> Vec<(&'static str, String)> {
    let a = "SENTINEL0123456789abcdefSENTINEL";
    vec![
        ("sk dash", format!("sk-or-v1-{a}")),
        ("anthropic", format!("sk-ant-api03-{a}")),
        ("github", format!("ghp_{a}")),
        ("github fine", format!("github_pat_{a}")),
        ("gitlab", format!("glpat-{a}")),
        ("slack", format!("xoxb-{a}")),
        ("aws", "AKIASENTINEL01234567".to_string()),
        ("google", format!("AIza{a}")),
        ("huggingface", format!("hf_{a}")),
        ("npm", format!("npm_{a}")),
        ("secrets manager", format!("dp.st.{a}")),
        ("jwt", format!("eyJ{a}.eyJ{a}.SENTINELSIGNATURE0123456789")),
    ]
}

fn pem(body: &str) -> String {
    format!("-----BEGIN PRIVATE KEY-----\n{body}\n-----END PRIVATE KEY-----")
}

#[test]
fn a_known_key_shape_is_replaced_wherever_it_sits() {
    for (name, s) in sentinels() {
        let text = format!("before {s} after");
        assert_eq!(redact(&text), format!("before {MARK} after"), "{name}");
        let text = format!("key={s}\n");
        assert!(!redact(&text).contains(&s), "{name}");
        let text = format!("(\"{s}\")");
        assert_eq!(redact(&text), format!("(\"{MARK}\")"), "{name}");
    }
}

#[test]
fn a_private_key_block_loses_its_body_and_keeps_its_frame() {
    let body = "MIIEvSENTINELBODYline1\nSENTINELBODYline2AAAA==";
    let text = format!("x\n{}\ny", pem(body));
    let out = redact(&text);
    assert_eq!(
        out,
        format!("x\n-----BEGIN PRIVATE KEY-----\n{MARK}\n-----END PRIVATE KEY-----\ny")
    );
    // A block cut off before its end line still loses what it shows.
    let cut = "-----BEGIN RSA PRIVATE KEY-----\nSENTINELBODYline1\nSENTINELBODYline2";
    let out = redact(cut);
    assert!(!out.contains("SENTINELBODY"), "{out}");
    assert!(out.starts_with("-----BEGIN RSA PRIVATE KEY-----"));
}

#[test]
fn an_auth_header_keeps_its_name_and_loses_its_value() {
    for (h, v) in [
        ("Authorization", "Bearer SENTINELtoken0123"),
        ("authorization", "Basic U0VOVElORUw6U0VOVElORUw="),
        ("X-Api-Key", "SENTINELvalue0123"),
        ("Proxy-Authorization", "Basic U0VOVElORUw6U0VOVElORUw="),
        ("Cookie", "sid=SENTINELsession; other=SENTINELmore"),
        ("Set-Cookie", "sid=SENTINELsession; HttpOnly"),
        ("mcp-session-id", "SENTINELsession0123"),
    ] {
        assert_eq!(
            redact(&format!("GET /x\n{h}: {v}\nHost: example.test\n")),
            format!("GET /x\n{h}: {MARK}\nHost: example.test\n"),
            "{h}"
        );
    }
    // Quoted on a command line: the closing quote survives.
    assert_eq!(
        redact("curl -H \"Authorization: Bearer SENTINELtoken0123\" https://example.test/"),
        format!("curl -H \"Authorization: {MARK}\" https://example.test/")
    );
}

#[test]
fn a_bearer_token_alone_is_replaced_but_prose_is_not() {
    assert_eq!(
        redact("sent Bearer SENTINELtoken0123 today"),
        format!("sent Bearer {MARK} today")
    );
    let prose = "the bearer authentication scheme and bearer tokens in general";
    assert_eq!(redact(prose), prose);
}

#[test]
fn an_env_style_assignment_keeps_its_name_and_loses_its_value() {
    for (text, want) in [
        (
            "RUNG_HOST_OPENROUTER_API_KEY=SENTINELvalue0123",
            format!("RUNG_HOST_OPENROUTER_API_KEY={MARK}"),
        ),
        (
            "export DB_PASSWORD='sentinel pass word'",
            format!("export DB_PASSWORD='{MARK}'"),
        ),
        (
            "env: GITHUB_TOKEN=SENTINELvalue0123 next",
            format!("env: GITHUB_TOKEN={MARK} next"),
        ),
        (
            "client_secret: SENTINELvalue0123",
            format!("client_secret: {MARK}"),
        ),
        (
            "{\"api_key\": \"SENTINELvalue0123\", \"n\": 1}",
            format!("{{\"api_key\": \"{MARK}\", \"n\": 1}}"),
        ),
        (
            "GET /v1?access_token=SENTINELvalue0123&page=2",
            format!("GET /v1?access_token={MARK}&page=2"),
        ),
        (
            "accessToken=SENTINELvalue0123",
            format!("accessToken={MARK}"),
        ),
    ] {
        assert_eq!(redact(text), want, "{text}");
    }
}

#[test]
fn url_credentials_lose_the_secret_part() {
    assert_eq!(
        redact("clone https://user:SENTINELpass0123@host.test/repo.git now"),
        format!("clone https://user:{MARK}@host.test/repo.git now")
    );
    assert_eq!(
        redact("https://SENTINELtoken0123@host.test/x"),
        format!("https://{MARK}@host.test/x")
    );
    // A plain ssh user is a name, not a secret.
    assert_eq!(
        redact("ssh://git@host.test/repo"),
        "ssh://git@host.test/repo"
    );
}

#[test]
fn the_exact_value_of_a_named_variable_is_removed_in_any_shape() {
    let canary = "canary-SENTINEL-not-shaped-like-anything-9";
    // Only the Redactor that was told the variable knows the value.
    // SAFETY: a name only this test reads; no other thread touches it.
    unsafe { std::env::set_var("RUNG_H4_TEST_CANARY", canary) };
    let r = Redactor::from_env_names(["RUNG_H4_TEST_CANARY", "RUNG_H4_TEST_UNSET"]);
    let text = format!("a {canary} b\nwords{canary}words\n{{\"x\":\"{canary}\"}}");
    let out = r.redact(&text);
    assert!(!out.contains(canary), "{out}");
    assert_eq!(
        out,
        format!("a {MARK} b\nwords{MARK}words\n{{\"x\":\"{MARK}\"}}")
    );
    assert!(
        redact(&text).contains(canary),
        "shapes alone cannot know it"
    );
    // A value too short to be a credential is left (it would shred prose).
    let r = Redactor::new().with_secret("ab");
    assert_eq!(r.redact("a cab b"), "a cab b");
}

#[test]
fn a_secret_with_a_quote_or_newline_is_removed_from_a_json_line() {
    let secret = "SENTINEL\"quote\nnewline0123";
    let r = Redactor::new().with_secret(secret);
    let line =
        serde_json::to_string(&json!({"seq": 7, "kind": "tool", "out": format!("got {secret}!")}))
            .unwrap();
    assert!(
        !line.contains(secret),
        "escaped on disk, so a raw match cannot see it"
    );
    let out = r.redact_json_line(&line);
    let v: serde_json::Value = serde_json::from_str(&out).expect("still one JSON object");
    assert_eq!(v["out"], format!("got {MARK}!"));
    assert_eq!(v["seq"], 7);
    assert!(!out.contains("SENTINEL"), "{out}");
}

#[test]
fn a_json_value_under_a_secret_name_is_replaced_and_counts_are_not() {
    let r = Redactor::new();
    let v = json!({
        "password": "sentinel-pass",
        "nested": {"api_key": "sentinel-key", "max_tokens": 4096, "prompt_tokens": 22114},
        "list": [{"client_secret": "sentinel-secret"}, "plain"],
        "api_key_env": "OPENROUTER_API_KEY",
        "note": "keep me",
    });
    let out = r.redact_value(&v);
    assert_eq!(out["password"], MARK);
    assert_eq!(out["nested"]["api_key"], MARK);
    assert_eq!(out["nested"]["max_tokens"], 4096);
    assert_eq!(out["nested"]["prompt_tokens"], 22114);
    assert_eq!(out["list"][0]["client_secret"], MARK);
    assert_eq!(out["list"][1], "plain");
    assert_eq!(
        out["api_key_env"], "OPENROUTER_API_KEY",
        "a variable's name is not its value"
    );
    assert_eq!(out["note"], "keep me");
}

#[test]
fn a_clean_json_line_comes_back_byte_for_byte() {
    let r = Redactor::new();
    let line = r#"{"seq":1,"at":5,"kind":"turn.started","n":3}"#;
    assert!(matches!(
        r.redact_json_line(line),
        std::borrow::Cow::Borrowed(_)
    ));
    // Not JSON at all: still redacted as text.
    let out = r.redact_json_line("not json Bearer SENTINELtoken0123");
    assert_eq!(out, format!("not json Bearer {MARK}"));
}

#[test]
fn text_that_holds_no_secret_comes_back_unchanged_and_whole() {
    let prose = "\
max_tokens: 4096, prompt_tokens=22114, cached_tokens: 21403\n\
the task-runner ran ask-me-later and sk-learn is a library\n\
a key point; sort_key: name; cache_key=abc; idempotency_key: once\n\
password reset flow, token budget, secret garden, authorization pending\n\
see https://example.test/a/b?x=1#frag and ssh://git@example.test/r\n\
email me at a@b.test; time 14:02:11 ratio 3:1 key: value\n\
-----BEGIN CERTIFICATE-----\nMIIBsentinelPUBLICcert\n-----END CERTIFICATE-----\n\
unicode: héllo ✓ 日本語 🚀 end\n";
    let r = Redactor::new();
    let out = r.redact(prose);
    assert_eq!(out, prose);
    assert!(matches!(out, std::borrow::Cow::Borrowed(_)));
}

#[test]
fn the_only_change_is_the_replaced_value_in_a_long_mixed_text() {
    // A "12 MB tool output" in miniature, with secrets sprinkled through it.
    let secrets = sentinels();
    let filler = "line of ordinary output with numbers 1234 and words — ünï ✓\n".repeat(40);
    let mut text = String::new();
    let mut want = String::new();
    for (_, s) in &secrets {
        text.push_str(&filler);
        want.push_str(&filler);
        text.push_str(&format!("token is {s} ok\n"));
        want.push_str(&format!("token is {MARK} ok\n"));
    }
    text.push_str(&filler);
    want.push_str(&filler);
    let out = redact(&text);
    assert_eq!(out, want);
    // Idempotent: a redacted text is stable.
    assert_eq!(redact(&out), want);
}

#[test]
fn a_very_large_line_is_redacted_whole_and_fast() {
    let big = format!(
        "{}\nAuthorization: Bearer SENTINELtoken0123\n",
        "x:=".repeat(2_000_000)
    );
    let t = std::time::Instant::now();
    let out = redact(&big);
    assert!(
        t.elapsed() < std::time::Duration::from_secs(20),
        "{:?}",
        t.elapsed()
    );
    assert!(!out.contains("SENTINELtoken0123"));
    assert_eq!(
        out.len(),
        big.len() - "Bearer SENTINELtoken0123".len() + MARK.len()
    );
}

#[test]
fn multibyte_text_around_a_secret_is_untouched() {
    let s = &sentinels()[1].1;
    let text = format!("日本語🚀{s}🚀日本語 ünï Authorization: Bearer SENTINELtoken0123\n✓");
    assert_eq!(
        redact(&text),
        format!("日本語🚀{MARK}🚀日本語 ünï Authorization: {MARK}\n✓")
    );
}
