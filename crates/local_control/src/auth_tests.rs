use chrono::Duration;

use super::*;
use crate::discovery::InstanceId;

#[test]
fn rejects_missing_authorization_header() {
    let token = AuthToken::from_secret("secret");
    let error = token
        .verify_authorization_header(None)
        .expect_err("rejected");
    assert_eq!(error.code, ErrorCode::UnauthorizedLocalClient);
}

#[test]
fn rejects_malformed_authorization_header() {
    let token = AuthToken::from_secret("secret");
    let error = token
        .verify_authorization_header(Some("Basic secret"))
        .expect_err("rejected");
    assert_eq!(error.code, ErrorCode::UnauthorizedLocalClient);
}

#[test]
fn rejects_wrong_bearer_token() {
    let token = AuthToken::from_secret("secret");
    let error = token
        .verify_authorization_header(Some("Bearer wrong"))
        .expect_err("rejected");
    assert_eq!(error.code, ErrorCode::UnauthorizedLocalClient);
}

#[test]
fn accepts_matching_bearer_token() {
    AuthToken::from_secret("secret")
        .verify_authorization_header(Some("Bearer secret"))
        .expect("accepted");
}

/// Fork (T11.3): the hand-written `PartialEq` must still be *equality*.
///
/// **This pins the semantics, not the timing.** A constant-time property cannot
/// be asserted from a unit test without a statistical timing measurement, which
/// would be flaky in exactly the way that gets a test deleted — so it is not
/// claimed here. What this catches is the likelier regression: a hand-written
/// comparison that quietly stops agreeing with the derived one it replaced.
#[test]
fn a_token_equals_itself_and_nothing_else() {
    let token = AuthToken::generate();

    assert_eq!(token, AuthToken::from_secret(token.secret()));
    assert_ne!(token, AuthToken::generate());
    // Differing in the first byte and in the last must both be rejected, and a
    // prefix of the real secret must not pass for it.
    let secret = token.secret();
    let mut first = secret.to_owned();
    first.replace_range(0..1, "~");
    let mut last = secret.to_owned();
    last.replace_range(secret.len() - 1.., "~");

    assert_ne!(token, AuthToken::from_secret(first));
    assert_ne!(token, AuthToken::from_secret(last));
    assert_ne!(token, AuthToken::from_secret(&secret[..secret.len() - 1]));
    assert_ne!(token, AuthToken::from_secret(format!("{secret}~")));
}

#[test]
fn scoped_credential_allows_only_granted_action() {
    let grant = CredentialGrant::new(
        InstanceId("inst_test".to_owned()),
        ActionKind::TabCreate,
        Duration::minutes(5),
    );
    grant
        .verify_for_action(&grant.instance_id, ActionKind::TabCreate)
        .expect("tab.create grant is accepted");
    let error = grant
        .verify_for_action(&grant.instance_id, ActionKind::WindowCreate)
        .expect_err("other actions are rejected");
    assert_eq!(error.code, ErrorCode::InsufficientPermissions);
}

#[test]
fn scoped_credential_rejects_different_instance() {
    let grant = CredentialGrant::new(
        InstanceId("inst_test".to_owned()),
        ActionKind::TabCreate,
        Duration::minutes(5),
    );
    let error = grant
        .verify_for_action(&InstanceId("inst_other".to_owned()), ActionKind::TabCreate)
        .expect_err("other instance is rejected");
    assert_eq!(error.code, ErrorCode::UnauthorizedLocalClient);
}

#[test]
fn scoped_credential_rejects_expired_grant() {
    let grant = CredentialGrant::new(
        InstanceId("inst_test".to_owned()),
        ActionKind::TabCreate,
        Duration::minutes(-1),
    );
    let error = grant
        .verify_for_action(&grant.instance_id, ActionKind::TabCreate)
        .expect_err("expired grant is rejected");
    assert_eq!(error.code, ErrorCode::UnauthorizedLocalClient);
}

#[test]
fn scoped_credential_allows_confirmation_required_action_scope() {
    let grant = CredentialGrant::new(
        InstanceId("inst_test".to_owned()),
        ActionKind::WindowClose,
        Duration::minutes(5),
    );
    grant
        .verify_for_action(&grant.instance_id, ActionKind::WindowClose)
        .expect("exact-action credential is separate from one-shot confirmation");
}

#[test]
fn credential_request_carries_only_action() {
    let request = CredentialRequest::new(ActionKind::TabCreate);
    assert_eq!(request.action, ActionKind::TabCreate);
    assert_eq!(request.protocol_version, crate::protocol::PROTOCOL_VERSION);
}
