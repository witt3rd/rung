//! H4: the credential redactor. Every secret here is a SENTINEL: a made-up
//! value shaped like a credential, never a real one.
//!
//! The design note's check: zero hits for a canary in every door, stream line
//! and console file; no output shorter than its input except by the replaced
//! value.

use rung_host::redact::{MARK, RedactJsonLine, Redactor, redact};
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
        ("xai", format!("xai-{a}")),
        ("groq", format!("gsk_{a}")),
        ("tailnet key", format!("tskey-auth-{a}")),
        ("slack app", format!("xapp-1-{a}")),
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
    // A token-only user is a secret in any scheme but a login one.
    assert_eq!(
        redact("redis://SENTINELtoken0123@host.test:6379/0"),
        format!("redis://{MARK}@host.test:6379/0")
    );
    assert_eq!(
        redact("mongodb+srv://SENTINELtoken0123@host.test/db"),
        format!("mongodb+srv://{MARK}@host.test/db")
    );
    // A plain ssh user is a name, not a secret, under any ssh-tailed scheme.
    assert_eq!(
        redact("git+ssh://git@host.test/repo"),
        "git+ssh://git@host.test/repo"
    );
    assert_eq!(
        redact("sftp://deploy@host.test/x"),
        "sftp://deploy@host.test/x"
    );
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
    // What this test sets, it retires, on a failed assertion too.
    struct Unset;
    impl Drop for Unset {
        fn drop(&mut self) {
            // SAFETY: as above.
            unsafe { std::env::remove_var("RUNG_H4_TEST_CANARY") };
        }
    }
    let _unset = Unset;
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

/// Hostile text that makes a naive scan run to the end of the text from every
/// match: each of these is a redaction input a tool or a model can print.
#[test]
fn hostile_text_is_redacted_in_time_linear_in_its_length() {
    let n = 100_000;
    let secret = "SENTINELvalue0123456789";
    for (name, unit) in [
        ("secret names, no stop", "token:"),
        ("secret names, assigned", "api_key="),
        ("variable references", "token:$"),
        ("header names", "authorization:"),
        ("header names, quoted", "cookie:\""),
        ("header names, single quoted", "'authorization: "),
        ("header names, escaped quote", "{\\\"cookie\\\": \\\""),
        ("cookie pairs with quotes", "cookie: a=\"b\"; "),
        ("token prefixes", "sk-"),
        ("xai prefixes", "xai-"),
        ("groq prefixes", "gsk_"),
        ("tailnet prefixes", "tskey-"),
        ("slack app prefixes", "xapp-"),
        ("github prefixes", "ghp_"),
        ("aws prefixes", "AKIA"),
        ("google prefixes", "AIza"),
        ("jwt-like runs", "eyJ-"),
        ("bearer words", "bearer a "),
        ("url schemes", "a://"),
        ("quotes", "password=\"x"),
        ("escaped quotes", "password=\\\"x"),
        ("escaped backslashes", "password=a\\\\"),
        ("webhook prefixes", "hooks.slack.com/services/"),
        ("discord webhooks", "discord.com/api/webhooks/"),
        ("backslash runs before quotes", "password=\"x\\\\\\\""),
        ("long backslash runs", "\\\\\\\\\\\\\\\\ token="),
        ("key block starts", "-----BEGIN "),
        ("private key starts", "-----BEGIN PRIVATE KEY-----\n"),
        ("private key stray starts", "-----BEGIN x\n"),
    ] {
        let text = format!("{}{} tail", unit.repeat(n), secret);
        let t = std::time::Instant::now();
        let out = redact(&text);
        let took = t.elapsed();
        assert!(
            took < std::time::Duration::from_secs(5),
            "{name}: {took:?} for {} bytes",
            text.len()
        );
        // The only change is the replaced values: the text between the marks
        // is the input's own, in order, and it still starts and ends the text.
        assert_replaced_only(&text, &out, name);
        assert_eq!(redact(&out), out, "{name}: not stable");
    }
}

/// `out` is `text` with some stretches replaced by `MARK` and nothing else
/// changed: the pieces of `out` between the marks occur in `text` in order,
/// the first as its start and the last as its end.
fn assert_replaced_only(text: &str, out: &str, name: &str) {
    let pieces: Vec<&str> = out.split(MARK).collect();
    if pieces.len() == 1 {
        assert_eq!(out, text, "{name}: changed with nothing replaced");
        return;
    }
    let (first, last) = (pieces[0], pieces[pieces.len() - 1]);
    assert!(text.starts_with(first), "{name}: start changed");
    assert!(text.ends_with(last), "{name}: end changed");
    let mut at = first.len();
    for p in &pieces[1..pieces.len() - 1] {
        let found = text[at..]
            .find(p)
            .unwrap_or_else(|| panic!("{name}: content dropped"));
        at += found + p.len();
    }
    assert!(text.len() - last.len() >= at, "{name}: content dropped");
}

#[test]
fn a_secret_after_a_long_run_of_secret_names_is_still_found() {
    let secret = "SENTINELvalue0123456789";
    let text = format!("{}api_key={secret}\nnext", "token:".repeat(50_000));
    let out = redact(&text);
    assert!(!out.contains(secret));
    assert!(out.ends_with("\nnext"));
}

#[test]
fn a_value_inside_escaped_json_text_stops_at_the_escape() {
    // A tool's input is recorded as JSON text, so a quote or a newline around
    // a value shows as a backslash pair; the backslash is not part of the value.
    assert_eq!(
        redact(r#"curl \"https://h.test/f?token=SENTINELvalue0123\"; ls"#),
        format!(r#"curl \"https://h.test/f?token={MARK}\"; ls"#)
    );
    assert_eq!(
        redact(r#"Authorization: Bearer SENTINELtoken0123\nHost: h.test"#),
        format!(r#"Authorization: {MARK}\nHost: h.test"#)
    );
    assert_eq!(
        redact(r#"Cookie: sid=SENTINELsession\nnext"#),
        format!(r#"Cookie: {MARK}\nnext"#)
    );
}

// ---- review items: skip-ahead, header keys, quotes, backslashes ------------

#[test]
fn a_secret_next_to_another_never_escapes_after_a_match() {
    let all = sentinels();
    for (na, a) in &all {
        for (nb, b) in &all {
            for sep in ["", " ", ",", "\"", "'", "\n", ";", "&", "|", ")", "/"] {
                let text = format!("{a}{sep}{b}");
                let out = redact(&text);
                assert!(
                    !out.contains(a.as_str()),
                    "{na} then {nb} with {sep:?}: {out}"
                );
                assert!(
                    !out.contains(b.as_str()),
                    "{na} then {nb} with {sep:?}: {out}"
                );
            }
        }
    }
    // After a header value, an assignment, a URL password, a key block.
    for text in [
        format!(
            "Authorization: Bearer SENTINELtoken0123,{}",
            sentinels()[2].1
        ),
        format!("token=SENTINELvalue0123&api_key={}&x=1", sentinels()[1].1),
        format!("https://u:SENTINELpass0123@h.test/ {}", sentinels()[2].1),
        format!("{}\n{}", pem("SENTINELBODY0123"), sentinels()[2].1),
        format!("GH_{}", sentinels()[2].1),
    ] {
        let out = redact(&text);
        assert!(!out.contains("SENTINEL"), "{text} -> {out}");
    }
}

#[test]
fn a_stray_key_block_start_does_not_hide_the_real_block() {
    let text = format!("-----BEGIN x\nnote {}\nafter", pem("SENTINELBODY0123"));
    let out = redact(&text);
    assert!(!out.contains("SENTINELBODY"), "{out}");
    assert!(out.starts_with("-----BEGIN x\nnote -----BEGIN PRIVATE KEY-----"));
    assert!(out.ends_with("-----END PRIVATE KEY-----\nafter"));
    // A certificate before the key is not a secret and survives.
    let text = format!(
        "-----BEGIN CERTIFICATE-----\nMIIBpublic\n-----END CERTIFICATE-----\n{}",
        pem("SENTINELBODY0123")
    );
    let out = redact(&text);
    assert!(
        out.contains("MIIBpublic") && !out.contains("SENTINELBODY"),
        "{out}"
    );
}

#[test]
fn a_map_key_that_is_a_header_name_redacts_its_value() {
    let r = Redactor::new();
    let v = json!({
        "Authorization": "Bearer x",
        "Cookie": "a=b",
        "X-Api-Key": "k",
        "headers": {"set-cookie": ["sid=1; HttpOnly", "b=2"], "Proxy-Authorization": "Basic abc"},
        "mcp-session-id": "s",
        "credentials": {"user": "alice", "pass": "sentinel-pass", "n": 3},
        "content-type": "application/json",
        "accept": "text/html",
    });
    let out = r.redact_value(&v);
    for k in ["Authorization", "Cookie", "X-Api-Key", "mcp-session-id"] {
        assert_eq!(out[k], MARK, "{k}");
    }
    assert_eq!(out["headers"]["set-cookie"], json!([MARK, MARK]));
    assert_eq!(out["headers"]["Proxy-Authorization"], MARK);
    assert_eq!(
        out["credentials"],
        json!({"user": MARK, "pass": MARK, "n": 3})
    );
    assert_eq!(out["content-type"], "application/json");
    assert_eq!(out["accept"], "text/html");
    // And as a record line.
    let line = serde_json::to_string(&json!({"seq": 1, "headers": {"Authorization": "Bearer x"}}))
        .unwrap();
    let red: serde_json::Value = serde_json::from_str(&r.redact_json_line(&line)).unwrap();
    assert_eq!(red["headers"]["Authorization"], MARK);
}

#[test]
fn quoted_header_values_are_replaced_to_their_closing_quote() {
    for (text, want) in [
        (
            "Cookie: a=\"b c\"; d=e\nnext",
            format!("Cookie: {MARK}\nnext"),
        ),
        (
            "curl -H 'Cookie: a=\"b\"; c=d' x",
            format!("curl -H 'Cookie: {MARK}' x"),
        ),
        (
            "{\"authorization\": \"Digest a=1, b=2\"}",
            format!("{{\"authorization\": \"{MARK}\"}}"),
        ),
        (
            "Authorization:\"Bearer SENTINELtoken0123\" ok",
            format!("Authorization:\"{MARK}\" ok"),
        ),
        (
            "{'authorization': 'Bearer SENTINELtoken0123', 'n': 1}",
            format!("{{'authorization': '{MARK}', 'n': 1}}"),
        ),
        (
            "-H \"X-Api-Key: SENTINELkey0123\" -H \"Accept: x\"",
            format!("-H \"X-Api-Key: {MARK}\" -H \"Accept: x\""),
        ),
    ] {
        assert_eq!(redact(text), want, "{text}");
    }
}

#[test]
fn escaped_quotes_and_backslashes_do_not_end_a_redaction_early() {
    for (text, want) in [
        // JSON-escaped text: the pairs around names and values are escapes.
        (
            r#"{\"password\": \"SENTINELpass0123\", \"n\": 1}"#,
            format!(r#"{{\"password\": \"{MARK}\", \"n\": 1}}"#),
        ),
        (
            r#"{\"Authorization\": \"Bearer SENTINELtoken0123\"}"#,
            format!(r#"{{\"Authorization\": \"{MARK}\"}}"#),
        ),
        (
            r#"{\"Cookie\": \"sid=SENTINELsession; a=b\", \"n\": 1}"#,
            format!(r#"{{\"Cookie\": \"{MARK}\", \"n\": 1}}"#),
        ),
        // A value holding an escaped backslash runs through it.
        (
            r"password=ab\\cdSENTINEL0123 next",
            format!("password={MARK} next"),
        ),
        // A quoted value holding an escaped quote runs through it.
        (
            r#"password="SENTINEL\"tail0123" ok"#,
            format!(r#"password="{MARK}" ok"#),
        ),
        (
            r#"api_key='SENTINEL\'tail0123' ok"#,
            format!(r#"api_key='{MARK}' ok"#),
        ),
    ] {
        assert_eq!(redact(text), want, "{text}");
    }
}

#[test]
fn webhook_urls_lose_their_token_path() {
    assert_eq!(
        redact(
            "post https://hooks.slack.com/services/T00000000/B00000000/SENTINELwebhook0123456789 now"
        ),
        format!("post https://hooks.slack.com/services/{MARK} now")
    );
    assert_eq!(
        redact(
            "https://discord.com/api/webhooks/123456789012345678/SENTINELwebhook0123456789_-x?wait=true"
        ),
        format!("https://discord.com/api/webhooks/{MARK}?wait=true")
    );
    assert_eq!(
        redact("https://discordapp.com/api/webhooks/1/SENTINELwebhook0123456789"),
        format!("https://discordapp.com/api/webhooks/{MARK}")
    );
    assert_eq!(
        redact("https://hooks.zapier.com/hooks/catch/123/SENTINELhook0123/"),
        format!("https://hooks.zapier.com/hooks/catch/{MARK}/")
    );
    assert_eq!(
        redact("https://example.test/webhooks/docs is a page"),
        "https://example.test/webhooks/docs is a page"
    );
}

#[test]
fn a_quote_is_escaped_by_the_parity_of_the_backslashes_before_it() {
    for (text, want) in [
        // An escaped backslash, then an escaped quote: the quote does not close.
        (
            r#"password="SENTINELabc\\\"xyz0123" ok"#,
            format!(r#"password="{MARK}" ok"#),
        ),
        // Five backslashes before a quote: two pairs and an escape.
        (
            r#"password="SENTINELabc\\\\\"xyz0123" ok"#,
            format!(r#"password="{MARK}" ok"#),
        ),
        // Four backslashes before a quote: two pairs, so the quote closes.
        (
            r#"password="SENTINELabc\\\\" ok"#,
            format!(r#"password="{MARK}" ok"#),
        ),
        (
            r#"api_key='SENTINELabc\\\'xyz0123' ok"#,
            format!(r#"api_key='{MARK}' ok"#),
        ),
        // In JSON-escaped text, an inner escaped quote has three backslashes.
        (
            r#"{\"password\": \"SENTINELabc\\\"xyz0123\", \"n\": 1}"#,
            format!(r#"{{\"password\": \"{MARK}\", \"n\": 1}}"#),
        ),
        // A line break escape after an escaped backslash still ends a value.
        (
            r"password=SENTINELabc\\\nnext ok",
            format!(r"password={MARK}\nnext ok"),
        ),
    ] {
        assert_eq!(redact(text), want, "{text}");
    }
}
