use axum::body::to_bytes;

use super::*;

async fn body_of(response: Response) -> String {
    let bytes = to_bytes(response.into_body(), 1 << 20)
        .await
        .expect("console bodies are small and finite");
    String::from_utf8(bytes.to_vec()).expect("both documents are UTF-8 source files")
}

fn header(response: &Response, name: HeaderName) -> String {
    response
        .headers()
        .get(&name)
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default()
        .to_owned()
}

/// Every line of `document` that mentions `needle` outside a `//` comment.
///
/// Scanning the whole file would fail on its own prose — these two documents
/// explain in comments exactly which constructs they avoid, and a test that
/// cannot tell an explanation from a use would force the explanation out. A
/// comment cannot execute, so excluding whole comment lines is sound; nothing
/// finer is attempted, because a half-correct JavaScript parser here would be a
/// worse liability than the property it checks.
fn executable_lines_mentioning<'a>(document: &'a str, needle: &str) -> Vec<&'a str> {
    document
        .lines()
        .filter(|line| !line.trim_start().starts_with("//"))
        .filter(|line| line.contains(needle))
        .collect()
}

/// The claim the module docs rest on, checked rather than asserted in prose: the
/// page is safe to serve unauthenticated because it *is* a constant, and a
/// constant cannot contain a secret unless someone typed one into it.
///
/// Deliberately a change-detector. Any future edit that pastes a token in while
/// debugging has to come through here.
#[test]
fn the_console_is_a_constant_and_names_no_secret() {
    for document in [CONSOLE_HTML, CONSOLE_SCRIPT] {
        for forbidden in ["device_token\":", "bearer_token\":"] {
            assert!(
                executable_lines_mentioning(document, forbidden).is_empty(),
                "a served document must not carry a literal {forbidden:?}"
            );
        }
    }
    assert!(
        executable_lines_mentioning(CONSOLE_HTML, "Bearer").is_empty(),
        "the page itself never authenticates anything"
    );
    // The script composes the header at runtime from a value it fetched. One
    // site, and it joins a constant prefix to a variable — a second occurrence
    // would mean a token had been written down somewhere.
    assert_eq!(
        executable_lines_mentioning(CONSOLE_SCRIPT, "Bearer"),
        vec!["    return { authorization: 'Bearer ' + token };"]
    );
}

/// `script-src 'self'` is worth nothing if the page also ships an inline
/// `<script>`, because the policy would have to be relaxed to `'unsafe-inline'`
/// to make it run — at which point the whole reason for the second route is
/// gone. Pinned so the two cannot drift apart.
#[test]
fn the_page_runs_no_script_of_its_own() {
    assert!(POLICY.contains("script-src 'self'"));
    assert!(!POLICY.contains("script-src 'unsafe-inline'"));

    let mut scripts = CONSOLE_HTML.match_indices("<script");
    let (at, _) = scripts.next().expect("the page loads exactly one script");
    assert!(scripts.next().is_none(), "one script tag, and it has a src");
    let tag = &CONSOLE_HTML[at..];
    let tag = &tag[..tag.find('>').expect("a well-formed tag")];
    assert!(
        tag.contains(&format!("src=\"{CONSOLE_SCRIPT_PATH}\"")),
        "the only script tag must load {CONSOLE_SCRIPT_PATH}, not inline code"
    );
}

/// The rule that makes rendering agent-authored text safe. `textContent` is the
/// only escaping in this file that cannot be got wrong by accident, so the
/// absence of its alternatives is the property worth pinning — a reviewer
/// reading a diff will not reliably notice one `innerHTML` among four hundred
/// lines.
#[test]
fn the_script_never_assigns_markup() {
    for forbidden in [
        "innerHTML",
        "outerHTML",
        "insertAdjacentHTML",
        "document.write",
        "eval(",
    ] {
        assert!(
            executable_lines_mentioning(CONSOLE_SCRIPT, forbidden).is_empty(),
            "the console renders untrusted text and must not use {forbidden}"
        );
    }
}

/// A control plane inside a frame is a control plane a page you did not write
/// can click on. Both headers, because `frame-ancestors` is the one that is
/// actually consulted and `X-Frame-Options` is the one an older browser has.
#[tokio::test]
async fn the_console_refuses_to_be_framed() {
    let response = handle_console_request().await;
    assert!(header(&response, CONTENT_SECURITY_POLICY).contains("frame-ancestors 'none'"));
    assert_eq!(header(&response, X_FRAME_OPTIONS), "DENY");
}

/// What each route is, and that it says so. `nosniff` matters more than usual
/// here: a browser that decided the script was HTML would be parsing a document
/// served from the control plane's own origin.
#[tokio::test]
async fn each_document_is_served_as_what_it_is() {
    let page = handle_console_request().await;
    assert_eq!(page.status(), StatusCode::OK);
    assert_eq!(header(&page, CONTENT_TYPE), "text/html; charset=utf-8");
    assert_eq!(header(&page, X_CONTENT_TYPE_OPTIONS), "nosniff");
    assert_eq!(header(&page, CACHE_CONTROL), "no-store");
    assert_eq!(header(&page, REFERRER_POLICY), "no-referrer");
    assert!(body_of(page).await.starts_with("<!doctype html>"));

    let script = handle_console_script_request().await;
    assert_eq!(script.status(), StatusCode::OK);
    assert_eq!(
        header(&script, CONTENT_TYPE),
        "text/javascript; charset=utf-8"
    );
    assert_eq!(header(&script, X_CONTENT_TYPE_OPTIONS), "nosniff");
    assert!(body_of(script).await.contains("'use strict'"));
}

/// The safety property T11.5 exists for, pinned where it can be deleted by
/// accident (T12.2).
///
/// Every answer carries back the digest of the request it was read from, so a
/// yes taken from one screen cannot be applied to whatever the agent is asking
/// by the time it arrives. Dropping `digest` from that object would not fail any
/// other test here — the server would refuse every answer, which reads as "the
/// buttons are broken" rather than as "the binding is gone".
#[test]
fn an_answer_carries_the_digest_of_what_was_shown() {
    let answering = executable_lines_mentioning(CONSOLE_SCRIPT, "approval_id:");
    assert_eq!(
        answering,
        vec!["    control(action, { approval_id: approval.approval_id, digest: approval.digest })"],
        "there is one place an answer is composed, and it sends a digest"
    );
}

/// The server has two error envelopes and the page has to read both (T12.2).
///
/// Found by running it: a stale answer was refused correctly and the page showed
/// `HTTP 400` and nothing else, because `describeFailure` looked only at
/// `ErrorResponseEnvelope`'s top-level `error`. A typed action that fails
/// answers with a `ResponseEnvelope`, which nests it — and that is the one
/// carrying the stale-digest message, the single most useful sentence this page
/// can print. Reading one shape swallows exactly the errors that matter.
#[test]
fn both_of_the_servers_error_shapes_are_read() {
    assert!(
        CONSOLE_SCRIPT
            .contains("var error = parsed.error || (parsed.response && parsed.response.error);"),
        "the one place that unwraps a failed response must try both envelopes"
    );
    // The shape of the bug, rather than the shape of the fix: reaching straight
    // through `parsed.error` for a message is what produced a bare `HTTP 400`.
    // Sites that read `envelope.response.error` are fine and deliberately not
    // matched here — they already know which envelope they hold.
    assert!(
        executable_lines_mentioning(CONSOLE_SCRIPT, "parsed.error.").is_empty(),
        "a top-level-only unwrap swallows every typed action's error message"
    );
}

/// Deliberately a change-detector, and the same shape as
/// `only_agents_whose_prompt_was_watched_can_be_answered_yes` in
/// `approvals_tests.rs` — for the same reason.
///
/// `agent.approve` reaches a paired device only when the machine's owner sets
/// `WARP_FORK_REMOTE_APPROVE`, and the page is supposed to learn that from the
/// action list `/v1/pair` returns rather than assume it. A button rendered
/// unconditionally would 403 on tap, which is worse than a button that is
/// absent: it teaches a person that the feature is unreliable rather than that
/// it is off.
#[test]
fn yes_is_drawn_only_when_the_server_says_this_device_may_say_it() {
    assert!(
        CONSOLE_SCRIPT.contains("if (can(ALLOW) && approval.can_approve) {"),
        "the Yes button is behind the device's capability *and* this entry's own"
    );
    assert!(
        CONSOLE_SCRIPT.contains("if (!approval.can_approve) {"),
        "a row with no Yes has to say why"
    );
    assert!(
        CONSOLE_SCRIPT.contains("if (approval.can_approve && !can(ALLOW)) {"),
        "…and the device-level explanation survives for entries that are approvable"
    );
    // **Both explanations, not one.** This was an `else if`, which was right
    // while an approvable entry had nothing else to say. Now it has: a yes names
    // the option it sends and the scope it covers, and a device that cannot send
    // one still has to say so. Chaining them would drop the caveat exactly where
    // it matters — an approvable row on an unpaired-for-yes phone.
    assert!(
        CONSOLE_SCRIPT.contains("} else if (approval.approve_selects) {"),
        "an approvable entry says what a Yes would select"
    );
    assert!(
        CONSOLE_SCRIPT.contains("this call only, nothing after it"),
        "…and how far it reaches, because Warp only ever selects a single-shot allow"
    );
    // `No` has no such guard, and must not grow one: `agent.deny` is pairable
    // unconditionally because saying no can only ever make less happen.
    let denials = executable_lines_mentioning(CONSOLE_SCRIPT, "can(DENY)");
    assert!(
        denials.is_empty(),
        "No is unconditional; gating it would be a regression, not a hardening"
    );
}

/// The two reasons a row has no Yes are different facts and must both be
/// consulted — this is the bug the entry-level check was added for.
///
/// `can(ALLOW)` is about the paired *device*: has the machine's owner set
/// `WARP_FORK_REMOTE_APPROVE`. `approval.can_approve` is about the *entry*:
/// would `agent.approve` accept this one. Warp lists every blocked session but
/// approves only verified agents, so drawing the button from the device alone
/// put a Yes on rows the handler always rejects — the affordance lie T14.3 names,
/// on the fork's only browser-reachable surface.
#[test]
fn the_device_capability_is_not_read_as_the_entrys_approvability() {
    let device_only = executable_lines_mentioning(CONSOLE_SCRIPT, "can(ALLOW)");

    for line in &device_only {
        assert!(
            !line.contains("var allow"),
            "the Yes button must not be built from a line that only knows about the device: {line}"
        );
    }
    assert!(
        !CONSOLE_SCRIPT.contains("if (can(ALLOW)) {"),
        "the device-only guard is what drew a Yes on unapprovable rows; it must not come back"
    );
}

/// The manifest is served, is valid JSON, and agrees with the routes that exist
/// (T12.3).
///
/// A manifest whose `start_url`, `scope` or icon `src` names a path this server
/// does not answer produces an installed app that opens a 404 — and nothing else
/// here would catch it, because the manifest is data and the routes are code.
#[tokio::test]
async fn the_manifest_names_only_paths_this_server_answers() {
    let response = handle_console_manifest_request().await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(header(&response, CONTENT_TYPE), "application/manifest+json");

    let manifest: serde_json::Value =
        serde_json::from_str(&body_of(response).await).expect("the manifest must be valid JSON");
    assert_eq!(manifest["start_url"], CONSOLE_PATH);
    assert_eq!(manifest["scope"], CONSOLE_PATH);
    // `standalone` is the whole point: without it an installed icon opens a tab
    // with browser chrome, which is a bookmark rather than an app.
    assert_eq!(manifest["display"], "standalone");
    let icons = manifest["icons"].as_array().expect("at least one icon");
    assert!(!icons.is_empty());
    for icon in icons {
        assert_eq!(icon["src"], CONSOLE_ICON_PATH);
        assert_eq!(icon["type"], "image/png");
    }
}

/// The icon is a real PNG at the size the manifest claims (T12.3).
///
/// Checked from the bytes rather than trusted, because the file is generated —
/// the manifest says `512x512`, and a regenerated icon at a different size would
/// leave a manifest that lies, which a browser resolves by silently declining to
/// use the icon.
#[tokio::test]
async fn the_icon_is_the_png_the_manifest_promises() {
    let response = handle_console_icon_request().await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(header(&response, CONTENT_TYPE), "image/png");

    let bytes = to_bytes(response.into_body(), 1 << 20)
        .await
        .expect("small");
    assert_eq!(&bytes[..8], b"\x89PNG\r\n\x1a\n", "PNG signature");
    // IHDR is the first chunk by specification: 8-byte signature, 4-byte length,
    // 4-byte type, then width and height as big-endian u32.
    assert_eq!(&bytes[12..16], b"IHDR");
    let dimension = |at: usize| u32::from_be_bytes(bytes[at..at + 4].try_into().expect("4 bytes"));
    assert_eq!((dimension(16), dimension(20)), (512, 512));

    let manifest: serde_json::Value = serde_json::from_str(CONSOLE_MANIFEST).expect("valid JSON");
    assert_eq!(manifest["icons"][0]["sizes"], "512x512");
}

/// The page has to *reference* the manifest, or it is a file nobody fetches
/// (T12.3) — and iOS needs its own tags, because it reads none of it.
#[test]
fn the_page_asks_to_be_installed() {
    for required in [
        "<link rel=\"manifest\" href=\"/manifest.webmanifest\">",
        "<link rel=\"apple-touch-icon\" href=\"/icon.png\">",
        "<meta name=\"apple-mobile-web-app-capable\" content=\"yes\">",
    ] {
        assert!(
            CONSOLE_HTML.contains(required),
            "the page must carry {required}"
        );
    }
}

/// `default-src 'none'` has to come first and `connect-src` has to be `'self'`,
/// or a page that ran hostile code could reach off the machine with what it
/// read — which, on this origin, is a live agent transcript.
#[tokio::test]
async fn the_policy_denies_by_default_and_talks_only_to_itself() {
    let policy = header(&handle_console_request().await, CONTENT_SECURITY_POLICY);
    assert!(policy.starts_with("default-src 'none'"));
    assert!(policy.contains("connect-src 'self'"));
    assert!(policy.contains("base-uri 'none'"));
    assert!(policy.contains("form-action 'none'"));
    assert!(
        !policy.contains('*'),
        "no wildcard belongs in this policy: {policy}"
    );
}

/// `cwd` is labelled from the population the server named, never from a guess.
///
/// The field means two different things: a pane's is the agent's own working
/// directory, an ACP entry's is the directory Warp chose for the session and
/// sent in `session/new`. T14.6 measured that directory deciding whether the
/// user's own permission rules loaded at all, and it is *not* necessarily where
/// the call acts — so an unlabelled path sitting directly under a command, on a
/// row that now carries a Yes button, is a misreading waiting to happen.
///
/// The pin that matters is the second one: the page must read `source`, which
/// the server states first-hand, rather than infer the population from `tab_id`
/// or from the shape of `approval_id`. Both of those are incidental fields, and
/// a structural fact read off one is the failure `PendingApproval::kind`'s own
/// doc comment warns about.
#[test]
fn the_directory_is_labelled_by_the_population_the_server_named() {
    assert!(
        CONSOLE_SCRIPT.contains("CWD_LABELS[approval.source]"),
        "the population is stated by the server, not derived on the page"
    );
    // **A population with no label draws none.** The first draft was a two-way
    // ternary, which would have called a future third population's directory a
    // "working directory" — confidently and wrongly. The whole point of the
    // label is that the two mean different things, so an unknown one has to say
    // less rather than pick.
    assert!(
        CONSOLE_SCRIPT.contains("label ? label + approval.cwd : approval.cwd"),
        "an unrecognised source shows the bare path rather than a guessed label"
    );
    for label in ["session directory ", "working directory "] {
        assert!(
            CONSOLE_SCRIPT.contains(label),
            "each population's directory says what kind of directory it is: {label}"
        );
    }
    assert!(
        !CONSOLE_SCRIPT.contains("[approval.project, approval.cwd]"),
        "the two must not be joined back into one unlabelled line"
    );
    for guessed in ["approval.tab_id ?", "approval.tab_id &&", "tab_id) ?"] {
        assert!(
            !CONSOLE_SCRIPT.contains(guessed),
            "the population must not be inferred from an incidental field: {guessed}"
        );
    }
}

/// An armed control is not thrown away by the list refresh.
///
/// **Found by tapping Yes in Firefox against a live instance (T14.6), not by
/// reading.** `renderApprovals` clears and rebuilds every row, and it runs on a
/// 5s poll as well as on every agent event — so a refresh inside the 4s arm
/// window replaced the armed button with a fresh `Yes`, and the tap meant to
/// confirm armed the new one instead. One tap and a pause did nothing; two fast
/// taps worked. That shape is worse than an outright failure, because it reads
/// as an unreliable feature rather than a broken one.
///
/// The counter is pinned rather than the deferral's duration: what matters is
/// that the refresh consults arm state at all. It fails safe either way — a
/// discarded arm can only lose a yes, never invent one — which is why this is a
/// counter and not a lock.
#[test]
fn a_refresh_does_not_disarm_a_control_mid_answer() {
    assert!(
        CONSOLE_SCRIPT.contains("if (armedControls > 0) return Promise.resolve();"),
        "the list refresh has to yield while an answer is half-given"
    );
    // Armed and disarmed in the same place the button's own state changes, so
    // the two cannot drift: a counter that is incremented without a matching
    // decrement would wedge the list permanently.
    assert_eq!(
        executable_lines_mentioning(CONSOLE_SCRIPT, "armedControls +=").len(),
        1,
        "exactly one place arms"
    );
    assert_eq!(
        executable_lines_mentioning(CONSOLE_SCRIPT, "armedControls -=").len(),
        1,
        "exactly one place disarms, and it is `disarm` itself"
    );
}
