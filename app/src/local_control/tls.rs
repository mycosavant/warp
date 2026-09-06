//! Trust on the wire for the wide listener (T19, 2026-09-06).
//!
//! # Why the console needs a certificate at all
//!
//! A phone at `http://192.168.254.3:41234` is not in a secure context: only
//! `https://`, `localhost`, `127.0.0.1`, `*.localhost` and `file://` are, and
//! a tailnet address over plain HTTP is no better however encrypted the tunnel
//! under it. Outside a secure context a browser withholds the Notification
//! API, service workers, push and Chrome's install prompt, so the phone can be
//! looked at and cannot be told anything. The decision on record
//! (`.fork/decisions/2026-09-06-the-phone-surface-four-decisions.md`) is a CA
//! this fork mints itself and the phone installs once. Refused there: a
//! self-signed server certificate on its own, which is a warning page and a
//! tap-through that teaches the habit a real attacker needs, and a public
//! certificate, which is a DNS name and an account for a console meant to be
//! found by one phone.
//!
//! # What is minted, and where it lives
//!
//! Two keys. The **authority** is minted once, under `fork::state_dir()`
//! beside the discovery record, through `create_private_file` so the key is
//! owner-only from its first byte; it is what the phone installs and it never
//! leaves this directory. The **server key** is minted on every launch, in
//! memory, and its certificate names the bind address as an IP subject
//! alternative name, so changing `WARP_FORK_CONTROL_BIND` changes nothing the
//! phone has to redo. The leaf lives 397 days because Apple refuses TLS
//! server certificates that live longer than 825 and a fresh one is minted
//! every launch anyway; the authority lives ten years because reinstalling it
//! is the one step that costs the person a walk through settings.
//!
//! # One port, two protocols, and why that is not a widening
//!
//! The authority has to reach the phone before the phone trusts anything this
//! listener says, and a certificate served only over TLS the phone does not
//! yet trust is a tap-through on the first visit -- exactly what the decision
//! refused. So [`serve`] peeks the first byte of every connection: a TLS
//! `ClientHello` (`0x16`) goes to the acceptor and the real router; anything
//! else is answered by a two-route plain router that serves the authority's
//! public half at [`CA_PATH`] and, for every other path, a constant page
//! saying how to install it. Both bodies are constants with no secret in
//! them, which is the same argument the console's own unauthenticated
//! documents rest on (`console.rs`). Nothing under `/v1/` is reachable in the
//! clear, and a test pins that by asking for it.
//!
//! The loopback listener stays plain and is served by `axum::serve` as it
//! always was: `warpctrl` is not a browser and `127.0.0.1` is already a
//! secure context. `keep_from_children` runs on the raw listener before any of
//! this sees it.
//!
//! # What this does not do
//!
//! It does not verify the phone. The credential story is unchanged -- a QR
//! code, a device token, five-minute action credentials -- and TLS here is
//! the phone verifying *Warp*, so that the token and every approval payload
//! stop travelling in the clear and the browser grants the page what a secure
//! context gets. A phone that never installs the authority can still tap
//! through the warning and reach the same console; it simply gets nothing a
//! secure context would have given it.
use std::net::IpAddr;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use anyhow::Context as _;
use axum::Router;
use axum::http::header::{CACHE_CONTROL, CONTENT_DISPOSITION, CONTENT_TYPE};
use axum::http::{HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use hyper_util::rt::{TokioExecutor, TokioIo};
use hyper_util::service::TowerToHyperService;
use rcgen::{
    BasicConstraints, CertificateParams, DistinguishedName, DnType, ExtendedKeyUsagePurpose, IsCa,
    Issuer, KeyPair, KeyUsagePurpose, SanType,
};
use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer};
use time::OffsetDateTime;
use tokio::net::TcpListener;
use tokio_rustls::TlsAcceptor;

/// Where the authority's public half is served, on the wide listener in the
/// clear and over TLS alike.
pub(crate) const CA_PATH: &str = "/ca.crt";

/// The media type both Android and iOS recognise as a certificate to install.
const CA_CONTENT_TYPE: &str = "application/x-x509-ca-cert";

const CA_KEY_FILE: &str = "console-ca.key";
const CA_CERTIFICATE_FILE: &str = "console-ca.crt";

/// Ten years for the authority: reinstalling it is the step that costs the
/// person a walk through their phone's settings.
const AUTHORITY_LIFETIME: time::Duration = time::Duration::days(3650);

/// 397 days for the leaf. Apple refuses TLS server certificates that live
/// longer than 825 days on any root and 398 on its own; a fresh leaf is minted
/// every launch, so the number only matters for a Warp left running for a
/// year.
const LEAF_LIFETIME: time::Duration = time::Duration::days(397);

/// One day of clock slack on `not_before`, because a phone's clock and this
/// machine's are two clocks and this fork has already measured them
/// disagreeing (`observability.md`, "two clocks").
const NOT_BEFORE_SLACK: time::Duration = time::Duration::days(1);

/// How long a connection may sit silent before its first byte is read. A
/// scanner that connects and says nothing would otherwise hold a task forever.
const FIRST_BYTE_TIMEOUT: Duration = Duration::from_secs(10);

/// What the wide listener needs to speak TLS: the acceptor, and the
/// authority's public half to hand out.
#[derive(Clone)]
pub(super) struct ConsoleTls {
    pub(super) acceptor: TlsAcceptor,
    pub(super) ca_certificate_pem: Arc<str>,
}

/// The authority, loaded or minted, as far as signing needs it.
struct Authority {
    key: KeyPair,
    params: CertificateParams,
    certificate_pem: String,
}

/// Prepares TLS for a wide listener bound at `address`, minting the authority
/// under the fork's state directory on first use.
pub(super) fn for_address(address: IpAddr) -> anyhow::Result<ConsoleTls> {
    for_address_in(
        &crate::fork::state_dir(),
        address,
        OffsetDateTime::now_utc(),
    )
}

/// [`for_address`] with the directory and the clock as arguments, so a test
/// can hold both.
pub(super) fn for_address_in(
    dir: &Path,
    address: IpAddr,
    now: OffsetDateTime,
) -> anyhow::Result<ConsoleTls> {
    let authority = load_or_mint_authority(dir, now)?;
    let issuer = Issuer::from_params(&authority.params, &authority.key);

    let leaf_key = KeyPair::generate().context("generating the server key")?;
    let mut leaf = CertificateParams::new(Vec::<String>::new())
        .context("empty certificate parameters are always valid")?;
    leaf.subject_alt_names = vec![SanType::IpAddress(address)];
    leaf.distinguished_name = DistinguishedName::new();
    leaf.distinguished_name
        .push(DnType::CommonName, format!("Warp console {address}"));
    leaf.is_ca = IsCa::NoCa;
    leaf.key_usages = vec![KeyUsagePurpose::DigitalSignature];
    leaf.extended_key_usages = vec![ExtendedKeyUsagePurpose::ServerAuth];
    leaf.use_authority_key_identifier_extension = true;
    leaf.not_before = now - NOT_BEFORE_SLACK;
    leaf.not_after = now + LEAF_LIFETIME;
    let leaf_certificate = leaf
        .signed_by(&leaf_key, &issuer)
        .context("signing the server certificate")?;

    // `aws_lc_rs` named rather than `CryptoProvider::get_default()`: this
    // graph compiles rustls with that provider and not `ring`, and asking for
    // the default when only one is compiled works until a dependency enables
    // the other, at which point it panics at the first connection.
    let mut config = rustls::ServerConfig::builder_with_provider(Arc::new(
        rustls::crypto::aws_lc_rs::default_provider(),
    ))
    .with_safe_default_protocol_versions()
    .context("rustls protocol versions")?
    .with_no_client_auth()
    .with_single_cert(
        vec![leaf_certificate.der().clone()],
        PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(leaf_key.serialize_der())),
    )
    .context("building the server configuration")?;
    config.alpn_protocols = vec![b"h2".to_vec(), b"http/1.1".to_vec()];
    Ok(ConsoleTls {
        acceptor: TlsAcceptor::from(Arc::new(config)),
        ca_certificate_pem: Arc::from(authority.certificate_pem),
    })
}

/// The authority's DER, for a client that wants to trust it.
#[cfg(test)]
fn ca_certificate_der(pem: &str) -> anyhow::Result<CertificateDer<'static>> {
    let parsed = pem::parse(pem).context("the authority's PEM")?;
    Ok(CertificateDer::from(parsed.into_contents()))
}

fn authority_paths(dir: &Path) -> (PathBuf, PathBuf) {
    (dir.join(CA_KEY_FILE), dir.join(CA_CERTIFICATE_FILE))
}

/// Reads the authority back if both halves are on disk and parse, and mints
/// a new one otherwise.
///
/// A key that is there and a certificate that is not, or either that does not
/// parse, is a fresh mint and a warning: the phone has to install the new one,
/// and the log is where the person learns why the old one stopped working.
fn load_or_mint_authority(dir: &Path, now: OffsetDateTime) -> anyhow::Result<Authority> {
    let (key_path, certificate_path) = authority_paths(dir);
    match load_authority(&key_path, &certificate_path) {
        Ok(Some(authority)) => return Ok(authority),
        Ok(None) => {}
        Err(err) => log::warn!(
            "local-control console authority under {} could not be read and is being minted \
             again; phones that installed the old one have to install this one: {err:#}",
            dir.display()
        ),
    }
    let authority = mint_authority(now)?;
    crate::fork::create_private_dir(dir).with_context(|| format!("creating {}", dir.display()))?;
    use std::io::Write as _;
    crate::fork::create_private_file(&key_path, false)
        .and_then(|mut file| file.write_all(authority.key.serialize_pem().as_bytes()))
        .with_context(|| format!("writing {}", key_path.display()))?;
    crate::fork::create_private_file(&certificate_path, false)
        .and_then(|mut file| file.write_all(authority.certificate_pem.as_bytes()))
        .with_context(|| format!("writing {}", certificate_path.display()))?;
    log::info!(
        "local-control console authority minted under {}; phones install {CA_PATH} once",
        dir.display()
    );
    Ok(authority)
}

fn load_authority(key_path: &Path, certificate_path: &Path) -> anyhow::Result<Option<Authority>> {
    if !key_path.exists() && !certificate_path.exists() {
        return Ok(None);
    }
    let key_pem = std::fs::read_to_string(key_path)
        .with_context(|| format!("reading {}", key_path.display()))?;
    let certificate_pem = std::fs::read_to_string(certificate_path)
        .with_context(|| format!("reading {}", certificate_path.display()))?;
    let key = KeyPair::from_pem(&key_pem).context("parsing the authority's key")?;
    pem::parse(&certificate_pem).context("parsing the authority's certificate")?;
    let params = authority_params(&key, OffsetDateTime::UNIX_EPOCH);
    Ok(Some(Authority {
        key,
        params,
        certificate_pem,
    }))
}

fn mint_authority(now: OffsetDateTime) -> anyhow::Result<Authority> {
    let key = KeyPair::generate().context("generating the authority's key")?;
    let params = authority_params(&key, now);
    let certificate = params
        .self_signed(&key)
        .context("self-signing the authority")?;
    Ok(Authority {
        certificate_pem: certificate.pem(),
        key,
        params,
    })
}

/// The authority's parameters, derived from its key alone so they can be
/// rebuilt on every launch without persisting anything but the two PEM files.
///
/// The issuer name on every leaf has to equal the authority's subject byte for
/// byte, and rcgen's [`Issuer`] takes that name from parameters rather than
/// from the certificate unless its `x509-parser` feature is on. Deriving the
/// name from the key keeps it stable across launches and different between
/// machines, which is what a phone that has two of these installed needs to
/// tell them apart. Validity is not part of the issuer, so the epoch passed on
/// reload is never written anywhere.
fn authority_params(key: &KeyPair, now: OffsetDateTime) -> CertificateParams {
    let mut params = CertificateParams::default();
    let key_id = params.key_identifier(key);
    let short_id: String = key_id
        .iter()
        .take(4)
        .map(|byte| format!("{byte:02x}"))
        .collect();
    params.distinguished_name = DistinguishedName::new();
    params.distinguished_name.push(
        DnType::CommonName,
        format!("Warp fork console CA {short_id}"),
    );
    params
        .distinguished_name
        .push(DnType::OrganizationName, "warp fork");
    params.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
    params.key_usages = vec![KeyUsagePurpose::KeyCertSign, KeyUsagePurpose::CrlSign];
    params.not_before = now - NOT_BEFORE_SLACK;
    params.not_after = now + AUTHORITY_LIFETIME;
    params
}

/// Answers `GET /ca.crt`: the authority's public half, as a download.
pub(super) fn ca_response(pem: &str) -> Response {
    let mut response = (StatusCode::OK, pem.to_owned()).into_response();
    let headers = response.headers_mut();
    headers.insert(CONTENT_TYPE, HeaderValue::from_static(CA_CONTENT_TYPE));
    headers.insert(
        CONTENT_DISPOSITION,
        HeaderValue::from_static("attachment; filename=\"warp-console-ca.crt\""),
    );
    headers.insert(CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response
}

/// The page a plain-HTTP request to any other path on the wide port gets.
///
/// A constant: no interpolation, no script, the same headers as the console's
/// own documents. It says what the address is and how to install the
/// authority, and links the certificate by a relative path so it works from
/// whatever address the phone typed. It does not redirect to `https://`,
/// because a redirect on a phone that has not installed the authority yet is
/// the warning page this whole module exists to avoid.
pub(super) const INSTALL_PAGE: &str = include_str!("console_install.html");

async fn handle_plain_ca(axum::extract::State(pem): axum::extract::State<Arc<str>>) -> Response {
    ca_response(&pem)
}

async fn handle_install_page() -> Response {
    super::console::served(INSTALL_PAGE.as_bytes(), "text/html; charset=utf-8")
}

/// The two routes a connection that did not start a TLS handshake gets.
pub(super) fn plain_router(ca_certificate_pem: Arc<str>) -> Router {
    Router::new()
        .route(CA_PATH, get(handle_plain_ca))
        .fallback(handle_install_page)
        .with_state(ca_certificate_pem)
}

/// Serves the wide listener: TLS to `router`, anything else to the plain
/// router. Runs until the listener fails to accept, which is the runtime
/// going away.
pub(super) async fn serve(listener: TcpListener, tls: ConsoleTls, router: Router) {
    let plain = plain_router(tls.ca_certificate_pem.clone());
    loop {
        let (stream, peer) = match listener.accept().await {
            Ok(accepted) => accepted,
            Err(err) => {
                // Out of descriptors, or the listener is gone. Neither is a
                // reason to spin, and the second is a reason to stop.
                log::warn!("local-control wide listener accept failed: {err:#}");
                tokio::time::sleep(Duration::from_millis(100)).await;
                continue;
            }
        };
        let acceptor = tls.acceptor.clone();
        let router = router.clone();
        let plain = plain.clone();
        tokio::spawn(async move {
            let mut first = [0u8; 1];
            let peeked = tokio::time::timeout(FIRST_BYTE_TIMEOUT, stream.peek(&mut first)).await;
            match peeked {
                Ok(Ok(1)) => {}
                // Closed before saying anything, or silent past the timeout.
                _ => return,
            }
            let builder = hyper_util::server::conn::auto::Builder::new(TokioExecutor::new());
            if first[0] == TLS_HANDSHAKE_RECORD {
                let stream = match acceptor.accept(stream).await {
                    Ok(stream) => stream,
                    Err(err) => {
                        // A phone that has not installed the authority yet
                        // fails here, once per visit; not worth more than a
                        // debug line.
                        log::debug!("local-control TLS handshake with {peer} failed: {err:#}");
                        return;
                    }
                };
                if let Err(err) = builder
                    .serve_connection_with_upgrades(
                        TokioIo::new(stream),
                        TowerToHyperService::new(router),
                    )
                    .await
                {
                    log::debug!("local-control TLS connection from {peer} ended: {err:#}");
                }
            } else if let Err(err) = builder
                .serve_connection_with_upgrades(
                    TokioIo::new(stream),
                    TowerToHyperService::new(plain),
                )
                .await
            {
                log::debug!("local-control plain connection from {peer} ended: {err:#}");
            }
        });
    }
}

/// The first byte of a TLS record carrying a handshake. Every `ClientHello`
/// starts with it and no HTTP method does.
const TLS_HANDSHAKE_RECORD: u8 = 0x16;

#[cfg(test)]
#[path = "tls_tests.rs"]
mod tests;
