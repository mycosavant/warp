use opentelemetry::trace::{TraceContextExt as _, TracerProvider as _};
use opentelemetry_sdk::trace::SdkTracerProvider;
use tracing_opentelemetry::OpenTelemetrySpanExt as _;
use tracing_subscriber::layer::SubscriberExt as _;

use super::*;

/// Runs `f` with a real OpenTelemetry subscriber installed and a span entered,
/// so `Span::current()` resolves to a valid OTEL span context.
fn with_active_span<R>(f: impl FnOnce() -> R) -> R {
    let provider = SdkTracerProvider::builder().build();
    let tracer = provider.tracer("http_client-test");
    let subscriber =
        tracing_subscriber::registry().with(tracing_opentelemetry::layer().with_tracer(tracer));
    tracing::subscriber::with_default(subscriber, || {
        let span = tracing::info_span!("test-request");
        let _enter = span.enter();
        f()
    })
}

#[test]
fn injects_trace_link_header_when_span_active() {
    let (header, span_context) = with_active_span(|| {
        let header = current_trace_link_header();
        let span_context = tracing::Span::current()
            .context()
            .span()
            .span_context()
            .clone();
        (header, span_context)
    });

    assert!(span_context.is_valid());
    let header = header.expect("header should be present when a valid span is active");

    // W3C traceparent wire format: 00-<32 hex trace-id>-<16 hex span-id>-<2 hex flags>.
    let parts: Vec<&str> = header.split('-').collect();
    assert_eq!(parts.len(), 4, "unexpected header shape: {header}");
    assert_eq!(parts[0], "00");
    assert_eq!(parts[1], span_context.trace_id().to_string());
    assert_eq!(parts[2], span_context.span_id().to_string());
    assert_eq!(parts[1].len(), 32);
    assert_eq!(parts[2].len(), 16);
    assert_eq!(parts[3].len(), 2);
}

#[test]
fn omits_trace_link_header_when_no_span() {
    // No OTEL subscriber installed on this thread => no valid span context.
    let header = tracing::subscriber::with_default(
        tracing::subscriber::NoSubscriber::new(),
        current_trace_link_header,
    );
    assert!(header.is_none());
}

#[test]
fn request_carries_trace_link_header_on_warp_header_path() {
    // The header rides the same `include_warp_http_headers` gate as every other
    // X-Warp-* header (added only inside `add_warp_http_headers`), so building a
    // request through the client while a span is active carries it.
    let value = with_active_span(|| {
        let client = Client::new();
        let request = client
            .get("http://example.com/")
            .build()
            .expect("request should build");
        request
            .wrapped
            .headers()
            .get(headers::TRACE_LINK_HEADER)
            .map(|value| value.to_str().unwrap().to_string())
    });

    let value = value.expect("trace-link header should be added on the warp-header path");
    assert!(value.starts_with("00-"), "unexpected header value: {value}");
}

/// **The SSE path is inside the backstop, not outside it.**
///
/// `eventsource` is the one way out of [`Client`] that never reaches
/// `execute_inner`, where the egress check lives — so for as long as this crate
/// has had a deny-list, a stream to a blocked host would have connected. It was
/// not a live leak: every call site targets Warp's own service. That is what
/// makes it worth a test rather than a note, because "no call site does this
/// today" is a fact about today.
#[test]
fn a_blocked_host_is_redirected_on_the_eventsource_path_too() {
    let client = Client::new();
    let mut builder = client.get("https://o12345.ingest.sentry.io/api/1/envelope/");

    assert!(
        builder.redirect_if_blocked(),
        "a deny-listed host was left pointing at itself on the SSE path"
    );

    let url = builder
        .wrapped
        .try_clone()
        .and_then(|builder| builder.build().ok())
        .map(|request| request.url().clone())
        .expect("the redirected request is buildable");
    assert_eq!(
        url,
        egress::blackhole_url(),
        "redirected somewhere other than the blackhole"
    );
}

/// The other half, and the one that cannot pass by accident.
///
/// A `redirect_if_blocked` that returned `true` unconditionally would satisfy
/// the test above perfectly while sending every SSE stream — including Warp's
/// own agent traffic, which is what these call sites actually carry — into a
/// hole. Checking that an ordinary host is left alone is what makes the first
/// assertion mean anything.
#[test]
fn an_ordinary_host_is_left_alone_on_the_eventsource_path() {
    let client = Client::new();
    let mut builder = client.get("https://app.warp.dev/api/stream");

    assert!(
        !builder.redirect_if_blocked(),
        "redirected a host that is not on the deny-list"
    );

    let url = builder
        .wrapped
        .try_clone()
        .and_then(|builder| builder.build().ok())
        .map(|request| request.url().clone())
        .expect("the untouched request is buildable");
    assert_eq!(url.as_str(), "https://app.warp.dev/api/stream");
}
