//! Credential request, issuance, and validation types for local control.
use base64::Engine as _;
use chrono::{DateTime, Duration, Utc};
use rand::RngCore as _;
use serde::{Deserialize, Serialize};
use subtle::ConstantTimeEq as _;
use uuid::Uuid;

use crate::discovery::InstanceId;
use crate::protocol::{ActionKind, ControlError, ErrorCode};

/// Bearer token used to authorize a single scoped local-control credential.
///
/// Fork (T11.3): **`PartialEq` is hand-written so the comparison is
/// constant-time in the token's contents**, which the derived one does not
/// promise.
///
/// **First, a correction to why this was filed.** T11.3 was written against
/// this function on the belief that it authenticates control requests. It does
/// not — it has no production callers at all. The live path is
/// `app/src/local_control/mod.rs::lookup_credential`, which is a
/// `HashMap<String, CredentialGrant>::get(secret)`, and a hash lookup is not a
/// prefix oracle: `RandomState` seeds SipHash per process, so the hash of a
/// near-miss tells an attacker nothing about how many leading bytes were right.
/// The severity that motivated the ticket is therefore lower than recorded, and
/// the recorded version has been corrected rather than quietly dropped.
///
/// **It is still worth fixing, for one specific reason.** This is public API of
/// the crate, it is named exactly what an auth check is named, and the next two
/// tasks are a new read route (T11.2) and a bind wider than loopback (T11.4) —
/// which is precisely when somebody reaches for the function that looks like the
/// answer. Hardening it now costs a dependency edge already present in the
/// lockfile.
///
/// And stated carefully, because the obvious version of the claim is also
/// wrong: `String == String` is not a visible byte loop, it lowers to `bcmp`,
/// and a `bcmp` over 43 bytes may well be one vectorised compare with no
/// data-dependent timing at all. The problem is that nothing *says* so — it is a
/// property of the libc and the codegen on the day, free to change under a
/// compiler upgrade, and the failure would be silent. Entropy was never in
/// question either way: the secret is 32 bytes of `OsRng`.
#[derive(Debug, Clone)]
pub struct AuthToken(String);

impl PartialEq for AuthToken {
    fn eq(&self, other: &Self) -> bool {
        // `ct_eq` on slices is constant-time in the *contents* and short-circuits
        // on length. That leak is real and accepted: these tokens are a fixed
        // 43-character base64 encoding of 32 bytes, so length carries nothing an
        // attacker does not already know, and padding to hide it would only move
        // the question.
        self.0.as_bytes().ct_eq(other.0.as_bytes()).unwrap_u8() == 1
    }
}

impl Eq for AuthToken {}

impl AuthToken {
    /// Generates a bearer secret from 32 bytes of operating-system CSPRNG output.
    ///
    /// Local-control bearer tokens are authentication material, so they use
    /// `OsRng` instead of a deterministic or fast userspace PRNG.
    pub fn generate() -> Self {
        let mut bytes = [0u8; 32];
        rand::rngs::OsRng.fill_bytes(&mut bytes);
        Self(base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes))
    }

    pub fn from_secret(secret: impl Into<String>) -> Self {
        Self(secret.into())
    }

    pub fn secret(&self) -> &str {
        &self.0
    }

    pub fn authorization_value(&self) -> String {
        format!("Bearer {}", self.0)
    }

    pub fn from_authorization_header(value: Option<&str>) -> Result<Self, ControlError> {
        let Some(value) = value else {
            return Err(ControlError::new(
                ErrorCode::UnauthorizedLocalClient,
                "Authorization header is required",
            ));
        };
        let Some(token) = value.strip_prefix("Bearer ") else {
            return Err(ControlError::new(
                ErrorCode::UnauthorizedLocalClient,
                "Authorization header must use the Bearer scheme",
            ));
        };
        Ok(Self::from_secret(token))
    }

    pub fn verify_authorization_header(&self, value: Option<&str>) -> Result<(), ControlError> {
        let token = Self::from_authorization_header(value)?;
        if token != *self {
            return Err(ControlError::new(
                ErrorCode::UnauthorizedLocalClient,
                "Authorization token is invalid",
            ));
        }
        Ok(())
    }
}

/// Request for a short-lived credential scoped to one exact action.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CredentialRequest {
    pub protocol_version: u32,
    pub request_id: Uuid,
    pub action: ActionKind,
}

impl CredentialRequest {
    pub fn new(action: ActionKind) -> Self {
        Self {
            protocol_version: crate::protocol::PROTOCOL_VERSION,
            request_id: Uuid::new_v4(),
            action,
        }
    }
}

/// Client-facing credential response containing a bearer secret and its grant metadata.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScopedCredential {
    pub bearer_token: String,
    pub grant: CredentialGrant,
}

impl ScopedCredential {
    pub fn authorization_value(&self) -> String {
        format!("Bearer {}", self.bearer_token)
    }
}

/// Authorization grant issued by the localhost server running inside Warp for a
/// single action.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CredentialGrant {
    pub credential_id: String,
    pub instance_id: InstanceId,
    pub action: ActionKind,
    pub issued_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
}

impl CredentialGrant {
    pub fn new(instance_id: InstanceId, action: ActionKind, ttl: Duration) -> Self {
        let issued_at = Utc::now();
        Self {
            credential_id: format!("cred_{}", Uuid::new_v4().simple()),
            instance_id,
            action,
            issued_at,
            expires_at: issued_at + ttl,
        }
    }

    pub fn is_expired(&self) -> bool {
        Utc::now() >= self.expires_at
    }

    pub fn verify_for_action(
        &self,
        instance_id: &InstanceId,
        action: ActionKind,
    ) -> Result<(), ControlError> {
        if self.is_expired() {
            return Err(ControlError::new(
                ErrorCode::UnauthorizedLocalClient,
                "local-control credential has expired",
            ));
        }
        if &self.instance_id != instance_id {
            return Err(ControlError::new(
                ErrorCode::UnauthorizedLocalClient,
                "local-control credential belongs to a different Warp instance",
            ));
        }
        if self.action != action {
            return Err(ControlError::new(
                ErrorCode::InsufficientPermissions,
                format!(
                    "credential for {} cannot invoke {}",
                    self.action.as_str(),
                    action.as_str()
                ),
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "auth_tests.rs"]
mod tests;
