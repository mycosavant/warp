//! Blocking client helpers used by the standalone `warpctrl` CLI.
//!
//! Authentication is a two-transport flow:
//!
//! 1. Discovery supplies instance metadata, an exact `127.0.0.1` control
//!    endpoint, and an instance-bound credential-broker socket reference. It
//!    never supplies a bearer credential.
//! 2. Before using either reference, the client validates that the endpoint is
//!    loopback and that the broker filename is derived from the selected
//!    instance ID.
//! 3. The client requests a credential for one action over the owner-only
//!    broker socket. On Unix, the server authenticates the
//!    connecting process through kernel-reported peer credentials before
//!    issuing a short-lived, action-scoped credential.
//! 4. The client keeps that credential in memory and presents it as a bearer
//!    token only to the selected instance's loopback HTTP endpoint. The running
//!    Warp app revalidates the credential, current settings, action scope, and
//!    request before dispatch.
//!
//! Client-side validation prevents accidental use of inconsistent discovery
//! authority, but it is not the authorization boundary. The broker and running
//! app enforce authorization, and credentials must never be written to
//! discovery records, logs, or command output.
#[cfg(unix)]
use std::io::{Read as _, Write as _};
#[cfg(unix)]
use std::net::Shutdown;
#[cfg(unix)]
use std::os::unix::net::UnixStream;
#[cfg(unix)]
use std::path::Path;

use crate::auth::{CredentialRequest, ScopedCredential};
use crate::discovery::InstanceRecord;
use crate::protocol::{
    Action, ActionKind, ControlError, ControlResponse, ErrorCode, ErrorResponseEnvelope,
    RequestEnvelope, ResponseEnvelope,
};

/// Requests an action-scoped credential and sends one authenticated control request.
#[cfg(not(target_family = "wasm"))]
pub fn send_request(
    instance: &InstanceRecord,
    request: &RequestEnvelope,
) -> Result<ResponseEnvelope, ControlError> {
    instance.validate_local_control_authority()?;
    let credential = request_credential(instance, request.action.kind)?;
    let endpoint = instance.endpoint.as_ref().ok_or_else(|| {
        ControlError::new(
            ErrorCode::LocalControlDisabled,
            "local control endpoint is disabled for this instance",
        )
    })?;
    let client = reqwest::blocking::Client::new();
    let response = client
        .post(endpoint.url())
        .header("Authorization", credential.authorization_value())
        .json(request)
        .send()
        .map_err(|err| {
            ControlError::with_details(
                ErrorCode::TransportUnavailable,
                "failed to send local-control request",
                err.to_string(),
            )
        })?;
    let status = response.status();
    let text = response.text().map_err(|err| {
        ControlError::with_details(
            ErrorCode::TransportUnavailable,
            "failed to read local-control response",
            err.to_string(),
        )
    })?;
    if let Ok(envelope) = serde_json::from_str::<ResponseEnvelope>(&text) {
        if let ControlResponse::Error { error } = &envelope.response {
            return Err(error.clone());
        }
        return Ok(envelope);
    }
    if let Ok(envelope) = serde_json::from_str::<ErrorResponseEnvelope>(&text) {
        return Err(envelope.error);
    }
    Err(ControlError::with_details(
        ErrorCode::TransportUnavailable,
        format!("local-control request failed with HTTP {status}"),
        text,
    ))
}

/// Fails closed on platforms without a native local-control HTTP transport.
#[cfg(target_family = "wasm")]
pub fn send_request(
    instance: &InstanceRecord,
    request: &RequestEnvelope,
) -> Result<ResponseEnvelope, ControlError> {
    request_credential(instance, request.action.kind)?;
    Err(ControlError::new(
        ErrorCode::LocalControlDisabled,
        "local control requires a native HTTP transport",
    ))
}
#[cfg(unix)]
/// Resolves the selected instance's validated broker path and requests a credential.
fn request_credential_over_owner_ipc(
    instance: &InstanceRecord,
    request: &CredentialRequest,
) -> Result<String, ControlError> {
    let path = instance.broker_socket_path()?;
    request_credential_over_socket(&path, request)
}

#[cfg(unix)]
/// Exchanges one credential request and response over an owner-authenticated socket.
///
/// Shutting down the write half delimits the JSON request so the broker can
/// read it to EOF before returning either a scoped credential or a structured
/// error response.
fn request_credential_over_socket(
    path: &Path,
    request: &CredentialRequest,
) -> Result<String, ControlError> {
    let mut stream = UnixStream::connect(path).map_err(|err| {
        ControlError::with_details(
            ErrorCode::TransportUnavailable,
            "failed to connect to the owner-authenticated local-control credential broker",
            err.to_string(),
        )
    })?;
    let request = serde_json::to_vec(request).map_err(|err| {
        ControlError::with_details(
            ErrorCode::InvalidRequest,
            "failed to serialize local-control credential request",
            err.to_string(),
        )
    })?;
    stream.write_all(&request).map_err(|err| {
        ControlError::with_details(
            ErrorCode::TransportUnavailable,
            "failed to write local-control credential request",
            err.to_string(),
        )
    })?;
    stream.shutdown(Shutdown::Write).map_err(|err| {
        ControlError::with_details(
            ErrorCode::TransportUnavailable,
            "failed to finish local-control credential request",
            err.to_string(),
        )
    })?;
    let mut response = String::new();
    stream.read_to_string(&mut response).map_err(|err| {
        ControlError::with_details(
            ErrorCode::TransportUnavailable,
            "failed to read local-control credential response",
            err.to_string(),
        )
    })?;
    Ok(response)
}

#[cfg(windows)]
/// Resolves the selected instance's validated broker pipe and requests a credential.
fn request_credential_over_owner_ipc(
    instance: &InstanceRecord,
    request: &CredentialRequest,
) -> Result<String, ControlError> {
    let pipe_name = instance.broker_pipe_name()?;
    request_credential_over_pipe(&pipe_name, request)
}

/// Exchanges one credential request and response over an owner-only named pipe.
///
/// The Unix broker delimits its request by shutting down the write half and
/// reading to EOF. A named pipe has no half-close, so both directions are
/// length-prefixed instead. Framing is chosen per platform rather than shared
/// because the Unix wire format is upstream's and left untouched; only this
/// fork's Windows transport is new.
#[cfg(windows)]
fn request_credential_over_pipe(
    pipe_name: &str,
    request: &CredentialRequest,
) -> Result<String, ControlError> {
    use std::fs::OpenOptions;
    use std::io::{Read as _, Write as _};

    let mut pipe = OpenOptions::new()
        .read(true)
        .write(true)
        .open(pipe_name)
        .map_err(|err| {
            ControlError::with_details(
                ErrorCode::TransportUnavailable,
                "failed to connect to the owner-authenticated local-control credential broker",
                err.to_string(),
            )
        })?;

    let payload = serde_json::to_vec(request).map_err(|err| {
        ControlError::with_details(
            ErrorCode::Internal,
            "failed to serialize local-control credential request",
            err.to_string(),
        )
    })?;
    let length = u32::try_from(payload.len()).map_err(|_| {
        ControlError::new(
            ErrorCode::InvalidRequest,
            "local-control credential request is too large",
        )
    })?;
    pipe.write_all(&length.to_le_bytes())
        .and_then(|()| pipe.write_all(&payload))
        .and_then(|()| pipe.flush())
        .map_err(|err| {
            ControlError::with_details(
                ErrorCode::TransportUnavailable,
                "failed to send local-control credential request",
                err.to_string(),
            )
        })?;

    let mut length = [0u8; 4];
    pipe.read_exact(&mut length).map_err(|err| {
        ControlError::with_details(
            ErrorCode::TransportUnavailable,
            "failed to read the local-control credential response length",
            err.to_string(),
        )
    })?;
    let length = u32::from_le_bytes(length) as usize;
    if length > MAX_BROKER_RESPONSE_BYTES {
        return Err(ControlError::new(
            ErrorCode::TransportUnavailable,
            "local-control credential response exceeded the maximum size",
        ));
    }
    let mut response = vec![0u8; length];
    pipe.read_exact(&mut response).map_err(|err| {
        ControlError::with_details(
            ErrorCode::TransportUnavailable,
            "failed to read the local-control credential response",
            err.to_string(),
        )
    })?;
    String::from_utf8(response).map_err(|err| {
        ControlError::with_details(
            ErrorCode::TransportUnavailable,
            "local-control credential response was not valid UTF-8",
            err.to_string(),
        )
    })
}

/// Caps the length-prefixed response so a hostile prefix cannot force a large
/// allocation before any of the payload has been read.
#[cfg(windows)]
const MAX_BROKER_RESPONSE_BYTES: usize = 64 * 1024;

#[cfg(all(not(unix), not(windows)))]
/// Fails closed on platforms without an owner-authenticated broker transport.
fn request_credential_over_owner_ipc(
    _instance: &InstanceRecord,
    _request: &CredentialRequest,
) -> Result<String, ControlError> {
    Err(ControlError::new(
        ErrorCode::LocalControlDisabled,
        "local control requires an owner-authenticated credential broker",
    ))
}

/// Requests and decodes a short-lived credential for one exact action.
pub fn request_credential(
    instance: &InstanceRecord,
    action: crate::protocol::ActionKind,
) -> Result<ScopedCredential, ControlError> {
    instance.validate_local_control_authority()?;
    let request = CredentialRequest::new(action);
    let text = request_credential_over_owner_ipc(instance, &request)?;
    if let Ok(credential) = serde_json::from_str::<ScopedCredential>(&text) {
        return Ok(credential);
    }
    if let Ok(envelope) = serde_json::from_str::<ErrorResponseEnvelope>(&text) {
        return Err(envelope.error);
    }
    Err(ControlError::with_details(
        ErrorCode::TransportUnavailable,
        "local-control credential broker returned an invalid response",
        text,
    ))
}

/// Authenticates an app-ping request and verifies the selected instance is live.
pub fn probe_instance(instance: &InstanceRecord) -> Result<(), ControlError> {
    let response = send_request(
        instance,
        &RequestEnvelope::new(Action::new(ActionKind::AppPing)),
    )?;
    validate_probe_response(instance, response)
}

/// Rejects a health response that does not prove the selected instance identity.
fn validate_probe_response(
    instance: &InstanceRecord,
    response: ResponseEnvelope,
) -> Result<(), ControlError> {
    let ControlResponse::Ok { data } = response.response else {
        return Err(ControlError::new(
            ErrorCode::TransportUnavailable,
            "local-control health probe returned an error response",
        ));
    };
    if data.get("instance_id").and_then(serde_json::Value::as_str)
        != Some(instance.instance_id.0.as_str())
    {
        return Err(ControlError::new(
            ErrorCode::TransportUnavailable,
            "local-control health probe returned a different instance identity",
        ));
    }
    Ok(())
}

#[cfg(test)]
#[path = "client_tests.rs"]
mod tests;
