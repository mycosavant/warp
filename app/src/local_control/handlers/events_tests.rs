use ::local_control::{ActionKind, EventStreamResult, InstanceId};
use chrono::Duration;

use super::*;

fn grant() -> CredentialGrant {
    CredentialGrant::new(
        InstanceId("inst_test".to_owned()),
        ActionKind::EventsSubscribe,
        Duration::minutes(5),
    )
}

#[test]
fn subscribe_returns_an_absolute_url_on_the_requesting_origin() {
    let grant = grant();
    let value = events_subscribe(Some("127.0.0.1:34969"), &grant).expect("subscribe answers");
    let result: EventStreamResult =
        serde_json::from_value(value).expect("result decodes as its declared type");

    assert_eq!(result.url, "http://127.0.0.1:34969/v1/events");
    assert_eq!(result.expires_at, grant.expires_at);
}

/// The URL is built from the same constant the router registers, so the two
/// cannot disagree. Asserted rather than assumed, because a client that is
/// handed a 404 has no way to tell it from the feature being off.
#[test]
fn the_advertised_path_is_the_route_that_is_served() {
    let value = events_subscribe(Some("127.0.0.1:1"), &grant()).expect("subscribe answers");
    let result: EventStreamResult = serde_json::from_value(value).expect("result decodes");
    assert!(
        result.url.ends_with(EVENT_STREAM_PATH),
        "{} should end with the registered route {EVENT_STREAM_PATH}",
        result.url
    );
}

/// Refusing beats guessing a port: a URL pointing at nothing is worse than an
/// error, because a client will retry it.
#[test]
fn subscribe_refuses_before_the_server_has_bound() {
    let error = events_subscribe(None, &grant()).expect_err("no origin is an error");
    assert_eq!(error.code, ErrorCode::BridgeUnavailable);
}

/// The caller already holds the bearer token; echoing it into a result puts a
/// secret in a shell scrollback.
#[test]
fn subscribe_does_not_echo_credential_material() {
    let value = events_subscribe(Some("127.0.0.1:1"), &grant()).expect("subscribe answers");
    let rendered = serde_json::to_string(&value).expect("result serializes");
    assert!(!rendered.contains("bearer"), "{rendered}");
    assert!(!rendered.contains("token"), "{rendered}");
    assert!(!rendered.contains("credential_id"), "{rendered}");
}
