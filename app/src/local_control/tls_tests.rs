use std::net::{IpAddr, Ipv4Addr};
use std::sync::Arc;

use rustls::pki_types::ServerName;
use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};

use super::*;

/// The real clock: rustls verifies validity against it, so a fixed instant
/// would pass or fail by the calendar.
fn now() -> OffsetDateTime {
    OffsetDateTime::now_utc()
}

/// A client that trusts exactly the authority in `pem` and nothing else.
fn client_trusting(pem: &str) -> Arc<rustls::ClientConfig> {
    let mut roots = rustls::RootCertStore::empty();
    roots
        .add(ca_certificate_der(pem).expect("the authority parses"))
        .expect("the authority is a CA certificate");
    Arc::new(
        rustls::ClientConfig::builder_with_provider(Arc::new(
            rustls::crypto::aws_lc_rs::default_provider(),
        ))
        .with_safe_default_protocol_versions()
        .expect("protocol versions")
        .with_root_certificates(roots)
        .with_no_client_auth(),
    )
}

/// Drives a rustls client and server against each other in memory until both
/// are out of the handshake, and returns the client's verdict.
fn handshake_in_memory(
    client: Arc<rustls::ClientConfig>,
    server_name: ServerName<'static>,
    acceptor: &TlsAcceptor,
) -> Result<(), rustls::Error> {
    let server_config: Arc<rustls::ServerConfig> = acceptor.config().clone();
    let mut client = rustls::ClientConnection::new(client, server_name)?;
    let mut server = rustls::ServerConnection::new(server_config)?;
    let mut wire = Vec::new();
    for _ in 0..16 {
        if !client.is_handshaking() && !server.is_handshaking() {
            return Ok(());
        }
        wire.clear();
        client.write_tls(&mut wire).expect("writing to a Vec");
        let mut cursor = std::io::Cursor::new(&wire);
        while (cursor.position() as usize) < wire.len() {
            server.read_tls(&mut cursor).expect("reading from a slice");
        }
        server.process_new_packets()?;
        wire.clear();
        server.write_tls(&mut wire).expect("writing to a Vec");
        let mut cursor = std::io::Cursor::new(&wire);
        while (cursor.position() as usize) < wire.len() {
            client.read_tls(&mut cursor).expect("reading from a slice");
        }
        client.process_new_packets()?;
    }
    panic!("the handshake did not settle in sixteen rounds");
}

/// The authority is minted once and read back after that: a phone installs
/// it one time, so a Warp restart must not produce a new one.
#[test]
fn the_authority_is_minted_once_and_survives_a_restart() {
    let dir = tempfile::tempdir().expect("a scratch state directory");
    let address = IpAddr::V4(Ipv4Addr::new(192, 168, 1, 5));

    let first = for_address_in(dir.path(), address, now()).expect("first launch mints");
    let second = for_address_in(dir.path(), address, now()).expect("second launch reloads");
    assert_eq!(
        first.ca_certificate_pem, second.ca_certificate_pem,
        "the authority a phone installed must still be the one in use"
    );
    assert!(dir.path().join(CA_KEY_FILE).is_file());
    assert!(dir.path().join(CA_CERTIFICATE_FILE).is_file());
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        let mode = std::fs::metadata(dir.path().join(CA_KEY_FILE))
            .expect("the key is on disk")
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(mode, 0o600, "the authority's key is owner-only");
    }

    // And the leaf a reloaded authority signs is one a client trusting the
    // authority from the first launch accepts: the issuer name was rebuilt
    // from the key and matches the certificate on disk byte for byte.
    handshake_in_memory(
        client_trusting(&first.ca_certificate_pem),
        ServerName::IpAddress(address.into()),
        &second.acceptor,
    )
    .expect("a phone that installed the authority yesterday still connects today");
}

/// The leaf names the bind address and only it. A phone connecting to the
/// address it scanned gets no warning; the same certificate at another address
/// is refused, which is what stops one machine's leaf standing in for
/// another's on the same authority.
#[test]
fn the_leaf_is_for_the_bind_address_and_the_authority_is_the_trust() {
    let dir = tempfile::tempdir().expect("a scratch state directory");
    let address = IpAddr::V4(Ipv4Addr::new(192, 168, 254, 3));
    let tls = for_address_in(dir.path(), address, now()).expect("mints");

    handshake_in_memory(
        client_trusting(&tls.ca_certificate_pem),
        ServerName::IpAddress(address.into()),
        &tls.acceptor,
    )
    .expect("the bind address is on the certificate");

    let other = IpAddr::V4(Ipv4Addr::new(10, 0, 0, 7));
    let err = handshake_in_memory(
        client_trusting(&tls.ca_certificate_pem),
        ServerName::IpAddress(other.into()),
        &tls.acceptor,
    )
    .expect_err("another address is not on the certificate");
    assert!(
        matches!(err, rustls::Error::InvalidCertificate(_)),
        "refused as a certificate error, not a transport one: {err:?}"
    );

    // A client that trusts nothing, which is what a phone is before it
    // installs the authority, refuses: this is the warning page, and it is why
    // the authority is served in the clear.
    let stranger = {
        let other_dir = tempfile::tempdir().expect("another scratch directory");
        let other = for_address_in(other_dir.path(), address, now()).expect("another machine");
        client_trusting(&other.ca_certificate_pem)
    };
    let err = handshake_in_memory(
        stranger,
        ServerName::IpAddress(address.into()),
        &tls.acceptor,
    )
    .expect_err("another machine's authority did not sign this leaf");
    assert!(
        matches!(err, rustls::Error::InvalidCertificate(_)),
        "{err:?}"
    );
}

/// The one port speaks two protocols, and what the plain one reaches is the
/// pin: the certificate and an install page, never a control route. A request
/// for `/v1/state` in the clear gets the install page, not the API and not a
/// `401` from it.
#[tokio::test]
async fn plain_http_on_the_wide_port_gets_the_certificate_and_nothing_else() {
    let dir = tempfile::tempdir().expect("a scratch state directory");
    let loopback = IpAddr::V4(Ipv4Addr::LOCALHOST);
    let tls = for_address_in(dir.path(), loopback, now()).expect("mints");
    let listener = TcpListener::bind((loopback, 0)).await.expect("binds");
    let bound = listener.local_addr().expect("bound");
    // A stand-in for the control router: any reply from it is the failure.
    let router = Router::new().route(
        "/v1/state",
        get(|| async { (StatusCode::OK, "the control plane answered in the clear") }),
    );
    let ca_pem = tls.ca_certificate_pem.clone();
    tokio::spawn(serve(listener, tls, router));

    async fn plain(bound: std::net::SocketAddr, path: &str) -> String {
        let mut stream = tokio::net::TcpStream::connect(bound)
            .await
            .expect("connects");
        stream
            .write_all(
                format!("GET {path} HTTP/1.1\r\nHost: {bound}\r\nConnection: close\r\n\r\n")
                    .as_bytes(),
            )
            .await
            .expect("writes");
        let mut response = String::new();
        stream.read_to_string(&mut response).await.expect("reads");
        response
    }

    let certificate = plain(bound, CA_PATH).await;
    assert!(certificate.starts_with("HTTP/1.1 200"), "{certificate}");
    assert!(certificate.contains(&format!("content-type: {CA_CONTENT_TYPE}")));
    assert!(certificate.contains("-----BEGIN CERTIFICATE-----"));
    assert!(certificate.ends_with(ca_pem.as_ref()));

    for path in ["/v1/state", "/", "/console.js", "/v1/pair"] {
        let reply = plain(bound, path).await;
        assert!(reply.starts_with("HTTP/1.1 200"), "{path}: {reply}");
        assert!(
            reply.contains("install the certificate") || reply.contains("get the certificate"),
            "{path} in the clear must get the install page: {reply}"
        );
        assert!(
            !reply.contains("the control plane answered in the clear"),
            "{path} reached the control router without TLS"
        );
    }

    // Over TLS, with the authority trusted, the same path reaches the router.
    let connector = tokio_rustls::TlsConnector::from(client_trusting(&ca_pem));
    let stream = tokio::net::TcpStream::connect(bound)
        .await
        .expect("connects");
    let mut stream = connector
        .connect(ServerName::IpAddress(loopback.into()), stream)
        .await
        .expect("a client trusting the authority completes the handshake");
    stream
        .write_all(
            format!("GET /v1/state HTTP/1.1\r\nHost: {bound}\r\nConnection: close\r\n\r\n")
                .as_bytes(),
        )
        .await
        .expect("writes");
    let mut reply = Vec::new();
    // A close_notify the peer skipped is not a failure of the request.
    let _ = stream.read_to_end(&mut reply).await;
    let reply = String::from_utf8_lossy(&reply);
    assert!(
        reply.contains("the control plane answered in the clear"),
        "over TLS the router answers: {reply}"
    );
}

/// The install page is a constant with no script, served with the console's
/// headers, and it links the certificate by a relative path so it works from
/// whichever address the phone typed.
#[tokio::test]
async fn the_install_page_is_a_constant_that_runs_nothing() {
    assert!(!INSTALL_PAGE.contains("<script"), "the page runs no script");
    assert!(INSTALL_PAGE.contains("href=\"/ca.crt\""));
    let response = handle_install_page().await;
    assert_eq!(response.status(), StatusCode::OK);
    let policy = response
        .headers()
        .get(axum::http::header::CONTENT_SECURITY_POLICY)
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default();
    assert!(policy.starts_with("default-src 'none'"), "{policy}");
}

/// Only the wide listener is wrapped. Loopback is served by `axum::serve` as it
/// always was, because `warpctrl` speaks `http://` to the address in the
/// discovery record and `127.0.0.1` is already a secure context. Pinned on the
/// source, since the two listeners are wired in one function that needs the
/// whole app to run.
#[test]
fn the_loopback_listener_stays_plain_and_only_the_wide_one_speaks_tls() {
    let module = include_str!("mod.rs");
    let plain_serves: Vec<&str> = module
        .lines()
        .filter(|line| line.contains("axum::serve(") && !line.trim_start().starts_with("//"))
        .collect();
    assert_eq!(
        plain_serves.len(),
        1,
        "exactly one listener is served plain: {plain_serves:?}"
    );
    assert!(
        plain_serves[0].contains("axum::serve(listener, router)"),
        "and it is the loopback one: {}",
        plain_serves[0]
    );
    assert!(
        module.contains("runtime.spawn(tls::serve(wide, tls, router));"),
        "the wide listener goes through tls::serve"
    );
    assert!(
        module.contains("format!(\"http://{}:{}/v1/control\"")
            || include_str!("../../../crates/local_control/src/discovery.rs")
                .contains("format!(\"http://{}:{}/v1/control\""),
        "the discovery record still names loopback over http://"
    );
}
