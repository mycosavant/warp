//! Running app-side server for local Warp control requests.
//!
//! This module owns the in-process listener, discovery registration, credential
//! broker socket, and request handoff from Axum into the WarpUI model graph.
//! It complements `crates/local_control/src/discovery.rs`: that shared module
//! defines how clients find and validate candidate instances, while this module
//! creates the app-owned endpoints and publishes their routing metadata through
//! `RegisteredInstance`.
//!
//! A client uses all three transports in order. It reads the filesystem record
//! to find an instance, connects to that instance's Unix socket to obtain
//! temporary authority, and presents that authority to the instance's loopback
//! HTTP endpoint with one typed action. The filesystem and socket are therefore
//! complementary parts of discovery and credential bootstrap, not competing
//! discovery mechanisms.
//!
//! Credential broker security flow:
//!
//! ```text
//! owner-only discovery record
//! (loopback endpoint + broker path; never a token)
//!                 |
//!                 v
//! CLI client -- instance-bound Unix socket --> credential broker
//!                 [0600 socket + kernel-reported peer UID]
//!                                             |
//!                                             v
//!                           feature flag + Settings > Scripting gate
//!                           + protocol + exact action metadata
//!                                             |
//!                                             v
//!                           short-lived, instance-bound, action-scoped
//!                           bearer grant stored only in process memory
//!                                             |
//!                                             v
//! CLI client -- loopback HTTP + bearer --> /v1/control
//!                 [reject browser Origin + require exact Host
//!                  + validate grant existence, expiry, instance, and scope]
//!                                             |
//!                                             v
//!                           typed allowlisted action
//!                                             |
//!                                             v
//!                           main-thread LocalControlBridge
//!                           [re-check current settings before dispatch]
//! ```
//!
//! These boundaries prevent browser-origin clients, other OS users,
//! unauthenticated clients that only obtain or guess the HTTP endpoint, stale
//! or wrong-instance credentials, and accidentally over-scoped credentials from
//! invoking actions. The broker authenticates the OS account, not the calling
//! application: malicious software already running as the same user remains
//! outside this boundary.
//!
//! The Settings > Scripting gates used here are local-only settings backed by
//! Warp's secure storage provider.
//!
//! Discovery records never include raw bearer tokens: discovery only exposes
//! endpoint metadata and credential broker references while Scripting is enabled.
mod bridge;
mod handlers;
mod permissions;
pub(crate) mod resolver;

use std::collections::HashMap;
#[cfg(unix)]
use std::fs::Permissions;
use std::net::SocketAddr;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt as _;
use std::sync::{Arc, Mutex};

use ::local_control::auth::CredentialGrant;
#[cfg(any(unix, windows, test))]
use ::local_control::auth::{CredentialRequest, ScopedCredential};
use ::local_control::{
    ActionKind, AuthToken, ControlEndpoint, ControlError, ControlResponse, ErrorCode,
    ErrorResponseEnvelope, InstanceId, InstanceRecord, RegisteredInstance, RequestEnvelope,
    ResponseEnvelope,
};
use axum::body::Bytes;
use axum::extract::State;
use axum::http::header::{AUTHORIZATION, HOST, ORIGIN};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
pub use bridge::LocalControlBridge;
#[cfg(any(unix, windows, test))]
use chrono::Duration;
use permissions::ensure_feature_enabled;
#[cfg(any(unix, windows, test))]
use permissions::{ensure_action_allowed, ensure_protocol_version};
#[cfg(any(unix, windows))]
use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};
use warp_core::channel::ChannelState;
use warpui::{Entity, ModelContext, ModelSpawner, SingletonEntity};

#[cfg(any(unix, windows, test))]
const MAX_ACTIVE_CREDENTIALS: usize = 128;

/// Path of the live event stream (T11.2). Shared with the `events.subscribe`
/// handler, which hands clients an absolute URL built from it — one definition,
/// so the advertised URL cannot disagree with the route that serves it.
pub(crate) const EVENT_STREAM_PATH: &str = "/v1/events";

/// Path of the state snapshot (T11.2).
const STATE_PATH: &str = "/v1/state";

/// How often an idle event stream wakes up.
///
/// Two jobs: notice an expired credential without waiting for the next event,
/// and emit a comment frame so a quiet connection is not mistaken for a dead
/// one. Short enough that expiry is prompt, long enough to be free.
const EVENT_STREAM_TICK: std::time::Duration = std::time::Duration::from_secs(15);

/// App-owned authority shared by one instance's broker and HTTP listener.
///
/// Broker-issued bearer tokens map to grants only in this process-local state.
/// Knowing the endpoint from discovery is therefore insufficient to authenticate
/// an HTTP request.
#[derive(Clone)]
struct ControlServerState {
    bridge_spawner: ModelSpawner<LocalControlBridge>,
    instance_id: InstanceId,
    expected_host: String,
    credentials: Arc<Mutex<HashMap<String, CredentialGrant>>>,
}
/// Process-local publisher, credential broker, and HTTP server for one Warp instance.
///
/// Holding the runtime and registration keeps both listeners and the discovery
/// route alive. Dropping them stops request handling and removes the app's
/// published record and broker socket.
pub struct LocalControlServer {
    _runtime: Option<tokio::runtime::Runtime>,
    control_endpoint: Option<ControlEndpoint>,
    registered_instance: Option<RegisteredInstance>,
}

impl Entity for LocalControlServer {
    type Event = ();
}

impl SingletonEntity for LocalControlServer {}

impl LocalControlServer {
    pub fn new(ctx: &mut ModelContext<Self>) -> Self {
        let mut server = Self {
            _runtime: None,
            control_endpoint: None,
            registered_instance: None,
        };
        if let Err(error) = server.refresh_for_settings(ctx) {
            log::warn!("Failed to refresh local-control server state: {error:#}");
        }
        ctx.subscribe_to_model(
            &crate::settings::LocalControlSettings::handle(ctx),
            |server, _, _, ctx| {
                if let Err(error) = server.refresh_for_settings(ctx) {
                    log::warn!("Failed to refresh local-control server state: {error:#}");
                }
            },
        );
        server
    }

    /// Starts, refreshes, or removes local-control publication as settings change.
    fn refresh_for_settings(&mut self, ctx: &mut ModelContext<Self>) -> Result<(), ControlError> {
        if !permissions::warp_control_cli_enabled() {
            self.stop(ctx);
            return Ok(());
        }
        if !local_control_publication_supported() {
            self.stop(ctx);
            return Ok(());
        }
        if !crate::settings::LocalControlSettings::as_ref(ctx).is_enabled() {
            self.stop(ctx);
            return Ok(());
        }
        if self._runtime.is_some() {
            return self.refresh_discovery_record(ctx);
        }
        self.start(ctx)
    }

    /// Stops both listeners and removes the discovery record and broker socket.
    fn stop(&mut self, _ctx: &mut ModelContext<Self>) {
        self.registered_instance = None;
        self.control_endpoint = None;
        self._runtime = None;
    }

    /// Binds both transports and publishes the routing record that connects them.
    ///
    /// Startup first binds an ephemeral loopback HTTP port, publishes that port
    /// plus the instance-derived broker filename, binds the broker socket, and
    /// then serves credential issuance and typed control requests concurrently.
    fn start(&mut self, ctx: &mut ModelContext<Self>) -> Result<(), ControlError> {
        if self._runtime.is_some() {
            return Err(ControlError::new(
                ErrorCode::Internal,
                "local-control server is already running",
            ));
        }
        ensure_feature_enabled()?;
        if !local_control_publication_supported() {
            return Err(ControlError::new(
                ErrorCode::LocalControlDisabled,
                "local control is disabled until this platform enforces discovery-record ACLs",
            ));
        }
        if !crate::settings::LocalControlSettings::as_ref(ctx).is_enabled() {
            return Ok(());
        }
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(1)
            .enable_io()
            // T11.2: the event stream's keepalive is a `tokio::time::interval`,
            // and a Tokio runtime without the time driver does not fail to build
            // or fail to compile — it panics on the first timer, inside the
            // connection task, where it surfaces to the client as a dropped
            // connection with no status. Found by running it; every check up to
            // that point was green.
            .enable_time()
            .build()
            .map_err(|err| {
                ControlError::with_details(
                    ErrorCode::Internal,
                    "failed to create local-control runtime",
                    err.to_string(),
                )
            })?;
        let listener = runtime
            .block_on(tokio::net::TcpListener::bind(SocketAddr::from((
                [127, 0, 0, 1],
                0,
            ))))
            .map_err(|err| {
                ControlError::with_details(
                    ErrorCode::Internal,
                    "failed to bind local-control listener",
                    err.to_string(),
                )
            })?;
        let port = listener.local_addr().map_err(|err| {
            ControlError::with_details(
                ErrorCode::Internal,
                "failed to read local-control listener address",
                err.to_string(),
            )
        })?;
        let control_endpoint = ControlEndpoint::localhost(port.port());
        let record = discovery_record_for_settings(ctx, control_endpoint.clone());
        let instance_id = record.instance_id.clone();
        let control_origin = format!("{}:{}", control_endpoint.host, control_endpoint.port);
        let bridge_spawner = LocalControlBridge::handle(ctx).update(ctx, |bridge, ctx| {
            bridge.set_instance_id(instance_id.clone());
            bridge.set_control_origin(control_origin.clone());
            ctx.spawner()
        });
        let registered_instance = RegisteredInstance::register(record)?;
        #[cfg(unix)]
        let broker_listener = {
            let runtime_guard = runtime.enter();
            let listener = bind_credential_broker(registered_instance.record())?;
            drop(runtime_guard);
            listener
        };
        // Bind the first pipe instance synchronously, so a name collision or
        // ACL failure surfaces from `start()` exactly as a Unix bind failure
        // does, rather than being logged from a detached task.
        #[cfg(windows)]
        let (broker_pipe_name, broker_pipe) = {
            let pipe_name = registered_instance.record().broker_pipe_name()?;
            let runtime_guard = runtime.enter();
            let pipe = create_broker_pipe(&pipe_name, true)?;
            drop(runtime_guard);
            (pipe_name, pipe)
        };
        let state = ControlServerState {
            bridge_spawner,
            instance_id,
            expected_host: format!("{}:{}", control_endpoint.host, control_endpoint.port),
            credentials: Arc::default(),
        };
        let router = Router::new()
            .route("/v1/control", post(handle_control_request))
            // The read surface (T11.2). Both are `GET` so a client that can only
            // fetch and subscribe needs no envelope, and both take the same
            // scoped credentials as `/v1/control`.
            .route(STATE_PATH, get(handle_state_request))
            .route(EVENT_STREAM_PATH, get(handle_event_stream))
            .with_state(state.clone());
        runtime.spawn(async move {
            if let Err(err) = axum::serve(listener, router).await {
                log::warn!("local-control listener stopped: {err:#}");
            }
        });
        #[cfg(unix)]
        runtime.spawn(run_credential_broker(broker_listener, state));
        #[cfg(windows)]
        runtime.spawn(run_credential_broker(broker_pipe_name, broker_pipe, state));
        let endpoint_url = control_endpoint.url();
        self._runtime = Some(runtime);
        self.control_endpoint = Some(control_endpoint);
        self.registered_instance = Some(registered_instance);
        log::info!("local-control server started at {endpoint_url}");
        Ok(())
    }

    fn refresh_discovery_record(
        &mut self,
        ctx: &mut ModelContext<Self>,
    ) -> Result<(), ControlError> {
        let Some(control_endpoint) = self.control_endpoint.clone() else {
            return Ok(());
        };
        let Some(registered_instance) = &mut self.registered_instance else {
            return Ok(());
        };
        let mut record = discovery_record_for_settings(ctx, control_endpoint);
        record.instance_id = registered_instance.record().instance_id.clone();
        record.credential_broker = registered_instance.record().credential_broker.clone();
        registered_instance.update(record)
    }
}

/// Builds routing metadata without embedding any bearer credential or secret.
///
/// The endpoint and derived broker reference are published only while the
/// protected Scripting setting permits clients to use them.
fn discovery_record_for_settings(
    ctx: &ModelContext<LocalControlServer>,
    control_endpoint: ControlEndpoint,
) -> InstanceRecord {
    let endpoint = crate::settings::LocalControlSettings::as_ref(ctx)
        .is_enabled()
        .then_some(control_endpoint);
    InstanceRecord::for_current_process(
        endpoint,
        ChannelState::channel().to_string(),
        ChannelState::app_id().to_string(),
        ChannelState::app_version().map(str::to_owned),
        ActionKind::implemented_metadata(),
    )
}

/// Binds the instance's credential-bootstrap socket and restricts it to the owning user.
///
/// Any stale socket at the instance-specific path is removed before binding, and
/// the new socket is set to owner-only permissions before it accepts clients.
/// The path came from a validated instance-derived discovery reference, so a
/// record cannot redirect credential requests to an arbitrary socket.
#[cfg(unix)]
fn bind_credential_broker(
    record: &InstanceRecord,
) -> Result<tokio::net::UnixListener, ControlError> {
    let socket_path = record.broker_socket_path()?;
    if socket_path.exists() {
        std::fs::remove_file(&socket_path).map_err(|err| {
            ControlError::with_details(
                ErrorCode::Internal,
                "failed to remove stale local-control credential broker socket",
                err.to_string(),
            )
        })?;
    }
    let listener = tokio::net::UnixListener::bind(&socket_path).map_err(|err| {
        ControlError::with_details(
            ErrorCode::Internal,
            "failed to bind owner-authenticated local-control credential broker",
            err.to_string(),
        )
    })?;
    std::fs::set_permissions(&socket_path, Permissions::from_mode(0o600)).map_err(|err| {
        ControlError::with_details(
            ErrorCode::Internal,
            "failed to protect local-control credential broker socket",
            err.to_string(),
        )
    })?;
    Ok(listener)
}

#[cfg(unix)]
/// Accepts same-user credential requests independently from the HTTP listener.
async fn run_credential_broker(listener: tokio::net::UnixListener, state: ControlServerState) {
    loop {
        let Ok((stream, _)) = listener.accept().await else {
            return;
        };
        let state = state.clone();
        tokio::spawn(async move {
            if let Err(err) = handle_credential_broker_connection(stream, state).await {
                log::warn!("local-control credential broker connection failed: {err:#}");
            }
        });
    }
}

#[cfg(unix)]
/// Authenticates the socket peer before decoding and evaluating its request.
///
/// This ordering makes the kernel-reported OS user, rather than any field in
/// caller-controlled JSON, the credential broker's client-identity boundary.
async fn handle_credential_broker_connection(
    mut stream: tokio::net::UnixStream,
    state: ControlServerState,
) -> Result<(), ControlError> {
    let response = match ensure_same_user_peer(&stream) {
        Ok(()) => {
            let mut bytes = Vec::new();
            stream.read_to_end(&mut bytes).await.map_err(|err| {
                ControlError::with_details(
                    ErrorCode::InvalidRequest,
                    "failed to read local-control credential request",
                    err.to_string(),
                )
            })?;
            match serde_json::from_slice::<CredentialRequest>(&bytes) {
                Ok(request) => issue_credential(&state, request)
                    .await
                    .and_then(|credential| serialize_credential_broker_response(&credential)),
                Err(err) => Err(ControlError::with_details(
                    ErrorCode::InvalidRequest,
                    "failed to decode local-control credential request",
                    err.to_string(),
                )),
            }
        }
        Err(error) => Err(error),
    };
    let bytes = match response {
        Ok(bytes) => bytes,
        Err(error) => serialize_credential_broker_response(&ErrorResponseEnvelope::new(error))?,
    };
    stream.write_all(&bytes).await.map_err(|err| {
        ControlError::with_details(
            ErrorCode::TransportUnavailable,
            "failed to write local-control credential response",
            err.to_string(),
        )
    })
}

#[cfg(unix)]
/// Requires the kernel-reported peer UID to match Warp's effective UID.
///
/// This excludes other OS users but does not distinguish trusted Warp code from
/// arbitrary processes already running as the same user.
fn ensure_same_user_peer(stream: &tokio::net::UnixStream) -> Result<(), ControlError> {
    ensure_peer_uid(stream, unsafe { libc::geteuid() })
}

#[cfg(unix)]
/// Verifies a socket peer against an expected UID obtained outside request data.
fn ensure_peer_uid(stream: &tokio::net::UnixStream, expected_uid: u32) -> Result<(), ControlError> {
    let peer = stream.peer_cred().map_err(|err| {
        ControlError::with_details(
            ErrorCode::UnauthorizedLocalClient,
            "failed to identify local-control credential broker peer",
            err.to_string(),
        )
    })?;
    if peer.uid() != expected_uid {
        return Err(ControlError::new(
            ErrorCode::UnauthorizedLocalClient,
            "local-control credential broker peer belongs to a different OS user",
        ));
    }
    Ok(())
}

/// Creates one named-pipe server instance protected by an owner-only DACL.
///
/// A named pipe serves a single client per instance, so the accept loop creates
/// the next instance before handing the connected one off. `first` sets
/// `FILE_FLAG_FIRST_PIPE_INSTANCE` on the initial creation, which makes the
/// call fail rather than silently join an existing pipe of the same name that
/// something else already owns.
///
/// The descriptor is rebuilt per instance instead of being shared: a raw
/// `PSECURITY_DESCRIPTOR` is not `Send`, and rebuilding sidesteps holding one
/// across the accept loop's awaits entirely. The kernel copies the descriptor
/// into the object at creation, so freeing it immediately afterwards is safe.
#[cfg(windows)]
fn create_broker_pipe(
    pipe_name: &str,
    first: bool,
) -> Result<tokio::net::windows::named_pipe::NamedPipeServer, ControlError> {
    use ::local_control::windows_security::OwnerOnlySecurityDescriptor;
    use tokio::net::windows::named_pipe::ServerOptions;

    let descriptor = OwnerOnlySecurityDescriptor::new()?;
    let mut attributes = descriptor.attributes();
    unsafe {
        ServerOptions::new()
            .first_pipe_instance(first)
            .create_with_security_attributes_raw(
                pipe_name,
                &mut attributes as *mut _ as *mut std::ffi::c_void,
            )
    }
    .map_err(|err| {
        ControlError::with_details(
            ErrorCode::Internal,
            "failed to bind owner-authenticated local-control credential broker",
            err.to_string(),
        )
    })
}

/// Accepts same-user credential requests independently from the HTTP listener.
///
/// Mirrors the Unix broker's accept loop. The next pipe instance is created
/// before the connected one is handed off, so there is no window in which a
/// client can find the pipe missing.
#[cfg(windows)]
async fn run_credential_broker(
    pipe_name: String,
    mut server: tokio::net::windows::named_pipe::NamedPipeServer,
    state: ControlServerState,
) {
    loop {
        if server.connect().await.is_err() {
            return;
        }
        let connected = server;
        server = match create_broker_pipe(&pipe_name, false) {
            Ok(next) => next,
            Err(err) => {
                log::warn!("local-control credential broker stopped accepting: {err:#}");
                return;
            }
        };
        let state = state.clone();
        tokio::spawn(async move {
            if let Err(err) = handle_credential_broker_connection(connected, state).await {
                log::warn!("local-control credential broker connection failed: {err:#}");
            }
        });
    }
}

/// Authenticates the pipe peer before decoding and evaluating its request.
///
/// Ordering matches the Unix broker: the OS-reported caller identity, not any
/// field in caller-controlled JSON, is the client-identity boundary.
///
/// Framing differs by necessity. The Unix broker reads to EOF after the client
/// shuts down its write half; a named pipe has no half-close, so both
/// directions carry a `u32` length prefix.
#[cfg(windows)]
async fn handle_credential_broker_connection(
    mut pipe: tokio::net::windows::named_pipe::NamedPipeServer,
    state: ControlServerState,
) -> Result<(), ControlError> {
    let response = match ensure_same_user_peer(&pipe) {
        Ok(()) => match read_broker_request(&mut pipe).await {
            Ok(bytes) => match serde_json::from_slice::<CredentialRequest>(&bytes) {
                Ok(request) => issue_credential(&state, request)
                    .await
                    .and_then(|credential| serialize_credential_broker_response(&credential)),
                Err(err) => Err(ControlError::with_details(
                    ErrorCode::InvalidRequest,
                    "failed to decode local-control credential request",
                    err.to_string(),
                )),
            },
            Err(error) => Err(error),
        },
        Err(error) => Err(error),
    };
    let bytes = match response {
        Ok(bytes) => bytes,
        Err(error) => serialize_credential_broker_response(&ErrorResponseEnvelope::new(error))?,
    };
    let length = u32::try_from(bytes.len()).map_err(|_| {
        ControlError::new(
            ErrorCode::Internal,
            "local-control credential response is too large to frame",
        )
    })?;
    pipe.write_all(&length.to_le_bytes()).await.map_err(|err| {
        broker_io_error("write the local-control credential response length", err)
    })?;
    pipe.write_all(&bytes)
        .await
        .map_err(|err| broker_io_error("write the local-control credential response", err))
}

/// Reads one length-prefixed credential request.
#[cfg(windows)]
async fn read_broker_request(
    pipe: &mut tokio::net::windows::named_pipe::NamedPipeServer,
) -> Result<Vec<u8>, ControlError> {
    /// Bounds the request so a hostile prefix cannot force a large allocation
    /// before any payload has arrived.
    const MAX_BROKER_REQUEST_BYTES: usize = 64 * 1024;

    let mut length = [0u8; 4];
    pipe.read_exact(&mut length)
        .await
        .map_err(|err| broker_io_error("read the local-control credential request length", err))?;
    let length = u32::from_le_bytes(length) as usize;
    if length > MAX_BROKER_REQUEST_BYTES {
        return Err(ControlError::new(
            ErrorCode::InvalidRequest,
            "local-control credential request exceeded the maximum size",
        ));
    }
    let mut bytes = vec![0u8; length];
    pipe.read_exact(&mut bytes)
        .await
        .map_err(|err| broker_io_error("read the local-control credential request", err))?;
    Ok(bytes)
}

#[cfg(windows)]
fn broker_io_error(operation: &str, error: std::io::Error) -> ControlError {
    ControlError::with_details(
        ErrorCode::TransportUnavailable,
        format!("failed to {operation}"),
        error.to_string(),
    )
}

/// Requires the impersonated pipe client's user SID to match Warp's own.
///
/// This is the Windows analogue of comparing the kernel-reported peer UID:
/// impersonation asks the OS who the caller is rather than trusting anything
/// the caller sent. Like the Unix check it excludes other OS users, and like
/// the Unix check it does not distinguish trusted Warp code from arbitrary
/// processes already running as the same user.
///
/// Impersonation is reverted on every path, including failure — leaving the
/// thread impersonating would let subsequent work on it run as the client.
#[cfg(windows)]
fn ensure_same_user_peer(
    pipe: &tokio::net::windows::named_pipe::NamedPipeServer,
) -> Result<(), ControlError> {
    use std::os::windows::io::AsRawHandle as _;

    use ::local_control::windows_security::{OwnedHandle, current_user_sid_string, token_user};
    use windows::Win32::Foundation::HANDLE;
    use windows::Win32::Security::{RevertToSelf, TOKEN_QUERY};
    use windows::Win32::System::Pipes::ImpersonateNamedPipeClient;
    use windows::Win32::System::Threading::{GetCurrentThread, OpenThreadToken};
    use windows::core::PWSTR;

    let handle = HANDLE(pipe.as_raw_handle());
    unsafe { ImpersonateNamedPipeClient(handle) }.map_err(|err| {
        ControlError::with_details(
            ErrorCode::UnauthorizedLocalClient,
            "failed to identify local-control credential broker peer",
            err.to_string(),
        )
    })?;

    let peer_sid = (|| {
        let mut token = HANDLE::default();
        unsafe { OpenThreadToken(GetCurrentThread(), TOKEN_QUERY, true, &mut token) }.map_err(
            |err| {
                ControlError::with_details(
                    ErrorCode::UnauthorizedLocalClient,
                    "failed to open the local-control credential broker peer token",
                    err.to_string(),
                )
            },
        )?;
        let token = OwnedHandle(token);
        let user = token_user(token.0)?;
        let mut sid_string = PWSTR::null();
        unsafe {
            windows::Win32::Security::Authorization::ConvertSidToStringSidW(
                user.sid(),
                &mut sid_string,
            )
        }
        .map_err(|err| {
            ControlError::with_details(
                ErrorCode::UnauthorizedLocalClient,
                "failed to convert the local-control credential broker peer SID",
                err.to_string(),
            )
        })?;
        let value = unsafe { sid_string.to_string() }.map_err(|err| {
            ControlError::with_details(
                ErrorCode::UnauthorizedLocalClient,
                "failed to decode the local-control credential broker peer SID",
                err.to_string(),
            )
        });
        unsafe {
            let _ = windows::Win32::Foundation::LocalFree(Some(std::mem::transmute::<
                *mut u16,
                windows::Win32::Foundation::HLOCAL,
            >(sid_string.0)));
        }
        value
    })();

    // Revert before evaluating, so an early return cannot leave the thread
    // impersonating the client.
    unsafe { RevertToSelf() }.map_err(|err| {
        ControlError::with_details(
            ErrorCode::Internal,
            "failed to revert local-control credential broker impersonation",
            err.to_string(),
        )
    })?;

    if peer_sid? != current_user_sid_string()? {
        return Err(ControlError::new(
            ErrorCode::UnauthorizedLocalClient,
            "local-control credential broker peer belongs to a different OS user",
        ));
    }
    Ok(())
}

#[cfg(any(unix, windows))]
fn serialize_credential_broker_response(
    response: &impl serde::Serialize,
) -> Result<Vec<u8>, ControlError> {
    serde_json::to_vec(response).map_err(|err| {
        ControlError::with_details(
            ErrorCode::Internal,
            "failed to serialize local-control credential response",
            err.to_string(),
        )
    })
}

/// Evaluates current action policy and mints one short-lived exact-action grant.
///
/// The bearer secret and its grant are retained only in the running instance's
/// process-local map; neither is written back into the discovery registry.
#[cfg(any(unix, windows, test))]
async fn issue_credential(
    state: &ControlServerState,
    request: CredentialRequest,
) -> Result<ScopedCredential, ControlError> {
    ensure_feature_enabled()?;
    ensure_protocol_version(request.protocol_version)?;
    if !request.action.is_implemented() {
        return Err(ControlError::new(
            ErrorCode::UnsupportedAction,
            format!(
                "{} is not implemented by this local-control bridge",
                request.action.as_str()
            ),
        ));
    }
    state
        .bridge_spawner
        .spawn({
            let action = request.action;
            move |_, ctx| ensure_action_allowed(action, ctx)
        })
        .await
        .map_err(|_| {
            ControlError::new(
                ErrorCode::BridgeUnavailable,
                "local-control app bridge is unavailable",
            )
        })??;
    let auth_token = AuthToken::generate();
    let grant = CredentialGrant::new(
        state.instance_id.clone(),
        request.action,
        Duration::minutes(5),
    );
    let mut credentials = state.credentials.lock().map_err(|_| {
        ControlError::new(
            ErrorCode::Internal,
            "local-control credential broker is unavailable",
        )
    })?;
    insert_credential(
        &mut credentials,
        auth_token.secret().to_owned(),
        grant.clone(),
    );
    Ok(ScopedCredential {
        bearer_token: auth_token.secret().to_owned(),
        grant,
    })
}

/// Authenticates and hands one typed HTTP request to the app bridge.
///
/// Header hardening rejects browser-origin and wrong-endpoint requests. The
/// process-local credential lookup authenticates the transport, after which the
/// bridge revalidates current settings and exact-action authority before
/// resolving targets or dispatching a handler.
async fn handle_control_request(
    State(state): State<ControlServerState>,
    headers: HeaderMap,
    payload: Bytes,
) -> Response {
    let grant = match authenticate(&state, &headers) {
        Ok(grant) => grant,
        Err(response) => return response,
    };
    let request = match serde_json::from_slice::<RequestEnvelope>(&payload) {
        Ok(request) => request,
        Err(err) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(ErrorResponseEnvelope::new(ControlError::with_details(
                    ErrorCode::InvalidRequest,
                    "failed to decode local-control request",
                    err.to_string(),
                ))),
            )
                .into_response();
        }
    };
    let request_id = request.request_id;
    let response = match state
        .bridge_spawner
        .spawn(move |bridge, ctx| bridge.handle_request(request, grant, ctx))
        .await
    {
        Ok(response) => response,
        Err(_) => ResponseEnvelope::error(
            request_id,
            ControlError::new(
                ErrorCode::BridgeUnavailable,
                "local-control app bridge is unavailable",
            ),
        ),
    };
    let status = match &response.response {
        ControlResponse::Ok { .. } => StatusCode::OK,
        ControlResponse::Error { .. } => StatusCode::BAD_REQUEST,
    };
    (status, Json(response)).into_response()
}

#[cfg(any(unix, windows, test))]
fn insert_credential(
    credentials: &mut HashMap<String, CredentialGrant>,
    secret: String,
    grant: CredentialGrant,
) {
    credentials.retain(|_, grant| !grant.is_expired());
    if credentials.len() >= MAX_ACTIVE_CREDENTIALS {
        let oldest_secret = credentials
            .iter()
            .min_by_key(|(_, grant)| grant.issued_at)
            .map(|(secret, _)| secret.clone());
        if let Some(oldest_secret) = oldest_secret {
            credentials.remove(&oldest_secret);
        }
    }
    credentials.insert(secret, grant);
}

/// Answers `GET /v1/state` — the snapshot a client primes itself with before
/// subscribing (T11.2).
///
/// The body is exactly what `agent.list` returns over `POST /v1/control`, and
/// the credential it requires is an `agent.list` one, because it *is*
/// `agent.list`: this route exists so a browser can fetch it without composing a
/// request envelope, not to expose anything new.
async fn handle_state_request(
    State(state): State<ControlServerState>,
    headers: HeaderMap,
) -> Response {
    let grant = match authenticate(&state, &headers) {
        Ok(grant) => grant,
        Err(response) => return response,
    };
    if let Err(error) = require_action(&grant, ActionKind::AgentList) {
        return (
            StatusCode::FORBIDDEN,
            Json(ErrorResponseEnvelope::new(error)),
        )
            .into_response();
    }
    let request = RequestEnvelope::new(::local_control::Action::new(ActionKind::AgentList));
    let request_id = request.request_id;
    let response = match state
        .bridge_spawner
        .spawn(move |bridge, ctx| bridge.handle_request(request, grant, ctx))
        .await
    {
        Ok(response) => response,
        Err(_) => ResponseEnvelope::error(
            request_id,
            ControlError::new(
                ErrorCode::BridgeUnavailable,
                "local-control app bridge is unavailable",
            ),
        ),
    };
    let status = match &response.response {
        ControlResponse::Ok { .. } => StatusCode::OK,
        ControlResponse::Error { .. } => StatusCode::BAD_REQUEST,
    };
    (status, Json(response)).into_response()
}

/// Answers `GET /v1/events` — the live stream (T11.2).
///
/// Each SSE `data:` frame is one line of the event log, forwarded verbatim: the
/// same string that was written to disk, so a subscriber re-broadcasts bytes it
/// never parsed and cannot drift from the on-disk format.
///
/// **The stream ends when the grant does.** A credential is good for five
/// minutes, and a connection authorized once at open would outlive its own
/// authority — which is exactly the "localhost, therefore fine" reasoning T11.4
/// exists to avoid. Expiry is re-checked before every frame and on a tick, so an
/// idle stream closes on time rather than at the next event. Clients are
/// expected to obtain a fresh credential and reconnect; `events.subscribe`
/// returns `expires_at` so they can do it before being cut off.
async fn handle_event_stream(
    State(state): State<ControlServerState>,
    headers: HeaderMap,
) -> Response {
    let grant = match authenticate(&state, &headers) {
        Ok(grant) => grant,
        Err(response) => return response,
    };
    if let Err(error) = require_action(&grant, ActionKind::EventsSubscribe) {
        return (
            StatusCode::FORBIDDEN,
            Json(ErrorResponseEnvelope::new(error)),
        )
            .into_response();
    }
    // Subscribe before returning: a receiver is what makes `event_log::record`
    // start producing when no file sink is configured, and taking it here rather
    // than inside the stream closes the window where events between the
    // handshake and the first poll would be missed.
    let receiver = crate::event_log::subscribe();
    let stream = async_stream::stream! {
        let mut receiver = receiver;
        // Cheap heartbeat so an idle stream still notices expiry, and so
        // intermediaries do not decide a quiet connection is a dead one.
        let mut tick = tokio::time::interval(EVENT_STREAM_TICK);
        tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        loop {
            if grant.is_expired() {
                yield Ok::<_, std::convert::Infallible>(
                    axum::response::sse::Event::default()
                        .event("expired")
                        .data("credential expired; obtain a new one and reconnect"),
                );
                break;
            }
            tokio::select! {
                received = receiver.recv() => match received {
                    Ok(line) => yield Ok(axum::response::sse::Event::default().data(line)),
                    // The subscriber fell behind the bounded channel. Say so
                    // rather than silently skipping: a reader that does not know
                    // it has a gap will draw the wrong conclusion from one.
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(missed)) => {
                        yield Ok(axum::response::sse::Event::default()
                            .event("lagged")
                            .data(missed.to_string()));
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                },
                _ = tick.tick() => {
                    yield Ok(axum::response::sse::Event::default().comment("keepalive"));
                }
            }
        }
    };
    axum::response::Sse::new(stream).into_response()
}

/// Rejects a credential minted for some other action.
///
/// The grant already proved it belongs to this instance and has not expired;
/// this is the exact-action half, stated separately because the `GET` routes
/// name their action rather than carrying it in a request envelope.
fn require_action(grant: &CredentialGrant, expected: ActionKind) -> Result<(), ControlError> {
    if grant.action != expected {
        return Err(ControlError::new(
            ErrorCode::InsufficientPermissions,
            format!(
                "credential for {} cannot open {}",
                grant.action.as_str(),
                expected.as_str()
            ),
        ));
    }
    Ok(())
}

/// The authentication preamble every route shares.
///
/// Factored out when T11.2 added the two `GET` routes, and deliberately *not*
/// duplicated for them: header hardening, the feature gate, bearer parsing and
/// the process-local credential lookup are the whole of this server's
/// authorization boundary, and two copies of a boundary is one copy that drifts.
///
/// The error half is already an HTTP response, because each failure has its own
/// status and callers should not have to re-derive which.
fn authenticate(
    state: &ControlServerState,
    headers: &HeaderMap,
) -> Result<CredentialGrant, Response> {
    let reject = |status: StatusCode, error: ControlError| -> Response {
        (status, Json(ErrorResponseEnvelope::new(error))).into_response()
    };
    if let Err(error) = validate_loopback_headers(headers, &state.expected_host) {
        return Err(reject(StatusCode::FORBIDDEN, error));
    }
    if let Err(error) = ensure_feature_enabled() {
        return Err(reject(StatusCode::FORBIDDEN, error));
    }
    let auth_header = headers
        .get(AUTHORIZATION)
        .and_then(|value| value.to_str().ok());
    let auth_token = AuthToken::from_authorization_header(auth_header)
        .map_err(|error| reject(StatusCode::UNAUTHORIZED, error))?;
    let mut credentials = state.credentials.lock().map_err(|_| {
        reject(
            StatusCode::INTERNAL_SERVER_ERROR,
            ControlError::new(
                ErrorCode::Internal,
                "local-control credential broker is unavailable",
            ),
        )
    })?;
    lookup_credential(&mut credentials, &auth_token, &state.instance_id)
        .map_err(|error| reject(StatusCode::UNAUTHORIZED, error))
}

/// Resolves an unexpired bearer token issued by this exact running instance.
fn lookup_credential(
    credentials: &mut HashMap<String, CredentialGrant>,
    auth_token: &AuthToken,
    instance_id: &InstanceId,
) -> Result<CredentialGrant, ControlError> {
    if credentials
        .get(auth_token.secret())
        .is_some_and(CredentialGrant::is_expired)
    {
        credentials.remove(auth_token.secret());
    }
    let grant = credentials
        .get(auth_token.secret())
        .cloned()
        .ok_or_else(|| {
            ControlError::new(
                ErrorCode::UnauthorizedLocalClient,
                "local-control credential is invalid",
            )
        })?;
    grant.verify_for_action(instance_id, grant.action)?;
    Ok(grant)
}
/// Whether this platform can publish a discovery record safely.
///
/// The requirement is an owner-only ACL on the registry directory, each record
/// and the credential broker transport — without it, publication would leak
/// routing metadata to other OS users. Upstream expresses this as
/// `not(target_os = "windows")` because only the Unix mode-bit path existed.
///
/// The fork implements the same guarantee on Windows with a protected DACL
/// (see `local_control::windows_security`), so the condition is now stated as
/// the capability it actually stands for rather than as a platform list.
fn local_control_publication_supported() -> bool {
    cfg!(any(unix, windows))
}

/// Performs browser-origin hardening for local-control endpoints.
///
/// These checks intentionally reject browser-style `Origin` requests and stale
/// endpoint selections, but they are not an authorization boundary. Scoped
/// bearer credentials and grant validation remain the authority for control
/// requests.
pub(crate) fn validate_loopback_headers(
    headers: &HeaderMap,
    expected_host: &str,
) -> Result<(), ControlError> {
    if headers.contains_key(ORIGIN) {
        return Err(ControlError::new(
            ErrorCode::UnauthorizedLocalClient,
            "browser-origin local-control requests are not allowed",
        ));
    }
    let host = headers
        .get(HOST)
        .and_then(|value| value.to_str().ok())
        .ok_or_else(|| {
            ControlError::new(
                ErrorCode::UnauthorizedLocalClient,
                "Host header is required for local-control requests",
            )
        })?;
    if host != expected_host {
        return Err(ControlError::new(
            ErrorCode::UnauthorizedLocalClient,
            "Host header does not match the selected local-control endpoint",
        ));
    }
    Ok(())
}

#[cfg(test)]
pub(crate) use bridge::validate_request_authority;
#[cfg(test)]
pub(crate) use permissions::{capabilities, ensure_settings_allow_action};
#[cfg(test)]
pub(crate) use resolver::{
    require_active_window_id, resolve_index_from_ids, resolve_title_from_matches,
    validate_action_params, validate_tab_create_target,
};

#[cfg(test)]
#[path = "mod_tests.rs"]
mod tests;
