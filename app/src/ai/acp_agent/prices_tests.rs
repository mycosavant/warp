//! What the price fetch may do, and — mostly — what it may not (T21.4).

use chrono::{Duration, TimeZone, Utc};

use super::*;

fn at(year: i32, month: u32, day: u32) -> chrono::DateTime<Utc> {
    Utc.with_ymd_and_hms(year, month, day, 12, 0, 0).unwrap()
}

/// **The board's first falsifier, as a test: no request is planned with the
/// variable unset, whatever is or is not on disk.**
///
/// The cache states matter as much as the switch. A missing cache is the
/// state that *wants* a fetch, and an ancient one is the state that wants a
/// refetch, so if the guard were ever ordered after the staleness check those
/// two rows would be the ones to fire. They are here so that reordering fails
/// rather than merely looks wrong.
#[test]
fn nothing_is_fetched_while_the_variable_is_unset() {
    let now = at(2026, 9, 9);
    for cached in [
        None,
        Some(now),
        Some(now - Duration::days(1)),
        Some(now - Duration::days(400)),
    ] {
        assert_eq!(
            Plan::of(false, cached, now),
            Plan::Off,
            "cache of {cached:?} should still plan nothing"
        );
    }
}

/// The other half: with the variable set, the plan follows the cache's age and
/// nothing else.
#[test]
fn an_enabled_launch_fetches_only_what_it_has_not_got() {
    let now = at(2026, 9, 9);

    assert_eq!(Plan::of(true, None, now), Plan::Fetch, "no cache at all");
    assert_eq!(
        Plan::of(true, Some(now - Duration::days(31)), now),
        Plan::Fetch,
        "a cache past the ceiling"
    );
    assert_eq!(
        Plan::of(true, Some(now), now),
        Plan::Cached { age_days: 0 },
        "fetched this launch"
    );
    assert_eq!(
        Plan::of(true, Some(now - Duration::days(29)), now),
        Plan::Cached { age_days: 29 },
        "inside the ceiling"
    );
}

/// A clock that has gone backwards — a laptop resuming, an NTP correction —
/// reports an age of zero rather than a negative one. It is a small thing and
/// it reaches the card: `days_ago(-3)` would print `-3 days ago`.
#[test]
fn a_cache_from_the_future_is_zero_days_old_not_a_negative_number() {
    let now = at(2026, 9, 9);
    assert_eq!(
        Plan::of(true, Some(now + Duration::days(3)), now),
        Plan::Cached { age_days: 0 }
    );

    let catalogue = Catalogue {
        fetched: Some(now + Duration::days(3)),
        ..Catalogue::default()
    };
    assert_eq!(catalogue.age_days(now), Some(0));
}

/// **The same falsifier from the other side: the module builds exactly one
/// HTTP client, and it is inside the function the plan guards.**
///
/// The test above pins the decision; this pins that there is nowhere else to
/// make a request from. A second call site added later — a retry, a
/// "just check whether it changed", a helper that seemed too small to route
/// through `refresh_once` — would compile, pass every other test in this file,
/// and quietly send a request the person did not ask for. That is the exact
/// shape of the `eventsource` bypass in `egress.rs`, which stood for months
/// under a comment claiming the check could not be skipped.
///
/// Calibrated by making it fail: adding a second `from_client_builder` to the
/// module reddens this and nothing else.
#[test]
fn the_module_has_one_door_and_the_plan_is_in_front_of_it() {
    // Doc comments are stripped first, and that is not a detail: the first
    // version of this test counted `http_client::Client::` across the whole
    // file, found two, and failed on the module doc that *names* the method it
    // is checking. A guard that counts prose is a guard that will be loosened
    // by whoever next writes a paragraph.
    let code: String = include_str!("prices.rs")
        .lines()
        .filter(|line| {
            let trimmed = line.trim_start();
            !trimmed.starts_with("//!") && !trimmed.starts_with("///")
        })
        .collect::<Vec<_>>()
        .join("\n");

    let clients = code.matches("http_client::Client::").count();
    assert_eq!(
        clients, 1,
        "prices.rs should construct exactly one HTTP client; found {clients}"
    );

    let (_, after_plan) = code
        .split_once("Plan::Fetch => match fetch().await {")
        .expect("refresh_once should reach the network only from the Fetch arm");
    assert!(
        after_plan.contains("http_client::Client::"),
        "the client should be built after the Fetch arm, not before it"
    );

    // `reqwest` is here only to hand `from_client_builder` a timeout. Reaching
    // for it directly would skip `execute_inner`, and with it the egress
    // policy -- the one thing this module must not do.
    assert_eq!(
        code.matches("reqwest::").count(),
        1,
        "the only mention of reqwest in code should be the builder handed to http_client"
    );
    assert!(
        code.contains("http_client::Client::from_client_builder(reqwest::Client::builder()"),
        "the reqwest builder should go straight into http_client, not be used on its own"
    );
}

/// OpenRouter quotes USD **per token**, as a decimal string. The card talks in
/// dollars per million, and the conversion happens once, on the way in.
///
/// The numbers here are the live ones for `anthropic/claude-opus-5`, read from
/// the real endpoint on 2026-09-09 — and they match the hand-written row in
/// `specs.default.toml` exactly, which is the calibration this whole feature
/// deserved: the fetch agrees with the four rows a person typed from the
/// vendor's page two days earlier.
#[test]
fn prices_arrive_per_token_and_are_kept_per_million() {
    let body = r#"{"data":[
        {"id":"anthropic/claude-opus-5","pricing":{"prompt":"0.000005","completion":"0.000025"}},
        {"id":"anthropic/claude-haiku-4.5","pricing":{"prompt":"0.000001","completion":"0.000005"}}
    ]}"#;
    let catalogue = parse(body, at(2026, 9, 9)).expect("the payload should parse");

    assert_eq!(
        catalogue.price("anthropic/claude-opus-5"),
        Some(Price {
            input: 5.0,
            output: 25.0
        })
    );
    assert_eq!(
        catalogue.price("anthropic/claude-haiku-4.5"),
        Some(Price {
            input: 1.0,
            output: 5.0
        })
    );
    assert_eq!(catalogue.source.as_deref(), Some(SOURCE));
    assert_eq!(catalogue.fetched, Some(at(2026, 9, 9)));
}

/// **A batch row is half price and one colon away**, and it is next to its
/// base model in the same list. Every `anthropic/*` slug in the live catalogue
/// has one. A prefix or "starts with" match would draw every Anthropic cost
/// bar at half its real width and look entirely plausible doing it.
#[test]
fn a_batch_slug_is_a_different_model_not_a_cheaper_spelling() {
    let body = r#"{"data":[
        {"id":"anthropic/claude-opus-5","pricing":{"prompt":"0.000005","completion":"0.000025"}},
        {"id":"anthropic/claude-opus-5:batch","pricing":{"prompt":"0.0000025","completion":"0.0000125"}}
    ]}"#;
    let catalogue = parse(body, at(2026, 9, 9)).expect("the payload should parse");

    assert_eq!(
        catalogue.price("anthropic/claude-opus-5").map(|p| p.output),
        Some(25.0)
    );
    assert_eq!(
        catalogue
            .price("anthropic/claude-opus-5:batch")
            .map(|p| p.output),
        Some(12.5)
    );
    assert_eq!(catalogue.price("anthropic/claude-opus"), None);
}

/// A row whose price will not parse is dropped, not stored as zero.
///
/// **`-1` is not a hypothesis.** The live catalogue read on 2026-09-09 had
/// **five** rows carrying `"prompt":"-1","completion":"-1"`, all of them
/// `openrouter/*` auto-routers (`auto`, `fusion`, `pareto-code`, …) — models
/// whose price is whatever the router picks, so the catalogue declines to
/// quote one. Twenty-one other rows cost a genuine `0`.
///
/// So the two cases sit side by side in the same list and must not draw the
/// same bar. Zero means free and draws an empty cost bar; unknown draws `?`.
/// A parse failure that fell back to zero would announce that an auto-router
/// is free.
#[test]
fn an_unparseable_price_is_dropped_and_a_free_one_is_kept() {
    let body = r#"{"data":[
        {"id":"openrouter/auto","pricing":{"prompt":"-1","completion":"-1"}},
        {"id":"vendor/broken","pricing":{"prompt":"-1","completion":"0.000001"}},
        {"id":"vendor/nonsense","pricing":{"prompt":"","completion":"0.000001"}},
        {"id":"vendor/free","pricing":{"prompt":"0","completion":"0"}}
    ]}"#;
    let catalogue = parse(body, at(2026, 9, 9)).expect("the payload should parse");

    assert_eq!(catalogue.price("openrouter/auto"), None);
    assert_eq!(catalogue.price("vendor/broken"), None);
    assert_eq!(catalogue.price("vendor/nonsense"), None);
    assert_eq!(
        catalogue.price("vendor/free"),
        Some(Price {
            input: 0.0,
            output: 0.0
        })
    );
}

/// A body that is not a catalogue is an error, not an empty catalogue that
/// would be written over a good cache.
#[test]
fn a_body_that_is_not_a_catalogue_does_not_become_an_empty_one() {
    assert!(parse("<html>502 Bad Gateway</html>", at(2026, 9, 9)).is_err());
    assert!(parse(r#"{"error":"rate limited"}"#, at(2026, 9, 9)).is_err());

    let empty = parse(r#"{"data":[]}"#, at(2026, 9, 9)).expect("a real, empty catalogue parses");
    assert!(empty.is_empty());
}

/// **And an empty one is never installed over a good cache**, which is the
/// case the parse test above cannot reach: `{"data":[]}` is well-formed, so
/// it arrives as `Ok`, and only `refresh_once`'s guard stops it replacing
/// every price with a `?` on the strength of a 200.
///
/// Pinned by source text because the arm is inside an `async fn` that dials a
/// network. That is a weaker instrument than an assertion and it is the
/// honest one available; the parse tests above carry the real coverage.
#[test]
fn an_empty_answer_does_not_replace_the_cached_prices() {
    let source = include_str!("prices.rs");
    let (before_install, _) = source
        .split_once("Ok(catalogue) => {")
        .expect("refresh_once should have an install arm");
    assert!(
        before_install.contains("Ok(catalogue) if catalogue.is_empty()"),
        "the empty-catalogue guard should come before the arm that installs one"
    );
}
